// dettivo-insert-target: Qt host that loads Main.qml from the DettivoInsertTarget
// module (ADR 0001). A plain window with one text field for QA text-insertion checks.
#include "host.h"

int main(int argc, char **argv)
{
    return dettivo::runHost(argc, argv, {DETTIVO_HOST_NAME, DETTIVO_HOST_MODULE});
}
