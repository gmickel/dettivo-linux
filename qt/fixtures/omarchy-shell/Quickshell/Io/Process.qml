import QtQuick
import Quickshell

// A child process the test answers by hand: the test finds it through
// Quickshell.process(...), then calls feed() with lines, finish() with
// the whole output, or exit() with a code. Nothing here runs anything.
QtObject {
    id: root

    property bool running: false
    property var command: []
    property var stdout: null
    property var stderr: null

    signal started
    signal exited(int exitCode, int exitStatus)

    Component.onCompleted: Quickshell.register(root)

    onRunningChanged: {
        if (root.running)
            root.started();
    }
    function feed(line) {
        if (root.stdout && root.stdout.read)
            root.stdout.read(line);
    }
    function finish(text) {
        if (root.stdout && root.stdout.streamFinished) {
            root.stdout.text = text;
            root.stdout.streamFinished();
        }
        root.exit(0);
    }
    function exit(code) {
        root.running = false;
        root.exited(code, 0);
    }
}
