// The pill's state as the daemon's event stream dictates it (fn-12): every
// `dictation.state`, `audio.level`, `engine.state` and `job.progress`
// notification maps onto the properties Dettivo.Osd binds to, the
// completion transition carries the insertion outcome and the first
// words, an overflow re-reads `dictation.status`, and the control socket
// and `DETTIVO_E2E_OSD_STATE` drive the same setters. Shared by
// dettivo-osd today and the Omarchy panel later, so it holds no window.
#pragma once

#include "daemon_link.h"
#include "osd_settings.h"

#include <QElapsedTimer>
#include <QJsonObject>
#include <QObject>
#include <QQmlEngine>
#include <QTimer>

#include <functional>

namespace dettivo {

class OsdModel : public QObject {
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON

    Q_PROPERTY(QString state READ state NOTIFY stateChanged)
    Q_PROPERTY(qreal level READ level NOTIFY levelChanged)
    Q_PROPERTY(QString title READ title NOTIFY textChanged)
    Q_PROPERTY(QString hint READ hint NOTIFY textChanged)
    Q_PROPERTY(QString engine READ engine NOTIFY textChanged)
    Q_PROPERTY(QString elapsed READ elapsed NOTIFY textChanged)
    Q_PROPERTY(QString words READ words NOTIFY textChanged)
    Q_PROPERTY(QString target READ target NOTIFY textChanged)
    Q_PROPERTY(QString reason READ reason NOTIFY textChanged)
    Q_PROPERTY(QString action READ action NOTIFY textChanged)
    Q_PROPERTY(bool visible READ visible NOTIFY stateChanged)
    Q_PROPERTY(int hideAfterMs READ hideAfterMs CONSTANT)
    Q_PROPERTY(int errorHideAfterMs READ errorHideAfterMs CONSTANT)
    Q_PROPERTY(bool showLevel READ showLevel CONSTANT)
    Q_PROPERTY(bool reducedMotion READ reducedMotion CONSTANT)
    Q_PROPERTY(bool daemonConnected READ daemonConnected NOTIFY daemonConnectedChanged)

public:
    OsdModel(DaemonLink *link, const OsdSettings &settings, QObject *parent = nullptr);

    /// The QML singleton hands out the instance the host created.
    static void setInstance(OsdModel *model);
    static OsdModel *create(QQmlEngine *, QJSEngine *);

    /// The states a command or the E2E variable may name.
    static QStringList states();
    /// The topics the pill subscribes to.
    static QStringList topics();
    /// How long Transcribing stays on screen at least, so a fast engine
    /// never makes the state flicker past (the pill's motion beat).
    static constexpr int kMinTranscribingMs = 1000;

    /// Subscribes and starts following the daemon.
    void start();

    QString state() const { return m_state; }
    qreal level() const { return m_level; }
    QString title() const { return m_title; }
    QString hint() const { return m_hint; }
    QString engine() const { return m_engine; }
    QString elapsed() const { return m_elapsed; }
    QString words() const { return m_words; }
    QString target() const { return m_target; }
    QString reason() const { return m_reason; }
    QString action() const { return m_action; }
    bool visible() const { return m_state != QStringLiteral("hidden"); }
    int hideAfterMs() const { return m_settings.hideAfterMs; }
    int errorHideAfterMs() const { return m_settings.errorHideAfterMs; }
    bool showLevel() const { return m_settings.showLevel; }
    bool reducedMotion() const { return m_settings.motion == QStringLiteral("reduced"); }
    bool daemonConnected() const { return m_link != nullptr && m_link->connected(); }

    /// One control-socket command (`show`, `hide`, `status`); the answer
    /// is one object with `ok` and, on refusal, `error`.
    QJsonObject applyCommand(const QJsonObject &command);
    /// The status object `dettivo osd status` and the doctor read.
    QJsonObject status() const;
    /// Shows one state with the baseline's sample text (E2E, docs).
    void showSample(const QString &state);
    /// What the host reports: `layer_shell`, `window` or `disabled`, the
    /// position and the monitor the pill sits on.
    void setHostFacts(const QString &kind, const QString &position, const QString &monitor);
    /// The `theme` block of the status: what the theme backend resolved
    /// (ThemeBackend::status), installed by the host once the engine is up.
    void setThemeProvider(std::function<QJsonObject()> provider);

    /// The pill's own timer hid it (bound from QML).
    Q_INVOKABLE void pillHidden();
    /// The meeting clock as the hint: `m:ss`, `h:mm:ss` past an hour.
    static QString meetingClock(qint64 ms);

public slots:
    void handleNotification(const QString &topic, const QJsonObject &payload);
    void handleOverflow(qint64 dropped);
    /// Re-reads `dictation.status` and shows the state it reports.
    void refreshFromStatus();

signals:
    void stateChanged();
    void levelChanged();
    void textChanged();
    void daemonConnectedChanged();

private:
    struct Texts {
        QString title, hint, engine, elapsed, words, target, reason, action;
    };
    void show(const QString &state, const Texts &texts);
    void hide();
    void applyDictationState(const QJsonObject &payload);
    void applyCompletion(const QJsonObject &payload);
    void completeAfterDwell(const QJsonObject &payload);
    void applyFailure(const QString &reason);
    void applyEngineState(const QJsonObject &payload);
    void applyMeetingState(const QJsonObject &payload);
    void tickElapsed();

    DaemonLink *m_link;
    OsdSettings m_settings;
    QString m_state = QStringLiteral("hidden");
    qreal m_level = 0.0;
    QString m_title, m_hint, m_engine, m_elapsed, m_words, m_target, m_reason, m_action;
    QString m_engineLabel;
    QString m_hostKind = QStringLiteral("window");
    QString m_hostPosition;
    QString m_hostMonitor;
    std::function<QJsonObject()> m_themeProvider;
    QElapsedTimer m_transcribingSince;
    // Counts the takes this pill has seen; a completion waiting out the
    // Transcribing beat applies only to the take it belongs to.
    quint64 m_take = 0;
    QTimer m_elapsedTimer;
    // A meeting is recording: Listening carries its elapsed time, counted
    // from the duration the state event brought plus the time since.
    bool m_meeting = false;
    qint64 m_meetingBaseMs = 0;
    QElapsedTimer m_meetingSince;
};

}  // namespace dettivo
