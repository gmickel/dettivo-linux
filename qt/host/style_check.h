// The negative style check (fn-20 R5, QA-17): after a surface has rendered,
// every control in its item tree must come from the DettivoStyle module and
// every text must use the theme's font family. A control drawn by the Basic
// or Fusion style, or a text left on Qt's default font, is a finding named
// by item and reason. Under QA mode DETTIVO_QA_PLANT plants one such item so
// the check itself is proven.
#pragma once

#include <QString>
#include <QStringList>

class QQmlEngine;
class QQuickWindow;

namespace dettivo {

/// The font family the Dettivo theme resolved (`Theme.fontFamily`), or an
/// empty string when the module is not loaded.
QString themeFontFamily(QQmlEngine *engine);

/// Every control not from DettivoStyle and every text not on `fontFamily`
/// under the window's content item, one line each: `<type> "<objectName>":
/// <reason>`.
QStringList styleFindings(QQuickWindow *window, const QString &fontFamily);

/// Every visible text in the window's tree, top to bottom in tree order,
/// one entry per item: what the surface says, for the copy review of the
/// beauty pass (docs/design/checklist.md).
QStringList visibleStrings(QQuickWindow *window);

/// Plants the item `DETTIVO_QA_PLANT` names into the window: `basic_control`
/// (a QtQuick.Controls.Basic button), `basic_background` and `basic_content`
/// (a style button with that one delegate drawn outside the style) or
/// `default_font` (a Text on the application font). Returns false with `error` set on an unknown value or
/// a failed component; an unset variable is a no-op that returns true.
bool plantForQa(QQuickWindow *window, QQmlEngine *engine, const QString &plant, QString *error);

}  // namespace dettivo
