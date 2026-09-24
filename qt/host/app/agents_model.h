// The Agents route's facts (agents.png, ADR 0033): the socket and its
// auth from the status facts, the four MCP hosts with the file each
// reads and whether Dettivo's entry is in it, the entry a host receives,
// and the tool count from `dettivo mcp check`. A host entry is written by
// running `dettivo mcp config --host <id> --write`, the one code path
// that writes host files, so the app and the terminal cannot drift.
#pragma once

#include "cli_runner.h"

#include <QObject>
#include <QString>
#include <QVariantList>

namespace dettivo {

class AgentsModel : public QObject {
    Q_OBJECT
    Q_PROPERTY(QVariantList hosts READ hosts NOTIFY hostsChanged)
    Q_PROPERTY(QString entryText READ entryText NOTIFY hostsChanged)
    Q_PROPERTY(QString toolsLine READ toolsLine NOTIFY checkChanged)
    Q_PROPERTY(QString lastError READ lastError NOTIFY hostsChanged)
    Q_PROPERTY(bool busy READ busy NOTIFY busyChanged)

public:
    struct Host {
        QString id;          // claude-code
        QString name;        // Claude Code
        QString path;        // the file the host reads
        QString entryKey;    // mcpServers.dettivo
        bool configured = false;
    };

    explicit AgentsModel(CliRunner *runner, QObject *parent = nullptr);

    /// The host ids in row order, with their display names.
    static QList<Host> knownHosts();
    /// Whether `text` (the host's file) holds the `name` server entry, by
    /// the host's format: JSON under `mcpServers`, TOML `[mcp_servers.<name>]`.
    static bool holdsEntry(const QString &id, const QString &text, const QString &name);

    /// Runs `mcp config` for every host and `mcp check` once.
    Q_INVOKABLE void refresh();
    /// `dettivo mcp config --host <id> --write`, then a refresh.
    Q_INVOKABLE void writeHost(const QString &id);

    QVariantList hosts() const;
    QString entryText() const { return m_entryText; }
    QString toolsLine() const { return m_toolsLine; }
    QString lastError() const { return m_lastError; }
    bool busy() const { return m_pending > 0; }

    /// The baseline's facts with no daemon (the renders and the tests).
    void applySample();

signals:
    void hostsChanged();
    void checkChanged();
    void busyChanged();

private:
    void readHost(int index);
    void readCheck();
    void finishOne();

    CliRunner *m_runner;
    QList<Host> m_hosts;
    QString m_entryText;
    QString m_toolsLine;
    QString m_lastError;
    int m_pending = 0;
};

}  // namespace dettivo
