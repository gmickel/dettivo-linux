pragma Singleton
import QtQuick

// The shell's Style singleton, reduced to what the plugin reads.
QtObject {
    id: root

    component BarTokens: QtObject {
        readonly property int sizeHorizontal: 26
        readonly property int iconSlot: 20
        readonly property int statusSlot: 24
    }
    component FontTokens: QtObject {
        readonly property string family: "monospace"
        readonly property int body: 12
    }
    component SpacingTokens: QtObject {
        readonly property int popupPadding: 14
    }

    readonly property int gapsOut: 5
    readonly property BarTokens bar: BarTokens {}
    readonly property FontTokens font: FontTokens {}
    readonly property SpacingTokens spacing: SpacingTokens {}

    function space(px) {
        return px;
    }
    function spaceReal(px) {
        return px;
    }
}
