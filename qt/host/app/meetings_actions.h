// What the meetings screens do to a meeting beyond reading it (fn-34 R1,
// R3, ADR 0038): a speaker renamed through `meetings.speakers.rename`
// with the remembered names from `suggest`, the analysis and the speaker
// pass run again, a delete under the configured artifact policy, a
// partial meeting recovered or discarded, and an audio file imported
// into a meeting through an upload transfer and `transcripts.import`
// with the job followed to its row. Every outcome is a signal the screens
// and the list follow; every refusal carries the daemon's reason.
#pragma once

#include "daemon_link.h"

#include <QJsonObject>
#include <QObject>
#include <QString>
#include <QVariantList>

#include <memory>

namespace dettivo {

class ConfigBinding;
class UploadSource;

class MeetingsActions : public QObject {
    Q_OBJECT
    Q_PROPERTY(bool busy READ busy NOTIFY busyChanged)
    Q_PROPERTY(QVariantList suggestions READ suggestions NOTIFY suggestionsChanged)
    Q_PROPERTY(QVariantList providers READ providers NOTIFY providersChanged)
    Q_PROPERTY(QString selectedProvider READ selectedProvider NOTIFY providersChanged)
    Q_PROPERTY(QString selectedModel READ selectedModel NOTIFY providersChanged)
    Q_PROPERTY(QString deletePolicy READ deletePolicy NOTIFY policyChanged)
    Q_PROPERTY(bool importing READ importing NOTIFY importChanged)
    Q_PROPERTY(double importProgress READ importProgress NOTIFY importChanged)
    Q_PROPERTY(QString importStage READ importStage NOTIFY importChanged)
    Q_PROPERTY(QString importMeetingId READ importMeetingId NOTIFY importChanged)

public:
    /// One megabyte per upload chunk, the contract's `chunk_max_bytes`.
    static constexpr qint64 kChunkBytes = 1024 * 1024;

    explicit MeetingsActions(DaemonLink *link, ConfigBinding *config, QObject *parent = nullptr);
    ~MeetingsActions() override;

    /// `meetings.speakers.rename`; an empty name restores the label.
    Q_INVOKABLE void rename(const QString &meetingId, const QString &speakerId, const QString &name);
    /// `meetings.rename` (ADR 0061): the meeting's title, trimmed; an
    /// empty one is refused before the daemon is asked.
    Q_INVOKABLE void retitle(const QString &meetingId, const QString &title);
    /// `meetings.speakers.suggest` for the popover's list.
    Q_INVOKABLE void suggest(const QString &prefix);
    /// `meetings.analyze`; `force` regenerates a ready analysis.
    Q_INVOKABLE void analyze(const QString &meetingId, bool force);
    /// `meetings.diarize`; `speakers` 0 lets the clustering decide.
    Q_INVOKABLE void diarize(const QString &meetingId, int speakers);
    /// `meetings.delete` under `policy` (empty takes `[meetings]
    /// delete_artifact_policy`).
    Q_INVOKABLE void remove(const QString &meetingId, const QString &policy);
    /// `meetings.recover` on a retained meeting the daemon can retry.
    Q_INVOKABLE void recover(const QString &meetingId);
    /// `meetings.cancel` stops a pending captured or imported meeting job.
    Q_INVOKABLE void cancel(const QString &meetingId);
    Q_INVOKABLE void discard(const QString &meetingId);
    /// Reads `speech.providers.list` and `speech.selection.get` for the
    /// engine pickers; only meeting-capable providers are kept.
    Q_INVOKABLE void loadProviders();
    /// The rail's engine choice: another provider goes through
    /// `speech.selection.set` (the daemon runs one provider), a model of
    /// the selected one is written as `[speech] meeting_model`.
    Q_INVOKABLE void pickEngine(const QString &providerId, const QString &modelId);
    /// Uploads `path` and runs `transcripts.import { target_kind: meeting }`
    /// with the dialog's choices; the job is followed on `importChanged`.
    Q_INVOKABLE void importFile(const QString &path, const QString &provider, const QString &model,
                                const QString &language, bool diarize, bool analyze);
    /// Cancels this upload, including its own meeting if acceptance arrives late.
    Q_INVOKABLE void cancelImport();
    /// The facts of a file for the import dialog: `{name, bytes, sizeText,
    /// durationMs, durationText, ok, reason}` (the duration from a WAV
    /// header; unknown for other containers).
    Q_INVOKABLE QVariantMap probeFile(const QString &path) const;

