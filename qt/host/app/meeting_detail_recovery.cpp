// Re-run on a meeting whose transcription did not finish (fn-76): a
// failed, partial, cancelled or stopped meeting is retried through
// `meetings.recover`, which the daemon accepts only while the meeting's
// audio is retained. `meetings.status.recoverable` is the daemon's own
// list of the meetings it can retry (their audio validated), so the
// detail asks it once per read instead of guessing from the row. A
// completed meeting keeps the reserved re-run.
#include "meeting_detail_model.h"

#include <QJsonArray>

namespace dettivo {

namespace {

bool retryable(const QString &status)
{
    return status == QStringLiteral("failed") || status == QStringLiteral("partial") || status == QStringLiteral("cancelled")
        || status == QStringLiteral("stopped");
}

}  // namespace

void MeetingDetailModel::checkRecovery()
{
    if (!retryable(m_status)) {
        m_recoverable = m_recoveryChecked = m_finalizing = false;
        return;
    }
    if (m_link == nullptr || !m_link->connected())
        return;
    const QString id = m_id;
    const quint64 generation = m_generation;
    m_link->call(QStringLiteral("meetings.status"), {{QStringLiteral("meeting_id"), id}}, [this, id, generation](const QJsonObject &result, const QJsonObject &error) {
        if (id != m_id || generation != m_generation)
            return;
        bool listed = false;
        for (const QJsonValue &v : result.value(QStringLiteral("recoverable")).toArray())
            listed = listed || v.toObject().value(QStringLiteral("ref")).toObject().value(QStringLiteral("id")).toString() == id;
        m_finalizing = result.value(QStringLiteral("is_finalizing")).toBool(false)
            || result.value(QStringLiteral("status")).toString() == QStringLiteral("transcribing");
        m_recoverable = error.isEmpty() && listed && !m_finalizing;
        m_recoveryChecked = true;
        emit changed();
    });
}

QString MeetingDetailModel::rerunReason() const
{
    if (m_status == QStringLiteral("recording") || m_status == QStringLiteral("stopping"))
        return tr("The meeting is still recording.");
    if (m_status == QStringLiteral("transcribing") || m_finalizing)
        return tr("The meeting is being transcribed.");
    if (!retryable(m_status))
        return tr("A meeting re-run is reserved in the contract; import the audio again to transcribe it with another engine.");
    if (m_recoverable)
        return tr("Transcribe the meeting again from its retained audio.");
    if (!m_recoveryChecked)
        return tr("Checking the meeting's audio.");
    return tr("The meeting's audio is gone, so it cannot be transcribed again.");
}

}  // namespace dettivo
