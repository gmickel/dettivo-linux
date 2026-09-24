// The text the Home surface derives from daemon facts: a chord in the
// `[hotkeys]` notation shown the way the baseline writes it, an app id
// reduced to the name people know it by, a duration in the list's units,
// a model id without its file dressing, and the clock line.
#pragma once

#include <QDateTime>
#include <QProcessEnvironment>
#include <QString>

namespace dettivo::format {

/// `SUPER CTRL, X` or `SUPER + CTRL + X` becomes `Super+Ctrl+X`; `F9` stays.
QString chord(const QString &configured);

/// `com.mitchellh.ghostty` becomes `ghostty`, `org.gnome.TextEditor`
/// `TextEditor`; a plain name stays; empty stays empty.
QString appName(const QString &appId);

/// `4 s` under a minute, `41 min` above, `1 h 12 min` above an hour.
QString duration(double seconds);

/// `ggml-large-v3-turbo.bin` or a model path becomes `large-v3-turbo`.
QString modelName(const QString &idOrPath);

/// `/home/u/.config/dettivo/config.toml` becomes `~/.config/dettivo/config.toml`;
/// a path under `XDG_CONFIG_HOME`, `XDG_DATA_HOME` or `XDG_STATE_HOME`
/// outside home is shown from that variable (`$XDG_DATA_HOME/dettivo/models`).
QString homePath(const QString &path, const QProcessEnvironment &env);

/// `Wed 3 Sep · 13:42` in the local time of `when`.
QString clock(const QDateTime &when);

/// `13:12` in local time from an ISO 8601 timestamp; empty when unreadable.
QString timeOfDay(const QString &iso);

/// `Today`, `Yesterday` or `Fri 13 Feb` for an ISO timestamp relative to
/// `today`.
QString dayLabel(const QString &iso, const QDate &today);

}  // namespace dettivo::format
