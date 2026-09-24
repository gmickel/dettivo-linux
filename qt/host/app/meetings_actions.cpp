#include "meetings_actions.h"
#include "config_binding.h"
#include "meeting_format.h"
#include "status_format.h"
#include "wav_metadata.h"
#include "upload_source.h"

#include <QFile>
#include <QFileInfo>
#include <QJsonArray>
#include <QLocale>
#include <QVariantMap>

namespace dettivo {

namespace {

/// The duration of a PCM WAV from its header; -1 for anything else.
qint64 wavDurationMs(const QString &path)
{
    QFile file(path);
    if (!file.open(QIODevice::ReadOnly))
        return -1;
    WavMetadata metadata;
    QString error;
    return readWavMetadata(file, &metadata, &error) ? metadata.durationMs : -1;
}

QString sizeText(qint64 bytes)
{
    const QLocale locale = QLocale::c();
    if (bytes >= 1024 * 1024)
        return QStringLiteral("%1 MB").arg(locale.toString(double(bytes) / (1024.0 * 1024.0), 'f', bytes >= 10 * 1024 * 1024 ? 0 : 1));
    return QStringLiteral("%1 kB").arg(qMax<qint64>(1, bytes / 1024));
}

}  // namespace

MeetingsActions::MeetingsActions(DaemonLink *link, ConfigBinding *config, QObject *parent)
    : QObject(parent), m_link(link), m_config(config)
{
    if (m_link != nullptr) {
        connect(m_link, &DaemonLink::notification, this, &MeetingsActions::handleNotification);
        // The pickers follow the daemon: read on every connect, so a rail
        // that opened before the socket answered still gets its engines.
        connect(m_link, &DaemonLink::connectedChanged, this, [this](bool connected) {
            if (connected)
                loadProviders();
            else if (m_uploadActive)
                abortUpload(tr("The daemon disconnected during upload."));
        });
    }
    if (m_config != nullptr)
        connect(m_config, &ConfigBinding::changed, this, &MeetingsActions::policyChanged);
}

QString MeetingsActions::reasonOf(const QJsonObject &error)
{
    const QString message = error.value(QStringLiteral("message")).toString();
    const QString kind = error.value(QStringLiteral("data")).toObject().value(QStringLiteral("details")).toObject().value(QStringLiteral("kind")).toString();
    if (message.isEmpty())
        return QObject::tr("the daemon did not answer");
    return kind.isEmpty() ? message : QStringLiteral("%1 (%2)").arg(message, kind);
}

QString MeetingsActions::deletePolicy() const
{
    const QString policy = m_config != nullptr ? m_config->text(QStringLiteral("meetings.delete_artifact_policy")) : QString();
    return policy.isEmpty() ? QStringLiteral("all") : policy;
}

void MeetingsActions::setBusy(bool busy)
{
    if (m_busy == busy)
        return;
    m_busy = busy;
    emit busyChanged();
}

bool MeetingsActions::ready(const QString &action)
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

void MeetingsActions::rename(const QString &meetingId, const QString &speakerId, const QString &name)
{
    if (!ready(QStringLiteral("rename")))
        return;
    setBusy(true);
    const QJsonObject params{{QStringLiteral("meeting_id"), meetingId}, {QStringLiteral("speaker_id"), speakerId}, {QStringLiteral("name"), name.trimmed()}};
    m_link->call(QStringLiteral("meetings.speakers.rename"), params, [this, meetingId, speakerId](const QJsonObject &result, const QJsonObject &error) {
        setBusy(false);
        if (!error.isEmpty()) {
            emit failed(QStringLiteral("rename"), reasonOf(error));
            return;
        }
        const QJsonObject speaker = result.value(QStringLiteral("speaker")).toObject();
        emit renamed(meetingId, speakerId, speaker.value(QStringLiteral("name")).toString(),
                     result.value(QStringLiteral("segments_updated")).toInt(0));
    });
}

void MeetingsActions::retitle(const QString &meetingId, const QString &title)
{
    const QString text = title.trimmed();
    if (text.isEmpty()) {
        emit failed(QStringLiteral("retitle"), tr("A meeting needs a title."));
        return;
    }
    if (!ready(QStringLiteral("retitle")))
        return;
    setBusy(true);
    const QJsonObject params{{QStringLiteral("meeting_id"), meetingId}, {QStringLiteral("title"), text}};
    m_link->call(QStringLiteral("meetings.rename"), params, [this, meetingId](const QJsonObject &result, const QJsonObject &error) {
        setBusy(false);
        if (!error.isEmpty()) {
            emit failed(QStringLiteral("retitle"), reasonOf(error));
            return;
        }
        emit retitled(meetingId, result.value(QStringLiteral("title")).toString());
    });
}

void MeetingsActions::suggest(const QString &prefix)
{
    if (m_link == nullptr || !m_link->connected())
        return;
    QJsonObject params{{QStringLiteral("limit"), 6}};
    if (!prefix.trimmed().isEmpty())
        params.insert(QStringLiteral("prefix"), prefix.trimmed());
    m_link->call(QStringLiteral("meetings.speakers.suggest"), params, [this](const QJsonObject &result, const QJsonObject &error) {
        if (error.isEmpty())
            applySuggestions(result.value(QStringLiteral("names")).toArray());
    });
}

void MeetingsActions::applySuggestions(const QJsonArray &names)
{
    m_suggestions.clear();
    for (const QJsonValue &v : names) {
        const QJsonObject name = v.toObject();
        m_suggestions.append(QVariantMap{{QStringLiteral("name"), name.value(QStringLiteral("name")).toString()},
                                         {QStringLiteral("uses"), name.value(QStringLiteral("uses")).toInt(0)}});
    }
    emit suggestionsChanged();
}

void MeetingsActions::analyze(const QString &meetingId, bool force)
{
    if (!ready(QStringLiteral("analyze")))
        return;
    setBusy(true);
    const QJsonObject params{{QStringLiteral("meeting_id"), meetingId}, {QStringLiteral("force"), force}};
    m_link->call(QStringLiteral("meetings.analyze"), params, [this, meetingId](const QJsonObject &result, const QJsonObject &error) {
        setBusy(false);
        if (!error.isEmpty()) {
            emit failed(QStringLiteral("analyze"), reasonOf(error));
            return;
        }
        emit analysisStarted(meetingId, result.value(QStringLiteral("job")).toObject().value(QStringLiteral("job_id")).toString());
    });
}

void MeetingsActions::diarize(const QString &meetingId, int speakers)
{
    if (!ready(QStringLiteral("diarize")))
        return;
    setBusy(true);
    QJsonObject params{{QStringLiteral("meeting_id"), meetingId}};
    if (speakers > 0)
        params.insert(QStringLiteral("speakers"), speakers);
    m_link->call(QStringLiteral("meetings.diarize"), params, [this, meetingId](const QJsonObject &result, const QJsonObject &error) {
        setBusy(false);
        if (!error.isEmpty()) {
            emit failed(QStringLiteral("diarize"), reasonOf(error));
            return;
        }
        emit diarizeStarted(meetingId, result.value(QStringLiteral("job")).toObject().value(QStringLiteral("job_id")).toString());
    });
}

void MeetingsActions::remove(const QString &meetingId, const QString &policy)
{
    if (!ready(QStringLiteral("delete")))
        return;
    setBusy(true);
    const QJsonObject params{{QStringLiteral("meeting_id"), meetingId},
                             {QStringLiteral("artifact_policy"), policy.isEmpty() ? deletePolicy() : policy}};
    m_link->call(QStringLiteral("meetings.delete"), params, [this, meetingId](const QJsonObject &, const QJsonObject &error) {
        setBusy(false);
        if (!error.isEmpty()) {
            emit failed(QStringLiteral("delete"), reasonOf(error));
            return;
        }
        emit removed(meetingId);
    });
}

void MeetingsActions::recover(const QString &meetingId)
{
    if (!ready(QStringLiteral("recover")))
        return;
    setBusy(true);
    m_link->call(QStringLiteral("meetings.recover"), {{QStringLiteral("meeting_id"), meetingId}}, [this, meetingId](const QJsonObject &, const QJsonObject &error) {
        setBusy(false);
        if (!error.isEmpty()) {
            emit failed(QStringLiteral("recover"), reasonOf(error));
            return;
        }
        emit recovered(meetingId);
    });
}

void MeetingsActions::discard(const QString &meetingId)
{
    if (!ready(QStringLiteral("discard")))
        return;
    setBusy(true);
    m_link->call(QStringLiteral("meetings.discard"), {{QStringLiteral("meeting_id"), meetingId}}, [this, meetingId](const QJsonObject &, const QJsonObject &error) {
        setBusy(false);
        if (!error.isEmpty()) {
            emit failed(QStringLiteral("discard"), reasonOf(error));
            return;
        }
        emit discarded(meetingId);
    });
}

QString MeetingsActions::cancellationError(const QString &meetingId, const QJsonObject &result, const QJsonObject &error)
{
    if (!error.isEmpty())
        return reasonOf(error);
    const auto reference = result.value(QStringLiteral("ref")).toObject();
    if (result.value(QStringLiteral("job")).toObject().value(QStringLiteral("state")).toString() != QStringLiteral("cancelled")
        || reference.value(QStringLiteral("id")).toString() != meetingId
        || reference.value(QStringLiteral("kind")).toString() != QStringLiteral("meeting"))
        return tr("The daemon did not confirm cancellation of this meeting.");
    return {};
}

void MeetingsActions::cancel(const QString &meetingId)
{
    if (!ready(QStringLiteral("cancel")))
        return;
    setBusy(true);
    m_link->call(QStringLiteral("meetings.cancel"), {{QStringLiteral("meeting_id"), meetingId}},
                 [this, meetingId](const QJsonObject &result, const QJsonObject &error) {
        setBusy(false);
        const QString refusal = cancellationError(meetingId, result, error);
        if (!refusal.isEmpty()) {
            emit failed(QStringLiteral("cancel"), refusal);
            return;
        }
        emit cancelled(meetingId);
    });
}

void MeetingsActions::loadProviders()
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

// Only a provider that supports meetings is offered (Parakeet stays
// dictation-only, ADR 0018); the daemon's selection is picked when it is
// one of them, the first otherwise.
void MeetingsActions::applyProviders(const QJsonObject &providers, const QJsonObject &selection)
{
    m_providers.clear();
    for (const QJsonValue &v : providers.value(QStringLiteral("providers")).toArray()) {
        const QJsonObject provider = v.toObject();
        // A provider that says nothing about meetings is offered; only one
        // that declares itself dictation-only (Parakeet, ADR 0018) is left out.
        if (!provider.value(QStringLiteral("supports_meetings")).toBool(provider.value(QStringLiteral("supports_timestamps")).toBool(true)))
            continue;
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
    const QJsonObject meeting = selection.value(QStringLiteral("meeting")).toObject();
    const QJsonObject dictation = selection.value(QStringLiteral("dictation")).toObject();
    const QJsonObject chosen = meeting.isEmpty() ? dictation : meeting;
    m_daemonProvider = chosen.value(QStringLiteral("provider_id")).toString();
    m_selectedProvider = m_daemonProvider;
    m_selectedModel = format::modelName(chosen.value(QStringLiteral("model_id")).toString());
    bool offered = false;
    for (const QVariant &p : m_providers)
        offered = offered || p.toMap().value(QStringLiteral("id")).toString() == m_selectedProvider;
    if (!offered && !m_providers.isEmpty()) {
        // Offered on the picker only; the daemon stays where it is until
        // the user picks it, which then goes through the selection.
        m_selectedProvider = m_providers.first().toMap().value(QStringLiteral("id")).toString();
        m_selectedModel.clear();
    }
    emit providersChanged();
}

// Meetings select Whisper independently of the dictation provider.
void MeetingsActions::pickEngine(const QString &providerId, const QString &modelId)
{
    if (m_link == nullptr || !m_link->connected() || providerId != QStringLiteral("whisper"))
        return;
    m_selectedProvider = providerId;
    m_selectedModel = format::modelName(modelId);
    emit providersChanged();
    if (m_config != nullptr)
        m_config->set(QStringLiteral("speech.meeting_model"), modelId);
}

QVariantMap MeetingsActions::probeFile(const QString &path) const
{
    QVariantMap facts{{QStringLiteral("ok"), false}, {QStringLiteral("name"), QFileInfo(path).fileName()}};
    const QFileInfo info(path.trimmed());
    if (path.trimmed().isEmpty()) {
        facts.insert(QStringLiteral("reason"), tr("Name an audio file to import."));
        return facts;
    }
    if (!info.isFile()) {
        facts.insert(QStringLiteral("reason"), tr("%1 is not a file.").arg(info.fileName()));
        return facts;
    }
    if (importContentType(info.filePath()).isEmpty()) {
        facts.insert(QStringLiteral("reason"), tr("%1 is not an audio file the import reads (wav, mp3, m4a, aac, flac, ogg, opus, caf, aiff).").arg(info.fileName()));
        return facts;
    }
    const qint64 duration = wavDurationMs(info.filePath());
    facts.insert(QStringLiteral("ok"), true);
    facts.insert(QStringLiteral("bytes"), info.size());
    facts.insert(QStringLiteral("sizeText"), sizeText(info.size()));
    facts.insert(QStringLiteral("durationMs"), duration);
    facts.insert(QStringLiteral("durationText"), duration >= 0 ? meeting_format::minutes(duration) : tr("length read on import"));
    return facts;
}

void MeetingsActions::handleNotification(const QString &topic, const QJsonObject &payload)
{
    if (topic != QStringLiteral("job.progress") || m_importJob.isEmpty())
        return;
    if (payload.value(QStringLiteral("job_id")).toString() != m_importJob)
        return;
    m_importStage = payload.value(QStringLiteral("stage")).toString(m_importStage);
    m_importProgress = qBound(0.0, payload.value(QStringLiteral("progress")).toDouble(), 1.0);
    const bool ended = m_importStage == QStringLiteral("done") || m_importStage == QStringLiteral("failed")
        || m_importStage == QStringLiteral("cancelled");
    if (ended) {
        const QString meeting = m_importMeeting;
        const QString stage = m_importStage;
        m_importJob.clear();
        emit importChanged();
        emit importEnded(meeting, stage);
        return;
    }
    emit importChanged();
}

}  // namespace dettivo
