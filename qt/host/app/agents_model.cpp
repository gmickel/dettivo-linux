#include "agents_model.h"

#include "status_format.h"

#include <QFile>
#include <QJsonDocument>
#include <QJsonObject>
#include <QProcessEnvironment>
#include <QVariantMap>

namespace dettivo {

AgentsModel::AgentsModel(CliRunner *runner, QObject *parent) : QObject(parent), m_runner(runner), m_hosts(knownHosts()) {}

QList<AgentsModel::Host> AgentsModel::knownHosts()
{
    return {{QStringLiteral("claude-code"), QStringLiteral("Claude Code"), QString(), QStringLiteral("mcpServers.dettivo"), false},
            {QStringLiteral("codex"), QStringLiteral("Codex"), QString(), QStringLiteral("mcp_servers.dettivo"), false},
            {QStringLiteral("cursor"), QStringLiteral("Cursor"), QString(), QStringLiteral("mcpServers.dettivo"), false},
            {QStringLiteral("claude-desktop"), QStringLiteral("Claude Desktop"), QString(), QStringLiteral("mcpServers.dettivo"), false}};
}

bool AgentsModel::holdsEntry(const QString &id, const QString &text, const QString &name)
{
    if (id == QStringLiteral("codex"))
        return text.contains(QStringLiteral("[mcp_servers.") + name + QLatin1Char(']'))
               || text.contains(QStringLiteral("[mcp_servers.\"") + name + QStringLiteral("\"]"));
    const QJsonDocument doc = QJsonDocument::fromJson(text.toUtf8());
    return doc.object().value(QStringLiteral("mcpServers")).toObject().contains(name);
}

QVariantList AgentsModel::hosts() const
{
    const QProcessEnvironment env = QProcessEnvironment::systemEnvironment();
    QVariantList out;
    for (const Host &h : m_hosts) {
        out.append(QVariantMap{{QStringLiteral("id"), h.id},
                               {QStringLiteral("name"), h.name},
                               {QStringLiteral("path"), format::homePath(h.path, env)},
                               {QStringLiteral("entryKey"), h.entryKey},
                               {QStringLiteral("configured"), h.configured},
                               {QStringLiteral("state"), h.path.isEmpty() ? tr("not checked") : (h.configured ? tr("configured") : tr("not configured"))}});
    }
    return out;
}

void AgentsModel::finishOne()
{
    --m_pending;
    if (m_pending == 0)
        emit busyChanged();
}

void AgentsModel::refresh()
{
    if (m_runner == nullptr)
        return;
    m_lastError.clear();
    m_pending += int(m_hosts.size()) + 1;
    emit busyChanged();
    for (int i = 0; i < m_hosts.size(); ++i)
        readHost(i);
    readCheck();
}

void AgentsModel::readHost(int index)
{
    const QString id = m_hosts[index].id;
    m_runner->run({QStringLiteral("--json"), QStringLiteral("mcp"), QStringLiteral("config"), QStringLiteral("--host"), id},
                  [this, index, id](int exitCode, const QString &out, const QString &err) {
                      finishOne();
                      if (index >= m_hosts.size() || m_hosts[index].id != id)
                          return;
                      if (exitCode != 0) {
                          m_lastError = err.trimmed().isEmpty() ? tr("dettivo mcp config failed") : err.trimmed();
                          emit hostsChanged();
                          return;
                      }
                      const QJsonObject result = QJsonDocument::fromJson(out.toUtf8()).object();
                      Host &h = m_hosts[index];
                      h.path = result.value(QStringLiteral("path")).toString();
                      const QString name = result.value(QStringLiteral("name")).toString(QStringLiteral("dettivo"));
                      QFile file(h.path);
                      h.configured = file.open(QIODevice::ReadOnly) && holdsEntry(id, QString::fromUtf8(file.readAll()), name);
                      if (id == QStringLiteral("claude-code"))
                          m_entryText = result.value(QStringLiteral("text")).toString().trimmed();
                      emit hostsChanged();
                  });
}

void AgentsModel::readCheck()
{
    m_runner->run({QStringLiteral("--json"), QStringLiteral("mcp"), QStringLiteral("check")}, [this](int exitCode, const QString &out, const QString &) {
        finishOne();
        if (exitCode != 0)
            return;
        const QJsonObject report = QJsonDocument::fromJson(out.toUtf8()).object();
        m_toolsLine = tr("%1 tools · %2 resource templates · bounded output · same names as macOS")
                          .arg(report.value(QStringLiteral("tools")).toInt())
                          .arg(report.value(QStringLiteral("resource_templates")).toInt());
        emit checkChanged();
    });
}

void AgentsModel::writeHost(const QString &id)
{
    if (m_runner == nullptr)
        return;
    ++m_pending;
    emit busyChanged();
    m_runner->run({QStringLiteral("--json"), QStringLiteral("mcp"), QStringLiteral("config"), QStringLiteral("--host"), id, QStringLiteral("--write")},
                  [this](int exitCode, const QString &, const QString &err) {
                      finishOne();
                      if (exitCode != 0) {
                          m_lastError = err.trimmed().isEmpty() ? tr("dettivo mcp config --write failed") : err.trimmed();
                          emit hostsChanged();
                          return;
                      }
                      refresh();
                  });
}

void AgentsModel::applySample()
{
    const QString home = QProcessEnvironment::systemEnvironment().value(QStringLiteral("HOME"), QStringLiteral("/home/user"));
    m_hosts = knownHosts();
    m_hosts[0].path = home + QStringLiteral("/.mcp.json");
    m_hosts[0].configured = true;
    m_hosts[1].path = home + QStringLiteral("/.codex/config.toml");
    m_hosts[1].configured = true;
    m_hosts[2].path = home + QStringLiteral("/.cursor/mcp.json");
    m_hosts[3].path = home + QStringLiteral("/.config/Claude/claude_desktop_config.json");
    m_entryText = QStringLiteral("{\n  \"mcpServers\": {\n    \"dettivo\": { \"command\": \"dettivo\", \"args\": [\"mcp\", \"serve\"] }\n  }\n}");
    m_toolsLine = tr("%1 tools · %2 resource templates · bounded output · same names as macOS").arg(19).arg(5);
    emit hostsChanged();
    emit checkChanged();
}

}  // namespace dettivo
