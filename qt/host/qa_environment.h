// QA mode for the Qt hosts (ADR 0011): the same environment variables the
// daemon reads (dettivo_core::qa), parsed once at start. A hook needs QA
// mode on, QA mode is refused in a release build unless allowed, and an
// unknown name under a reserved prefix is rejected by name so a typo in a
// scenario never silently runs the real thing. docs/qa.md is the table.
#pragma once

#include <QProcessEnvironment>
#include <QString>
#include <QStringList>

#include <optional>

namespace dettivo {

struct QaEnvironment {
    bool enabled = false;
    bool allowRelease = false;
    bool e2eComplete = false;
    QString e2eStep;
    bool e2eSeed = false;
    QString e2eOpen;
    QString e2eRoute;
    QString e2eExportDir;
    QString e2eMeetingState;
    QString e2eState;
    QString e2eOsdState;
    QString e2eBarState;
    QString qaPlant;

    /// The pill states `DETTIVO_E2E_OSD_STATE` accepts, in step with
    /// dettivo_core::qa::OSD_STATES.
    static QStringList osdStates();
    /// The `DETTIVO_E2E_BAR_STATE` values dettivo-bar renders.
    static QStringList barStates();
    /// The `DETTIVO_E2E_MEETING_STATE` values the meetings routes render.
    static QStringList meetingStates();
    /// The `DETTIVO_E2E_STATE` values the app renders as a designed state
    /// (states-and-hint-sheet.png).
    static QStringList designedStates();

    /// The routes `DETTIVO_E2E_OPEN` accepts, in step with
    /// dettivo_core::qa::ROUTES (docs/app.md).
    static QStringList routes();

    /// The documented names, kept in step with dettivo_core::qa::KNOWN.
    static QStringList knownVariables();

    /// Parses `env`; `releaseBuild` says whether QA mode needs the allow
    /// switch. On failure `error` names the variable and the reason.
    static std::optional<QaEnvironment> parse(const QProcessEnvironment &env, bool releaseBuild,
                                              QString *error);

    /// `parse` over the process environment and this build's kind.
    static std::optional<QaEnvironment> fromProcess(QString *error);
};

}  // namespace dettivo
