// ThemeBackend coverage: Omarchy resolution, a live re-resolve within 100 ms
// on a file swap, a malformed file falling back with one report, the
// portal-scheme fallback to the built-in palettes, and reduced motion.
#include "theme_backend.h"

#include <QElapsedTimer>
#include <QFileInfo>
#include <QJsonObject>
#include <QSignalSpy>
#include <QTemporaryDir>
#include <QTest>

using dettivo::ThemeBackend;

namespace {

void writeFile(const QString &path, const QString &contents)
{
    QFile f(path);
    QVERIFY(f.open(QIODevice::WriteOnly | QIODevice::Truncate));
    f.write(contents.toUtf8());
    f.close();
}

const char *kBlackGoldColors = R"(
accent = "#6E6A58"
cursor = "#a3850e"
foreground = "#ebdbb2"
background = "#0D0D0D"
selection_foreground = "#0D0D0D"
selection_background = "#ebdbb2"
color0 = "#0D0D0D"
color1 = "#D35F5F"
color2 = "#a3850e"
color3 = "#4D574E"
color4 = "#6E6A58"
color5 = "#BFA75D"
color6 = "#7A6A2C"
color7 = "#F6F1DD"
color8 = "#303531"
color9 = "#D35F5F"
color10 = "#a3850e"
color11 = "#4D574E"
color12 = "#6E6A58"
color13 = "#BFA75D"
color14 = "#7A6A2C"
color15 = "#F6F1DD"
)";

const char *kBlackGoldShell = R"(
[bar]
background = "#120F02"
active = "#F5BF03"

[font]
base-size = 12
heading = 16
icon-large = 24

[spacing]
scale = 1.0
xs = 2
md = 3
control-padding-y = 4
panel-padding = 6

[controls]
normal-color = "#EBDBB2"
normal-fill-alpha = 0.03
normal-border-width = 1
normal-border-alpha = 0.35
hover-cursor-color = "#BFA75D"
hover-cursor-fill-alpha = 0.12
hover-cursor-border-width = 2
hover-cursor-border-alpha = 0.65
focus-color = "#F5BF03"
focus-fill-alpha = 0.14
focus-border-width = 2
focus-border-alpha = 0.90
selected-color = "#A3850E"
selected-fill-alpha = 0.20
selected-border-width = 2
selected-border-alpha = 0.78
pressed-color = "#F5BF03"
pressed-fill-alpha = 0.24
selection-color = "#A3850E"
selection-fill-alpha = 0.40
)";

const char *kTokyoNightColors = R"(
accent = "#7aa2f7"
foreground = "#c0caf5"
background = "#1a1b26"
color0 = "#1a1b26"
color1 = "#f7768e"
)";

const char *kTokyoNightShell = R"(
[font]
base-size = 14
heading = 18

[spacing]
xs = 3

[controls]
normal-color = "#c0caf5"
focus-color = "#7aa2f7"
radius = 6
)";

}  // namespace

class ThemeBackendTest : public QObject {
    Q_OBJECT

private slots:
    void resolvesEveryTokenFromOmarchyFiles();
    void resolvesTheLastCallFixtureAsPublishedByOmarchy();
    void colorsAloneMakeAnOmarchyTheme();
    void reResolvesWithin100msOnThemeSwap();
    void malformedFileFallsBackAndReportsOnce();
    void noOmarchyFollowsPortalScheme();
    void reducedMotionFollowsPortalAndOverride();
};

void ThemeBackendTest::resolvesEveryTokenFromOmarchyFiles()
{
    QTemporaryDir dir;
    QVERIFY(dir.isValid());
    writeFile(dir.filePath("colors.toml"), QString::fromUtf8(kBlackGoldColors));
    writeFile(dir.filePath("shell.toml"), QString::fromUtf8(kBlackGoldShell));

    ThemeBackend theme;
    theme.setThemeDirForTesting(dir.path());

    QCOMPARE(theme.source(), QStringLiteral("omarchy"));
    QCOMPARE(theme.colorForeground(), QColor("#ebdbb2"));
    QCOMPARE(theme.colorBackground(), QColor("#0D0D0D"));
    QCOMPARE(theme.colorAccent(), QColor("#6E6A58"));
    QCOMPARE(theme.colorBar(), QColor("#120F02"));
    QCOMPARE(theme.colorBarActive(), QColor("#F5BF03"));
    QCOMPARE(theme.palette16().size(), 16);
    QCOMPARE(theme.palette16().at(5).value<QColor>(), QColor("#BFA75D"));
    QCOMPARE(theme.fontBaseSize(), 12);
    QCOMPARE(theme.fontHeadingSize(), 16);
    QCOMPARE(theme.spacingXs(), 2);
    QCOMPARE(theme.controlPaddingY(), 4);
    QCOMPARE(theme.radius(), 0);
    QCOMPARE(theme.stateNormalFillAlpha(), 0.03);
    QCOMPARE(theme.stateHoverColor(), QColor("#BFA75D"));
    QCOMPARE(theme.stateFocusBorderWidth(), 2);
    QCOMPARE(theme.stateSelectedBorderAlpha(), 0.78);
    QCOMPARE(theme.statePressedFillAlpha(), 0.24);
    QCOMPARE(theme.stateSelectionFillAlpha(), 0.40);
    QVERIFY(theme.lastParseError().isEmpty());
}

