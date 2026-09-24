// dettivo-osd: the recording pill outside Omarchy (fn-12). Hosts
// Dettivo.Osd on a layer-shell overlay where the compositor offers one, in
// a frameless always-on-top window elsewhere, and exits 0 with a notice
// when the settings or the session rule both out. `--render <png>` writes
// one state headlessly for the visual diff, `--pacing <seconds>` prints
// the frame timing summary, `--smoke` and `--version` are the host
// conventions of ADR 0001.
#include "daemon_client.h"
#include "daemon_paths.h"
#include "osd_control.h"
#include "osd_host.h"
#include "osd_model.h"
#include "osd_settings.h"
#include "pacing_collector.h"
#include "qa_environment.h"
#include "render.h"
#include "theme_backend.h"
#include "unix_signals.h"
#include "wayland_probe.h"

#include <QDBusConnection>
#include <QDBusConnectionInterface>
#include <QGuiApplication>
#include <QJsonDocument>
#include <QQmlApplicationEngine>
#include <QQuickItem>
#include <QQuickStyle>
#include <QQuickWindow>

#include <cstdio>
#include <cstring>

using namespace dettivo;

namespace {

/// The session-bus name the Omarchy panel plugin claims; one pill per session.
constexpr auto kPanelBusName = "dev.dettivo.OmarchyPanel";

struct Args {
    bool version = false;
    bool smoke = false;
    QString render;
    int pacingSeconds = 0;
};

Args parseArgs(int argc, char **argv)
{
    Args a;
    for (int i = 1; i < argc; ++i) {
        if (std::strcmp(argv[i], "--version") == 0)
            a.version = true;
        else if (std::strcmp(argv[i], "--smoke") == 0)
            a.smoke = true;
        else if (std::strcmp(argv[i], "--render") == 0 && i + 1 < argc)
            a.render = QString::fromLocal8Bit(argv[++i]);
        else if (std::strcmp(argv[i], "--pacing") == 0 && i + 1 < argc)
            a.pacingSeconds = std::atoi(argv[++i]);
    }
    return a;
}

int exitDisabled(const QString &socketDir, const QString &notice)
{
    OsdControl::writeNotice(socketDir, QStringLiteral("disabled"), notice);
    std::printf("dettivo-osd: not showing the pill: %s\n", qUtf8Printable(notice));
    return 0;
}

bool panelPluginPresent()
{
    QDBusConnection bus = QDBusConnection::sessionBus();
    if (!bus.isConnected() || bus.interface() == nullptr)
        return false;
    return bus.interface()->isServiceRegistered(QString::fromLatin1(kPanelBusName));
}

}  // namespace

