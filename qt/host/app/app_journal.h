// The app's QA journal (ADR 0011): under QA mode the host appends one JSON
// line per fact a drive wants to read back (the first frame time, a theme
// applied with its timing, the resident set) to
// `$XDG_STATE_HOME/dettivo/qa/app.jsonl`, beside the daemon's own QA
// files. Off QA mode nothing is written.
#pragma once

#include <QJsonObject>
#include <QString>

namespace dettivo {

class AppJournal {
public:
    /// `enabled` false makes every `record` a no-op.
    AppJournal(const QString &stateDir, bool enabled);

    /// Appends `{"event": <event>, ...fields}`.
    void record(const QString &event, const QJsonObject &fields = {});

    QString path() const { return m_path; }
    bool enabled() const { return m_enabled; }

    /// The process's resident set in KiB from /proc, or -1 when unavailable.
    static qint64 residentKb();

private:
    QString m_path;
    bool m_enabled;
};

}  // namespace dettivo
