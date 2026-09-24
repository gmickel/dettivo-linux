// dettivo-sheet: the component sheet host. `--render <path>` (the host
// convention every Qt binary keeps) writes the sheet offscreen as a PNG
// for the visual regression job (ADR 0010, ADR 0021).
#include "host.h"

int main(int argc, char **argv)
{
    return dettivo::runHost(argc, argv, {DETTIVO_HOST_NAME, DETTIVO_HOST_MODULE});
}
