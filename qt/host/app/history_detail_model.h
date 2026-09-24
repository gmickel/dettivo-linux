// One dictation as `transcripts.get` answers it (fn-22 R1): the inserted
// text with the mode that produced it, the raw text, the take (retained
// or why not), and the facts grid with the app, the engine, the insertion
// backend and the stop-to-insert time. `load` reads an item; a re-run's
// progress and its end land here through `job.progress`.
#pragma once

#include "daemon_link.h"

#include <QJsonObject>
#include <QObject>
#include <QString>
#include <QVariantList>

namespace dettivo {

class HistoryDetailModel : public QObject {
    Q_OBJECT
    Q_PROPERTY(QString itemId READ itemId NOTIFY changed)
    Q_PROPERTY(bool loaded READ loaded NOTIFY changed)
    Q_PROPERTY(bool loading READ loading NOTIFY changed)
    Q_PROPERTY(QString error READ error NOTIFY changed)
    Q_PROPERTY(QString title READ title NOTIFY changed)
    Q_PROPERTY(QString whenLine READ whenLine NOTIFY changed)
    Q_PROPERTY(QString enhancedText READ enhancedText NOTIFY changed)
    Q_PROPERTY(QString rawText READ rawText NOTIFY changed)
    Q_PROPERTY(QString mode READ mode NOTIFY changed)
    Q_PROPERTY(QString modeLabel READ modeLabel NOTIFY changed)
    Q_PROPERTY(QString outcome READ outcome NOTIFY changed)
    Q_PROPERTY(QString outcomeLabel READ outcomeLabel NOTIFY changed)
    Q_PROPERTY(QString enhancedMeta READ enhancedMeta NOTIFY changed)
    Q_PROPERTY(QString rawMeta READ rawMeta NOTIFY changed)
    Q_PROPERTY(QString provider READ provider NOTIFY changed)
    Q_PROPERTY(QString model READ model NOTIFY changed)
    Q_PROPERTY(QString status READ status NOTIFY changed)
    Q_PROPERTY(QString source READ source NOTIFY changed)
    Q_PROPERTY(QString rerunOf READ rerunOf NOTIFY changed)
    Q_PROPERTY(bool audioRetained READ audioRetained NOTIFY changed)
    Q_PROPERTY(QString audioPath READ audioPath NOTIFY changed)
    Q_PROPERTY(QString audioReason READ audioReason NOTIFY changed)
    Q_PROPERTY(QString audioMeta READ audioMeta NOTIFY changed)
    Q_PROPERTY(QVariantList facts READ facts NOTIFY changed)
    Q_PROPERTY(bool canRerun READ canRerun NOTIFY changed)
    Q_PROPERTY(QString rerunBlockedReason READ rerunBlockedReason NOTIFY changed)
    Q_PROPERTY(double progress READ progress NOTIFY progressChanged)
    Q_PROPERTY(QString stage READ stage NOTIFY progressChanged)

public:
    explicit HistoryDetailModel(DaemonLink *link, QObject *parent = nullptr);

    /// Reads an item; an empty id clears the detail.
    Q_INVOKABLE void load(const QString &id);
    /// Reads the current item again (after a re-run ended).
    Q_INVOKABLE void reload();
    /// Follows a re-run job on this item.
    Q_INVOKABLE void trackJob(const QString &jobId);
    /// Fills the detail from a `transcripts.get` result (tests, the sample).
    void apply(const QString &id, const QJsonObject &result);
    /// Empties the detail.
    void clear();

    QString itemId() const { return m_id; }
    bool loaded() const { return m_loaded; }
    bool loading() const { return m_loading; }
    QString error() const { return m_error; }
    QString title() const { return m_title; }
    QString whenLine() const { return m_whenLine; }
    QString enhancedText() const { return m_enhanced; }
    QString rawText() const { return m_raw; }
    QString mode() const { return m_mode; }
    QString modeLabel() const;
    QString outcome() const { return m_outcome; }
    QString outcomeLabel() const;
    QString enhancedMeta() const;
    QString rawMeta() const;
    QString provider() const { return m_provider; }
    QString model() const { return m_model; }
    QString status() const { return m_status; }
    QString source() const { return m_source; }
    QString rerunOf() const { return m_rerunOf; }
    bool audioRetained() const { return m_audioRetained; }
    QString audioPath() const { return m_audioPath; }
    /// Why there is no take: `Audio was not retained.` or `Audio expired.`
    QString audioReason() const;
    /// `4.2 s · 16 kHz` (the take is 16 kHz mono by the store's contract).
    QString audioMeta() const;
    /// The facts grid, row-major over two columns: `{label, value}` maps.
    QVariantList facts() const { return m_facts; }
    bool canRerun() const;
    QString rerunBlockedReason() const;
    double progress() const { return m_progress; }
    QString stage() const { return m_stage; }

public slots:
    void handleNotification(const QString &topic, const QJsonObject &payload);

signals:
    void changed();
    void progressChanged();

private:
    void applyFacts(const QJsonObject &facts);
    QVariantList buildFacts(const QJsonObject &facts) const;

    DaemonLink *m_link;
    QString m_id, m_error, m_title, m_whenLine, m_enhanced, m_raw, m_mode, m_outcome, m_backend;
    QString m_provider, m_model, m_language, m_app, m_status, m_source, m_rerunOf, m_audioPath;
    QString m_audioReasonCode, m_stage, m_jobId;
    QVariantList m_facts;
    double m_durationSeconds = 0;
    double m_progress = -1;
    qint64 m_stopToInsertMs = -1;
    qint64 m_transcribeMs = -1;
    bool m_loaded = false;
    bool m_loading = false;
    bool m_audioRetained = false;
};

}  // namespace dettivo
