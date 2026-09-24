// The live meeting (fn-34 R2, ADR 0038): the disclosure gate, then
// `meetings.start` with the rail's choices; `meeting.state`,
// `meeting.segment` and the two `audio.level` sources while it records;
// the elapsed clock from `started_at`; the notes editor saved through the
// shared `NotesSaver` with `source = live`, kept until acknowledged; and
// `meetings.stop`, acknowledged the moment it is asked for (`finishing`
// holds from the request, the clock freezes on the recorded length) and
// followed through `stopping` and `transcribing` (the chunk counts from
// `job.progress`) into `completed`, which the route turns into the
// detail. A meeting started elsewhere (the bar, the command line) is
// adopted from its first `recording` event.
#pragma once

#include "daemon_link.h"
#include "notes_saver.h"

#include <QDate>
#include <QJsonObject>
#include <QObject>
#include <QString>
#include <QTimer>
#include <QSet>

namespace dettivo {

class LiveSegmentsModel;

class MeetingLiveModel : public QObject {
    Q_OBJECT
    Q_PROPERTY(QString meetingId READ meetingId NOTIFY changed)
    Q_PROPERTY(QString state READ state NOTIFY changed)
    Q_PROPERTY(bool active READ active NOTIFY changed)
    Q_PROPERTY(bool recording READ recording NOTIFY changed)
    Q_PROPERTY(bool finishing READ finishing NOTIFY changed)
    Q_PROPERTY(bool starting READ starting NOTIFY changed)
    Q_PROPERTY(QString title READ title NOTIFY changed)
    Q_PROPERTY(QString startedAt READ startedAt NOTIFY changed)
    Q_PROPERTY(QString engineLabel READ engineLabel NOTIFY changed)
    Q_PROPERTY(bool systemAudio READ systemAudio NOTIFY changed)
    Q_PROPERTY(QString micDevice READ micDevice NOTIFY changed)
    Q_PROPERTY(QString systemDevice READ systemDevice NOTIFY changed)
    Q_PROPERTY(QString error READ error NOTIFY changed)
    Q_PROPERTY(qint64 elapsedMs READ elapsedMs NOTIFY elapsedChanged)
    Q_PROPERTY(QString elapsedText READ elapsedText NOTIFY elapsedChanged)
    Q_PROPERTY(double micLevel READ micLevel NOTIFY levelsChanged)
    Q_PROPERTY(double micPeak READ micPeak NOTIFY levelsChanged)
    Q_PROPERTY(double systemLevel READ systemLevel NOTIFY levelsChanged)
    Q_PROPERTY(double systemPeak READ systemPeak NOTIFY levelsChanged)
    Q_PROPERTY(int chunksDone READ chunksDone NOTIFY progressChanged)
    Q_PROPERTY(int chunksTotal READ chunksTotal NOTIFY progressChanged)
    Q_PROPERTY(QObject *segments READ segmentsObject CONSTANT)
    Q_PROPERTY(QString notes READ notes NOTIFY notesChanged)
    Q_PROPERTY(QString notesState READ notesState NOTIFY notesChanged)
    Q_PROPERTY(bool disclosureAcknowledged READ disclosureAcknowledged NOTIFY disclosureChanged)
    Q_PROPERTY(QString disclosureAt READ disclosureAt NOTIFY disclosureChanged)
    Q_PROPERTY(QString disclosureMessage READ disclosureMessage NOTIFY disclosureChanged)

public:
    explicit MeetingLiveModel(DaemonLink *link, QObject *parent = nullptr);

    /// Starts a meeting with the rail's choices. The disclosure is read
    /// first: an acknowledged one starts at once, a pending one raises
    /// `disclosureRequired` and waits for `acknowledgeAndStart` or
    /// `dismissStart`. `provider` and `model` empty keep the selection;
    /// `expectedSpeakers` 0 lets the clustering decide; `analyze` and
    /// `diarize` are sent only when they hold a bool, so an untouched
    /// choice leaves the daemon on `[meetings.analysis] auto` and
    /// `[meetings.diarization] auto`.
    Q_INVOKABLE void start(const QString &title, bool systemAudio, const QString &provider, const QString &model,
                           int expectedSpeakers, const QVariant &analyze = QVariant(), const QVariant &diarize = QVariant());
    /// Starts the pending meeting with the acknowledgement on the request.
    Q_INVOKABLE void acknowledgeAndStart();
    /// Drops the pending start (`Not now`).
    Q_INVOKABLE void dismissStart();
    /// `meetings.stop`: `finishing` holds from this call; a refusal
    /// releases it and lands in `error` and `failed`. The states that
    /// follow arrive on the topic.
    Q_INVOKABLE void stop();
    /// The notes editor's text; written after a second of quiet.
    Q_INVOKABLE void setNotes(const QString &markdown);
    /// Writes the notes now (Stop, leaving the screen).
    Q_INVOKABLE void flushNotes();
    /// Reads `meetings.disclosure.get` for the rail's footer.
    Q_INVOKABLE void loadDisclosure();
    /// The disclosure text onto the clipboard.
    Q_INVOKABLE void copyDisclosure();
    /// Follows a meeting that is already recording (a `--id`, the bar).
    Q_INVOKABLE void attach(const QString &meetingId);

