// Every Dettivo window opens even when the GPU cannot draw it (ADR 0077).
// The GPU scene graph stays the default; `[app] renderer = "software"` in
// config.toml or a user-set QT_QUICK_BACKEND picks the renderer instead;
// and a window whose GPU scene graph fails to start restarts its process
// once on the software renderer, in place of Qt's default abort.
#pragma once

#include <QString>

class QQuickWindow;

namespace dettivo::renderer {

/// The variable a restarted process carries: the scene-graph error that
/// caused the restart. A process that has it is the one restart and never
/// restarts again.
inline constexpr auto kFallbackVariable = "DETTIVO_RENDER_FALLBACK";

/// Picks the renderer before the QGuiApplication exists: a restart, then
/// a user-set QT_QUICK_BACKEND, then `[app] renderer` in config.toml.
/// `name` prefixes the log lines; `argv` is kept for the restart.
void choose(const char *name, int argc, char **argv);

/// Handles `window`'s scene-graph error: the first one restarts the
/// process on the software renderer, one in a process already drawing in
/// software (or already restarted) exits 1 naming the error.
void guard(QQuickWindow *window);

/// Why the renderer is what it is: `default`, `config: ...`,
/// `environment: QT_QUICK_BACKEND=...` or `fallback: <the error>`.
QString reason();

/// `[app] renderer` in `toml`: `auto` or `software`. An absent key or a
/// file that does not parse is `auto`; a bad value is `auto` with a
/// `warning`.
QString configured(const QString &toml, QString *warning);

}  // namespace dettivo::renderer
