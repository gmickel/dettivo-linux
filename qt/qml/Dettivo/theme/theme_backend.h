// Live theme resolution for the Dettivo design system (ADR 0010).
//
// Reads the active Omarchy theme's colors.toml and, when the theme has
// one, its shell.toml, watches them
// and re-resolves within the acceptance window when `omarchy theme set`
// swaps them. Without Omarchy it follows the XDG desktop portal colour
// scheme with the built-in dark and light palettes, and it tracks the
// portal's reduced-motion preference for the motion library. Exposed to
// QML as the `ThemeBackend` singleton; Theme.qml derives the semantic
// roles, the type and spacing scales and the control geometry on top.
#pragma once

#include <QColor>
#include <QDBusVariant>
#include <QFileSystemWatcher>
#include <QJsonObject>
#include <QObject>
#include <QQmlEngine>
#include <QString>
#include <QVariantList>

#include <optional>

namespace dettivo {

/// One control state's fill and border tokens from shell.toml [controls].
struct ControlState {
    QColor color;
    qreal fillAlpha = 0.0;
    int borderWidth = 0;
    qreal borderAlpha = 0.0;
};

class ThemeBackend : public QObject {
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON

    Q_PROPERTY(QString source READ source NOTIFY themeChanged)
    Q_PROPERTY(QString themeDir READ themeDir NOTIFY themeChanged)
    Q_PROPERTY(QString lastParseError READ lastParseError NOTIFY parseErrorChanged)
    Q_PROPERTY(bool reducedMotion READ reducedMotion NOTIFY reducedMotionChanged)

    Q_PROPERTY(QColor colorAccent READ colorAccent NOTIFY themeChanged)
    Q_PROPERTY(QColor colorCursor READ colorCursor NOTIFY themeChanged)
    Q_PROPERTY(QColor colorForeground READ colorForeground NOTIFY themeChanged)
    Q_PROPERTY(QColor colorBackground READ colorBackground NOTIFY themeChanged)
    Q_PROPERTY(QColor colorSelectionForeground READ colorSelectionForeground NOTIFY themeChanged)
    Q_PROPERTY(QColor colorSelectionBackground READ colorSelectionBackground NOTIFY themeChanged)
    Q_PROPERTY(QVariantList palette16 READ palette16 NOTIFY themeChanged)
    Q_PROPERTY(QColor colorBar READ colorBar NOTIFY themeChanged)
    Q_PROPERTY(QColor colorBarActive READ colorBarActive NOTIFY themeChanged)

    Q_PROPERTY(QString fontFamily READ fontFamily NOTIFY themeChanged)
    Q_PROPERTY(int fontBaseSize READ fontBaseSize NOTIFY themeChanged)
    Q_PROPERTY(int fontHeadingSize READ fontHeadingSize NOTIFY themeChanged)
    Q_PROPERTY(int fontIconLargeSize READ fontIconLargeSize NOTIFY themeChanged)

    Q_PROPERTY(qreal spacingScale READ spacingScale NOTIFY themeChanged)
    Q_PROPERTY(int spacingXs READ spacingXs NOTIFY themeChanged)
    Q_PROPERTY(int spacingMd READ spacingMd NOTIFY themeChanged)
    Q_PROPERTY(int controlPaddingY READ controlPaddingY NOTIFY themeChanged)
    Q_PROPERTY(int panelPadding READ panelPadding NOTIFY themeChanged)
    Q_PROPERTY(int radius READ radius NOTIFY themeChanged)

