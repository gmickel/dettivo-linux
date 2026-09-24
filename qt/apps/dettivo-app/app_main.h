// dettivo-app's entry (fn-17, ADR 0020): the arguments, the first route,
// the single-instance forward, the host objects, the window, the first
// frame and theme timings, the render mode and the state file.
#pragma once

#include <QString>

namespace dettivo {

struct AppArgs {
    bool version = false;
    bool smoke = false;
    bool sample = false;
    QString render;
    QString open;
    QString id;
};

/// Parses `--version`, `--smoke`, `--sample`, `--render <png>`, `--open <route>`
/// and `--id <item>` (the item a detail route opens).
AppArgs parseAppArgs(int argc, char **argv);

/// Runs the app; the process exit code.
int runApp(int argc, char **argv);

}  // namespace dettivo
