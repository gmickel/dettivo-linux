#include "meetings_actions.h"
#include "upload_source.h"

#include <QFileInfo>
#include <QJsonArray>
#include <QFutureWatcher>
#include <QPointer>
#include <QPromise>
#include <QThreadPool>

#include <utility>

namespace dettivo {
static_assert(UploadSource::kChunkBytes == MeetingsActions::kChunkBytes);
namespace {
template<typename Work, typename Done>
void readInBackground(QObject *owner, Work work, Done done)
{
    using Result = decltype(work());
    auto promise = std::make_shared<QPromise<Result>>();
    auto *watcher = new QFutureWatcher<Result>(owner);
    QObject::connect(watcher, &QFutureWatcher<Result>::finished, owner, [watcher, done]() mutable {
        const auto result = watcher->result();
        watcher->deleteLater();
        done(result);
    });
    watcher->setFuture(promise->future());
    QThreadPool::globalInstance()->start([promise, work]() {
        promise->start();
        promise->addResult(work());
        promise->finish();
    });
}

void cancelTransfer(DaemonLink *link, const QString &id)
{
    if (link != nullptr && link->connected() && !id.isEmpty())
        link->call(QStringLiteral("transfer.cancel"), {{QStringLiteral("transfer_id"), id}}, {});
}
}  // namespace

MeetingsActions::~MeetingsActions()
{
    cancelUpload();
}

void MeetingsActions::cancelUpload()
{
    ++m_uploadGeneration;
    m_uploadActive = false;
    const auto id = std::exchange(m_uploadTransfer, {});
    cancelTransfer(m_link, id);
}

void MeetingsActions::cancelImport()
{
    if (!m_uploadActive)
        return;
    cancelUpload();
    setBusy(false);
}

void MeetingsActions::abortUpload(const QString &reason)
{
    cancelUpload();
    setBusy(false);
    emit failed(QStringLiteral("import"), reason);
}

void MeetingsActions::importFile(const QString &path, const QString &provider, const QString &model,
                                 const QString &language, bool diarize, bool analyze)
{
    if (!ready(QStringLiteral("import")))
        return;
    Upload upload;
    upload.source = std::make_shared<UploadSource>(path.trimmed());
    upload.filename = QFileInfo(path.trimmed()).fileName();
    upload.provider = provider;
    upload.model = model;
    upload.language = language.isEmpty() ? QStringLiteral("auto") : language;
    upload.diarize = diarize;
    upload.analyze = analyze;
    upload.generation = ++m_uploadGeneration;
    m_uploadActive = true;
    setBusy(true);
    // Preview is bounded too, but opening and probing the media for the
    // actual import must not perform filesystem I/O on the GUI thread.
    readInBackground(this, [source = upload.source]() { return source->open(); },
        [this, upload, path](const QString &error) mutable {
            if (upload.generation != m_uploadGeneration)
                return;
            if (!error.isEmpty()) {
                abortUpload(error);
                return;
            }
            const QString contentType = importContentType(path.trimmed());
            if (contentType.isEmpty()) {
                abortUpload(tr("The file is not a supported audio recording."));
                return;
            }
            const QJsonObject begin{{QStringLiteral("direction"), QStringLiteral("upload")},
                {QStringLiteral("content_type"), contentType}, {QStringLiteral("size_hint"), double(upload.source->size())}};
            QPointer<MeetingsActions> self(this);
            QPointer<DaemonLink> link(m_link);
            m_link->call(QStringLiteral("transfer.begin"), begin, [self, link, upload](const QJsonObject &result, const QJsonObject &refusal) mutable {
                const QString id = result.value(QStringLiteral("transfer_id")).toString();
                if (!self || upload.generation != self->m_uploadGeneration) {
                    cancelTransfer(link, id);
                    return;
                }
                if (!refusal.isEmpty()) {
                    self->abortUpload(reasonOf(refusal));
                    return;
                }
                upload.transferId = id;
                self->m_uploadTransfer = id;
                self->negotiateUpload(upload, result.value(QStringLiteral("chunk_max_bytes")).toInteger());
            });
        });
}

void MeetingsActions::negotiateUpload(Upload upload, qint64 transferChunkMax)
{
    QPointer<MeetingsActions> self(this);
    m_link->call(QStringLiteral("config.get"), {{QStringLiteral("key"), QStringLiteral("ipc.max_line_bytes")}},
        [self, upload, transferChunkMax](const QJsonObject &result, const QJsonObject &error) mutable {
            if (!self || upload.generation != self->m_uploadGeneration)
                return;
            if (!error.isEmpty()) {
                self->abortUpload(reasonOf(error));
                return;
            }
            for (const auto &entry : result.value(QStringLiteral("entries")).toArray()) {
                const auto value = entry.toObject();
                if (value.value(QStringLiteral("key")) == QStringLiteral("ipc.max_line_bytes"))
                    upload.lineBytes = value.value(QStringLiteral("value")).toInteger();
            }
            const QJsonObject envelope{{QStringLiteral("transfer_id"), upload.transferId},
                {QStringLiteral("seq"), 1}, {QStringLiteral("data_b64"), QString()}};
            // Reserve the longest JSON number representation for future sequences.
            const qint64 overhead = self->m_link->requestBytes(QStringLiteral("transfer.chunk"), envelope) + 32;
            const qint64 payloadBytes = qMax<qint64>(0, upload.lineBytes - overhead) / 4 * 3;
            upload.chunkBytes = qMin(qMin(transferChunkMax, kChunkBytes), payloadBytes);
            if (upload.chunkBytes <= 0) {
                self->abortUpload(tr("The daemon's IPC line limit cannot carry an upload chunk."));
                return;
            }
            self->pushChunk(upload);
        });
}

void MeetingsActions::pushChunk(Upload upload)
{
    readInBackground(this, [source = upload.source, bytes = upload.chunkBytes]() { return source->next(bytes); },
        [this, upload](const UploadSource::Chunk &chunk) mutable {
            if (upload.generation != m_uploadGeneration)
                return;
            if (!chunk.error.isEmpty()) {
                abortUpload(chunk.error);
                return;
            }
            if (chunk.done) {
                upload.digest = chunk.digest;
                commitUpload(upload);
                return;
            }
            const QJsonObject params{{QStringLiteral("transfer_id"), upload.transferId},
                {QStringLiteral("seq"), double(upload.seq)}, {QStringLiteral("data_b64"), QString::fromLatin1(chunk.bytes.toBase64())}};
            if (m_link->requestBytes(QStringLiteral("transfer.chunk"), params) > upload.lineBytes) {
                abortUpload(tr("The upload chunk exceeds the daemon's IPC line limit."));
                return;
            }
            QPointer<MeetingsActions> self(this);
            m_link->call(QStringLiteral("transfer.chunk"), params, [self, upload](const QJsonObject &, const QJsonObject &error) mutable {
                if (!self || upload.generation != self->m_uploadGeneration)
                    return;
                if (!error.isEmpty()) {
                    self->abortUpload(reasonOf(error));
                    return;
                }
                ++upload.seq;
                self->pushChunk(upload);
            });
        });
}

void MeetingsActions::commitUpload(Upload upload)
{
    const QJsonObject commit{{QStringLiteral("transfer_id"), upload.transferId},
        {QStringLiteral("total_chunks"), double(upload.seq - 1)}, {QStringLiteral("sha256"), upload.digest}};
    QPointer<MeetingsActions> self(this);
    m_link->call(QStringLiteral("transfer.commit"), commit, [self, upload](const QJsonObject &, const QJsonObject &error) {
        if (!self || upload.generation != self->m_uploadGeneration)
            return;
        if (!error.isEmpty()) {
            self->abortUpload(reasonOf(error));
            return;
        }
        QJsonObject params{{QStringLiteral("transfer_id"), upload.transferId},
            {QStringLiteral("target_kind"), QStringLiteral("meeting")}, {QStringLiteral("filename"), upload.filename},
            {QStringLiteral("language"), upload.language}, {QStringLiteral("mode"), QStringLiteral("raw")},
            {QStringLiteral("acknowledge_meeting_disclosure"), true}, {QStringLiteral("diarize"), upload.diarize},
            {QStringLiteral("analyze"), upload.analyze}};
        if (!upload.provider.isEmpty())
            params.insert(QStringLiteral("provider"), upload.provider);
        if (!upload.model.isEmpty())
            params.insert(QStringLiteral("model"), upload.model);
        QPointer<DaemonLink> link(self->m_link);
        self->m_link->call(QStringLiteral("transcripts.import"), params, [self, link, upload](const QJsonObject &result, const QJsonObject &refusal) {
            if (!self || upload.generation != self->m_uploadGeneration) {
                const auto reference = result.value(QStringLiteral("ref")).toObject();
                const QString meetingId = reference.value(QStringLiteral("id")).toString();
                if (refusal.isEmpty() && link && link->connected() && !meetingId.isEmpty()
                    && reference.value(QStringLiteral("kind")) == QStringLiteral("meeting")) {
                    link->call(QStringLiteral("meetings.cancel"), {{QStringLiteral("meeting_id"), meetingId}},
                        [self, meetingId](const QJsonObject &cancelledResult, const QJsonObject &error) {
                            if (!self)
                                return;
                            const QString refusal = cancellationError(meetingId, cancelledResult, error);
                            if (refusal.isEmpty())
                                emit self->cancelled(meetingId);
                            else
                                emit self->failed(QStringLiteral("cancel"), refusal);
                        });
                }
                return;
            }
            if (!refusal.isEmpty()) {
                self->abortUpload(reasonOf(refusal));
                return;
            }
            self->m_uploadTransfer.clear();
            self->m_uploadActive = false;
            self->setBusy(false);
            self->m_importMeeting = result.value(QStringLiteral("ref")).toObject().value(QStringLiteral("id")).toString();
            self->m_importJob = result.value(QStringLiteral("job")).toObject().value(QStringLiteral("job_id")).toString();
            self->m_importStage = result.value(QStringLiteral("job")).toObject().value(QStringLiteral("message")).toString(QStringLiteral("queued"));
            self->m_importProgress = 0;
            emit self->importChanged();
            emit self->importStarted(self->m_importMeeting, self->m_importJob);
        });
    });
}

}  // namespace dettivo
