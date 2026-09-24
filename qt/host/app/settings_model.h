// The settings routes' editor over config.toml (ADR 0033): every value
// with its source through ConfigBinding, the key registry from
// `config.keys` (type, default, the file's own sentence), writes through
// `config.set` and `config.unset` with the daemon's refusal kept per key
// for the row to show inline, the environment variable that locks a row,
// the "This route writes" TOML block per section, and a refresh on the
// `config.changed` event so a hand edit and the CLI converge with the
// open route.
#pragma once

#include "daemon_link.h"

#include <QHash>
#include <QJsonArray>
#include <QObject>
#include <QString>
#include <QStringList>
#include <QVariant>

namespace dettivo {

class ConfigBinding;

class SettingsModel : public QObject {
    Q_OBJECT
    Q_PROPERTY(int revision READ revision NOTIFY changed)
    Q_PROPERTY(QString configPath READ configPath NOTIFY pathChanged)
    Q_PROPERTY(QString notice READ notice NOTIFY noticeChanged)
    Q_PROPERTY(bool registryLoaded READ registryLoaded NOTIFY registryChanged)

public:
    explicit SettingsModel(DaemonLink *link, ConfigBinding *config, QObject *parent = nullptr);

    /// Reads the registry and the path on every connect.
    void start();
    /// Refreshes on entry and reconnect only while this settings route is active.
    void setActive(bool active);

    int revision() const { return m_revision; }
    QString configPath() const { return m_configPath; }
    QString notice() const { return m_notice; }
    bool registryLoaded() const { return !m_registry.isEmpty(); }

    /// The value in force, its text for a field and where it came from.
    Q_INVOKABLE QVariant value(const QString &key) const;
    Q_INVOKABLE QString text(const QString &key) const;
    Q_INVOKABLE QString source(const QString &key) const;
    /// The variable that overrides the key, when the value came from it.
    Q_INVOKABLE QString lockedBy(const QString &key) const;
    /// The registry's sentence, type and default for a key.
    Q_INVOKABLE QString doc(const QString &key) const;
    Q_INVOKABLE QString kind(const QString &key) const;
    Q_INVOKABLE QVariant defaultValue(const QString &key) const;
    Q_INVOKABLE QString defaultText(const QString &key) const;
    /// The daemon's refusal of the last write to the key, or empty.
    Q_INVOKABLE QString error(const QString &key) const;
    Q_INVOKABLE bool pending(const QString &key) const { return m_pending.contains(key); }
    /// Writes a key (a string is coerced by the daemon) or removes it.
    Q_INVOKABLE void set(const QString &key, const QVariant &value);
    Q_INVOKABLE void unset(const QString &key);
    /// The keys a section edits and the TOML block naming them.
    Q_INVOKABLE QStringList keysFor(const QString &section) const;
    Q_INVOKABLE QString writesBlock(const QString &section) const;
    /// Opens the file in the desktop's editor.
    Q_INVOKABLE void openConfig();
    /// Puts text on the clipboard (the copy actions).
    Q_INVOKABLE void copyText(const QString &text) const;

    /// Applies a `config.keys` answer (tests, the sample).
    void applyRegistry(const QJsonArray &keys);
    /// The baseline's path with no daemon (the renders).
    void applySample();
    /// The TOML rendering of one value.
    static QString tomlValue(const QVariant &value);

public slots:
    void handleNotification(const QString &topic, const QJsonObject &payload);

signals:
    void changed();
    void pathChanged();
    void noticeChanged();
    void registryChanged();
    /// The write to `key` was refused, or its refusal cleared.
    void errorChanged(const QString &key);
    /// The file changed under the route; `keys` moved.
    void fileChanged(const QStringList &keys);

private:
    struct Info {
        QString section, kind, doc;
        QVariant defaultValue;
    };
    void writeList(const QString &key, const QJsonObject &params, const QString &method);
    void finishList(const QString &key, const QJsonObject &error);
    void onConnected(bool connected);
    void readRegistry();
    void readPath();
    void setNotice(const QString &notice);

    bool m_active = false;
    DaemonLink *m_link;
    ConfigBinding *m_config;
    QHash<QString, Info> m_registry;
    QHash<QString, QString> m_errors;
    QStringList m_pending;
    QString m_configPath;
    QString m_notice;
    int m_revision = 0;
};

}  // namespace dettivo
