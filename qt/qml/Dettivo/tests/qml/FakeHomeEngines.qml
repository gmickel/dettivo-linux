import QtQuick

ListModel {
    property string speechName: "parakeet v3"
    property string speechState: "warm"
    property string languageModelName: "qwen3 4b"
    property string languageModelState: "idle"

    ListElement {
        name: "parakeet v3"
        state: "warm"
        detail: "vulkan"
        progress: 1
        busy: false
    }
    ListElement {
        name: "whisper small"
        state: "downloading"
        detail: ""
        progress: 0.5
        busy: true
    }
}
