import QtQuick
import QtQuick.Controls
import Dettivo

// Meetings: the meeting and analysis models, what is kept beside the
// database and for how long, the chunked pipeline every import and
// re-run runs through, the upload limit, the speaker pass (ADR 0035) and
// the recording disclosure with its copy action.
SettingsPage {
    id: root

    readonly property string disclosure: qsTr("This meeting is being recorded and transcribed by Dettivo on this computer. The audio and the transcript stay on this machine.")

    section: "meetings"
    subtitle: qsTr("Meetings, imports and re-runs share the chunked pipeline and the history store.")

    property var table: null
    ModelSelections {
        settings: root.settings
        table: root.table
    }

    SettingsGroup {
        title: qsTr("History store")

        SettingRow {
            hint: qsTr("Save recorded audio so you can transcribe it again with another model")
            key: "history.keep_audio"
            kind: "switch"
            label: qsTr("Keep audio")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("days before the sweep removes audio; 0 keeps it")
            key: "history.audio_retention_days"
            label: qsTr("Audio retention")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("items kept; 0 keeps all")
            key: "history.max_items"
            label: qsTr("Items kept")
            settings: root.settings
        }

        SettingRow {
            choices: ["keep", "audio_only", "none"]
            hint: qsTr("what lives beside the database")
            key: "history.artifacts"
            kind: "choice"
            label: qsTr("Artifacts")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("empty means <data_dir>/dettivo.db")
            key: "history.db_path"
            label: qsTr("Database")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("seconds the longest import may run")
            key: "history.max_import_seconds"
            label: qsTr("Longest import")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("bytes the largest upload may hold")
            key: "transfer.max_upload_bytes"
            label: qsTr("Upload limit")
            settings: root.settings
        }
    }

    SettingsGroup {
        title: qsTr("Chunked pipeline")

        SettingRow {
            hint: qsTr("seconds of audio per chunk")
            key: "transcribe.chunk_seconds"
            label: qsTr("Chunk")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("seconds two chunks share")
            key: "transcribe.overlap_seconds"
            label: qsTr("Overlap")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("seconds before the end the cut may move to silence")
            key: "transcribe.safety_margin_seconds"
            label: qsTr("Safety margin")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("RMS under which a chunk is silence")
            key: "transcribe.silence_rms_floor"
            label: qsTr("Silence floor")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("drop the known filler hallucinations")
            key: "transcribe.filler_filter"
            kind: "switch"
            label: qsTr("Filler filter")
            settings: root.settings
        }
    }

    SettingsGroup {
        title: qsTr("Capture")

        SettingRow {
            hint: qsTr("default_monitor follows the default sink; a node name pins one")
            key: "audio.system_source"
            label: qsTr("System source")
            placeholder: qsTr("default_monitor")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("Save microphone and system audio after the meeting ends")
            key: "meetings.keep_audio"
            kind: "switch"
            label: qsTr("Keep the tracks")
            settings: root.settings
        }

        SettingRow {
            choices: ["keep", "audio_only", "none"]
            hint: qsTr("what stays in the meeting directory once it stopped")
            key: "meetings.artifacts"
            kind: "choice"
            label: qsTr("Artifacts")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("seconds between two writes of live-checkpoint.json")
            key: "meetings.checkpoint_interval_seconds"
            label: qsTr("Checkpoint")
            settings: root.settings
        }
    }

    SettingsGroup {
        title: qsTr("Live transcription")

        SettingRow {
            hint: qsTr("windows of both tracks arrive as segments while recording")
            key: "meetings.live"
            kind: "switch"
            label: qsTr("Transcribe live")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("milliseconds the longest live window holds")
            key: "meetings.live_window_ms"
            label: qsTr("Window")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("milliseconds of new audio that cut the next window")
            key: "meetings.live_tick_ms"
            label: qsTr("Tick")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("milliseconds two consecutive windows share")
            key: "meetings.live_overlap_ms"
            label: qsTr("Overlap")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("RMS (0 to 1) a window must reach to be sent")
            key: "meetings.speech_floor_rms"
            label: qsTr("Speech floor")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("milliseconds behind the newest audio a segment is final")
            key: "meetings.boundary_merge_gap_ms"
            label: qsTr("Final after")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("milliseconds around a remote segment that drop a microphone filler")
            key: "meetings.cross_source_padding_ms"
            label: qsTr("Cross-source padding")
            settings: root.settings
        }
    }

    MeetingsSpeakerKeys {
        settings: root.settings
        width: parent.width
    }

    MeetingsAnalysisKeys {
        settings: root.settings
        width: parent.width
    }

    SettingsGroup {
        title: qsTr("Disclosure")

        Item {
            height: disclosureText.implicitHeight + Theme.space4 * 2
            width: parent.width

            Text {
                id: disclosureText
                anchors.left: parent.left
                anchors.right: copyButton.left
                anchors.rightMargin: Theme.space5
                anchors.verticalCenter: parent.verticalCenter
                color: Theme.roleMutedText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.typeBodySize
                text: root.disclosure
                wrapMode: Text.WordWrap
                Accessible.role: Accessible.StaticText
                Accessible.name: qsTr("Disclosure message")
            }

            Button {
                id: copyButton
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                text: qsTr("Copy message")
                onClicked: {
                    if (root.settings)
                        root.settings.copyText(root.disclosure);
                }
            }
        }
    }
}
