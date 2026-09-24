#include "style_check.h"

#include <QFont>
#include <QQmlComponent>
#include <QQmlContext>
#include <QQmlEngine>
#include <QQuickItem>
#include <QQuickWindow>
#include <QUrl>

#include <utility>

namespace dettivo {

namespace {

constexpr auto kStyleModule = "/DettivoStyle/";

QString describe(const QQuickItem *item)
{
    const QString type = QString::fromLatin1(item->metaObject()->className()).section(QLatin1Char('_'), 0, 0);
    return QStringLiteral("%1 \"%2\"").arg(type, item->objectName());
}

/// The document an object was created in.
QUrl documentOf(const QObject *object)
{
    const QQmlContext *context = object != nullptr ? QQmlEngine::contextForObject(object) : nullptr;
    return context != nullptr ? context->baseUrl() : QUrl();
}

/// The findings for one control's drawing delegates. Its background and
/// content item are created inside the style's Button.qml (or
/// TextField.qml, ...), so each delegate's document names the style,
/// whichever document instantiated the control itself; a delegate created
/// anywhere else is named on its own, so replacing one of the two never
/// hides behind the other. A control with no delegate document at all is
/// one finding.
QStringList delegateFindings(QQuickItem *control)
{
    QStringList findings;
    bool anyDocument = false;
    for (const char *delegate : {"background", "contentItem"}) {
        const QUrl url = documentOf(control->property(delegate).value<QObject *>());
        if (url.isEmpty())
            continue;
        anyDocument = true;
        if (!url.toString().contains(QLatin1String(kStyleModule))) {
            findings.append(QStringLiteral("%1: %2 from %3, not the Dettivo style")
                                .arg(describe(control), QLatin1String(delegate), url.toString()));
        }
    }
    if (!anyDocument)
        findings.append(QStringLiteral("%1: control from no style document, not the Dettivo style").arg(describe(control)));
    return findings;
}

/// The visual tree under `root`, depth first. The object tree would miss
/// items whose QObject parent is the window rather than its content item.
void collectItems(QQuickItem *root, QList<QQuickItem *> *out)
{
    const QList<QQuickItem *> children = root->childItems();
    for (QQuickItem *child : children) {
        out->append(child);
        collectItems(child, out);
    }
}

}  // namespace

QString themeFontFamily(QQmlEngine *engine)
{
    QObject *theme = engine != nullptr
        ? engine->singletonInstance<QObject *>(QStringLiteral("Dettivo"), QStringLiteral("Theme"))
        : nullptr;
    return theme != nullptr ? theme->property("fontFamily").toString() : QString();
}

QStringList styleFindings(QQuickWindow *window, const QString &fontFamily)
{
    QStringList findings;
    if (window == nullptr || window->contentItem() == nullptr)
        return findings;
    QList<QQuickItem *> items;
    collectItems(window->contentItem(), &items);
    for (QQuickItem *item : std::as_const(items)) {
        if (item->inherits("QQuickControl"))
            findings.append(delegateFindings(item));
        if (item->inherits("QQuickText") || item->inherits("QQuickTextInput") || item->inherits("QQuickTextEdit")) {
            const QString family = item->property("font").value<QFont>().family();
            if (family != fontFamily) {
                findings.append(QStringLiteral("%1: font family \"%2\", the theme's is \"%3\"")
                                    .arg(describe(item), family, fontFamily));
            }
        }
    }
    return findings;
}

QStringList visibleStrings(QQuickWindow *window)
{
    QStringList out;
    if (window == nullptr || window->contentItem() == nullptr)
        return out;
    QList<QQuickItem *> items;
    collectItems(window->contentItem(), &items);
    for (QQuickItem *item : std::as_const(items)) {
        if (!item->isVisible() || item->opacity() <= 0.0)
            continue;
        if (!item->inherits("QQuickText") && !item->inherits("QQuickTextInput") && !item->inherits("QQuickTextEdit"))
            continue;
        const QString text = item->property("text").toString().trimmed();
        if (!text.isEmpty())
            out.append(text);
    }
    return out;
}

bool plantForQa(QQuickWindow *window, QQmlEngine *engine, const QString &plant, QString *error)
{
    if (plant.isEmpty())
        return true;
    QByteArray source;
    if (plant == QStringLiteral("basic_control")) {
        source = "import QtQuick.Controls.Basic as Basic\n"
                 "Basic.Button { objectName: \"qaPlant\"; text: \"plant\" }\n";
    } else if (plant == QStringLiteral("basic_background")) {
        // A style button whose background alone is drawn outside the style.
        source = "import QtQuick\n"
                 "import QtQuick.Controls as Controls\n"
                 "Controls.Button { objectName: \"qaPlant\"; text: \"plant\"; background: Rectangle { color: \"red\" } }\n";
    } else if (plant == QStringLiteral("basic_content")) {
        // A style button whose content item alone is drawn outside the style.
        source = "import QtQuick\n"
                 "import QtQuick.Controls as Controls\n"
                 "Controls.Button { objectName: \"qaPlant\"; text: \"plant\"; contentItem: Text { text: \"plant\" } }\n";
    } else if (plant == QStringLiteral("default_font")) {
        source = "import QtQuick\n"
                 "Text { objectName: \"qaPlant\"; text: \"plant\" }\n";
    } else {
        if (error != nullptr)
            *error = QStringLiteral("DETTIVO_QA_PLANT: expected basic_control, basic_background, basic_content or default_font, got %1").arg(plant);
        return false;
    }
    if (window == nullptr || engine == nullptr) {
        if (error != nullptr)
            *error = QStringLiteral("DETTIVO_QA_PLANT: no window to plant into");
        return false;
    }
    QQmlComponent component(engine);
    component.setData(source, QUrl(QStringLiteral("qa-plant.qml")));
    auto *item = qobject_cast<QQuickItem *>(component.create());
    if (item == nullptr) {
        if (error != nullptr)
            *error = QStringLiteral("DETTIVO_QA_PLANT: %1").arg(component.errorString().trimmed());
        return false;
    }
    item->setParentItem(window->contentItem());
    item->setParent(window->contentItem());
    return true;
}

}  // namespace dettivo
