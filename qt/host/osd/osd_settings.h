// The `[osd]` section of config.toml as dettivo-osd reads it (ADR 0009:
// the file is the source of truth; the pill has no settings of its own).
// Missing keys keep their defaults, a bad value is reported once and
// replaced by the default, and the schema lives in
// crates/dettivo-core/src/config/schema.rs.
#pragma once

#include <QProcessEnvironment>
#include <QString>
#include <QStringList>

namespace dettivo {

struct OsdSettings {
    bool enabled = true;
    QString host = QStringLiteral("auto");
    QString position = QStringLiteral("top");
    int margin = 24;
    QString monitor = QStringLiteral("focused");
    int hideAfterMs = 1800;
    int errorHideAfterMs = 4000;
    bool showLevel = true;
    QString motion = QStringLiteral("full");

    /// The accepted `position` values, in the order the docs list them.
    static QStringList positions();
    /// The accepted `host` values.
    static QStringList hosts();

    /// `DETTIVO_CONFIG`, then `$XDG_CONFIG_HOME/dettivo/config.toml`.
    static QString configPath(const QProcessEnvironment &env);
    /// The directory the daemon socket lives in, where `osd.sock` and the
    /// disabled notice go: the directory of `DETTIVO_IPC_SOCKET`, else
    /// `$XDG_RUNTIME_DIR/dettivo`, else `/tmp/dettivo-$USER`.
    static QString socketDir(const QProcessEnvironment &env);
    /// The daemon socket the pill subscribes on.
    static QString daemonSocket(const QProcessEnvironment &env);

    /// Reads `path`; an absent file is the defaults. `warning` collects one
    /// line per key that could not be used.
    static OsdSettings load(const QString &path, QString *warning);
    /// Parses TOML text the same way.
    static OsdSettings fromToml(const QString &text, QString *warning);

    /// True when the anchored edge is the top.
    bool anchoredTop() const { return position.startsWith(QStringLiteral("top")); }
};

}  // namespace dettivo
