// One meeting as `meetings.get` answers it (fn-34 R1, ADR 0038): the
// title and the facts line, the speakers with their talk time and share
// for the talk-time bar, the segments with their speaker, colour, raw and
// polished text, the notes (saved through the shared `NotesSaver`, which
// keeps a draft until the daemon acknowledged it), the analysis with its summary, decisions and
// action items, the diarization block and the audio facts. `load` reads
// a meeting; the speaker and analysis passes land here through
// `meeting.state` and `job.progress`; a rename relabels every row at once.
// The post-processing stages (ADR 0061) read the same facts as one list
// the strip draws: the transcript, the speakers and the analysis, each
// `pending`, `queued`, `running`, `done`, `failed` or `skipped`
// (`meeting_detail_stages.cpp`).
#pragma once

#include "daemon_link.h"
#include "notes_saver.h"

#include <QJsonArray>
#include <QJsonObject>
#include <QObject>
#include <QString>
#include <QStringList>
#include <QTimer>
#include <QVariantList>

namespace dettivo {

class MeetingDetailModel : public QObject {
    Q_OBJECT
    Q_PROPERTY(QString meetingId READ meetingId NOTIFY changed)
    Q_PROPERTY(bool loaded READ loaded NOTIFY changed)
    Q_PROPERTY(bool loading READ loading NOTIFY changed)
    Q_PROPERTY(QString error READ error NOTIFY changed)
    Q_PROPERTY(QString title READ title NOTIFY changed)
    Q_PROPERTY(QString factsLine READ factsLine NOTIFY changed)
    Q_PROPERTY(QString status READ status NOTIFY changed)
    Q_PROPERTY(bool partial READ partial NOTIFY changed)
    Q_PROPERTY(QString partialLine READ partialLine NOTIFY changed)
    Q_PROPERTY(QVariantList speakers READ speakers NOTIFY changed)
    Q_PROPERTY(QVariantList segments READ segments NOTIFY changed)
    Q_PROPERTY(bool hasPolished READ hasPolished NOTIFY changed)
    Q_PROPERTY(QString notes READ notes NOTIFY notesChanged)
    Q_PROPERTY(QString notesMeta READ notesMeta NOTIFY notesChanged)
    Q_PROPERTY(QString notesState READ notesState NOTIFY notesChanged)
    Q_PROPERTY(QString analysisStatus READ analysisStatus NOTIFY changed)
    Q_PROPERTY(QString summary READ summary NOTIFY changed)
    Q_PROPERTY(QStringList decisions READ decisions NOTIFY changed)
    Q_PROPERTY(QStringList actionItems READ actionItems NOTIFY changed)
    Q_PROPERTY(QString analysisMeta READ analysisMeta NOTIFY changed)
    Q_PROPERTY(QString analysisError READ analysisError NOTIFY changed)
    Q_PROPERTY(QString diarizationStatus READ diarizationStatus NOTIFY changed)
    Q_PROPERTY(QString diarizationLine READ diarizationLine NOTIFY changed)
    Q_PROPERTY(QString audioFacts READ audioFacts NOTIFY changed)
    Q_PROPERTY(QString exportFacts READ exportFacts NOTIFY changed)
    Q_PROPERTY(QString lengthText READ lengthText NOTIFY changed)
    Q_PROPERTY(double progress READ progress NOTIFY progressChanged)
    Q_PROPERTY(QString stage READ stage NOTIFY progressChanged)
    Q_PROPERTY(QString lastTab READ lastTab WRITE setLastTab NOTIFY lastTabChanged)
    Q_PROPERTY(QVariantList stages READ stages NOTIFY stagesChanged)
    Q_PROPERTY(QString processingState READ processingState NOTIFY stagesChanged)
    Q_PROPERTY(QString processingLine READ processingLine NOTIFY stagesChanged)

public:
    explicit MeetingDetailModel(DaemonLink *link, QObject *parent = nullptr);

    /// Reads a meeting; an empty id clears the detail.
    Q_INVOKABLE void load(const QString &id);
    /// Reads the current meeting again.
    Q_INVOKABLE void reload();
    /// Empties the detail.
    Q_INVOKABLE void clear();
    /// The notes editor's text; written after a second of quiet with
    /// `source = user` and kept until the daemon acknowledged it.
    Q_INVOKABLE void setNotes(const QString &markdown);
    /// Writes the notes now (leaving the screen, closing the app).
    Q_INVOKABLE void flushNotes();
    /// Follows a job (an analysis, a speaker pass) on this meeting.
    Q_INVOKABLE void trackJob(const QString &jobId, const QString &stage);
    /// Relabels a speaker across the loaded rows (after `meetings.speakers.rename`).
    Q_INVOKABLE void applyRename(const QString &speakerId, const QString &name);
    /// The title as the daemon stored it (after `meetings.rename`).
    Q_INVOKABLE void applyTitle(const QString &title);
    /// The speaker at a bar position, `{speakerId, name, colorIndex}`;
    /// empty past the end.
    Q_INVOKABLE QVariantMap speakerAt(int index) const;

