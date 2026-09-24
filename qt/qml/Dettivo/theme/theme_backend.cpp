#include "theme_backend.h"

#include <toml++/toml.h>

#include <QDBusConnection>
#include <QDBusInterface>
#include <QDBusReply>
#include <QDir>
#include <QFileInfo>
#include <QStandardPaths>
#include <QVariant>

namespace dettivo {

namespace {

constexpr auto kPortalService = "org.freedesktop.portal.Desktop";
constexpr auto kPortalPath = "/org/freedesktop/portal/desktop";
constexpr auto kPortalSettings = "org.freedesktop.portal.Settings";
constexpr auto kAppearance = "org.freedesktop.appearance";
constexpr auto kGnomeInterface = "org.gnome.desktop.interface";

QColor parseHexColor(const std::optional<std::string> &value, const QColor &fallback)
{
    if (!value)
        return fallback;
    const QColor c(QString::fromStdString(*value));
    return c.isValid() ? c : fallback;
}

template <typename Table>
std::optional<std::string> tomlString(const Table &table, std::string_view key)
{
    if (auto node = table[key]) {
        if (auto s = node.template value<std::string>())
            return s;
    }
    return std::nullopt;
}

template <typename Table>
double tomlDouble(const Table &table, std::string_view key, double fallback)
{
    if (auto node = table[key]) {
        if (auto d = node.template value<double>())
            return *d;
        if (auto i = node.template value<int64_t>())
            return static_cast<double>(*i);
    }
    return fallback;
}

template <typename Table>
int tomlInt(const Table &table, std::string_view key, int fallback)
{
    if (auto node = table[key]) {
        if (auto i = node.template value<int64_t>())
            return static_cast<int>(*i);
        if (auto d = node.template value<double>())
            return static_cast<int>(*d);
    }
    return fallback;
}

// Reads one control state's four tokens; `border` false for the states the
// shell defines fill-only (pressed, selection).
template <typename Table>
ControlState readState(const Table &controls, const char *prefix, const ControlState &fallback,
                       bool border)
{
    const std::string p(prefix);
    ControlState s = fallback;
    s.color = parseHexColor(tomlString(controls, p + "-color"), fallback.color);
    s.fillAlpha = tomlDouble(controls, p + "-fill-alpha", fallback.fillAlpha);
    if (border) {
        s.borderWidth = tomlInt(controls, p + "-border-width", fallback.borderWidth);
        s.borderAlpha = tomlDouble(controls, p + "-border-alpha", fallback.borderAlpha);
    }
    return s;
}

// The portal's Read answers with a variant that may wrap another variant;
// peel every layer so the caller sees the plain value.
QVariant unwrap(QVariant value)
{
    while (value.canConvert<QDBusVariant>())
        value = value.value<QDBusVariant>().variant();
    return value;
}

}  // namespace

QString ThemeBackend::defaultThemeDir()
{
    const QByteArray override = qgetenv("DETTIVO_OMARCHY_THEME_DIR");
    if (!override.isEmpty())
        return QString::fromLocal8Bit(override);
    const QString home = QStandardPaths::writableLocation(QStandardPaths::HomeLocation);
    // Omarchy publishes the active theme here; ~/.config/omarchy/current is
    // the older location and is tried second.
    const QString state = home + QStringLiteral("/.local/state/omarchy/current/theme");
    if (QFileInfo::exists(state + QStringLiteral("/colors.toml")))
        return state;
    return home + QStringLiteral("/.config/omarchy/current/theme");
}

ThemeBackend::ThemeBackend(QObject *parent) : QObject(parent)
{
    m_themeDir = defaultThemeDir();
    connectPortal();
    resolve();
    setReducedMotion(portalReducedMotion());
    connect(&m_watcher, &QFileSystemWatcher::fileChanged, this, &ThemeBackend::resolve);
    connect(&m_watcher, &QFileSystemWatcher::directoryChanged, this, &ThemeBackend::resolve);
}

ThemeBackend *ThemeBackend::create(QQmlEngine *, QJSEngine *)
{
    return new ThemeBackend();
}

QVariantList ThemeBackend::palette16() const
{
    QVariantList out;
    out.reserve(m_palette16.size());
    for (const QColor &c : m_palette16)
        out.append(c);
    return out;
}

QJsonObject ThemeBackend::status() const
{
    return {{QStringLiteral("source"), m_source},
            {QStringLiteral("dir"), m_themeDir},
            {QStringLiteral("background"), m_colorBackground.name()},
            {QStringLiteral("accent"), m_colorAccent.name()},
            {QStringLiteral("parse_error"), m_lastParseError}};
}

void ThemeBackend::setThemeDirForTesting(const QString &dir)
{
    m_themeDir = dir;
    m_testOverride = true;
    resolve();
}

void ThemeBackend::setPortalColorSchemeForTesting(const QString &scheme)
{
    m_forcedScheme = scheme;
    m_testOverride = true;
    resolve();
}

void ThemeBackend::setReducedMotionForTesting(bool reduced)
{
    m_forcedReducedMotion = reduced;
    setReducedMotion(reduced);
}

void ThemeBackend::setReducedMotion(bool reduced)
{
    if (m_reducedMotion == reduced)
        return;
    m_reducedMotion = reduced;
    emit reducedMotionChanged();
}

void ThemeBackend::reportParseError(const QString &message)
{
    if (m_lastParseError == message)
        return;
    m_lastParseError = message;
    emit parseErrorChanged();
    qWarning("Dettivo theme: %s", qUtf8Printable(message));
}

void ThemeBackend::armWatcher(const QString &dir)
{
    if (!m_watcher.files().isEmpty())
        m_watcher.removePaths(m_watcher.files());
    if (!m_watcher.directories().isEmpty())
        m_watcher.removePaths(m_watcher.directories());
    // Omarchy swaps the whole `current/theme` symlink, so the parent
    // directory is watched as well as the files themselves.
    const QFileInfo info(dir);
    if (info.exists())
        m_watcher.addPath(dir);
    const QString parent = info.dir().absolutePath();
    if (QFileInfo::exists(parent))
        m_watcher.addPath(parent);
    for (const auto *name : {"/colors.toml", "/shell.toml"}) {
        const QString path = dir + QLatin1String(name);
        if (QFileInfo::exists(path))
            m_watcher.addPath(path);
    }
}

void ThemeBackend::resolve()
{
    armWatcher(m_themeDir);
    // colors.toml alone makes a theme: Omarchy generates shell.toml from
    // its template when a theme ships none, older releases never wrote one
    // and the shell tokens all have defaults, so its absence must not send
    // the whole palette to the built-in one.
    const bool omarchyPresent = QFileInfo::exists(m_themeDir + QStringLiteral("/colors.toml"));
    if (omarchyPresent && resolveFromOmarchy(m_themeDir)) {
        m_source = QStringLiteral("omarchy");
        if (!m_lastParseError.isEmpty()) {
            m_lastParseError.clear();
            emit parseErrorChanged();
        }
        applyCanary();
        emit themeChanged();
        return;
    }
    const bool light = portalColorScheme() == QStringLiteral("light");
    applyBuiltin(light);
    m_source = light ? QStringLiteral("builtin-light") : QStringLiteral("builtin-dark");
    applyCanary();
    emit themeChanged();
}

bool ThemeBackend::qaModeOn()
{
    for (const char *name : {"DETTIVO_QA_MODE", "DETTIVO_QA", "DETTIVO_MOCK_MODE"}) {
        const QByteArray value = qgetenv(name).trimmed().toLower();
        if (value == "1" || value == "true" || value == "yes" || value == "on")
            return true;
    }
    return false;
}

void ThemeBackend::applyCanary()
{
    // The visual job's canary (fn-20 R3): under QA mode DETTIVO_QA_CANARY
    // regresses the tokens on purpose so the diff has to fail. `1`, `true`
    // or `all` moves the accent and raises the type and spacing; `colour`
    // moves the accent alone, which the structure score is blind to by
    // design and the strict comparison against an approved render must
    // catch; `type` raises the font sizes; `spacing` widens the units.
    const QByteArray canary = qgetenv("DETTIVO_QA_CANARY").trimmed().toLower();
    if (canary.isEmpty() || canary == "0" || canary == "false" || !qaModeOn())
        return;
    const bool all = canary == "1" || canary == "true" || canary == "all";
    if (all || canary == "colour")
        m_colorAccent = m_colorCursor;
    if (all || canary == "type") {
        m_fontBaseSize += 8;
        m_fontHeadingSize += 8;
    }
    if (all || canary == "spacing") {
        m_spacingXs += 3;
        m_spacingMd += 5;
    }
}

bool ThemeBackend::resolveFromOmarchy(const QString &dir)
{
    toml::table colors;
    toml::table shell;
    const QString shellPath = dir + QStringLiteral("/shell.toml");
    try {
        colors = toml::parse_file((dir + QStringLiteral("/colors.toml")).toStdString());
        if (QFileInfo::exists(shellPath))
            shell = toml::parse_file(shellPath.toStdString());
    } catch (const toml::parse_error &err) {
        reportParseError(QStringLiteral("malformed theme file, using the built-in palette: %1")
                             .arg(QString::fromStdString(std::string(err.description()))));
        return false;
    }

    // Start from the built-in palette that matches the theme's own
    // brightness, so a theme that omits a key still renders with a value
    // from the right side of the spectrum rather than an invalid colour.
    const QColor themeBackground = parseHexColor(tomlString(colors, "background"), QColor());
    const bool lightTheme = themeBackground.isValid() && themeBackground.lightness() > 127;
    applyBuiltin(lightTheme);

    m_colorForeground = parseHexColor(tomlString(colors, "foreground"), m_colorForeground);
    m_colorBackground = parseHexColor(tomlString(colors, "background"), m_colorBackground);
    m_colorAccent = parseHexColor(tomlString(colors, "accent"), m_colorForeground);
    m_colorCursor = parseHexColor(tomlString(colors, "cursor"), m_colorAccent);
    m_colorSelectionForeground =
        parseHexColor(tomlString(colors, "selection_foreground"), m_colorBackground);
    m_colorSelectionBackground =
        parseHexColor(tomlString(colors, "selection_background"), m_colorForeground);
    for (int i = 0; i < 16; ++i) {
        const std::string key = "color" + std::to_string(i);
        m_palette16[i] = parseHexColor(tomlString(colors, key), m_palette16[i]);
    }

    // The shell tokens a theme leaves out fall back to the theme's own
    // roles, the way Omarchy's generated shell.toml fills them, never to
    // the built-in palette's literal colours: a blue theme without a
    // pressed-color gets its accent, not Black Gold's.
    m_colorBar = m_colorBackground;
    m_colorBarActive = m_colorAccent;
    m_normal.color = m_colorForeground;
    m_hover.color = m_colorForeground;
    m_focus.color = m_colorAccent;
    m_selected.color = m_colorAccent;
    m_pressed.color = m_colorAccent;
    m_selection.color = m_colorAccent;

    if (auto bar = shell["bar"].as_table()) {
        m_colorBar = parseHexColor(tomlString(*bar, "background"), m_colorBackground);
        m_colorBarActive = parseHexColor(tomlString(*bar, "active"), m_colorAccent);
    }
    if (auto font = shell["font"].as_table()) {
        m_fontBaseSize = tomlInt(*font, "base-size", m_fontBaseSize);
        m_fontHeadingSize = tomlInt(*font, "heading", m_fontHeadingSize);
        m_fontIconLargeSize = tomlInt(*font, "icon-large", m_fontIconLargeSize);
        if (auto family = tomlString(*font, "family"))
            m_fontFamily = QString::fromStdString(*family);
    }
    if (auto spacing = shell["spacing"].as_table()) {
        m_spacingScale = tomlDouble(*spacing, "scale", m_spacingScale);
        m_spacingXs = tomlInt(*spacing, "xs", m_spacingXs);
        m_spacingMd = tomlInt(*spacing, "md", m_spacingMd);
        m_controlPaddingY = tomlInt(*spacing, "control-padding-y", m_controlPaddingY);
        m_panelPadding = tomlInt(*spacing, "panel-padding", m_panelPadding);
    }
    if (auto controls = shell["controls"].as_table()) {
        m_radius = tomlInt(*controls, "radius", 0);
        m_normal = readState(*controls, "normal", m_normal, true);
        m_hover = readState(*controls, "hover-cursor", m_hover, true);
        m_focus = readState(*controls, "focus", m_focus, true);
        m_selected = readState(*controls, "selected", m_selected, true);
        m_pressed = readState(*controls, "pressed", m_pressed, false);
        m_selection = readState(*controls, "selection", m_selection, false);
    }
    return true;
}

void ThemeBackend::applyBuiltin(bool light)
{
    // The built-in palettes carry the same roles as an Omarchy theme, so
    // everything downstream reads one shape. Dark is Black Gold, the
    // reference theme; light keeps its warmth on a paper ground.
    const QStringList dark = {"#0D0D0D", "#D35F5F", "#A3850E", "#4D574E", "#6E6A58", "#BFA75D",
                              "#7A6A2C", "#F6F1DD", "#303531", "#D35F5F", "#A3850E", "#4D574E",
                              "#6E6A58", "#BFA75D", "#7A6A2C", "#F6F1DD"};
    const QStringList paper = {"#FBF8F1", "#B5473F", "#8A6D1B", "#5B6A5C", "#7C7460", "#B08B2A",
                               "#6B5A22", "#2B2A25", "#DCD5C0", "#B5473F", "#8A6D1B", "#5B6A5C",
                               "#7C7460", "#B08B2A", "#6B5A22", "#2B2A25"};
    m_palette16.clear();
    for (const QString &hex : (light ? paper : dark))
        m_palette16.append(QColor(hex));

    m_colorBackground = light ? QColor("#FBF8F1") : QColor("#0D0D0D");
    m_colorForeground = light ? QColor("#2B2A25") : QColor("#EBDBB2");
    m_colorAccent = light ? QColor("#8A6D1B") : QColor("#F5BF03");
    m_colorCursor = light ? QColor("#B08B2A") : QColor("#A3850E");
    m_colorSelectionForeground = m_colorBackground;
    m_colorSelectionBackground = m_colorForeground;
    m_colorBar = light ? QColor("#F3EEE2") : QColor("#120F02");
    m_colorBarActive = m_colorAccent;

    m_fontFamily = QStringLiteral("monospace");
    m_fontBaseSize = 12;
    m_fontHeadingSize = 16;
    m_fontIconLargeSize = 24;
    m_spacingScale = 1.0;
    m_spacingXs = 2;
    m_spacingMd = 3;
    m_controlPaddingY = 4;
    m_panelPadding = 6;
    m_radius = 0;

    m_normal = {m_colorForeground, 0.03, 1, 0.35};
    m_hover = {light ? QColor("#B08B2A") : QColor("#BFA75D"), 0.12, 2, 0.65};
    m_focus = {m_colorAccent, 0.14, 2, 0.90};
    m_selected = {light ? QColor("#8A6D1B") : QColor("#A3850E"), 0.20, 2, 0.78};
    m_pressed = {m_colorAccent, 0.24, 0, 0.0};
    m_selection = {light ? QColor("#8A6D1B") : QColor("#A3850E"), 0.40, 0, 0.0};
}

void ThemeBackend::connectPortal()
{
    if (m_testOverride)
        return;
    QDBusConnection::sessionBus().connect(
        QString::fromLatin1(kPortalService), QString::fromLatin1(kPortalPath),
        QString::fromLatin1(kPortalSettings), QStringLiteral("SettingChanged"), this,
        SLOT(onPortalSettingChanged(QString, QString, QDBusVariant)));
}

void ThemeBackend::onPortalSettingChanged(const QString &ns, const QString &key,
                                          const QDBusVariant &value)
{
    if (ns == QLatin1String(kAppearance) && key == QLatin1String("color-scheme")) {
        if (m_source != QStringLiteral("omarchy"))
            resolve();
        return;
    }
    if (ns == QLatin1String(kAppearance) && key == QLatin1String("reduced-motion")) {
        setReducedMotion(unwrap(value.variant()).toUInt() == 1);
        return;
    }
    if (ns == QLatin1String(kGnomeInterface) && key == QLatin1String("enable-animations")) {
        setReducedMotion(!unwrap(value.variant()).toBool());
    }
}

QString ThemeBackend::portalColorScheme() const
{
    if (m_forcedScheme)
        return *m_forcedScheme;
    // A test override wins over the environment so the unit tests stay
    // deterministic whatever the developer's shell exports.
    if (m_testOverride)
        return QStringLiteral("dark");
    // DETTIVO_COLOR_SCHEME pins the built-in palette off Omarchy the way
    // DETTIVO_REDUCED_MOTION pins motion: the visual job renders both.
    const QByteArray env = qgetenv("DETTIVO_COLOR_SCHEME").trimmed().toLower();
    if (env == "light" || env == "dark")
        return QString::fromLatin1(env);
    QDBusInterface portal(QString::fromLatin1(kPortalService), QString::fromLatin1(kPortalPath),
                          QString::fromLatin1(kPortalSettings));
    if (!portal.isValid())
        return QStringLiteral("dark");
    const QDBusReply<QDBusVariant> reply = portal.call(
        QStringLiteral("Read"), QString::fromLatin1(kAppearance), QStringLiteral("color-scheme"));
    if (!reply.isValid())
        return QStringLiteral("dark");
    bool ok = false;
    const uint scheme = unwrap(reply.value().variant()).toUInt(&ok);
    return (ok && scheme == 2) ? QStringLiteral("light") : QStringLiteral("dark");
}

bool ThemeBackend::portalReducedMotion() const
{
    if (m_forcedReducedMotion)
        return *m_forcedReducedMotion;
    const QByteArray env = qgetenv("DETTIVO_REDUCED_MOTION");
    if (!env.isEmpty())
        return env == "1" || env.toLower() == "true";
    if (m_testOverride)
        return false;
    QDBusInterface portal(QString::fromLatin1(kPortalService), QString::fromLatin1(kPortalPath),
                          QString::fromLatin1(kPortalSettings));
    if (!portal.isValid())
        return false;
    // The appearance namespace carries reduced-motion on newer portals; the
    // GNOME interface namespace carries enable-animations everywhere else.
    const QDBusReply<QDBusVariant> reduced = portal.call(
        QStringLiteral("Read"), QString::fromLatin1(kAppearance), QStringLiteral("reduced-motion"));
    if (reduced.isValid())
        return unwrap(reduced.value().variant()).toUInt() == 1;
    const QDBusReply<QDBusVariant> animations =
        portal.call(QStringLiteral("Read"), QString::fromLatin1(kGnomeInterface),
                    QStringLiteral("enable-animations"));
    if (animations.isValid())
        return !unwrap(animations.value().variant()).toBool();
    return false;
}

}  // namespace dettivo