    bool busy() const { return m_busy; }
    /// `{name, uses}` most recently used first.
    QVariantList suggestions() const { return m_suggestions; }
    /// `{id, label, models: [{id, label, downloaded}]}` per meeting-capable provider.
    QVariantList providers() const { return m_providers; }
    QString selectedProvider() const { return m_selectedProvider; }
    QString selectedModel() const { return m_selectedModel; }
    /// `[meetings] delete_artifact_policy` as the binding holds it.
    QString deletePolicy() const;
    bool importing() const { return !m_importJob.isEmpty(); }
    double importProgress() const { return m_importProgress; }
    QString importStage() const { return m_importStage; }
    QString importMeetingId() const { return m_importMeeting; }

    /// Fills the pickers from the two results (tests, the sample).
    void applyProviders(const QJsonObject &providers, const QJsonObject &selection);
    /// Fills the suggestions from a `suggest` result (tests, the sample).
    void applySuggestions(const QJsonArray &names);

public slots:
    void handleNotification(const QString &topic, const QJsonObject &payload);

signals:
    void renamed(const QString &meetingId, const QString &speakerId, const QString &name, int segmentsUpdated);
    /// The daemon stored `title` for the meeting.
    void retitled(const QString &meetingId, const QString &title);
    void analysisStarted(const QString &meetingId, const QString &jobId);
    void diarizeStarted(const QString &meetingId, const QString &jobId);
    void removed(const QString &meetingId);
    void recovered(const QString &meetingId);
    void cancelled(const QString &meetingId);
    void discarded(const QString &meetingId);
    /// The import is running on its new meeting.
    void importStarted(const QString &meetingId, const QString &jobId);
    /// The import's job ended (`done`, `failed` or `cancelled`).
    void importEnded(const QString &meetingId, const QString &stage);
    /// The daemon (or the file system) refused `action` (`rename`,
    /// `retitle`, `analyze`, `diarize`, `delete`, `recover`, `discard`,
    /// `import`).
    void failed(const QString &action, const QString &reason);
    void busyChanged();
    void suggestionsChanged();
    void providersChanged();
    void policyChanged();
    void importChanged();

private:
    struct Upload {
        QString transferId, filename, provider, model, language;
        std::shared_ptr<UploadSource> source;
        QString digest;
        quint64 generation = 0;
        qint64 seq = 1;
        qint64 chunkBytes = 0, lineBytes = 0;
        bool diarize = true;
        bool analyze = true;
    };
    void setBusy(bool busy);
    bool ready(const QString &action);
    void abortUpload(const QString &reason);
    void cancelUpload();
    void negotiateUpload(Upload upload, qint64 transferChunkMax);
    void pushChunk(Upload upload);
    void commitUpload(Upload upload);
    static QString reasonOf(const QJsonObject &error);
    static QString cancellationError(const QString &meetingId, const QJsonObject &result, const QJsonObject &error);

    DaemonLink *m_link;
    ConfigBinding *m_config;
    QVariantList m_suggestions, m_providers;
    /// What the picker shows: the daemon's selection when it is offered,
    /// the first offered provider otherwise.
    QString m_selectedProvider, m_selectedModel;
    /// The provider the daemon actually has selected, offered or not.
    QString m_daemonProvider;
    QString m_importJob, m_importMeeting, m_importStage;
    double m_importProgress = 0;
    bool m_busy = false;
    bool m_uploadActive = false;
    quint64 m_uploadGeneration = 0;
    QString m_uploadTransfer;
};

}  // namespace dettivo
