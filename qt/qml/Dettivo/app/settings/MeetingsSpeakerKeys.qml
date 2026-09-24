import QtQuick
import Dettivo

// The Speakers group of the Meetings route: the post-meeting speaker pass
// (ADR 0035), its model set, the rule that labels a segment and the
// clustering bounds, on the settings pattern.
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
            hint: qsTr("share of a segment inside diarized speech for a label (0 to 1)")
            key: "meetings.diarization.min_coverage"
            label: qsTr("Minimum coverage")
            settings: root.settings
        }

        SettingRow {
            hint: qsTr("share of that speech the winning speaker must hold (0 to 1)")
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
