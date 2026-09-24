// Does the Wayland compositor offer zwlr_layer_shell_v1? Asked on a fresh
// connection before any Qt window exists, so dettivo-osd can choose the
// layer-shell host, the window fallback or the disabled notice up front.
#pragma once

#include <QString>

namespace dettivo {

/// True when `WAYLAND_DISPLAY` names a compositor that advertises the
/// layer-shell global. `detail` says what was found or why not.
bool waylandHasLayerShell(QString *detail);

}  // namespace dettivo
