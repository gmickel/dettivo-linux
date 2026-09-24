#include "daemon_paths.h"

#include <QFile>
#include <QFileInfo>

namespace dettivo::paths {

namespace {

QString home(const QProcessEnvironment &env)
{
    return env.value(QStringLiteral("HOME"), QStringLiteral("/"));
}

QString configHome(const QProcessEnvironment &env)
{
    const QString configured = env.value(QStringLiteral("XDG_CONFIG_HOME"));
    return configured.isEmpty() ? home(env) + QStringLiteral("/.config") : configured;
}

}  // namespace

QString configFile(const QProcessEnvironment &env)
{
    const QString override = env.value(QStringLiteral("DETTIVO_CONFIG"));
    if (!override.isEmpty())
        return override;
    return configHome(env) + QStringLiteral("/dettivo/config.toml");
}

QString socketDir(const QProcessEnvironment &env)
{
    const QString socket = env.value(QStringLiteral("DETTIVO_IPC_SOCKET"));
    if (!socket.isEmpty())
        return QFileInfo(socket).absolutePath();
    const QString runtime = env.value(QStringLiteral("XDG_RUNTIME_DIR"));
    if (!runtime.isEmpty())
        return runtime + QStringLiteral("/dettivo");
    return QStringLiteral("/tmp/dettivo-") + env.value(QStringLiteral("USER"), QStringLiteral("user"));
}

QString daemonSocket(const QProcessEnvironment &env)
{
    const QString socket = env.value(QStringLiteral("DETTIVO_IPC_SOCKET"));
    if (!socket.isEmpty())
        return socket;
    return socketDir(env) + QStringLiteral("/dettivo.sock");
}

QString stateDir(const QProcessEnvironment &env)
{
    const QString configured = env.value(QStringLiteral("XDG_STATE_HOME"));
    const QString base = configured.isEmpty() ? home(env) + QStringLiteral("/.local/state") : configured;
    return base + QStringLiteral("/dettivo");
}

QString ipcToken(const QProcessEnvironment &env)
{
    const QString fromEnv = env.value(QStringLiteral("DETTIVO_IPC_TOKEN"));
    if (!fromEnv.isEmpty())
        return fromEnv;
    QFile file(configHome(env) + QStringLiteral("/dettivo/ipc.token"));
    if (!file.open(QIODevice::ReadOnly))
        return QString();
    return QString::fromUtf8(file.readLine()).trimmed();
}

}  // namespace dettivo::paths