    Q_PROPERTY(QColor stateNormalColor READ stateNormalColor NOTIFY themeChanged)
    Q_PROPERTY(qreal stateNormalFillAlpha READ stateNormalFillAlpha NOTIFY themeChanged)
    Q_PROPERTY(int stateNormalBorderWidth READ stateNormalBorderWidth NOTIFY themeChanged)
    Q_PROPERTY(qreal stateNormalBorderAlpha READ stateNormalBorderAlpha NOTIFY themeChanged)
    Q_PROPERTY(QColor stateHoverColor READ stateHoverColor NOTIFY themeChanged)
    Q_PROPERTY(qreal stateHoverFillAlpha READ stateHoverFillAlpha NOTIFY themeChanged)
    Q_PROPERTY(int stateHoverBorderWidth READ stateHoverBorderWidth NOTIFY themeChanged)
    Q_PROPERTY(qreal stateHoverBorderAlpha READ stateHoverBorderAlpha NOTIFY themeChanged)
    Q_PROPERTY(QColor stateFocusColor READ stateFocusColor NOTIFY themeChanged)
    Q_PROPERTY(qreal stateFocusFillAlpha READ stateFocusFillAlpha NOTIFY themeChanged)
    Q_PROPERTY(int stateFocusBorderWidth READ stateFocusBorderWidth NOTIFY themeChanged)
    Q_PROPERTY(qreal stateFocusBorderAlpha READ stateFocusBorderAlpha NOTIFY themeChanged)
    Q_PROPERTY(QColor stateSelectedColor READ stateSelectedColor NOTIFY themeChanged)
    Q_PROPERTY(qreal stateSelectedFillAlpha READ stateSelectedFillAlpha NOTIFY themeChanged)
    Q_PROPERTY(int stateSelectedBorderWidth READ stateSelectedBorderWidth NOTIFY themeChanged)
    Q_PROPERTY(qreal stateSelectedBorderAlpha READ stateSelectedBorderAlpha NOTIFY themeChanged)
    Q_PROPERTY(QColor statePressedColor READ statePressedColor NOTIFY themeChanged)
    Q_PROPERTY(qreal statePressedFillAlpha READ statePressedFillAlpha NOTIFY themeChanged)
    Q_PROPERTY(QColor stateSelectionColor READ stateSelectionColor NOTIFY themeChanged)
    Q_PROPERTY(qreal stateSelectionFillAlpha READ stateSelectionFillAlpha NOTIFY themeChanged)

public:
    explicit ThemeBackend(QObject *parent = nullptr);

    static ThemeBackend *create(QQmlEngine *, QJSEngine *);

    /// The directory Omarchy publishes the active theme in, unless
    /// DETTIVO_OMARCHY_THEME_DIR overrides it.
    static QString defaultThemeDir();

    QString source() const { return m_source; }
    QString themeDir() const { return m_themeDir; }
    QString lastParseError() const { return m_lastParseError; }
    bool reducedMotion() const { return m_reducedMotion; }

    QColor colorAccent() const { return m_colorAccent; }
    QColor colorCursor() const { return m_colorCursor; }
    QColor colorForeground() const { return m_colorForeground; }
    QColor colorBackground() const { return m_colorBackground; }
    QColor colorSelectionForeground() const { return m_colorSelectionForeground; }
    QColor colorSelectionBackground() const { return m_colorSelectionBackground; }
    QVariantList palette16() const;
    QColor colorBar() const { return m_colorBar; }
    QColor colorBarActive() const { return m_colorBarActive; }

    QString fontFamily() const { return m_fontFamily; }
    int fontBaseSize() const { return m_fontBaseSize; }
    int fontHeadingSize() const { return m_fontHeadingSize; }
    int fontIconLargeSize() const { return m_fontIconLargeSize; }

    qreal spacingScale() const { return m_spacingScale; }
    int spacingXs() const { return m_spacingXs; }
    int spacingMd() const { return m_spacingMd; }
    int controlPaddingY() const { return m_controlPaddingY; }
    int panelPadding() const { return m_panelPadding; }
    int radius() const { return m_radius; }

