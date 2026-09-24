#include "wayland_probe.h"

#include <cstdlib>
#include <cstring>

#ifdef DETTIVO_HAVE_WAYLAND_CLIENT
#include <wayland-client.h>
#endif

namespace dettivo {

#ifdef DETTIVO_HAVE_WAYLAND_CLIENT

namespace {

struct Probe {
    bool found = false;
};

void onGlobal(void *data, wl_registry *, uint32_t, const char *interface, uint32_t)
{
    auto *probe = static_cast<Probe *>(data);
    if (std::strcmp(interface, "zwlr_layer_shell_v1") == 0)
        probe->found = true;
}

void onGlobalRemove(void *, wl_registry *, uint32_t) {}

const wl_registry_listener kListener = {onGlobal, onGlobalRemove};

}  // namespace

bool waylandHasLayerShell(QString *detail)
{
    const char *display = std::getenv("WAYLAND_DISPLAY");
    if (display == nullptr || *display == '\0') {
        if (detail != nullptr)
            *detail = QStringLiteral("no Wayland display");
        return false;
    }
    wl_display *conn = wl_display_connect(nullptr);
    if (conn == nullptr) {
        if (detail != nullptr)
            *detail = QStringLiteral("cannot connect to the Wayland display");
        return false;
    }
    Probe probe;
    wl_registry *registry = wl_display_get_registry(conn);
    wl_registry_add_listener(registry, &kListener, &probe);
    wl_display_roundtrip(conn);
    wl_registry_destroy(registry);
    wl_display_disconnect(conn);
    if (detail != nullptr) {
        *detail = probe.found ? QStringLiteral("zwlr_layer_shell_v1 offered by the compositor")
                              : QStringLiteral("the compositor does not offer zwlr_layer_shell_v1");
    }
    return probe.found;
}

#else

bool waylandHasLayerShell(QString *detail)
{
    if (detail != nullptr)
        *detail = QStringLiteral("built without wayland-client; the layer-shell probe is unavailable");
    return false;
}

#endif

}  // namespace dettivo
