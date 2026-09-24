#include "history_actions.h"
#include "status_format.h"

#include <QClipboard>
#include <QGuiApplication>
#include <QDBusArgument>
#include <QDBusConnection>
#include <QDBusInterface>
#include <QDBusReply>
#include <QDir>
#include <QFile>
#include <QJsonArray>
#include <QUrl>
#include <QVariantMap>

namespace dettivo {

namespace {

constexpr auto kPortalService = "org.freedesktop.portal.Desktop";
constexpr auto kPortalPath = "/org/freedesktop/portal/desktop";
constexpr auto kFileChooser = "org.freedesktop.portal.FileChooser";
constexpr auto kRequest = "org.freedesktop.portal.Request";

QJsonObject dictationRef(const QString &id)
{
    return {{QStringLiteral("kind"), QStringLiteral("dictation")}, {QStringLiteral("id"), id}};
}

QString reasonOf(const QJsonObject &error)
{
    const QString message = error.value(QStringLiteral("message")).toString();
    return message.isEmpty() ? QObject::tr("the daemon did not answer") : message;
}

}  // namespace

HistoryActions::HistoryActions(DaemonLink *link, QString exportDir, QObject *parent)
    : QObject(parent), m_link(link), m_exportDir(std::move(exportDir))
{
}

QString HistoryActions::contentType(const QString &format)
{
    if (format == QStringLiteral("json"))
        return QStringLiteral("application/json");
    if (format == QStringLiteral("zip"))
        return QStringLiteral("application/zip");
    if (format == QStringLiteral("md"))
        return QStringLiteral("text/markdown");
    if (format == QStringLiteral("srt"))
        return QStringLiteral("application/x-subrip");
    if (format == QStringLiteral("vtt"))
        return QStringLiteral("text/vtt");
    return QStringLiteral("text/plain");
}

void HistoryActions::setBusy(bool busy)
{
    if (m_busy == busy)
        return;
    m_busy = busy;
    emit busyChanged();
}

bool HistoryActions::ready(const QString &action)
{
    if (m_link == nullptr || !m_link->connected()) {
        emit failed(action, tr("Daemon unavailable."));
        return false;
    }
    if (m_busy) {
        emit failed(action, tr("Another action is still running."));
        return false;
    }
    return true;
}

void HistoryActions::loadProviders()
{
    if (m_link == nullptr || !m_link->connected())
        return;
    m_link->call(QStringLiteral("speech.providers.list"), {}, [this](const QJsonObject &providers, const QJsonObject &error) {
        if (!error.isEmpty())
            return;
        m_link->call(QStringLiteral("speech.selection.get"), {}, [this, providers](const QJsonObject &selection, const QJsonObject &) {
            applyProviders(providers, selection);
        });
    });
}

void HistoryActions::applyProviders(const QJsonObject &providers, const QJsonObject &selection)
{
    m_providers.clear();
    for (const QJsonValue &v : providers.value(QStringLiteral("providers")).toArray()) {
        const QJsonObject provider = v.toObject();
        QVariantList models;
        for (const QJsonValue &m : provider.value(QStringLiteral("models")).toArray()) {
            const QJsonObject model = m.toObject();
            models.append(QVariantMap{{QStringLiteral("id"), model.value(QStringLiteral("id")).toString()},
                                      {QStringLiteral("label"), model.value(QStringLiteral("label")).toString()},
                                      {QStringLiteral("downloaded"), model.value(QStringLiteral("is_downloaded")).toBool()}});
        }
        m_providers.append(QVariantMap{{QStringLiteral("id"), provider.value(QStringLiteral("id")).toString()},
                                       {QStringLiteral("label"), provider.value(QStringLiteral("display_name")).toString()},
                                       {QStringLiteral("models"), models}});
    }
    const QJsonObject dictation = selection.value(QStringLiteral("dictation")).toObject();
    m_selectedProvider = dictation.value(QStringLiteral("provider_id")).toString();
    m_selectedModel = format::modelName(dictation.value(QStringLiteral("model_id")).toString());
    emit providersChanged();
}

void HistoryActions::rerun(const QString &id, const QString &provider, const QString &model)
{
    if (!ready(QStringLiteral("rerun")))
        return;
    QJsonObject params{{QStringLiteral("ref"), dictationRef(id)}};
    if (!provider.isEmpty())
        params.insert(QStringLiteral("provider"), provider);
    if (!model.isEmpty())
        params.insert(QStringLiteral("model"), model);
    setBusy(true);
    m_link->call(QStringLiteral("transcripts.rerun"), params, [this, id](const QJsonObject &result, const QJsonObject &error) {
        setBusy(false);
        if (!error.isEmpty()) {
            emit failed(QStringLiteral("rerun"), reasonOf(error));
            return;
        }
        emit rerunStarted(id, result.value(QStringLiteral("ref")).toObject().value(QStringLiteral("id")).toString(),
                          result.value(QStringLiteral("job")).toObject().value(QStringLiteral("job_id")).toString());
    });
}

void HistoryActions::remove(const QString &id)
{
    if (!ready(QStringLiteral("delete")))
        return;
    setBusy(true);
    m_link->call(QStringLiteral("transcripts.delete"), {{QStringLiteral("ref"), dictationRef(id)}},
                 [this, id](const QJsonObject &, const QJsonObject &error) {
                     setBusy(false);
                     if (!error.isEmpty()) {
                         emit failed(QStringLiteral("delete"), reasonOf(error));
                         return;
                     }
                     emit removed(id);
                 });
}

bool HistoryActions::copyText(const QString &text) const
{
    QClipboard *clipboard = QGuiApplication::clipboard();
    if (clipboard == nullptr || text.isEmpty())
        return false;
    clipboard->setText(text);
    return true;
}

void HistoryActions::exportItems(const QString &id, const QString &format, const QString &scope)
{
    QJsonObject params{{QStringLiteral("format"), format}, {QStringLiteral("scope"), scope}};
    if (scope == QStringLiteral("item"))
        params.insert(QStringLiteral("ref"), dictationRef(id));
    beginExport(params);
}

void HistoryActions::exportMeeting(const QString &id, const QString &format, bool raw, const QVariant &notesOverride)
{
    QJsonObject params{{QStringLiteral("format"), format},
                       {QStringLiteral("ref"), QJsonObject{{QStringLiteral("kind"), QStringLiteral("meeting")}, {QStringLiteral("id"), id}}}};
    if (raw)
        params.insert(QStringLiteral("raw"), true);
    if (notesOverride.isValid() && !notesOverride.isNull())
        params.insert(QStringLiteral("notes_override"), notesOverride.toString());
    beginExport(params);
}

// `transfer.begin { download }`, then `transcripts.export` with `params`
// (the transfer id filled in), then the chunks into the chosen file.
void HistoryActions::beginExport(QJsonObject params)
{
    if (!ready(QStringLiteral("export")))
        return;
    setBusy(true);
    const QString format = params.value(QStringLiteral("format")).toString();
    const QJsonObject begin{{QStringLiteral("direction"), QStringLiteral("download")},
                            {QStringLiteral("content_type"), contentType(format)},
                            {QStringLiteral("size_hint"), 0}};
    m_link->call(QStringLiteral("transfer.begin"), begin, [this, params](const QJsonObject &result, const QJsonObject &error) mutable {
        if (!error.isEmpty()) {
            setBusy(false);
            emit failed(QStringLiteral("export"), reasonOf(error));
            return;
        }
        Transfer transfer;
        transfer.id = result.value(QStringLiteral("transfer_id")).toString();
        params.insert(QStringLiteral("transfer_id"), transfer.id);
        m_link->call(QStringLiteral("transcripts.export"), params, [this, transfer](const QJsonObject &exported, const QJsonObject &refusal) mutable {
            if (!refusal.isEmpty()) {
                setBusy(false);
                emit failed(QStringLiteral("export"), reasonOf(refusal));
                return;
            }
            transfer.filename = exported.value(QStringLiteral("filename")).toString();
            chooseTarget(transfer);
        });
    });
}

void HistoryActions::chooseTarget(Transfer transfer)
{
    if (!m_exportDir.isEmpty()) {
        QDir dir(m_exportDir);
        if (!dir.exists() && !dir.mkpath(QStringLiteral("."))) {
            setBusy(false);
            emit failed(QStringLiteral("export"), tr("cannot create %1").arg(m_exportDir));
            return;
        }
        transfer.path = dir.filePath(transfer.filename);
        pullChunks(transfer);
        return;
    }
    // The portal's save dialog: the reply is a request object whose
    // Response signal carries the chosen uri.
    QDBusInterface chooser(QString::fromLatin1(kPortalService), QString::fromLatin1(kPortalPath),
                           QString::fromLatin1(kFileChooser), QDBusConnection::sessionBus());
    const QVariantMap options{{QStringLiteral("current_name"), transfer.filename}};
    const QDBusReply<QDBusObjectPath> reply = chooser.call(QStringLiteral("SaveFile"), QString(), tr("Export"), options);
    if (!reply.isValid()) {
        setBusy(false);
        emit failed(QStringLiteral("export"), tr("the file chooser portal did not answer: %1").arg(reply.error().message()));
        return;
    }
    // The Response signal arrives on the request path once the dialog
    // closes; the chosen uri is in its results.
    const QString request = reply.value().path();
    QDBusConnection::sessionBus().connect(QString::fromLatin1(kPortalService), request, QString::fromLatin1(kRequest),
                                          QStringLiteral("Response"), this, SLOT(portalResponse(uint, QVariantMap)));
    m_pending = transfer;
}

void HistoryActions::portalResponse(uint code, const QVariantMap &results)
{
    Transfer transfer = m_pending;
    m_pending = Transfer();
    if (code != 0) {
        setBusy(false);
        emit failed(QStringLiteral("export"), tr("no file chosen"));
        return;
    }
    const QStringList uris = results.value(QStringLiteral("uris")).toStringList();
    if (uris.isEmpty()) {
        setBusy(false);
        emit failed(QStringLiteral("export"), tr("the file chooser named no file"));
        return;
    }
    transfer.path = QUrl(uris.first()).toLocalFile();
    pullChunks(transfer);
}

void HistoryActions::pullChunks(Transfer transfer)
{
    if (transfer.seq == 1) {
        // One QSaveFile for the whole transfer: the previous export stays
        // untouched until the last chunk landed and the file commits.
        transfer.file = std::make_shared<QSaveFile>(transfer.path);
        if (!transfer.file->open(QIODevice::WriteOnly | QIODevice::Truncate)) {
            setBusy(false);
            emit failed(QStringLiteral("export"), tr("cannot write %1: %2").arg(transfer.path, transfer.file->errorString()));
            return;
        }
    }
    const QJsonObject params{{QStringLiteral("transfer_id"), transfer.id}, {QStringLiteral("seq"), double(transfer.seq)}};
    m_link->call(QStringLiteral("transfer.pull"), params, [this, transfer](const QJsonObject &result, const QJsonObject &error) {
        if (!error.isEmpty()) {
            setBusy(false);
            emit failed(QStringLiteral("export"), reasonOf(error));
            return;
        }
        const QByteArray data = QByteArray::fromBase64(result.value(QStringLiteral("data_b64")).toString().toLatin1());
        writeChunk(transfer, data, result.value(QStringLiteral("eof")).toBool());
    });
}

void HistoryActions::writeChunk(Transfer transfer, const QByteArray &data, bool eof)
{
    if (transfer.file->write(data) != data.size()) {
        setBusy(false);
        emit failed(QStringLiteral("export"), tr("cannot write %1: %2").arg(transfer.path, transfer.file->errorString()));
        return;
    }
    transfer.bytes += data.size();
    if (eof) {
        if (!transfer.file->commit()) {
            setBusy(false);
            emit failed(QStringLiteral("export"), tr("cannot write %1: %2").arg(transfer.path, transfer.file->errorString()));
            return;
        }
        setBusy(false);
        emit exported(transfer.path, transfer.bytes);
        return;
    }
    transfer.seq += 1;
    pullChunks(transfer);
}

}  // namespace dettivo
