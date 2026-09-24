pragma Singleton
import QtQuick
import Dettivo

// The design tokens every Dettivo surface reads (ADR 0010). ThemeBackend
// owns the theme files, the portal fallback and live re-resolution; this
// singleton derives the semantic roles, the type and spacing scales and
// the control geometry from those values, so a theme switch re-evaluates
// every binding here without a restart.
QtObject {
    id: theme

    // Where the tokens came from: omarchy, builtin-dark or builtin-light.
    readonly property string source: ThemeBackend.source
    readonly property string themeDir: ThemeBackend.themeDir

    // Palette from colors.toml.
    readonly property color colorAccent: ThemeBackend.colorAccent
    readonly property color colorCursor: ThemeBackend.colorCursor
    readonly property color colorForeground: ThemeBackend.colorForeground
    readonly property color colorBackground: ThemeBackend.colorBackground
    readonly property color colorSelectionForeground: ThemeBackend.colorSelectionForeground
    readonly property color colorSelectionBackground: ThemeBackend.colorSelectionBackground
    readonly property var palette16: ThemeBackend.palette16
    readonly property color colorBar: ThemeBackend.colorBar
    readonly property color colorBarActive: ThemeBackend.colorBarActive

    // Font from shell.toml [font]: the monospace alias on Omarchy.
    readonly property string fontFamily: ThemeBackend.fontFamily
    readonly property int fontBaseSize: ThemeBackend.fontBaseSize
    readonly property int fontHeadingSize: ThemeBackend.fontHeadingSize
    readonly property int fontIconLargeSize: ThemeBackend.fontIconLargeSize

    // Spacing from shell.toml [spacing]: xs is the unit, the scale is
    // 2, 4, 8, 12, 16, 24, 32, 48 at the default size.
    readonly property real spacingScale: ThemeBackend.spacingScale
    readonly property int spacingXs: ThemeBackend.spacingXs
    readonly property int spacingMd: ThemeBackend.spacingMd
    readonly property int controlPaddingY: ThemeBackend.controlPaddingY
    readonly property int panelPadding: ThemeBackend.panelPadding
    readonly property int space1: Math.round(spacingXs * spacingScale)
    readonly property int space2: Math.round(spacingXs * 2 * spacingScale)
    readonly property int space3: Math.round(spacingXs * 4 * spacingScale)
    readonly property int space4: Math.round(spacingXs * 6 * spacingScale)
    readonly property int space5: Math.round(spacingXs * 8 * spacingScale)
    readonly property int space6: Math.round(spacingXs * 12 * spacingScale)
    readonly property int space7: Math.round(spacingXs * 16 * spacingScale)
    readonly property int space8: Math.round(spacingXs * 24 * spacingScale)
    readonly property int spacingSm: space2
    readonly property int spacingLg: space5
    readonly property int spacingXl: space6

    // Shape: the shell's corner radius; Black Gold renders square.
    readonly property int radius: ThemeBackend.radius
    readonly property int hairlineWidth: 1
    readonly property int railWidth: space1

    // Control geometry: body type plus the shell's vertical padding on both
    // edges and one unit of breathing room gives 28 at the default size.
    readonly property int controlHeight: typeBodySize + 2 * controlPaddingY + space3
    readonly property int rowHeight: controlHeight + space3
    readonly property int rowPaddingX: space4
    readonly property int controlGap: space3
    readonly property int controlPaddingX: space3 + space1

    // Control states from shell.toml [controls].
    readonly property color stateNormalColor: ThemeBackend.stateNormalColor
    readonly property real stateNormalFillAlpha: ThemeBackend.stateNormalFillAlpha
    readonly property int stateNormalBorderWidth: ThemeBackend.stateNormalBorderWidth
    readonly property real stateNormalBorderAlpha: ThemeBackend.stateNormalBorderAlpha
    readonly property color stateHoverColor: ThemeBackend.stateHoverColor
    readonly property real stateHoverFillAlpha: ThemeBackend.stateHoverFillAlpha
    readonly property int stateHoverBorderWidth: ThemeBackend.stateHoverBorderWidth
    readonly property real stateHoverBorderAlpha: ThemeBackend.stateHoverBorderAlpha
    readonly property color stateFocusColor: ThemeBackend.stateFocusColor
    readonly property real stateFocusFillAlpha: ThemeBackend.stateFocusFillAlpha
    readonly property int stateFocusBorderWidth: ThemeBackend.stateFocusBorderWidth
    readonly property real stateFocusBorderAlpha: ThemeBackend.stateFocusBorderAlpha
    readonly property color stateSelectedColor: ThemeBackend.stateSelectedColor
    readonly property real stateSelectedFillAlpha: ThemeBackend.stateSelectedFillAlpha
    readonly property int stateSelectedBorderWidth: ThemeBackend.stateSelectedBorderWidth
    readonly property real stateSelectedBorderAlpha: ThemeBackend.stateSelectedBorderAlpha
    readonly property color statePressedColor: ThemeBackend.statePressedColor
    readonly property real statePressedFillAlpha: ThemeBackend.statePressedFillAlpha
    readonly property color stateSelectionColor: ThemeBackend.stateSelectionColor
    readonly property real stateSelectionFillAlpha: ThemeBackend.stateSelectionFillAlpha

    // Semantic roles, derived once from the palette and the shell's alphas.
    readonly property color roleText: colorForeground
    readonly property color roleMutedText: Qt.alpha(colorForeground, 0.62)
    readonly property color roleFaintText: Qt.alpha(colorForeground, 0.38)
    readonly property color roleSurface: colorBackground
    readonly property color roleRaisedSurface: Qt.alpha(stateNormalColor, stateNormalFillAlpha)
    readonly property color roleSunkenSurface: Qt.alpha(stateNormalColor, stateNormalFillAlpha * 2)
    readonly property color roleBorder: Qt.alpha(stateNormalColor, stateNormalBorderAlpha)
    readonly property color roleHairline: Qt.alpha(colorForeground, 0.14)
    readonly property color roleAccent: colorAccent
    readonly property color roleSelected: stateSelectedColor
    readonly property color roleSelectedFill: Qt.alpha(stateSelectedColor, 0.18)
    readonly property color roleHoverFill: Qt.alpha(stateHoverColor, 0.08)
    readonly property color roleSearchHighlight: Qt.alpha(colorAccent, 0.12)
    readonly property color roleUrgent: palette16 && palette16.length > 1 ? palette16[1] : colorAccent
    readonly property color roleHighlight: palette16 && palette16.length > 7 ? palette16[7] : colorForeground
    // Speakers: you take color5, then color4, color6, color3 and color8, a
    // stable mapping that never lands on the urgent slot.
    readonly property var roleSpeakerColors: palette16 && palette16.length >= 9 ? [palette16[5], palette16[4], palette16[6], palette16[3], palette16[8]] : [colorAccent, colorCursor]

    // Type scale: ratios of the shell's base size, so 12 gives display 28,
    // title 20, heading 16, body 12, caption 11 and tracked label 10.
    readonly property int typeDisplaySize: Math.round(fontBaseSize * 7 / 3)
    readonly property int typeTitleSize: Math.round(fontBaseSize * 5 / 3)
    readonly property int typeHeadingSize: fontHeadingSize
    readonly property int typeBodySize: fontBaseSize
    readonly property int typeCaptionSize: Math.max(9, fontBaseSize - 1)
    readonly property int typeLabelSize: Math.max(8, fontBaseSize - 2)
    readonly property real typeLabelTracking: 0.14
    readonly property real typeTitleTracking: -0.02
    // Body leading for a paragraph that is read, not scanned: the reason of
    // a designed state (states-and-hint-sheet.png).
    readonly property real typeBodyLeading: 1.5
    readonly property int typeProvisionalWeight: Font.Light
    readonly property int typeEmphasisWeight: Font.DemiBold
    readonly property var typeTabularNumerals: ({
            "tnum": 1
        })

    // Icons sit on a 16 px grid with a 1.5 px stroke.
    readonly property int iconSize: Math.round(fontBaseSize * 4 / 3)
    readonly property int iconSizeLarge: fontIconLargeSize
    readonly property real iconStrokeWidth: 1.5

    // Waveform bars are 3 px wide with a 2 px gap.
    readonly property int waveformBarWidth: space1 + hairlineWidth
    readonly property int waveformBarGap: space1

    // The app's frame (home.png): a 200 px sidebar, 30 px page margins,
    // a 320 px right rail, a 100 px instrument strip, a 1280 by 820 window.
    readonly property int sidebarWidth: space5 * 12 + space3
    readonly property int pagePaddingX: space7 - space1
    readonly property int pagePaddingY: space6
    readonly property int rightRailWidth: space8 * 6 + space7
    readonly property int instrumentStripHeight: space8 * 2 + space2
    readonly property int appWindowWidth: space8 * 26 + space7
    // The narrowest window every route still lays out: the sidebar, the
    // widest fixed column (History's list) and a detail at least as wide
    // as the meeting detail's rail with its margin, so no route anchors
    // its content into negative space.
    readonly property int appWindowMinWidth: sidebarWidth + historyListWidth + hairlineWidth + rightRailWidth + space8
    readonly property int appWindowHeight: space8 * 17 + space2
    readonly property int progressHairlineWidth: space1

    // First run (first-run-*.png): one 820 px column centred in the 1280
    // frame, the mark 56 px down, 45 px binding rows, 63 px model rows, a
    // 496 px snippet box beside a 300 px panel, a 130 px try-it field.
    readonly property int firstRunContentWidth: space8 * 17 + space2
    readonly property int firstRunPaddingY: space8 + space3
    readonly property int firstRunRowHeight: space8 - space1 - hairlineWidth
    readonly property int firstRunModelRowHeight: space8 + space4 + space2 - hairlineWidth
    readonly property int firstRunSnippetWidth: space8 * 10 + space5
    readonly property int firstRunPanelWidth: space8 * 6 + space4
    readonly property int firstRunFieldHeight: space8 * 2 + space7 + space1
    readonly property int firstRunActionColumn: space8 * 4 + space8 - space2
    readonly property int firstRunRunsColumn: space8 * 3 + space5

    // History (history.png): a 400 px list column, two-line rows, the
    // 48 bars of the audio strip.
    readonly property int historyListWidth: space8 * 8 + space5
    readonly property int historyRowHeight: rowHeight + space6
    readonly property int historyBarCount: 48

    // The Omarchy panel (omarchy-bar-panel.png): 340 px wide.
    readonly property int barPanelWidth: space8 * 7 + space2
    // Settings (settings-models.png, settings-hotkeys.png, agents.png): a
    // 200 px section column beside the sidebar, 45 px binding rows, 50 px
    // setting rows, 59 px model rows, a 256 px control column, a 140 px
    // chord field, a 120 px state box, a 480 px snippet box, a 320 px
    // toggle panel, a 400 px fact panel.
    readonly property int settingsNavWidth: sidebarWidth
    readonly property int settingsBindingRowHeight: space8 - space1 - hairlineWidth
    readonly property int settingsRowHeight: space8 + space1
    readonly property int settingsModelRowHeight: space8 + space4 - hairlineWidth
    readonly property int settingsControlWidth: space8 * 5 + space5
    readonly property int settingsFieldWidth: space8 * 2 + space7 + space4
    readonly property int settingsStateWidth: space8 * 2 + space6
    readonly property int settingsSnippetWidth: space8 * 10
    readonly property int settingsPanelWidth: rightRailWidth
    readonly property int settingsFactPanelWidth: space8 * 8 + space5
    readonly property int settingsFactRowHeight: space7 + hairlineWidth
}