// The regression for the window that stayed in the built-in palette on
// the development desktop: the checked-in copy of the theme Omarchy had
// published (qt/fixtures/themes/last-call, note in colors.toml) resolves
// as omarchy with its own accent and background, and the status block
// says so.
void ThemeBackendTest::resolvesTheLastCallFixtureAsPublishedByOmarchy()
{
    const QString dir = QStringLiteral(DETTIVO_THEME_FIXTURES_DIR "/last-call");
    QVERIFY2(QFileInfo::exists(dir + QStringLiteral("/colors.toml")), qPrintable(dir));

    ThemeBackend theme;
    theme.setThemeDirForTesting(dir);

    QCOMPARE(theme.source(), QStringLiteral("omarchy"));
    QCOMPARE(theme.colorAccent(), QColor("#00c6c2"));
    QCOMPARE(theme.colorBackground(), QColor("#0b1d20"));
    QCOMPARE(theme.colorForeground(), QColor("#94b3b5"));
    QCOMPARE(theme.colorBarActive(), QColor("#ed634c"));
    QCOMPARE(theme.spacingScale(), 0.95);
    QCOMPARE(theme.panelPadding(), 16);
    QCOMPARE(theme.stateHoverColor(), QColor("#e0f5f2"));
    // Last Call names no pressed-color or selection-color: they follow its
    // accent rather than the gold of the built-in palette.
    QCOMPARE(theme.statePressedColor(), QColor("#00c6c2"));
    QCOMPARE(theme.stateSelectionColor(), QColor("#00c6c2"));
    QVERIFY(theme.lastParseError().isEmpty());

    const QJsonObject status = theme.status();
    QCOMPARE(status.value(QStringLiteral("source")).toString(), QStringLiteral("omarchy"));
    QCOMPARE(status.value(QStringLiteral("dir")).toString(), dir);
    QCOMPARE(status.value(QStringLiteral("accent")).toString(), QStringLiteral("#00c6c2"));
    QCOMPARE(status.value(QStringLiteral("background")).toString(), QStringLiteral("#0b1d20"));
    QCOMPARE(status.value(QStringLiteral("parse_error")).toString(), QString());
}

// Every official Omarchy theme ships colors.toml alone; shell.toml is
// generated, and older releases never wrote one. The palette is the
// theme's, the shell tokens take their defaults, and the source is still
// omarchy rather than the built-in palette.
void ThemeBackendTest::colorsAloneMakeAnOmarchyTheme()
{
    QTemporaryDir dir;
    QVERIFY(dir.isValid());
    writeFile(dir.filePath("colors.toml"), QString::fromUtf8(kTokyoNightColors));

    ThemeBackend theme;
    theme.setThemeDirForTesting(dir.path());

    QCOMPARE(theme.source(), QStringLiteral("omarchy"));
    QCOMPARE(theme.colorAccent(), QColor("#7aa2f7"));
    QCOMPARE(theme.colorBackground(), QColor("#1a1b26"));
    QCOMPARE(theme.colorBar(), QColor("#1a1b26"));
    QCOMPARE(theme.colorBarActive(), QColor("#7aa2f7"));
    // The state colours are the theme's roles, not Black Gold's literals.
    QCOMPARE(theme.stateNormalColor(), QColor("#c0caf5"));
    QCOMPARE(theme.stateHoverColor(), QColor("#c0caf5"));
    QCOMPARE(theme.stateFocusColor(), QColor("#7aa2f7"));
    QCOMPARE(theme.statePressedColor(), QColor("#7aa2f7"));
    QCOMPARE(theme.stateSelectionColor(), QColor("#7aa2f7"));
    QCOMPARE(theme.stateNormalFillAlpha(), 0.03);
    QCOMPARE(theme.fontBaseSize(), 12);
    QCOMPARE(theme.spacingXs(), 2);
    QCOMPARE(theme.radius(), 0);
    QVERIFY(theme.lastParseError().isEmpty());

    // A shell.toml that appears later is picked up like any other change.
    QSignalSpy spy(&theme, &ThemeBackend::themeChanged);
    writeFile(dir.filePath("shell.toml"), QString::fromUtf8(kTokyoNightShell));
    QVERIFY(spy.wait(200));
    QTRY_COMPARE(theme.radius(), 6);
    QCOMPARE(theme.fontBaseSize(), 14);
    QCOMPARE(theme.source(), QStringLiteral("omarchy"));
}

