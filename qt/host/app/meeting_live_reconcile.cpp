#include "meeting_live_model.h"
#include "live_segments_model.h"
#include "status_format.h"

#include <QDateTime>
#include <QJsonArray>

namespace dettivo {

void MeetingLiveModel::reconcile()
{
    if (m_link == nullptr || !m_link->connected() || m_sample)
        return;
    if (!m_id.isEmpty()) {
        attach(m_id);
        return;
    }
    discoverActive({}, ++m_reconcileGeneration);
}

void MeetingLiveModel::discoverActive(const QString &cursor, quint64 generation, QSet<QString> seen)
{
    seen.insert(cursor);
    m_link->call(QStringLiteral("meetings.list"),
                 {{QStringLiteral("limit"), 100}, {QStringLiteral("cursor"), cursor.isEmpty() ? QJsonValue::Null : QJsonValue(cursor)}},
                 [this, generation, seen](const QJsonObject &result, const QJsonObject &error) {
        if (generation != m_reconcileGeneration || !error.isEmpty())
            return;
        for (const auto &value : result.value(QStringLiteral("items")).toArray()) {
            const auto row = value.toObject();
            const QString status = row.value(QStringLiteral("status")).toString();
            const QString id = row.value(QStringLiteral("ref")).toObject().value(QStringLiteral("id")).toString();
            if (!id.isEmpty() && (status == QStringLiteral("recording") || status == QStringLiteral("stopping"))) {
                attach(id);
                return;
            }
        }
        const QString next = result.value(QStringLiteral("next_cursor")).toString();
        if (!next.isEmpty() && !seen.contains(next))
            discoverActive(next, generation, seen);
    });
}

void MeetingLiveModel::attach(const QString &meetingId)
{
    if (meetingId.isEmpty() || m_link == nullptr || !m_link->connected())
        return;
    const quint64 generation = ++m_reconcileGeneration;
    m_attachingId = meetingId;
    m_link->call(QStringLiteral("meetings.status"), {{QStringLiteral("meeting_id"), meetingId}},
                 [this, meetingId, generation](const QJsonObject &result, const QJsonObject &error) {
        if (generation != m_reconcileGeneration)
            return;
        m_attachingId.clear();
        if (!error.isEmpty())
            return;
        const QString state = result.value(QStringLiteral("status")).toString();
        if (state == QStringLiteral("completed") || state == QStringLiteral("partial")
            || state == QStringLiteral("cancelled") || state == QStringLiteral("failed")) {
            const bool wasCurrent = meetingId == m_id;
            if (wasCurrent)
                reset();
            if (state == QStringLiteral("completed") || state == QStringLiteral("partial"))
                emit completed(meetingId);
            if (wasCurrent)
                discoverActive({}, ++m_reconcileGeneration);
            return;
        }
        if (meetingId != m_id) {
            reset();
            m_id = meetingId;
        }
        m_jobId = result.value(QStringLiteral("job")).toObject().value(QStringLiteral("job_id")).toString();
        const QJsonObject capture = result.value(QStringLiteral("capture")).toObject();
        if (m_startedAt.isEmpty()) {
            const qint64 duration = qint64(capture.value(QStringLiteral("duration_ms")).toDouble());
            m_startedAt = QDateTime::currentDateTimeUtc().addMSecs(-duration).toString(Qt::ISODateWithMs);
            m_segments->setStartedAt(m_startedAt);
        }
        m_systemAudio = capture.value(QStringLiteral("system_audio")).toBool(m_systemAudio);
        setState(state.isEmpty() ? QStringLiteral("recording") : state);
        if (recording() && !m_clock.isActive())
            m_clock.start();
        readMeetingFacts();
        emit changed();
    });
}

void MeetingLiveModel::readMeetingFacts()
{
    const QString id = m_id;
    if (id.isEmpty() || m_link == nullptr || !m_link->connected())
        return;
    m_link->call(QStringLiteral("meetings.get"), {{QStringLiteral("meeting_id"), id}},
                 [this, id](const QJsonObject &result, const QJsonObject &error) {
        if (id != m_id || !error.isEmpty())
            return;
        const QString provider = result.value(QStringLiteral("stt_provider_id")).toString();
        const QString model = format::modelName(result.value(QStringLiteral("stt_model_id")).toString());
        m_engineLabel = provider.isEmpty() ? QString() : (model.isEmpty() ? provider : provider + QLatin1Char(' ') + model);
        emit changed();
    });
}

}  // namespace dettivo
