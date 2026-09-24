import QtQuick
import Quickshell.Io

QtObject {
    id: root

    required property var state
    property string output: ""
    property string errorOutput: ""

    function fail(reason) {
        root.state.showPill("error", {
            title: "Meeting action failed",
            reason: reason
        });
    }

    function run(args) {
        if (root.process.running)
            return;
        root.output = "";
        root.errorOutput = "";
        root.process.command = [root.state.binary, "--json"].concat(args);
        root.process.running = true;
    }

    function start() {
        root.run(["app", "open", "meetings"]);
    }

    // Only a meeting that still records can be stopped; past its stop the
    // daemon builds the transcript and the panel offers no Stop.
    function stop() {
        if (!root.state.daemonAvailable || !root.state.meetingRecording || root.state.meetingId === "") {
            root.fail("No active meeting is available. Open Dettivo to check the recording.");
            return;
        }
        root.run(["call", "meetings.stop", JSON.stringify({
                meeting_id: root.state.meetingId
            })]);
    }

    readonly property Process process: Process {
        stdout: StdioCollector {
            onStreamFinished: root.output = text
        }
        stderr: StdioCollector {
            onStreamFinished: root.errorOutput = text
        }
        onExited: (code, status) => {
            let error = "";
            try {
                const response = JSON.parse(root.output);
                if (response.error)
                    error = response.error.message || "Meeting action failed";
            } catch (e) {}
            if (code !== 0 || status !== 0 || error !== "")
                root.fail(error || root.errorOutput.trim() || "The Dettivo command could not complete.");
            root.state.refresh();
        }
    }
}
