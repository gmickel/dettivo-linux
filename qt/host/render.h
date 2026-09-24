// Headless rendering for the visual regression job (fn-20 R1, R5): every
// Qt host writes its window to a PNG on `--render <path>` after the first
// frame, writes every visible string beside it as `<path>.strings.txt`
// (the copy review of the beauty pass reads it), then runs the negative
// style check over the rendered tree. The
// process exits 0 with the file written, 1 when the file cannot be
// written, and 3 when the style check has findings (each printed to
// stderr, prefixed `style-check:`), the file still written so the finding
// can be looked at.
#pragma once

#include <QString>

class QGuiApplication;
class QQmlEngine;
class QQuickWindow;

namespace dettivo {

/// Exit code of a render whose style check found something.
constexpr int kRenderStyleFailure = 3;

/// Arms the render: plants the QA item when DETTIVO_QA_PLANT asks for one,
/// shows the window, grabs it after the first frame, saves it to `target`,
/// runs the style check and quits the application. The result is the exit
/// code written into `*result` when the loop ends.
void renderAndQuit(QGuiApplication *app, QQmlEngine *engine, QQuickWindow *window, const QString &target,
                   const QString &plant, int *result);

}  // namespace dettivo
