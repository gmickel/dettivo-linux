#include "qa_environment.h"

#include <QFileInfo>

namespace dettivo {
namespace {

const QStringList kKnown = {
    QStringLiteral("DETTIVO_QA_MODE"),          QStringLiteral("DETTIVO_QA"),
    QStringLiteral("DETTIVO_QA_ALLOW_RELEASE"), QStringLiteral("DETTIVO_MOCK_MODE"),
    QStringLiteral("DETTIVO_MOCK_MIC"),         QStringLiteral("DETTIVO_MOCK_SYSTEM_AUDIO"),
    QStringLiteral("DETTIVO_MOCK_INSERT"),      QStringLiteral("DETTIVO_MOCK_INPUT"),
    QStringLiteral("DETTIVO_MOCK_A11Y"),        QStringLiteral("DETTIVO_MOCK_LLM"),
    QStringLiteral("DETTIVO_MOCK_ENGINE"),      QStringLiteral("DETTIVO_FORCE_CPU"),
    QStringLiteral("DETTIVO_MODEL_SERVER"),     QStringLiteral("DETTIVO_E2E_COMPLETE"),
    QStringLiteral("DETTIVO_E2E_STEP"),         QStringLiteral("DETTIVO_E2E_SEED"),
    QStringLiteral("DETTIVO_E2E_OPEN"),         QStringLiteral("DETTIVO_E2E_ROUTE"),
    QStringLiteral("DETTIVO_E2E_EXPORT_DIR"),   QStringLiteral("DETTIVO_E2E_DISCLOSURE"),
    QStringLiteral("DETTIVO_E2E_OSD_STATE"),    QStringLiteral("DETTIVO_E2E_BAR_STATE"),
    QStringLiteral("DETTIVO_E2E_MEETING_STATE"), QStringLiteral("DETTIVO_E2E_STATE"),
    QStringLiteral("DETTIVO_QA_PACING"),
    QStringLiteral("DETTIVO_QA_PLANT"),         QStringLiteral("DETTIVO_QA_CANARY"),
};

const QStringList kPlants = {QStringLiteral("basic_control"), QStringLiteral("basic_background"),
                             QStringLiteral("basic_content"), QStringLiteral("default_font")};

const QStringList kOsdStates = {
    QStringLiteral("listening"), QStringLiteral("transcribing"), QStringLiteral("enhancing"),
    QStringLiteral("inserted"),  QStringLiteral("copied"),       QStringLiteral("error"),
    QStringLiteral("hidden"),
};

// What dettivo-bar renders: the panel in one of its states, or one bar
// glyph cell (docs/omarchy.md).
const QStringList kBarStates = {
    QStringLiteral("panel-idle"),   QStringLiteral("panel-recording"),    QStringLiteral("panel-transcribing"),
    QStringLiteral("panel-meeting"), QStringLiteral("panel-hint"),        QStringLiteral("glyph-idle"),
    QStringLiteral("glyph-listening"), QStringLiteral("glyph-transcribing"), QStringLiteral("glyph-meeting"),
    QStringLiteral("glyph-error"),
};

// The meetings screens `DETTIVO_E2E_MEETING_STATE` renders with the
// baselines' sample (docs/app.md).
const QStringList kMeetingStates = {
    QStringLiteral("list"),   QStringLiteral("list-empty"),        QStringLiteral("live"),
    QStringLiteral("detail-transcript"), QStringLiteral("detail-notes"), QStringLiteral("detail-analysis"),
    QStringLiteral("rename"), QStringLiteral("import"),            QStringLiteral("disclosure"),
};

// The designed states `DETTIVO_E2E_STATE` renders with sample facts and no
// daemon (states-and-hint-sheet.png, docs/app.md): nine states and the
// keyboard hint sheet.
const QStringList kDesignedStates = {
    QStringLiteral("history-empty"),    QStringLiteral("meetings-empty"),   QStringLiteral("search-no-results"),
    QStringLiteral("engine-loading"),   QStringLiteral("engine-crashed"),   QStringLiteral("download-failed"),
    QStringLiteral("microphone-missing"), QStringLiteral("insertion-fell-back"), QStringLiteral("meeting-recovered"),
    QStringLiteral("hint-sheet"),
};

// The routes DETTIVO_E2E_OPEN names, in step with dettivo_core::qa::ROUTES.
const QStringList kRoutes = {
    QStringLiteral("home"),          QStringLiteral("history"),  QStringLiteral("history.detail"),
    QStringLiteral("meetings"),      QStringLiteral("meetings.live"), QStringLiteral("meetings.detail"),
    QStringLiteral("settings"),      QStringLiteral("onboarding"),
};

const QStringList kReservedPrefixes = {
    QStringLiteral("DETTIVO_QA"),
    QStringLiteral("DETTIVO_MOCK_"),
    QStringLiteral("DETTIVO_E2E_"),
};

bool truthy(const QString &value)
{
    const QString v = value.trimmed().toLower();
    return v == QStringLiteral("1") || v == QStringLiteral("true") || v == QStringLiteral("yes")
        || v == QStringLiteral("on");
}

bool fail(QString *error, const QString &variable, const QString &message)
{
    if (error != nullptr)
        *error = variable + QStringLiteral(": ") + message;
    return false;
}

}  // namespace

QStringList QaEnvironment::knownVariables()
{
    return kKnown;
}

QStringList QaEnvironment::osdStates()
{
    return kOsdStates;
}

QStringList QaEnvironment::barStates()
{
    return kBarStates;
}

QStringList QaEnvironment::meetingStates()
{
    return kMeetingStates;
}

QStringList QaEnvironment::designedStates()
{
    return kDesignedStates;
}

QStringList QaEnvironment::routes()
{
    QStringList out;
    for (const QString &route : kRoutes) {
        if (route == QStringLiteral("onboarding"))
            out.append(QStringLiteral("settings.<section>"));
        out.append(route);
    }
    return out;
}

std::optional<QaEnvironment> QaEnvironment::parse(const QProcessEnvironment &env, bool releaseBuild,
                                                  QString *error)
{
    QaEnvironment qa;
    const QStringList keys = env.keys();
    for (const QString &key : keys) {
        bool reserved = false;
        for (const QString &prefix : kReservedPrefixes)
            reserved = reserved || key.startsWith(prefix);
        if (reserved && !kKnown.contains(key)) {
            fail(error, key, QStringLiteral("unknown QA variable (see docs/qa.md)"));
            return std::nullopt;
        }
    }
    auto get = [&env](const char *name) -> QString {
        return env.value(QString::fromLatin1(name)).trimmed();
    };

    qa.enabled = truthy(get("DETTIVO_QA_MODE")) || truthy(get("DETTIVO_QA")) || truthy(get("DETTIVO_MOCK_MODE"));
    qa.allowRelease = truthy(get("DETTIVO_QA_ALLOW_RELEASE"));
    if (qa.enabled && releaseBuild && !qa.allowRelease) {
        fail(error, QStringLiteral("DETTIVO_QA_MODE"),
             QStringLiteral("QA mode is refused in a release build; set DETTIVO_QA_ALLOW_RELEASE=1 to allow it"));
        return std::nullopt;
    }

    for (const QString &name : kKnown) {
        if (name == QStringLiteral("DETTIVO_QA_MODE") || name == QStringLiteral("DETTIVO_QA")
            || name == QStringLiteral("DETTIVO_MOCK_MODE") || name == QStringLiteral("DETTIVO_QA_ALLOW_RELEASE"))
            continue;
        if (!env.value(name).isEmpty() && !qa.enabled) {
            fail(error, name, QStringLiteral("needs QA mode (DETTIVO_QA_MODE=1)"));
            return std::nullopt;
        }
    }
    const QString mockMic = get("DETTIVO_MOCK_MIC");
    const QString mockSystemAudio = get("DETTIVO_MOCK_SYSTEM_AUDIO");
    const QString mockLlm = get("DETTIVO_MOCK_LLM");
    if (!mockLlm.isEmpty() && mockLlm != QStringLiteral("echo")
        && !(mockLlm.startsWith(QStringLiteral("fixture:")) && mockLlm.size() > 8)) {
        fail(error, QStringLiteral("DETTIVO_MOCK_LLM"), QStringLiteral("expected `echo` or `fixture:<dir>`"));
        return std::nullopt;
    }
    const QString mockEngine = get("DETTIVO_MOCK_ENGINE");
    if (!mockEngine.isEmpty()) {
        const QStringList entries = mockEngine.split(QLatin1Char(','), Qt::SkipEmptyParts);
        for (const QString &entry : entries) {
            const int eq = entry.indexOf(QLatin1Char('='));
            const QString spec = eq < 0 ? QString() : entry.mid(eq + 1).trimmed();
            if (eq <= 0 || !spec.startsWith(QStringLiteral("fixture:")) || spec.size() <= 8) {
                fail(error, QStringLiteral("DETTIVO_MOCK_ENGINE"),
                     QStringLiteral("expected `<name>=fixture:<dir>`"));
                return std::nullopt;
            }
        }
    }
    const QString modelServer = get("DETTIVO_MODEL_SERVER");
    if (!modelServer.isEmpty() && !modelServer.startsWith(QStringLiteral("http://"))
        && !modelServer.startsWith(QStringLiteral("https://"))) {
        fail(error, QStringLiteral("DETTIVO_MODEL_SERVER"), QStringLiteral("expected an http(s) URL"));
        return std::nullopt;
    }
    const QString disclosure = get("DETTIVO_E2E_DISCLOSURE");
    if (!disclosure.isEmpty() && disclosure != QStringLiteral("acknowledged")
        && disclosure != QStringLiteral("pending")) {
        fail(error, QStringLiteral("DETTIVO_E2E_DISCLOSURE"), QStringLiteral("expected acknowledged or pending"));
        return std::nullopt;
    }
    qa.e2eComplete = truthy(get("DETTIVO_E2E_COMPLETE"));
    qa.e2eSeed = truthy(get("DETTIVO_E2E_SEED"));
    qa.e2eStep = get("DETTIVO_E2E_STEP");
    if (!qa.e2eStep.isEmpty() && qa.e2eStep != QStringLiteral("keys")
        && qa.e2eStep != QStringLiteral("models") && qa.e2eStep != QStringLiteral("try")) {
        fail(error, QStringLiteral("DETTIVO_E2E_STEP"), QStringLiteral("expected keys, models or try"));
        return std::nullopt;
    }
    qa.e2eOpen = get("DETTIVO_E2E_OPEN");
    if (!qa.e2eOpen.isEmpty()) {
        const bool settingsSub = qa.e2eOpen.startsWith(QStringLiteral("settings."))
            && qa.e2eOpen.size() > 9;
        if (!kRoutes.contains(qa.e2eOpen) && !settingsSub) {
            fail(error, QStringLiteral("DETTIVO_E2E_OPEN"),
                 QStringLiteral("unknown route; the routes are ") + routes().join(QStringLiteral(", ")));
            return std::nullopt;
        }
    }
    qa.e2eRoute = get("DETTIVO_E2E_ROUTE");
    qa.e2eExportDir = get("DETTIVO_E2E_EXPORT_DIR");
    qa.e2eOsdState = get("DETTIVO_E2E_OSD_STATE");
    if (!qa.e2eOsdState.isEmpty() && !kOsdStates.contains(qa.e2eOsdState)) {
        fail(error, QStringLiteral("DETTIVO_E2E_OSD_STATE"),
             QStringLiteral("expected one of ") + kOsdStates.join(QStringLiteral(", ")));
        return std::nullopt;
    }

    qa.e2eBarState = get("DETTIVO_E2E_BAR_STATE");
    if (!qa.e2eBarState.isEmpty() && !kBarStates.contains(qa.e2eBarState)) {
        fail(error, QStringLiteral("DETTIVO_E2E_BAR_STATE"),
             QStringLiteral("expected one of ") + kBarStates.join(QStringLiteral(", ")));
        return std::nullopt;
    }

    qa.e2eMeetingState = get("DETTIVO_E2E_MEETING_STATE");
    if (!qa.e2eMeetingState.isEmpty() && !kMeetingStates.contains(qa.e2eMeetingState)) {
        fail(error, QStringLiteral("DETTIVO_E2E_MEETING_STATE"),
             QStringLiteral("expected one of ") + kMeetingStates.join(QStringLiteral(", ")));
        return std::nullopt;
    }

    qa.e2eState = get("DETTIVO_E2E_STATE");
    if (!qa.e2eState.isEmpty() && !kDesignedStates.contains(qa.e2eState)) {
        fail(error, QStringLiteral("DETTIVO_E2E_STATE"),
             QStringLiteral("expected one of ") + kDesignedStates.join(QStringLiteral(", ")));
        return std::nullopt;
    }

    qa.qaPlant = get("DETTIVO_QA_PLANT");
    if (!qa.qaPlant.isEmpty() && !kPlants.contains(qa.qaPlant)) {
        fail(error, QStringLiteral("DETTIVO_QA_PLANT"),
             QStringLiteral("expected one of ") + kPlants.join(QStringLiteral(", ")));
        return std::nullopt;
    }
    for (const auto &[variable, path] : {std::pair{QStringLiteral("DETTIVO_MOCK_MIC"), mockMic},
                                         std::pair{QStringLiteral("DETTIVO_MOCK_SYSTEM_AUDIO"), mockSystemAudio}}) {
        if (!path.isEmpty() && !QFileInfo(path).isFile()) {
            fail(error, variable, path + QStringLiteral(" is not a file"));
            return std::nullopt;
        }
    }
    return qa;
}

std::optional<QaEnvironment> QaEnvironment::fromProcess(QString *error)
{
#ifdef QT_NO_DEBUG
    const bool releaseBuild = true;
#else
    const bool releaseBuild = false;
#endif
    return parse(QProcessEnvironment::systemEnvironment(), releaseBuild, error);
}

}  // namespace dettivo
