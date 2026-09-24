// The facts the Home surface states (fn-17 R2): whether the daemon is
// there and healthy, what dictation is doing, the hotkeys and the app the
// next take lands in, the input, the socket mode and the platform, plus
// the live level for the waveform. Read from `system.*`, `config.get`,
// `insert.target`, `audio.devices` and `dictation.status` on every
// connect, kept current by the event stream.
#pragma once

#include "daemon_link.h"

#include <QJsonObject>
#include <QObject>
#include <QString>
#include <QTimer>
#include <QVariantList>

namespace dettivo {

class ConfigBinding;

class StatusModel : public QObject {
    Q_OBJECT
    Q_PROPERTY(bool daemonConnected READ daemonConnected NOTIFY connectionChanged)
    Q_PROPERTY(QString daemonState READ daemonState NOTIFY connectionChanged)
    Q_PROPERTY(bool healthOk READ healthOk NOTIFY factsChanged)
    Q_PROPERTY(QString dictationState READ dictationState NOTIFY dictationChanged)
    Q_PROPERTY(QString dictationError READ dictationError NOTIFY dictationChanged)
    Q_PROPERTY(QString holdChord READ holdChord NOTIFY factsChanged)
    Q_PROPERTY(QString toggleChord READ toggleChord NOTIFY factsChanged)
    Q_PROPERTY(QString targetApp READ targetApp NOTIFY factsChanged)
    Q_PROPERTY(QString socketMode READ socketMode NOTIFY factsChanged)
    Q_PROPERTY(QString gpu READ gpu NOTIFY factsChanged)
    Q_PROPERTY(QString compositor READ compositor NOTIFY factsChanged)
    Q_PROPERTY(QString inputName READ inputName NOTIFY factsChanged)
    Q_PROPERTY(QString inputRate READ inputRate CONSTANT)
    Q_PROPERTY(QString restState READ restState NOTIFY factsChanged)
    Q_PROPERTY(QString mcpHosts READ mcpHosts NOTIFY factsChanged)
    Q_PROPERTY(QString lastCall READ lastCall NOTIFY factsChanged)
    Q_PROPERTY(QString clock READ clock NOTIFY clockChanged)
    Q_PROPERTY(QVariantList levels READ levels NOTIFY levelsChanged)
    Q_PROPERTY(QString version READ version NOTIFY factsChanged)

public:
    explicit StatusModel(DaemonLink *link, ConfigBinding *config, QObject *parent = nullptr);

    /// The topics the app subscribes to.
    static QStringList topics();
    /// How many level samples the waveform keeps.
    static constexpr int kLevelSamples = 96;
    /// How long a daemon may stay unreachable before the app says so.
    static constexpr int kAwayGraceMs = 1500;

    /// Subscribes, refreshes on every connect and starts the clock.
    void start();
    /// Re-reads every fact.
    void refresh();

    bool daemonConnected() const { return m_link != nullptr && m_link->connected(); }
    /// `connecting` before the first answer, `connected`, or `away`.
    QString daemonState() const { return m_daemonState; }
    bool healthOk() const { return m_healthOk; }
    QString dictationState() const { return m_dictationState; }
    QString dictationError() const { return m_dictationError; }
    QString holdChord() const { return m_holdChord; }
    QString toggleChord() const { return m_toggleChord; }
    QString targetApp() const { return m_targetApp; }
    QString socketMode() const { return m_socketMode; }
    QString gpu() const { return m_gpu; }
    QString compositor() const { return m_compositor; }
    QString inputName() const { return m_inputName; }
    QString inputRate() const { return QStringLiteral("16 kHz"); }
    QString restState() const { return m_restState; }
    QString mcpHosts() const { return m_mcpHosts; }
    void setMcpHosts(const QStringList &hosts);
    QString lastCall() const { return m_lastCall; }
    QString clock() const { return m_clock; }
    QVariantList levels() const { return m_levels; }
    QString version() const { return m_version; }

    Q_INVOKABLE void startDictation();
    Q_INVOKABLE void stopDictation();
    Q_INVOKABLE void toggleDictation();

    /// The baseline's facts with no daemon (the visual renders).
    void applySample(const QJsonObject &sample);
    /// Freezes the clock line (the visual renders).
    void setClockForTesting(const QString &text);

public slots:
    void handleNotification(const QString &topic, const QJsonObject &payload);

signals:
    void connectionChanged();
    void factsChanged();
    void dictationChanged();
    void clockChanged();
    void levelsChanged();

private:
    void onConnected(bool connected);
    void readHealth();
    void setDictationError(const QString &message);
    void readCapabilities();
    void readTarget();
    void readDevices();
    void readDictation();
    void readConfig();
    void tickClock();
    void setDaemonState(const QString &state);

    DaemonLink *m_link;
    ConfigBinding *m_config;
    QTimer m_clockTimer;
    QTimer m_awayTimer;
    bool m_clockFrozen = false;
    QString m_daemonState = QStringLiteral("connecting");
    bool m_healthOk = true;
    QString m_dictationError;
    QString m_dictationState = QStringLiteral("idle");
    QString m_holdChord;
    QString m_toggleChord;
    QString m_targetApp;
    QString m_socketMode;
    QString m_gpu;
    QString m_compositor;
    QString m_inputName;
    QString m_restState = QStringLiteral("off");
    QString m_mcpHosts;
    QString m_lastCall;
    QString m_clock;
    QString m_version;
    QVariantList m_levels;
};

}  // namespace dettivo
