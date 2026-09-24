// The one fact dettivo-bar's Main.qml needs: which surface and state
// DETTIVO_E2E_BAR_STATE asks for ("panel-meeting", "glyph-listening"),
// split into `surface` and `state`. Without the variable the window
// shows the panel in its idle state with the baseline's sample data.
#pragma once

#include <QObject>
#include <QString>
#include <QtQml/qqmlregistration.h>

namespace dettivo {

class BarPreview : public QObject {
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON
    Q_PROPERTY(QString surface READ surface CONSTANT)
    Q_PROPERTY(QString state READ state CONSTANT)

public:
    explicit BarPreview(QObject *parent = nullptr);

    QString surface() const { return m_surface; }
    QString state() const { return m_state; }

private:
    QString m_surface;
    QString m_state;
};

}  // namespace dettivo