    QString meetingId() const { return m_id; }
    /// `idle`, `recording`, `stopping`, `stopped`, `transcribing`.
    QString state() const { return m_state; }
    bool active() const { return !m_id.isEmpty(); }
    /// Capturing, and no stop has been asked for.
    bool recording() const { return m_state == QStringLiteral("recording") && !m_stopRequested; }
    /// A stop was asked for, or the daemon is closing the takes or
    /// building the transcript.
    bool finishing() const;
    bool starting() const { return m_starting; }
    QString title() const { return m_title; }
    QString startedAt() const { return m_startedAt; }
    QString engineLabel() const { return m_engineLabel; }
    bool systemAudio() const { return m_systemAudio; }
    QString micDevice() const { return m_micDevice; }
    QString systemDevice() const { return m_systemDevice; }
    QString error() const { return m_error; }
    qint64 elapsedMs() const { return m_elapsedMs; }
    QString elapsedText() const;
    double micLevel() const { return m_micLevel; }
    double micPeak() const { return m_micPeak; }
    double systemLevel() const { return m_systemLevel; }
    double systemPeak() const { return m_systemPeak; }
    int chunksDone() const { return m_chunksDone; }
    int chunksTotal() const { return m_chunksTotal; }
    LiveSegmentsModel *segments() const { return m_segments; }
    QObject *segmentsObject() const;
    QString notes() const { return m_notes; }
    /// `Saved`, `Saving` or `Not saved: <reason>`.
    QString notesState() const { return m_notesState.isEmpty() ? m_saver->state() : m_notesState; }
    bool disclosureAcknowledged() const { return m_disclosureAcknowledged; }
    QString disclosureAt() const { return m_disclosureAt; }
    QString disclosureMessage() const { return m_disclosureMessage; }

    /// Fills the screen the way meeting-live.png shows it (the renders).
    void applySample(const QJsonObject &facts, const QJsonArray &segments);
    /// Applies a `meetings.disclosure.get` result (tests, the sample).
    void applyDisclosure(const QJsonObject &result, const QDate &today = QDate::currentDate());
    /// Applies an `audio.devices` result: the two device names.
    void applyDevices(const QJsonObject &result);
    /// The `meetings.start` parameters the last `start` built.
    QJsonObject pendingParams() const { return m_pending; }

public slots:
    void handleNotification(const QString &topic, const QJsonObject &payload);

signals:
    /// The disclosure has not been acknowledged; the dialog shows `message`.
    void disclosureRequired(const QString &message);
    /// `meetings.start` answered with the meeting.
    void started(const QString &meetingId);
    /// The meeting settled `completed`; the detail opens on it.
    void completed(const QString &meetingId);
    /// The daemon refused `action` (`start`, `stop`, `notes`) with `reason`;
    /// for a start the reason names the gate.
    void failed(const QString &action, const QString &reason);
    void changed();
    void elapsedChanged();
    void levelsChanged();
    void progressChanged();
    void notesChanged();
    void disclosureChanged();

private:
    void reconcile();
    void discoverActive(const QString &cursor, quint64 generation, QSet<QString> seen = {});
    void sendStart();
    void readDevices();
    void readMeetingFacts();
    void reset();
    void setState(const QString &state);
    void tick();
    static QString gateReason(const QJsonObject &error);

    DaemonLink *m_link;
    LiveSegmentsModel *m_segments;
    NotesSaver *m_saver;
    QTimer m_clock;
    QJsonObject m_pending;
    quint64 m_reconcileGeneration = 0;
    QString m_attachingId;
    QString m_id, m_state = QStringLiteral("idle"), m_title, m_startedAt, m_engineLabel, m_error;
    QString m_micDevice, m_systemDevice, m_notes, m_notesState, m_jobId;
    QString m_disclosureAt, m_disclosureMessage;
    qint64 m_elapsedMs = 0;
    qint64 m_startedMonotonicMs = 0;
    double m_micLevel = 0, m_micPeak = 0, m_systemLevel = 0, m_systemPeak = 0;
    int m_chunksDone = 0, m_chunksTotal = 0;
    bool m_systemAudio = true;
    bool m_starting = false;
    bool m_disclosureAcknowledged = false;
    bool m_stopRequested = false;
    bool m_sample = false;
};

}  // namespace dettivo
