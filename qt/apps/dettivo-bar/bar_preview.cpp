#include "bar_preview.h"

#include "qa_environment.h"

namespace dettivo {

BarPreview::BarPreview(QObject *parent)
    : QObject(parent)
    , m_surface(QStringLiteral("panel"))
    , m_state(QStringLiteral("idle"))
{
    QString error;
    const auto qa = QaEnvironment::fromProcess(&error);
    if (!qa.has_value() || qa->e2eBarState.isEmpty())
        return;
    // The host refused an unknown value before the engine existed, so the
    // split below always finds a surface and a state.
    const qsizetype dash = qa->e2eBarState.indexOf(QLatin1Char('-'));
    m_surface = qa->e2eBarState.left(dash);
    m_state = qa->e2eBarState.mid(dash + 1);
}

}  // namespace dettivo