    QColor stateNormalColor() const { return m_normal.color; }
    qreal stateNormalFillAlpha() const { return m_normal.fillAlpha; }
    int stateNormalBorderWidth() const { return m_normal.borderWidth; }
    qreal stateNormalBorderAlpha() const { return m_normal.borderAlpha; }
    QColor stateHoverColor() const { return m_hover.color; }
    qreal stateHoverFillAlpha() const { return m_hover.fillAlpha; }
    int stateHoverBorderWidth() const { return m_hover.borderWidth; }
    qreal stateHoverBorderAlpha() const { return m_hover.borderAlpha; }
    QColor stateFocusColor() const { return m_focus.color; }
    qreal stateFocusFillAlpha() const { return m_focus.fillAlpha; }
    int stateFocusBorderWidth() const { return m_focus.borderWidth; }
    qreal stateFocusBorderAlpha() const { return m_focus.borderAlpha; }
    QColor stateSelectedColor() const { return m_selected.color; }
    qreal stateSelectedFillAlpha() const { return m_selected.fillAlpha; }
    int stateSelectedBorderWidth() const { return m_selected.borderWidth; }
    qreal stateSelectedBorderAlpha() const { return m_selected.borderAlpha; }
    QColor statePressedColor() const { return m_pressed.color; }
    qreal statePressedFillAlpha() const { return m_pressed.fillAlpha; }
    QColor stateSelectionColor() const { return m_selection.color; }
    qreal stateSelectionFillAlpha() const { return m_selection.fillAlpha; }

    /// The theme block `dettivo app status` and `dettivo osd status` print:
    /// `source`, `dir`, `background`, `accent` and `parse_error`, so a
    /// window in the wrong palette says which files it read and why it
    /// fell back.
    Q_INVOKABLE QJsonObject status() const;

    /// Points resolution at another directory and re-resolves at once. Tests
    /// and tooling use it; the portal is never consulted afterwards, so the
    /// fallback is deterministic.
    Q_INVOKABLE void setThemeDirForTesting(const QString &dir);

    /// Forces the fallback colour scheme (`"dark"` or `"light"`), as the
    /// portal would report it. Tests use it to cover the built-in palettes.
    Q_INVOKABLE void setPortalColorSchemeForTesting(const QString &scheme);

    /// Forces the reduced-motion preference, as the portal would report it.
    Q_INVOKABLE void setReducedMotionForTesting(bool reduced);

signals:
    void themeChanged();
    void parseErrorChanged();
    void reducedMotionChanged();

private slots:
    void onPortalSettingChanged(const QString &ns, const QString &key, const QDBusVariant &value);

private:
    void resolve();
    bool resolveFromOmarchy(const QString &dir);
    void applyBuiltin(bool light);
    void applyCanary();
    static bool qaModeOn();
    void armWatcher(const QString &dir);
    void reportParseError(const QString &message);
    void connectPortal();
    QString portalColorScheme() const;
    bool portalReducedMotion() const;
    void setReducedMotion(bool reduced);

    QFileSystemWatcher m_watcher;
    QString m_themeDir;
    bool m_testOverride = false;
    std::optional<QString> m_forcedScheme;
    std::optional<bool> m_forcedReducedMotion;

    QString m_source = QStringLiteral("builtin-dark");
    QString m_lastParseError;
    bool m_reducedMotion = false;

    QColor m_colorAccent;
    QColor m_colorCursor;
    QColor m_colorForeground;
    QColor m_colorBackground;
    QColor m_colorSelectionForeground;
    QColor m_colorSelectionBackground;
    QList<QColor> m_palette16;
    QColor m_colorBar;
    QColor m_colorBarActive;

    QString m_fontFamily = QStringLiteral("monospace");
    int m_fontBaseSize = 12;
    int m_fontHeadingSize = 16;
    int m_fontIconLargeSize = 24;

    qreal m_spacingScale = 1.0;
    int m_spacingXs = 2;
    int m_spacingMd = 3;
    int m_controlPaddingY = 4;
    int m_panelPadding = 6;
    int m_radius = 0;

    ControlState m_normal;
    ControlState m_hover;
    ControlState m_focus;
    ControlState m_selected;
    ControlState m_pressed;
    ControlState m_selection;
};

}  // namespace dettivo
