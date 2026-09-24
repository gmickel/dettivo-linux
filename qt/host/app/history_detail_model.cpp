#include "history_detail_model.h"
#include "history_model.h"
#include "status_format.h"

#include <QDateTime>
#include <QJsonArray>
#include <QLocale>
#include <QVariantMap>

namespace dettivo {

namespace {

QString seconds(double value)
{
    return QStringLiteral("%1 s").arg(QLocale::c().toString(value, 'f', 1));
}

QString shortId(const QString &id)
{
    return id.size() > 8 ? id.left(4) + QStringLiteral("…") + id.right(4) : id;
}

QVariantMap fact(const QString &label, const QString &value)
{
    return {{QStringLiteral("label"), label}, {QStringLiteral("value"), value}};
}

}  // namespace

HistoryDetailModel::HistoryDetailModel(DaemonLink *link, QObject *parent) : QObject(parent), m_link(link)
{
    if (m_link != nullptr) {
        connect(m_link, &DaemonLink::notification, this, &HistoryDetailModel::handleNotification);
        connect(m_link, &DaemonLink::connectedChanged, this, [this](bool connected) {
            if (connected && !m_id.isEmpty())
                reload();
        });
    }
}

void HistoryDetailModel::load(const QString &id)
{
    if (id.isEmpty()) {
        clear();
        return;
    }
    if (id != m_id) {
        clear();
        m_id = id;
    }
    reload();
}

void HistoryDetailModel::reload()
{
    if (m_id.isEmpty() || m_link == nullptr || !m_link->connected())
        return;
    const QString id = m_id;
    m_loading = true;
    m_error.clear();
    emit changed();
    const QJsonObject params{{QStringLiteral("ref"), QJsonObject{{QStringLiteral("kind"), QStringLiteral("dictation")}, {QStringLiteral("id"), id}}}};
    m_link->call(QStringLiteral("transcripts.get"), params, [this, id](const QJsonObject &result, const QJsonObject &error) {
        if (id != m_id)
            return;
        m_loading = false;
        if (!error.isEmpty()) {
            m_error = error.value(QStringLiteral("message")).toString();
            m_loaded = false;
            emit changed();
            return;
        }
        apply(id, result);
    });
}

void HistoryDetailModel::trackJob(const QString &jobId)
{
    m_jobId = jobId;
    m_progress = 0;
    m_stage = QStringLiteral("queued");
    emit progressChanged();
}

void HistoryDetailModel::clear()
{
    m_id.clear();
    m_error.clear();
    m_title.clear();
    m_whenLine.clear();
    m_enhanced.clear();
    m_raw.clear();
    m_mode.clear();
    m_outcome.clear();
    m_backend.clear();
    m_provider.clear();
    m_model.clear();
    m_language.clear();
    m_app.clear();
    m_status.clear();
    m_source.clear();
    m_rerunOf.clear();
    m_audioPath.clear();
    m_audioReasonCode.clear();
    m_facts.clear();
    m_durationSeconds = 0;
    m_stopToInsertMs = -1;
    m_transcribeMs = -1;
    m_loaded = false;
    m_loading = false;
    m_audioRetained = false;
    m_jobId.clear();
    m_progress = -1;
    m_stage.clear();
    emit changed();
    emit progressChanged();
}

void HistoryDetailModel::apply(const QString &id, const QJsonObject &result)
{
    m_id = id;
    m_loading = false;
    m_error.clear();
    m_enhanced = result.value(QStringLiteral("text_polish")).toString();
    m_raw = result.value(QStringLiteral("text_raw")).toString();
    m_mode = result.value(QStringLiteral("mode")).toString();
    applyFacts(result.value(QStringLiteral("facts")).toObject());
    m_loaded = true;
    if (m_status != QStringLiteral("transcribing") && m_progress >= 0) {
        m_progress = -1;
        m_stage.clear();
        emit progressChanged();
    }
    emit changed();
}

void HistoryDetailModel::applyFacts(const QJsonObject &facts)
{
    m_title = facts.value(QStringLiteral("title")).toString();
    m_app = format::appName(facts.value(QStringLiteral("app_id")).toString());
    m_provider = facts.value(QStringLiteral("provider")).toString();
    m_model = format::modelName(facts.value(QStringLiteral("model")).toString());
    m_language = facts.value(QStringLiteral("language")).toString();
    m_status = facts.value(QStringLiteral("status")).toString();
    m_source = facts.value(QStringLiteral("source")).toString();
    m_rerunOf = facts.value(QStringLiteral("rerun_of")).toObject().value(QStringLiteral("id")).toString();
    m_durationSeconds = facts.value(QStringLiteral("duration_seconds")).toDouble();
    const QJsonObject audio = facts.value(QStringLiteral("audio")).toObject();
    m_audioRetained = audio.value(QStringLiteral("retained")).toBool();
    m_audioPath = audio.value(QStringLiteral("path")).toString();
    m_audioReasonCode = audio.value(QStringLiteral("reason")).toString();
    const QJsonObject insertion = facts.value(QStringLiteral("insertion")).toObject();
    m_outcome = insertion.value(QStringLiteral("outcome")).toString();
    m_backend = insertion.value(QStringLiteral("backend")).toObject().value(QStringLiteral("name")).toString();
    if (m_backend.isEmpty())
        m_backend = insertion.value(QStringLiteral("method")).toString();
    const QJsonObject timings = facts.value(QStringLiteral("timings")).toObject();
    m_stopToInsertMs = timings.isEmpty() ? -1 : qint64(timings.value(QStringLiteral("stop_to_insert_ms")).toDouble());
    m_transcribeMs = timings.isEmpty() ? -1 : qint64(timings.value(QStringLiteral("transcribe_ms")).toDouble());
    const QDateTime when = QDateTime::fromString(facts.value(QStringLiteral("created_at")).toString(), Qt::ISODate);
    QStringList line;
    if (when.isValid())
        line.append(format::clock(when.toLocalTime()));
    if (m_status == QStringLiteral("completed") && !m_app.isEmpty() && m_source == QStringLiteral("dictation"))
        line.append(m_outcome == QStringLiteral("inserted") ? tr("inserted into %1").arg(m_app) : tr("into %1").arg(m_app));
    else if (m_source == QStringLiteral("rerun"))
        line.append(tr("re-run of %1").arg(shortId(m_rerunOf)));
    else if (m_source == QStringLiteral("audioImport"))
        line.append(tr("imported audio"));
    if (m_status == QStringLiteral("transcribing"))
        line.append(tr("transcribing"));
    else if (m_status == QStringLiteral("failed") || m_status == QStringLiteral("cancelled"))
        line.append(m_status);
    m_whenLine = line.join(QStringLiteral(" · "));
    m_facts = buildFacts(facts);
}

QVariantList HistoryDetailModel::buildFacts(const QJsonObject &facts) const
{
    const QString none = QStringLiteral("—");
    QString engine = m_provider;
    if (!m_model.isEmpty())
        engine = engine.isEmpty() ? m_model : QStringLiteral("%1 · %2").arg(m_provider, m_model);
    QString mode = modeLabel();
    if (m_source == QStringLiteral("rerun"))
        mode = tr("Re-run · %1").arg(mode);
    else if (m_source == QStringLiteral("audioImport"))
        mode = tr("Import · %1").arg(mode);
    const QString error = facts.value(QStringLiteral("error")).toString();
    QString take = m_audioRetained ? tr("retained · %1").arg(seconds(m_durationSeconds)) : audioReason();
    if (!error.isEmpty())
        take = error;
    QString backend = m_backend;
    backend.replace(QLatin1Char('_'), QLatin1Char(' '));
    return {fact(tr("app"), m_app.isEmpty() ? none : m_app),
            fact(tr("stop to insert"), m_stopToInsertMs < 0 ? none : seconds(double(m_stopToInsertMs) / 1000.0)),
            fact(tr("mode"), mode.isEmpty() ? none : mode),
            fact(tr("insertion"), backend.isEmpty() ? none : backend),
            fact(tr("engine"), engine.isEmpty() ? none : engine),
            fact(tr("take"), take),
            fact(tr("language"), m_language.isEmpty() ? none : m_language),
            fact(tr("id"), shortId(m_id))};
}

QString HistoryDetailModel::modeLabel() const
{
    return HistoryModel::modeLabel(m_mode);
}

QString HistoryDetailModel::outcomeLabel() const
{
    if (m_outcome == QStringLiteral("inserted"))
        return tr("Inserted");
    if (m_outcome == QStringLiteral("copied_to_clipboard"))
        return tr("Copied");
    if (m_outcome == QStringLiteral("failed"))
        return tr("Not inserted");
    return QString();
}

QString HistoryDetailModel::enhancedMeta() const
{
    QStringList parts;
    if (!m_mode.isEmpty())
        parts.append(modeLabel());
    if (m_source == QStringLiteral("rerun"))
        parts.append(tr("re-run"));
    return parts.join(QStringLiteral(" · "));
}

QString HistoryDetailModel::rawMeta() const
{
    QStringList parts;
    if (!m_model.isEmpty())
        parts.append(m_model);
    if (m_transcribeMs >= 0)
        parts.append(seconds(double(m_transcribeMs) / 1000.0));
    return parts.join(QStringLiteral(" · "));
}

QString HistoryDetailModel::audioReason() const
{
    if (m_audioRetained)
        return QString();
    if (m_audioReasonCode == QStringLiteral("expired"))
        return tr("Audio expired.");
    return tr("Audio was not retained.");
}

QString HistoryDetailModel::audioMeta() const
{
    if (!m_audioRetained)
        return QString();
    return QStringLiteral("%1 · 16 kHz").arg(seconds(m_durationSeconds));
}

bool HistoryDetailModel::canRerun() const
{
    return m_loaded && m_audioRetained && m_status != QStringLiteral("transcribing") && m_progress < 0;
}

QString HistoryDetailModel::rerunBlockedReason() const
{
    if (!m_loaded)
        return QString();
    if (m_status == QStringLiteral("transcribing") || m_progress >= 0)
        return tr("A re-run is already running on this item.");
    if (!m_audioRetained)
        return audioReason();
    return QString();
}

void HistoryDetailModel::handleNotification(const QString &topic, const QJsonObject &payload)
{
    if (topic != QStringLiteral("job.progress") || m_jobId.isEmpty())
        return;
    if (payload.value(QStringLiteral("job_id")).toString() != m_jobId)
        return;
    m_stage = payload.value(QStringLiteral("stage")).toString();
    const bool ended = m_stage == QStringLiteral("done") || m_stage == QStringLiteral("failed")
        || m_stage == QStringLiteral("cancelled");
    m_progress = ended ? -1 : qBound(0.0, payload.value(QStringLiteral("progress")).toDouble(), 1.0);
    emit progressChanged();
    if (ended) {
        m_jobId.clear();
        reload();
    }
}

}  // namespace dettivo
