// config.toml through the daemon (ADR 0009): every key's effective value
// with the source it came from (`default`, `file`, `env`), read with one
// `config.get` and written with `config.set`, so a QML control binds to
// `value("dictation.mode")` and the file stays the source of truth.
#pragma once

#include "daemon_link.h"

#include <QHash>
#include <QJsonArray>
#include <QObject>
#include <QString>
#include <QVariant>

namespace dettivo {

class ConfigBinding : public QObject {
    Q_OBJECT
    Q_PROPERTY(int revision READ revision NOTIFY changed)

public:
    explicit ConfigBinding(DaemonLink *link, QObject *parent = nullptr);

    /// The effective value of a dotted key, or an invalid variant.
    Q_INVOKABLE QVariant value(const QString &key) const;
    /// The value as text for a label; empty when unknown.
    Q_INVOKABLE QString text(const QString &key) const;
    /// Where the value came from: `default`, `file`, `env`, or empty.
    Q_INVOKABLE QString source(const QString &key) const;
    /// Re-reads every key.
    Q_INVOKABLE void refresh();
    /// Writes one key through the daemon and re-reads it; a refusal is
    /// reported on `setFailed` with the daemon's message.
    Q_INVOKABLE void set(const QString &key, const QVariant &value);

    /// Bumps on every refresh so QML bindings re-evaluate `value(...)`.
    int revision() const { return m_revision; }

    /// Applies `config.get` entries directly (tests, the sample data).
    void applyEntries(const QJsonArray &entries);

signals:
    void changed();
    void setFailed(const QString &key, const QString &message);

private:
    struct Entry {
        QVariant value;
        QString source;
    };
    DaemonLink *m_link;
    QHash<QString, Entry> m_entries;
    int m_revision = 0;
};

}  // namespace dettivo