    /// Fills the detail from a `meetings.get` result (tests, the sample).
    void apply(const QString &id, const QJsonObject &result);

    QString meetingId() const { return m_id; }
    bool loaded() const { return m_loaded; }
    bool loading() const { return m_loading; }
    QString error() const { return m_error; }
    QString title() const { return m_title; }
    /// `Wed 3 Sep · 11:05 to 11:46 · 41 min · whisper small · diarized`.
    QString factsLine() const;
    QString status() const { return m_status; }
    bool partial() const { return m_partial; }
    /// `Recovered after a crash · 12 of 14 chunks` for a partial meeting.
    QString partialLine() const;
    /// `{speakerId, name, colorIndex, talkMs, talk, share, shareText}` in
    /// swatch order.
    QVariantList speakers() const { return m_speakers; }
    /// `{time, speaker, speakerId, colorIndex, text, polished, gapBefore, startMs}`.
    QVariantList segments() const { return m_segments; }
    bool hasPolished() const { return m_hasPolished; }
    QString notes() const { return m_notes; }
    /// `2 sections · edited 11:31`.
    QString notesMeta() const;
    QString notesState() const { return m_saver->state(); }
    QString analysisStatus() const { return m_analysisStatus; }
    QString summary() const { return m_summary; }
    QStringList decisions() const { return m_decisions; }
    QStringList actionItems() const { return m_actionItems; }
    /// `Qwen3 4B · 11:47` once an analysis is ready.
    QString analysisMeta() const;
    QString analysisError() const { return m_analysisError; }
    QString diarizationStatus() const { return m_diarizationStatus; }
    /// What the rail says about the speakers: the pass's state, or the
    /// download that would make it available.
    QString diarizationLine() const;
    /// `2 tracks · 41 min · kept`.
    QString audioFacts() const;
    /// `md · txt · srt · vtt · json`.
    QString exportFacts() const;
    QString lengthText() const;
    double progress() const { return m_progress; }
    QString stage() const { return m_stage; }
    /// The tab a meeting reopens on (`transcript`, `notes`, `analysis`),
    /// kept in the state file as `[app] last_meeting_tab`.
    QString lastTab() const { return m_lastTab; }
    void setLastTab(const QString &tab);
    /// `{key, label, state, detail, progress}` for `transcript`,
    /// `speakers` and `analysis`; `progress` is -1 while unknown.
    QVariantList stages() const;
    /// `running` while a stage is pending, queued or running; `failed`
    /// once every stage settled and one failed; `done` otherwise.
    QString processingState() const;
    /// The sentence over the strip: what is ready and what runs next.
    QString processingLine() const;

public slots:
    void handleNotification(const QString &topic, const QJsonObject &payload);

signals:
    void changed();
    void notesChanged();
    void progressChanged();
    void lastTabChanged();
    /// Follows `changed` and `progressChanged`.
    void stagesChanged();
    /// The daemon refused the notes write with `reason`.
    void failed(const QString &action, const QString &reason);

private:
    void applySpeakers(const QJsonArray &speakers);
    static QJsonArray speakersBySource(const QJsonArray &segments);
    void applySegments(const QJsonArray &segments);
    void applyAnalysis(const QJsonObject &analysis);
    QVariantMap transcriptStage() const;
    QVariantMap speakersStage() const;
    QVariantMap analysisStage() const;
    /// A pass's progress, from the job followed under `stage`.
    double stageProgress(const QString &stage) const;
    /// Reads `meetings.status` for the job of this meeting's running
    /// speaker pass, so its `job.progress` is followed and no other's.
    void bindPassJob();

    DaemonLink *m_link;
    NotesSaver *m_saver;
    QString m_id, m_error, m_title, m_status, m_startedAt, m_endedAt, m_language, m_provider, m_model;
    QString m_notes, m_notesSource, m_notesUpdatedAt;
    QString m_analysisStatus, m_summary, m_analysisMeta, m_analysisModel, m_analysisAt, m_analysisError;
    QString m_diarizationStatus, m_diarizationError, m_diarizationEngine, m_stage, m_jobId;
    QStringList m_decisions, m_actionItems;
    QString m_lastTab = QStringLiteral("transcript");
    QVariantList m_speakers, m_segments;
    qint64 m_durationMs = 0;
    double m_progress = -1;
    int m_chunksCompleted = 0, m_chunksTotal = 0, m_microphoneTakes = 0;
    /// Segments the row carries without a `speaker_id`.
    int m_unassignedSegments = 0;
    /// Bumped by every read and by `clear`; an answer from an older
    /// generation is dropped, so two reads of one meeting that return
    /// out of order never put a stale row over a newer one.
    quint64 m_generation = 0;
    bool m_loaded = false, m_loading = false, m_partial = false, m_hasPolished = false;
    bool m_systemAudio = false, m_audioKept = false;
};

}  // namespace dettivo
