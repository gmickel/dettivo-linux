// The notes save shared by the live meeting and the detail (ADR 0048):
// the editor's draft and the daemon's acknowledgement are two revisions,
// and a draft counts as dirty until `meetings.notes.set` answered for it.
// Saves go out one at a time per meeting after a second of quiet, a
// failed or unanswered save keeps its draft, an edit during a save is
// sent after the answer, and a draft that cannot be sent when the
// meeting is left (the link is down, the save is in flight) is parked
// under its meeting id and sent on the next connection or the next
// visit. A late answer is matched by meeting and revision, so it never
// marks newer text saved or touches another meeting.
#pragma once

#include "daemon_link.h"

#include <QHash>
#include <QObject>
#include <QSet>
#include <QString>
#include <QTimer>

namespace dettivo {

class NotesSaver : public QObject {
    Q_OBJECT

public:
    /// `source` is what the write carries (`user`, `live`).
    NotesSaver(DaemonLink *link, const QString &source, QObject *parent = nullptr);

    /// The editor's text for `meetingId`; a different meeting than the
    /// current one is opened first.
    void edit(const QString &meetingId, const QString &markdown);
    /// Makes `meetingId` current. Returns the parked draft it still owes
    /// the daemon (and sends it), or a null string when there is none.
    QString open(const QString &meetingId);
    /// Sends a dirty draft now.
    void flush();
    /// Leaves the current meeting: a dirty draft is sent when it can be
    /// and parked otherwise.
    void leave();

    /// True while the daemon has not acknowledged the current draft.
    bool dirty() const { return m_draftRev > m_ackedRev; }
    /// Empty, `Saving`, `Saved` or `Not saved: <reason>`.
    QString state() const { return m_state; }
    /// The `updated_at` of the last acknowledged save.
    QString updatedAt() const { return m_updatedAt; }
    QString meetingId() const { return m_id; }

signals:
    void stateChanged();
    /// The daemon refused or did not answer a save with `reason`.
    void failed(const QString &reason);

private:
    void send();
    void sendParked();

    DaemonLink *m_link;
    QString m_source;
    QTimer m_debounce;
    QString m_id, m_draft, m_state, m_updatedAt;
    int m_draftRev = 0, m_ackedRev = 0;
    bool m_inFlight = false;
    QHash<QString, QString> m_parked;
    QSet<QString> m_parkedSending;
};

}  // namespace dettivo
