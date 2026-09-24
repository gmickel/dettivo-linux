import QtQuick
import QtTest
import Dettivo

// Theme and Motion expose their tokens, derive the roles from the palette,
// and every size is a token rather than a literal.
TestCase {
    id: root
    name: "Theme"

    function test_source_is_builtin_without_omarchy() {
        compare(Theme.source, "builtin-dark");
        verify(Theme.themeDir.length > 0);
    }

    function test_type_scale_follows_base_size() {
        compare(Theme.typeBodySize, Theme.fontBaseSize);
        compare(Theme.typeDisplaySize, Math.round(Theme.fontBaseSize * 7 / 3));
        compare(Theme.typeTitleSize, Math.round(Theme.fontBaseSize * 5 / 3));
        compare(Theme.typeHeadingSize, Theme.fontHeadingSize);
        compare(Theme.typeCaptionSize, Theme.fontBaseSize - 1);
        compare(Theme.typeLabelSize, Theme.fontBaseSize - 2);
        compare(Theme.typeProvisionalWeight, Font.Light);
        compare(Theme.typeTabularNumerals.tnum, 1);
    }

    function test_spacing_scale_doubles_from_xs() {
        compare(Theme.space1, Theme.spacingXs);
        compare(Theme.space2, Theme.spacingXs * 2);
        compare(Theme.space3, Theme.spacingXs * 4);
        compare(Theme.space4, Theme.spacingXs * 6);
        compare(Theme.space5, Theme.spacingXs * 8);
        compare(Theme.space8, Theme.spacingXs * 24);
        compare(Theme.controlHeight, Theme.typeBodySize + 2 * Theme.controlPaddingY + Theme.space3);
        compare(Theme.rowPaddingX, Theme.space4);
        compare(Theme.controlGap, Theme.space3);
    }

    function test_roles_derive_from_palette_and_alphas() {
        compare(Theme.roleText, Theme.colorForeground);
        fuzzyCompare(Theme.roleMutedText.a, 0.62, 0.01);
        fuzzyCompare(Theme.roleHairline.a, 0.14, 0.01);
        fuzzyCompare(Theme.roleBorder.a, Theme.stateNormalBorderAlpha, 0.01);
        fuzzyCompare(Theme.roleSelectedFill.a, 0.18, 0.01);
        compare(Theme.roleUrgent, Theme.palette16[1]);
        compare(Theme.roleSpeakerColors.length, 5);
        compare(Theme.roleSpeakerColors[0], Theme.palette16[5]);
        compare(Theme.radius, 0);
    }

    function test_motion_tokens_and_reduced_motion() {
        compare(Motion.durationEnter, 180);
        compare(Motion.durationExit, 120);
        compare(Motion.durationReveal, 240);
        compare(Motion.durationPill, 400);
        compare(Motion.durationShimmer, 1200);
        compare(Motion.easingEnter.length, 6);
        compare(Motion.reducedMotion, false);
        compare(Motion.duration(180), 180);
        ThemeBackend.setReducedMotionForTesting(true);
        compare(Motion.reducedMotion, true);
        compare(Motion.duration(180), 0);
        ThemeBackend.setReducedMotionForTesting(false);
    }
}
