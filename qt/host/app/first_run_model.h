// First run (FR-U3, ADR 0024): whether the three screens are needed at
// all, which step is on, and the facts each step shows. Keys reads the
// snippet the daemon renders and writes it through `hotkeys.setup`, then
// confirms a press live from the event stream or `last_press_at`; Try it
// arms the self-target allowance while it is on screen and reads the
// completion event for the backend and the time to insert; Done records
// `first_run.completed_at`. The Models step's rows live in
// FirstRunModels (first_run_models.h). Every fact comes from the daemon;
// the tests hand in a fake link, the visual renders a sample.
#pragma once

#include "daemon_link.h"

#include <QJsonObject>
#include <QObject>
#include <QString>
#include <QTimer>
#include <QVariantList>

namespace dettivo {

class ConfigBinding;
class FirstRunModels;

class FirstRunModel : public QObject {
    Q_OBJECT
    Q_PROPERTY(bool decided READ decided NOTIFY decisionChanged)
    Q_PROPERTY(bool required READ required NOTIFY decisionChanged)
    Q_PROPERTY(QString step READ step NOTIFY stepChanged)
    Q_PROPERTY(int stepIndex READ stepIndex NOTIFY stepChanged)
    Q_PROPERTY(QString compositor READ compositor NOTIFY keysChanged)
    Q_PROPERTY(bool snippetSupported READ snippetSupported NOTIFY keysChanged)
    Q_PROPERTY(QString snippetText READ snippetText NOTIFY keysChanged)
    Q_PROPERTY(QString snippetPath READ snippetPath NOTIFY keysChanged)
    Q_PROPERTY(QString includeLine READ includeLine NOTIFY keysChanged)
    Q_PROPERTY(QString mainConfigPath READ mainConfigPath NOTIFY keysChanged)
    Q_PROPERTY(bool snippetWritten READ snippetWritten NOTIFY keysChanged)
    Q_PROPERTY(bool snippetSourced READ snippetSourced NOTIFY keysChanged)
    Q_PROPERTY(QString portalLine READ portalLine NOTIFY keysChanged)
    Q_PROPERTY(QString keysError READ keysError NOTIFY keysChanged)
    Q_PROPERTY(QVariantList bindings READ bindings NOTIFY keysChanged)
    Q_PROPERTY(QString holdKey READ holdKey NOTIFY keysChanged)
    Q_PROPERTY(bool pressed READ pressed NOTIFY pressChanged)
    Q_PROPERTY(QString pressLine READ pressLine NOTIFY pressChanged)
    Q_PROPERTY(QString configPath READ configPath NOTIFY keysChanged)
    Q_PROPERTY(QObject *models READ models CONSTANT)
    Q_PROPERTY(bool micAvailable READ micAvailable NOTIFY tryChanged)
    Q_PROPERTY(QString micLine READ micLine NOTIFY tryChanged)
    Q_PROPERTY(QString dictationState READ dictationState NOTIFY tryChanged)
    Q_PROPERTY(QString engineLine READ engineLine NOTIFY tryChanged)
    Q_PROPERTY(bool resultKnown READ resultKnown NOTIFY tryChanged)
    Q_PROPERTY(QString resultOutcome READ resultOutcome NOTIFY tryChanged)
    Q_PROPERTY(QString insertedVia READ insertedVia NOTIFY tryChanged)
    Q_PROPERTY(QString stopToInsert READ stopToInsert NOTIFY tryChanged)
    Q_PROPERTY(QString modeLine READ modeLine NOTIFY tryChanged)
    Q_PROPERTY(QString resultReason READ resultReason NOTIFY tryChanged)
    Q_PROPERTY(bool allowanceArmed READ allowanceArmed NOTIFY tryChanged)
    Q_PROPERTY(QString sampleText READ sampleText NOTIFY tryChanged)

public:
    explicit FirstRunModel(DaemonLink *link, ConfigBinding *config, QObject *parent = nullptr);

    static QStringList steps();
    /// Where the hotkeys of a compositor land, for the sentence under the
    /// title (`Hyprland`, `sway`, `niri`, or empty when unknown).
    static QString compositorLabel(const QString &platformCompositor);

    /// Starts the decision on the next connect and keeps the facts current.
    void start();
    /// `DETTIVO_E2E_COMPLETE`: never required.
    void markComplete();
    /// Opens the flow at `step` (`keys`, `models`, `try`, or empty for the
    /// remembered step) regardless of the decision.
    void openAt(const QString &step);
    void leave();

