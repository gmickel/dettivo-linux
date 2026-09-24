#include "host.h"
#include "pacing_collector.h"
#include "qa_environment.h"
#include "render.h"

#include <QCoreApplication>
#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQuickStyle>
#include <QQuickWindow>
#include <QString>
#include <QTimer>
#include <QUrl>

#include <cstdio>
#include <cstring>

namespace dettivo {

namespace {

bool hasFlag(int argc, char **argv, const char *flag)
{
    for (int i = 1; i < argc; ++i) {
        if (std::strcmp(argv[i], flag) == 0)
            return true;
    }
    return false;
}

/// The value after `flag`, or an empty string.
QString flagValue(int argc, char **argv, const char *flag)
{
    for (int i = 1; i + 1 < argc; ++i) {
        if (std::strcmp(argv[i], flag) == 0)
            return QString::fromLocal8Bit(argv[i + 1]);
    }
    return QString();
}

constexpr auto kStyle = "DettivoStyle";

}  // namespace

int runHost(int argc, char **argv, const HostSpec &spec)
{
    if (hasFlag(argc, argv, "--version")) {
        std::printf("%s %s\n", spec.name, DETTIVO_VERSION);
        return 0;
    }
    const bool smoke = hasFlag(argc, argv, "--smoke");
    const QString render = flagValue(argc, argv, "--render");

    // QA mode (ADR 0011): a bad or unknown QA variable is a refusal, not a
    // silent real run, and it is decided before any GUI object exists.
    QString qaError;
    const auto qa = QaEnvironment::fromProcess(&qaError);
    if (!qa.has_value()) {
        std::fprintf(stderr, "%s: %s\n", spec.name, qPrintable(qaError));
        return 2;
    }
    QGuiApplication app(argc, argv);
    QGuiApplication::setApplicationName(QString::fromLatin1(spec.name));
    QGuiApplication::setApplicationVersion(QStringLiteral(DETTIVO_VERSION));
    QGuiApplication::setOrganizationName(QStringLiteral("dettivo"));

    // Every Dettivo surface draws its controls with the DettivoStyle module
    // (ADR 0010). The style has to be chosen before the first control is
    // created, and the assertion below refuses to run with any other style:
    // a control drawn by Basic or Fusion would break the theme contract
    // silently, so it is a startup failure instead.
    QQuickStyle::setStyle(QString::fromLatin1(kStyle));
    QQuickStyle::setFallbackStyle(QStringLiteral("Basic"));

    QQmlApplicationEngine engine;
    // The installed shared modules live here (ADR 0001); the build tree works
    // without it because the modules are linked into the binary.
    engine.addImportPath(QStringLiteral(DETTIVO_QML_INSTALL_DIR));

    // `--icon` asks the sheet for its icon page (docs/design/checklist.md,
    // C-23); the property exists on the sheet's Main alone.
    if (hasFlag(argc, argv, "--icon"))
        engine.setInitialProperties({{QStringLiteral("icon"), true}});

    if (smoke) {
        QObject::connect(&engine, &QQmlApplicationEngine::objectCreated, &app,
                         [](QObject *object, const QUrl &) {
                             if (object != nullptr)
                                 QTimer::singleShot(0, qApp, &QCoreApplication::quit);
                         });
    }

    engine.loadFromModule(QString::fromLatin1(spec.module), QStringLiteral("Main"));
    if (engine.rootObjects().isEmpty()) {
        std::fprintf(stderr, "%s: failed to load %s/Main.qml\n", spec.name, spec.module);
        return 1;
    }
    if (QQuickStyle::name() != QLatin1String(kStyle)) {
        std::fprintf(stderr, "%s: controls resolved to the %s style instead of %s; refusing to run\n",
                     spec.name, qUtf8Printable(QQuickStyle::name()), kStyle);
        return 2;
    }
    auto *window = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
    int result = 0;
    if (!render.isEmpty()) {
        if (window == nullptr) {
            std::fprintf(stderr, "%s: the root object is not a window; nothing to render\n", spec.name);
            return 1;
        }
        renderAndQuit(&app, &engine, window, render, qa->qaPlant, &result);
    }
    // Frame pacing evidence for the drives (fn-20 R4): DETTIVO_QA_PACING
    // names the file the summary lands in.
    PacingCollector::attachFromEnvironment(window, &engine);
    const int code = QGuiApplication::exec();
    return result != 0 ? result : code;
}

}  // namespace dettivo
