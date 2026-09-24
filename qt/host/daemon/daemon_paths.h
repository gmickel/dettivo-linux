// The locations every Dettivo client resolves the same way the daemon and
// the `dettivo` command do (docs/config.md): the configuration file, the
// socket directory, the daemon socket, the state directory and the shared
// token in `peer_token` mode. One resolver, so the app, the pill and the
// tests agree on where things are.
#pragma once

#include <QProcessEnvironment>
#include <QString>

namespace dettivo::paths {

/// `DETTIVO_CONFIG`, then `$XDG_CONFIG_HOME/dettivo/config.toml`.
QString configFile(const QProcessEnvironment &env);

/// The directory the daemon socket lives in, where the app and pill
/// sockets go too: the directory of `DETTIVO_IPC_SOCKET`, else
/// `$XDG_RUNTIME_DIR/dettivo`, else `/tmp/dettivo-$USER`.
QString socketDir(const QProcessEnvironment &env);

/// The daemon socket: `DETTIVO_IPC_SOCKET`, else `<socketDir>/dettivo.sock`.
QString daemonSocket(const QProcessEnvironment &env);

/// `$XDG_STATE_HOME/dettivo`, else `$HOME/.local/state/dettivo`.
QString stateDir(const QProcessEnvironment &env);

/// The shared token for `peer_token` mode: `DETTIVO_IPC_TOKEN`, else the
/// first line of `$XDG_CONFIG_HOME/dettivo/ipc.token`, else empty (the
/// daemon in `peer` mode needs none).
QString ipcToken(const QProcessEnvironment &env);

}  // namespace dettivo::paths
