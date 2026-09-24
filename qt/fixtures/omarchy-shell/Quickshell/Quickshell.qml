pragma Singleton
import QtQuick

// The Quickshell singleton: env() answers from the table a test fills,
// execDetached() records the command instead of running it, and every
// shim Process registers here so a test can answer it by its command.
QtObject {
    id: root

    property var environment: ({})
    property var detached: []
    property var processes: []

    function env(name) {
        const value = root.environment[name];
        return value === undefined ? "" : String(value);
    }
    function execDetached(command) {
        root.detached = root.detached.concat([command]);
    }
    function register(process) {
        root.processes = root.processes.concat([process]);
    }
    // The registered process whose command contains every word given.
    function process(...words) {
        return root.processes.find(p => words.every(w => (p.command || []).indexOf(w) >= 0)) || null;
    }
}
