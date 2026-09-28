#include "render_fallback.h"

#include "daemon_paths.h"

#include <QCoreApplication>
#include <QFile>
#include <QProcessEnvironment>
#include <QQuickWindow>
#include <QSGRendererInterface>

#include <toml++/toml.hpp>

#include <cerrno>
#include <cstdio>
#include <cstring>
#include <string>
#include <vector>

#include <unistd.h>

namespace dettivo::renderer {

namespace {

struct State {
    std::string name = "dettivo";
    std::vector<std::string> argv;
    QString reason = QStringLiteral("default");
    bool software = false;
    bool restarted = false;
    bool failed = false;
};

State &state()
{
    static State s;
    return s;
}

/// The same binary with the same arguments, on the software renderer.
/// exec keeps the process id, so a systemd unit keeps its main process;
/// the instance lock and the sockets are close-on-exec and free again.
void restartInSoftware(const QString &message)
{
    State &s = state();
    qputenv("QT_QUICK_BACKEND", "software");
    qputenv(kFallbackVariable, message.toUtf8());
    std::vector<char *> args;
    for (std::string &arg : s.argv)
        args.push_back(arg.data());
    args.push_back(nullptr);
    std::fflush(nullptr);
    ::execv("/proc/self/exe", args.data());
    std::fprintf(stderr, "%s: cannot restart with the software renderer: %s\n", s.name.c_str(), std::strerror(errno));
    QCoreApplication::exit(1);
}

}  // namespace

QString configured(const QString &toml, QString *warning)
{
    const QString fallback = QStringLiteral("auto");
    toml::table doc;
    try {
        doc = toml::parse(toml.toStdString());
    } catch (const toml::parse_error &) {
        return fallback;
    }
    const auto node = doc["app"]["renderer"];
    if (!node)
        return fallback;
    const auto value = node.value<std::string>();
    if (value && (*value == "auto" || *value == "software"))
        return QString::fromStdString(*value);
    if (warning != nullptr)
        *warning = QStringLiteral("app.renderer: expected auto or software; using auto");
    return fallback;
}

void choose(const char *name, int argc, char **argv)
{
    State &s = state();
    s.name = name;
    s.argv.assign(argv, argv + argc);
    const QByteArray backend = qgetenv("QT_QUICK_BACKEND");
    if (qEnvironmentVariableIsSet(kFallbackVariable)) {
        s.restarted = true;
        s.reason = QStringLiteral("fallback: ") + qEnvironmentVariable(kFallbackVariable);
        // What this process starts is not a restart.
        qunsetenv(kFallbackVariable);
    } else if (!backend.isEmpty()) {
        s.reason = QStringLiteral("environment: QT_QUICK_BACKEND=") + QString::fromUtf8(backend);
    } else {
        QFile file(paths::configFile(QProcessEnvironment::systemEnvironment()));
        QString warning;
        const QString value = file.open(QIODevice::ReadOnly) ? configured(QString::fromUtf8(file.readAll()), &warning)
                                                             : QStringLiteral("auto");
        if (!warning.isEmpty())
            std::fprintf(stderr, "%s: %s\n", name, qUtf8Printable(warning));
        if (value == QStringLiteral("software")) {
            QQuickWindow::setGraphicsApi(QSGRendererInterface::Software);
            s.reason = QStringLiteral("config: [app] renderer = software");
            s.software = true;
        }
    }
    s.software = s.software || backend == "software";
    if (s.reason != QStringLiteral("default"))
        std::fprintf(stderr, "%s: renderer: %s\n", name, qUtf8Printable(s.reason));
}

void guard(QQuickWindow *window)
{
    if (window == nullptr)
        return;
    // A connected handler is what stops Qt from aborting the process.
    QObject::connect(window, &QQuickWindow::sceneGraphError, window,
                     [](QQuickWindow::SceneGraphError, const QString &message) {
                         State &s = state();
                         if (s.failed)
                             return;
                         s.failed = true;
                         if (s.restarted || s.software) {
                             std::fprintf(stderr, "%s: cannot draw the window with the software renderer either: %s\n",
                                          s.name.c_str(), qUtf8Printable(message));
                             QCoreApplication::exit(1);
                             return;
                         }
                         std::fprintf(stderr,
                                      "%s: warning: the GPU could not start the scene graph (%s); restarting with "
                                      "the software renderer\n",
                                      s.name.c_str(), qUtf8Printable(message));
                         restartInSoftware(message);
                     });
}

QString reason()
{
    return state().reason;
}

}  // namespace dettivo::renderer
