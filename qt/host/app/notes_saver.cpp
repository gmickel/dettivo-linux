#include "notes_saver.h"

#include <QJsonObject>

namespace dettivo {

namespace {

constexpr int kDebounceMs = 1000;

QString message(const QJsonObject &error)
{
    const QString text = error.value(QStringLiteral("message")).toString();
    return text.isEmpty() ? QObject::tr("the daemon did not answer") : text;
}

}  // namespace

NotesSaver::NotesSaver(DaemonLink *link, const QString &source, QObject *parent)
    : QObject(parent), m_link(link), m_source(source)
{
    m_debounce.setSingleShot(true);
    m_debounce.setInterval(kDebounceMs);
    connect(&m_debounce, &QTimer::timeout, this, &NotesSaver::send);
    if (m_link != nullptr) {
        connect(m_link, &DaemonLink::connectedChanged, this, [this](bool connected) {
            if (!connected)
                return;
            send();
            sendParked();
        });
    }
}

void NotesSaver::edit(const QString &meetingId, const QString &markdown)
{
    if (meetingId != m_id)
        open(meetingId);
    if (markdown == m_draft)
        return;
    m_draft = markdown;
    ++m_draftRev;
    m_state = tr("Saving");
    emit stateChanged();
    m_debounce.start();
}

QString NotesSaver::open(const QString &meetingId)
{
    if (meetingId == m_id)
        return QString();
    leave();
    m_id = meetingId;
    m_ackedRev = m_draftRev;
    m_inFlight = false;
    m_draft.clear();
    m_state.clear();
    m_updatedAt.clear();
    if (!m_parked.contains(meetingId))
        return QString();
    m_draft = m_parked.take(meetingId);
    ++m_draftRev;
    m_state = tr("Saving");
    send();
    return m_draft;
}

void NotesSaver::flush()
{
    m_debounce.stop();
    send();
}

void NotesSaver::leave()
{
    m_debounce.stop();
    if (m_id.isEmpty())
        return;
    if (dirty()) {
        if (!m_inFlight)
            send();
        if (dirty())
            m_parked.insert(m_id, m_draft);
    }
    m_id.clear();
    m_draft.clear();
    m_ackedRev = m_draftRev;
    m_inFlight = false;
    m_state.clear();
}

void NotesSaver::send()
{
    if (!dirty() || m_id.isEmpty() || m_inFlight)
        return;
    if (m_link == nullptr || !m_link->connected()) {
        m_state = tr("Not saved: the daemon is not connected");
        emit stateChanged();
        return;
    }
    m_inFlight = true;
    const QString id = m_id;
    const QString draft = m_draft;
    const int rev = m_draftRev;
    const QJsonObject params{{QStringLiteral("meeting_id"), id}, {QStringLiteral("markdown"), draft}, {QStringLiteral("source"), m_source}};
    m_link->call(QStringLiteral("meetings.notes.set"), params, [this, id, draft, rev](const QJsonObject &result, const QJsonObject &error) {
        if (id != m_id) {
            // The meeting was left meanwhile: settle its parked draft.
            if (error.isEmpty() && m_parked.value(id) == draft)
                m_parked.remove(id);
            else if (error.isEmpty())
                sendParked();
            return;
        }
        if (rev <= m_ackedRev)
            return;
        m_inFlight = false;
        if (!error.isEmpty()) {
            m_state = tr("Not saved: %1").arg(message(error));
            emit failed(message(error));
            emit stateChanged();
            return;
        }
        m_ackedRev = rev;
        if (dirty()) {
            send();
        } else {
            m_state = tr("Saved");
            m_updatedAt = result.value(QStringLiteral("updated_at")).toString(m_updatedAt);
        }
        emit stateChanged();
    });
}

// Parked drafts go out one call each; a draft that is edited again
// before its answer stays parked and goes out after it.
void NotesSaver::sendParked()
{
    if (m_link == nullptr || !m_link->connected())
        return;
    const QHash<QString, QString> parked = m_parked;
    for (auto it = parked.cbegin(); it != parked.cend(); ++it) {
        const QString id = it.key();
        const QString draft = it.value();
        if (m_parkedSending.contains(id))
            continue;
        m_parkedSending.insert(id);
        const QJsonObject params{{QStringLiteral("meeting_id"), id}, {QStringLiteral("markdown"), draft}, {QStringLiteral("source"), m_source}};
        m_link->call(QStringLiteral("meetings.notes.set"), params, [this, id, draft](const QJsonObject &, const QJsonObject &error) {
            m_parkedSending.remove(id);
            if (!error.isEmpty())
                return;
            if (m_parked.value(id) == draft)
                m_parked.remove(id);
            else if (m_parked.contains(id))
                sendParked();
        });
    }
}

}  // namespace dettivo
