// Qt Quick Test host for the design-system QML tests. Activates the
// DettivoStyle style before any control exists and points the engine at
// the build tree's QML output so `import Dettivo` resolves without an
// install.
#include <QAccessible>
#include <QQmlContext>
#include <QQmlEngine>
#include <QQuickStyle>
#include <QtQuickTest/quicktest.h>

class Setup : public QObject {
    Q_OBJECT

public slots:
    bool accessibleAction(QObject *object, const QString &action)
    {
        auto *interface = QAccessible::queryAccessibleInterface(object);
        auto *actions = interface ? interface->actionInterface() : nullptr;
        const QString name = action == QStringLiteral("toggle")
                                 ? QAccessibleActionInterface::toggleAction()
                                 : QAccessibleActionInterface::pressAction();
        if (!actions || !actions->actionNames().contains(name))
            return false;
        actions->doAction(name);
        return true;
    }

    void applicationAvailable()
    {
        QQuickStyle::setStyle(qEnvironmentVariableIsSet("DETTIVO_TEST_SHELL_HOST")
                                  ? QStringLiteral("Basic") : QStringLiteral("DettivoStyle"));
        QQuickStyle::setFallbackStyle(QStringLiteral("Basic"));
    }

    void qmlEngineAvailable(QQmlEngine *engine)
    {
        if (!qEnvironmentVariableIsSet("DETTIVO_TEST_SHELL_HOST"))
            engine->addImportPath(QStringLiteral(DETTIVO_QML_BUILD_DIR));
        engine->rootContext()->setContextProperty(
            QStringLiteral("accessibilityDriver"), this);
    }
};

QUICK_TEST_MAIN_WITH_SETUP(dettivo, Setup)

#include "quick_test_main.moc"