    bool decided() const { return m_decided; }
    bool required() const { return m_required; }
    QString step() const { return m_step; }
    int stepIndex() const { return steps().indexOf(m_step); }
    QString compositor() const { return m_compositor; }
    bool snippetSupported() const { return m_snippetSupported; }
    QString snippetText() const { return m_snippetText; }
    QString snippetPath() const { return m_snippetPath; }
    QString includeLine() const { return m_includeLine; }
    QString mainConfigPath() const { return m_mainConfigPath; }
    bool snippetWritten() const { return m_snippetWritten; }
    bool snippetSourced() const { return m_snippetSourced; }
    QString portalLine() const { return m_portalLine; }
    QString keysError() const { return m_keysError; }
    QVariantList bindings() const;
    QString holdKey() const { return m_hold; }
    bool pressed() const { return m_pressed; }
    QString pressLine() const { return m_pressLine; }
    QString configPath() const { return m_configPath; }
    QObject *models() const;
    bool micAvailable() const { return m_micAvailable; }
    QString micLine() const { return m_micLine; }
    QString dictationState() const { return m_dictationState; }
    QString engineLine() const { return m_engineLine; }
    bool resultKnown() const { return m_resultKnown; }
    /// The insertion outcome of the take (`inserted`, `copied_to_clipboard`,
    /// `failed`), empty until one completes.
    QString resultOutcome() const { return m_resultOutcome; }
    QString insertedVia() const { return m_insertedVia; }
    QString stopToInsert() const { return m_stopToInsert; }
    QString modeLine() const { return m_modeLine; }
    QString resultReason() const { return m_resultReason; }
    bool allowanceArmed() const { return m_allowanceArmed; }
    QString sampleText() const { return m_sampleText; }

    /// Keys: writes the snippet (`hotkeys.setup { write: true }`).
    Q_INVOKABLE void writeSnippet();
    /// Navigation: the next step, the previous one, Skip on Keys.
    Q_INVOKABLE void next();
    Q_INVOKABLE void back();
    Q_INVOKABLE void skip();
    /// Try it: arms or disarms the self-target allowance with the step.
    Q_INVOKABLE void setAllowance(bool enabled);
    /// Done: records the completion and leaves the flow.
    Q_INVOKABLE void finish();
    /// Opens `config.toml` in the desktop's editor.
    Q_INVOKABLE void openConfig();

    /// The baseline's facts for one step, with no daemon (the renders).
    void applySample(const QString &step);
    /// The baseline's `speech.models.status` answer.
    static QJsonObject sampleModels();

public slots:
    void handleNotification(const QString &topic, const QJsonObject &payload);

signals:
    void decisionChanged();
    void stepChanged();
    void keysChanged();
    void pressChanged();
    void tryChanged();
    /// The flow was finished (Done) or skipped to its end.
    void completed(const QString &completedAt);

private:
    struct Binding {
        QString action, hint, runs;
        QStringList keys;
    };
    bool m_connectionProcessed = false;
    bool active() const { return m_required || m_forcedOpen; }
    void activate(bool reuseEligibility);
    void applySetup(const QJsonObject &result);
    void applyHotkeysStatus(const QJsonObject &result);
    void onConnected(bool connected);
    void decide();
    void settle();
    void readKeys();
    void readSnippet(bool includeSetup = true);
    void readHotkeysStatus();
    void readDevices();
    void readSelectionLine();
    void setStep(const QString &step);
    void writeSnippetAndContinue(bool advance);
    void pollPress();

    DaemonLink *m_link;
    ConfigBinding *m_config;
    FirstRunModels *m_models;
    QTimer m_pressPoll;
    bool m_decided = false;
    bool m_required = false;
    bool m_forcedOpen = false;
    bool m_completed = false;
    QString m_step = QStringLiteral("keys");
    // The three answers the decision waits for.
    int m_answers = 0;
    bool m_bound = false;
    QString m_platformCompositor;
    QString m_compositor;
    bool m_snippetSupported = true;
    QString m_snippetText, m_snippetPath, m_includeLine, m_mainConfigPath;
    bool m_setupPending = false;
    bool m_snippetWritten = false;
    bool m_snippetSourced = false;
    QString m_portalLine, m_keysError;
    QString m_hold, m_toggle, m_cancel, m_reinsert;
    bool m_pressed = false;
    QString m_pressLine;
    QString m_lastPressAt;
    QString m_configPath;
    QString m_inputName;
    bool m_micAvailable = true;
    QString m_micLine;
    QString m_dictationState = QStringLiteral("idle");
    QString m_engineLine;
    bool m_resultKnown = false;
    QString m_resultOutcome, m_insertedVia, m_stopToInsert, m_modeLine, m_resultReason;
    bool m_allowanceArmed = false;
    QString m_sampleText;
};

}  // namespace dettivo