int main(int argc, char **argv)
{
    const Args args = parseArgs(argc, argv);
    if (args.version) {
        std::printf("dettivo-osd %s\n", DETTIVO_VERSION);
        return 0;
    }
    QString qaError;
    const auto qa = QaEnvironment::fromProcess(&qaError);
    if (!qa.has_value()) {
        std::fprintf(stderr, "dettivo-osd: %s\n", qPrintable(qaError));
        return 2;
    }
    const bool headless = args.smoke || !args.render.isEmpty();
    const QProcessEnvironment env = QProcessEnvironment::systemEnvironment();
    QString settingsWarning;
    const OsdSettings settings = OsdSettings::load(OsdSettings::configPath(env), &settingsWarning);
    if (!settingsWarning.isEmpty())
        std::fprintf(stderr, "dettivo-osd: %s\n", qUtf8Printable(settingsWarning));
    const QString socketDir = OsdSettings::socketDir(env);
    if (!settings.enabled && !headless)
        return exitDisabled(socketDir, QStringLiteral("[osd] enabled = false in config.toml"));

    // The probe opens its own Wayland connection, so it runs before Qt
    // decides on a platform and costs one roundtrip.
    QString probeDetail;
    const bool layerShellOffered = headless ? false : waylandHasLayerShell(&probeDetail);

    QGuiApplication app(argc, argv);
    QGuiApplication::setApplicationName(QStringLiteral("dettivo-osd"));
    QGuiApplication::setApplicationVersion(QStringLiteral(DETTIVO_VERSION));
    QGuiApplication::setOrganizationName(QStringLiteral("dettivo"));
    QGuiApplication::setQuitOnLastWindowClosed(false);
    QQuickStyle::setStyle(QStringLiteral("DettivoStyle"));
    QQuickStyle::setFallbackStyle(QStringLiteral("Basic"));

#ifdef DETTIVO_HAVE_LAYER_SHELL
    const bool layerShellBuilt = true;
#else
    const bool layerShellBuilt = false;
#endif
    QString notice;
    OsdHost::Kind kind = headless
        ? OsdHost::Kind::Window
        : OsdHost::decide(settings.host, app.platformName(), layerShellBuilt, layerShellOffered, &notice);
    if (kind == OsdHost::Kind::Disabled)
        return exitDisabled(socketDir, notice);
    if (!headless && qa->e2eOsdState.isEmpty() && panelPluginPresent())
        return exitDisabled(socketDir, QStringLiteral("the Omarchy panel plugin hosts the pill on this session (%1 is on the bus)").arg(QLatin1String(kPanelBusName)));

    DaemonClient client(OsdSettings::daemonSocket(env), paths::ipcToken(env));
    OsdModel model(&client, settings);
    OsdModel::setInstance(&model);

    QQmlApplicationEngine engine;
    engine.addImportPath(QStringLiteral(DETTIVO_QML_INSTALL_DIR));
    engine.loadFromModule(QStringLiteral("DettivoOsd"), QStringLiteral("Main"));
    if (engine.rootObjects().isEmpty()) {
        std::fprintf(stderr, "dettivo-osd: failed to load DettivoOsd/Main.qml\n");
        return 1;
    }
    if (QQuickStyle::name() != QLatin1String("DettivoStyle")) {
        std::fprintf(stderr, "dettivo-osd: controls resolved to the %s style instead of DettivoStyle; refusing to run\n",
                     qUtf8Printable(QQuickStyle::name()));
        return 2;
    }
    if (args.smoke)
        return 0;
    auto *window = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
    auto *pill = window != nullptr ? window->findChild<QQuickItem *>(QStringLiteral("pill")) : nullptr;
    if (window == nullptr || pill == nullptr) {
        std::fprintf(stderr, "dettivo-osd: Main.qml has no window with a pill\n");
        return 1;
    }

    auto *theme = engine.singletonInstance<ThemeBackend *>(QStringLiteral("Dettivo"), QStringLiteral("ThemeBackend"));
    model.setThemeProvider([theme]() { return theme != nullptr ? theme->status() : QJsonObject(); });

    OsdHost host(settings);
    host.attach(kind, window, pill, &model);
    std::fprintf(stderr, "dettivo-osd: host %s (%s), position %s\n", qUtf8Printable(OsdHost::kindName(kind)),
                 qUtf8Printable(probeDetail.isEmpty() ? app.platformName() : probeDetail),
                 qUtf8Printable(settings.position));

    OsdControl control(&model);
    if (args.render.isEmpty()) {
        QString error;
        if (!control.listen(socketDir, &error))
            std::fprintf(stderr, "dettivo-osd: control socket unavailable: %s\n", qUtf8Printable(error));
        quitOnTerminationSignals();
    }

    if (!qa->e2eOsdState.isEmpty()) {
        model.showSample(qa->e2eOsdState);
    } else {
        client.start();
        model.start();
    }

    int result = 0;
    if (!args.render.isEmpty()) {
        if (!model.visible()) {
            std::fprintf(stderr, "dettivo-osd: --render needs DETTIVO_E2E_OSD_STATE naming a visible state\n");
            return 2;
        }
        renderAndQuit(&app, &engine, window, args.render, qa->qaPlant, &result);
    }
    if (args.pacingSeconds > 0) {
        auto *collector = new PacingCollector(window, args.pacingSeconds, &app);
        QObject::connect(collector, &PacingCollector::finished, &app, [&](const QJsonObject &summary) {
            std::printf("%s\n", QJsonDocument(summary).toJson(QJsonDocument::Compact).constData());
            std::fflush(stdout);
            app.quit();
        });
        collector->start();
    }
    PacingCollector::attachFromEnvironment(window, &engine);
    const int code = QGuiApplication::exec();
    return result != 0 ? result : code;
}