void ThemeBackendTest::reResolvesWithin100msOnThemeSwap()
{
    QTemporaryDir dir;
    QVERIFY(dir.isValid());
    writeFile(dir.filePath("colors.toml"), QString::fromUtf8(kBlackGoldColors));
    writeFile(dir.filePath("shell.toml"), QString::fromUtf8(kBlackGoldShell));

    ThemeBackend theme;
    theme.setThemeDirForTesting(dir.path());
    QCOMPARE(theme.colorAccent(), QColor("#6E6A58"));

    QSignalSpy spy(&theme, &ThemeBackend::themeChanged);
    QElapsedTimer timer;
    timer.start();
    writeFile(dir.filePath("colors.toml"), QString::fromUtf8(kTokyoNightColors));
    writeFile(dir.filePath("shell.toml"), QString::fromUtf8(kTokyoNightShell));

    QVERIFY(spy.wait(200));
    QVERIFY2(timer.elapsed() < 100,
             qPrintable(QStringLiteral("re-resolve took %1 ms, want under 100").arg(timer.elapsed())));
    QTRY_COMPARE(theme.colorAccent(), QColor("#7aa2f7"));
    QCOMPARE(theme.fontBaseSize(), 14);
    QCOMPARE(theme.spacingXs(), 3);
    QCOMPARE(theme.radius(), 6);
    QCOMPARE(theme.source(), QStringLiteral("omarchy"));
}

void ThemeBackendTest::malformedFileFallsBackAndReportsOnce()
{
    QTemporaryDir dir;
    QVERIFY(dir.isValid());
    writeFile(dir.filePath("colors.toml"), QStringLiteral("this is not [valid toml"));
    writeFile(dir.filePath("shell.toml"), QString::fromUtf8(kBlackGoldShell));

    ThemeBackend theme;
    QSignalSpy errors(&theme, &ThemeBackend::parseErrorChanged);
    theme.setThemeDirForTesting(dir.path());

    QCOMPARE(theme.source(), QStringLiteral("builtin-dark"));
    QCOMPARE(theme.colorBackground(), QColor("#0D0D0D"));
    QVERIFY(!theme.lastParseError().isEmpty());
    QCOMPARE(errors.count(), 1);

    theme.setThemeDirForTesting(dir.path());
    QCOMPARE(errors.count(), 1);
}

void ThemeBackendTest::noOmarchyFollowsPortalScheme()
{
    QTemporaryDir dir;
    QVERIFY(dir.isValid());

    ThemeBackend theme;
    theme.setThemeDirForTesting(dir.path());
    QCOMPARE(theme.source(), QStringLiteral("builtin-dark"));
    QCOMPARE(theme.palette16().size(), 16);

    theme.setPortalColorSchemeForTesting(QStringLiteral("light"));
    QCOMPARE(theme.source(), QStringLiteral("builtin-light"));
    QVERIFY(theme.colorBackground().lightness() > theme.colorForeground().lightness());
    QCOMPARE(theme.palette16().size(), 16);
    QCOMPARE(theme.stateNormalBorderWidth(), 1);

    theme.setPortalColorSchemeForTesting(QStringLiteral("dark"));
    QCOMPARE(theme.source(), QStringLiteral("builtin-dark"));
}

void ThemeBackendTest::reducedMotionFollowsPortalAndOverride()
{
    QTemporaryDir dir;
    QVERIFY(dir.isValid());
    ThemeBackend theme;
    theme.setThemeDirForTesting(dir.path());
    QCOMPARE(theme.reducedMotion(), false);

    QSignalSpy spy(&theme, &ThemeBackend::reducedMotionChanged);
    theme.setReducedMotionForTesting(true);
    QCOMPARE(theme.reducedMotion(), true);
    QCOMPARE(spy.count(), 1);
    theme.setReducedMotionForTesting(true);
    QCOMPARE(spy.count(), 1);
}

QTEST_MAIN(ThemeBackendTest)
#include "theme_backend_test.moc"
