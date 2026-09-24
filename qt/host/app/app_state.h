// Runtime state that is not configuration (docs/config.md): the window's
// geometry, the last route and the first-run progress, in
// `$XDG_STATE_HOME/dettivo/state.toml`. The daemon keeps its own tables
// (`[daemon]`, `[acknowledgements]`) in the same file and writes them
// while the app is open, so a save writes this app's tables and reads
// every other table fresh from the file at that moment; nothing is kept
// from the load. Nothing here ever lands in config.toml; a file that
// does not parse is reported once and replaced by the defaults on the
// next save.
#pragma once

#include <QProcessEnvironment>
#include <QString>

namespace dettivo {

struct AppState {
    int width = 0;
    int height = 0;
    int x = -1;
    int y = -1;
    bool maximized = false;
    QString lastRoute = QStringLiteral("home");
    // The detail tab a meeting reopens on (`transcript`, `notes`, `analysis`).
    QString lastMeetingTab = QStringLiteral("transcript");
    // When first run finished (ISO 8601 UTC); empty until it does.
    QString firstRunCompletedAt;
    // The step first run was on when the window closed mid-flow.
    QString firstRunStep;

    /// `<stateDir>/state.toml`.
    static QString path(const QProcessEnvironment &env);
    /// Reads `path`; an absent file is the defaults, an unreadable one
    /// too with `warning` set.
    static AppState load(const QString &path, QString *warning);
    /// Parses TOML text the same way.
    static AppState fromToml(const QString &text, QString *warning);
    /// The TOML text of this app's tables.
    QString toToml() const;
    /// Writes atomically, creating the directory, with the daemon's
    /// tables as the file holds them now; `error` says why not.
    bool save(const QString &path, QString *error) const;
    /// True once first run has been completed.
    bool firstRunComplete() const { return !firstRunCompletedAt.isEmpty(); }
};

}  // namespace dettivo
