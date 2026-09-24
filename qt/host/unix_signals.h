// SIGTERM and SIGINT end the Qt event loop cleanly, so the control socket
// is removed and the QA leak check finds nothing left behind.
#pragma once

namespace dettivo {

/// Installs the handlers; call once after the application exists.
void quitOnTerminationSignals();

}  // namespace dettivo
