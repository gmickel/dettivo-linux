// dettivo-bar: the Omarchy plugin's surfaces (the panel and the bar
// glyph) rendered outside Quickshell for the visual regression job and
// for a look at them on any desktop. `--render <path>` (the host
// convention every Qt binary keeps) writes the window offscreen as a PNG;
// DETTIVO_E2E_BAR_STATE picks the surface and state (ADR 0021).
#include "host.h"

int main(int argc, char **argv)
{
    return dettivo::runHost(argc, argv, {DETTIVO_HOST_NAME, DETTIVO_HOST_MODULE});
}
