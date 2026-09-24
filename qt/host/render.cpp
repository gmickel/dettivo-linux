#include "render.h"
#include "style_check.h"

#include <QFile>
#include <QGuiApplication>
#include <QImage>
#include <QQuickWindow>
#include <QTimer>

#include <cstdio>

namespace dettivo {

void renderAndQuit(QGuiApplication *app, QQmlEngine *engine, QQuickWindow *window, const QString &target,
                   const QString &plant, int *result)
{
    QString plantError;
    if (!plantForQa(window, engine, plant, &plantError)) {
        std::fprintf(stderr, "%s: %s\n", qUtf8Printable(QGuiApplication::applicationName()), qUtf8Printable(plantError));
        *result = 2;
        QTimer::singleShot(0, app, &QCoreApplication::quit);
        return;
    }
    const QString fontFamily = themeFontFamily(engine);
    QObject::connect(window, &QQuickWindow::afterRendering, app, [=]() {
        static bool done = false;
        if (done)
            return;
        done = true;
        // The grab and the tree walk run on the GUI thread once the frame
        // has landed; afterRendering itself is on the render thread.
        QTimer::singleShot(0, app, [=]() {
            const QImage image = window->grabWindow();
            *result = image.save(target) ? 0 : 1;
            if (*result != 0) {
                std::fprintf(stderr, "%s: could not write %s\n", qUtf8Printable(QGuiApplication::applicationName()),
                             qUtf8Printable(target));
            } else {
                // The strings beside the picture, for the copy review.
                QFile strings(target + QStringLiteral(".strings.txt"));
                if (strings.open(QIODevice::WriteOnly | QIODevice::Truncate))
                    strings.write((visibleStrings(window).join(QLatin1Char('\n')) + QLatin1Char('\n')).toUtf8());
                const QStringList findings = styleFindings(window, fontFamily);
                for (const QString &finding : findings)
                    std::fprintf(stderr, "style-check: %s\n", qUtf8Printable(finding));
                if (!findings.isEmpty())
                    *result = kRenderStyleFailure;
            }
            app->quit();
        });
    });
    window->show();
}

}  // namespace dettivo
