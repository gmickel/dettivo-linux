// The Hotkeys route's snippet panel (settings-hotkeys.png, ADR 0033):
// the compositor snippet the daemon renders from `[hotkeys]`
// (`hotkeys.snippet`, the same text `dettivo setup <compositor> --stdout`
// prints), where it goes and whether the main configuration sources it
// (`hotkeys.setup { write: false }`), the rewrite through
// `hotkeys.setup { write: true }`, and the include line to copy. The
// chords themselves are config keys the rows edit; this model only shows
// what the daemon makes of them.
#pragma once

#include "daemon_link.h"

#include <QJsonObject>
#include <QObject>
#include <QString>

namespace dettivo {

class ConfigBinding;

class HotkeysSetupModel : public QObject {
    Q_OBJECT
    Q_PROPERTY(bool supported READ supported NOTIFY changed)
    Q_PROPERTY(QString compositor READ compositor NOTIFY changed)
    Q_PROPERTY(QString snippetText READ snippetText NOTIFY changed)
    Q_PROPERTY(QString snippetPath READ snippetPath NOTIFY changed)
    Q_PROPERTY(QString includeLine READ includeLine NOTIFY changed)
    Q_PROPERTY(QString mainConfigPath READ mainConfigPath NOTIFY changed)
    Q_PROPERTY(bool written READ written NOTIFY changed)
    Q_PROPERTY(bool sourced READ sourced NOTIFY changed)
    Q_PROPERTY(QString statusLine READ statusLine NOTIFY changed)
    Q_PROPERTY(QString outcome READ outcome NOTIFY changed)
    Q_PROPERTY(QString daemonBackend READ daemonBackend NOTIFY changed)

public:
    explicit HotkeysSetupModel(DaemonLink *link, ConfigBinding *config, QObject *parent = nullptr);

    /// Reads the snippet and the check on every connect and after every
    /// `[hotkeys]` change.
    void start();
    /// Refreshes on entry and reconnect only while this settings route is active.
    void setActive(bool active);
    Q_INVOKABLE void refresh();
    /// Writes the snippet through the daemon (idempotent).
    Q_INVOKABLE void rewrite();
    /// Puts the include line on the clipboard.
    Q_INVOKABLE void copyIncludeLine() const;

    bool supported() const { return m_supported; }
    QString compositor() const { return m_compositor; }
    QString snippetText() const { return m_snippetText; }
    QString snippetPath() const { return m_snippetPath; }
    QString includeLine() const { return m_includeLine; }
    QString mainConfigPath() const { return m_mainConfigPath; }
    bool written() const { return m_written; }
    bool sourced() const { return m_sourced; }
    QString statusLine() const;
    QString outcome() const { return m_outcome; }
    QString daemonBackend() const { return m_daemonBackend; }

    /// Applies a `hotkeys.snippet`, `hotkeys.setup` or `hotkeys.status`
    /// answer (tests, the sample).
    void applySnippet(const QJsonObject &result, const QJsonObject &error);
    void applySetup(const QJsonObject &result);
    void applyStatus(const QJsonObject &result);
    void applySample();

signals:
    void changed();

private:
    void onConnected(bool connected);

    bool m_active = false;
    DaemonLink *m_link;
    ConfigBinding *m_config;
    bool m_supported = true;
    QString m_compositor;
    QString m_snippetText, m_snippetPath, m_includeLine, m_mainConfigPath;
    bool m_written = false;
    bool m_sourced = false;
    QString m_outcome;
    QString m_daemonBackend;
};

}  // namespace dettivo
