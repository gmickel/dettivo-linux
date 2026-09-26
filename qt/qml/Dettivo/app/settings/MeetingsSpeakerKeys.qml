import QtQuick
import Dettivo

// The Speakers group of the Meetings route: the post-meeting speaker pass
// (ADR 0035), its model set, the sentence rule that labels a line (ADR
// 0072) and the clustering bounds, on the settings pattern.
Column {
    id: root

    property var settings: null

    spacing: 0

    SettingsGroup {
        title: qsTr("Speakers")

        SettingRow {
            hint: qsTr("learn who spoke once a meeting is finalised")
            key: "meetings.diarization.enabled"
            kind: "switch"
            label: qsTr("Speaker pass")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("run the pass by itself when the finalisation completes")
            key: "meetings.diarization.auto"
            kind: "switch"
            label: qsTr("Run automatically")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("the model set under <models>/diarize")
            key: "meetings.diarization.model"
            label: qsTr("Model set")
            placeholder: qsTr("diarization-en")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("a pause this long ends a sentence where the speaker changes (ms)")
            key: "meetings.diarization.pause_ms"
            label: qsTr("Sentence pause")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("a sentence outside every turn takes the nearest one within this (ms)")
            key: "meetings.diarization.nearest_turn_ms"
            label: qsTr("Nearest turn")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("share of a sentence the speaker must hold; 0 labels every line (0 to 1)")
            key: "meetings.diarization.min_speaker_share"
            label: qsTr("Minimum speaker share")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("speakers the clustering is told; 0 lets it decide")
            key: "meetings.diarization.max_speakers"
            label: qsTr("Speakers")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("initial cosine cutoff; low-support clusters are reassigned afterward")
            key: "meetings.diarization.clustering_threshold"
            label: qsTr("Clustering threshold")
            settings: root.settings
        }
    }

    Accessible.role: Accessible.Grouping
    Accessible.name: qsTr("Speaker keys")
}
