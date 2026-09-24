// Shared entry point for the Dettivo Qt hosts (ADR 0001: a C++20 host per Qt
// binary, all UI in QML). Each main.cpp names itself and its QML module and
// hands control here.
#pragma once

namespace dettivo {

struct HostSpec {
    const char *name;    // binary name, printed by --version
    const char *module;  // URI of the app's own QML module that holds Main.qml
};

// Handles --version (prints "<name> <version>" and returns 0 before any GUI
// object exists) and --smoke (loads Main.qml headlessly and quits right after
// the engine reports success; returns 1 if the load fails). Otherwise runs
// the application normally.
int runHost(int argc, char **argv, const HostSpec &spec);

}  // namespace dettivo
