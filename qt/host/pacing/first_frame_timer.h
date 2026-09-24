#pragma once

#include <QElapsedTimer>
#include <QObject>
#include <QQuickWindow>

#include <atomic>

namespace dettivo {

class FirstFrameTimer : public QObject {
    Q_OBJECT
public:
    FirstFrameTimer(QQuickWindow *window, QElapsedTimer origin, QObject *parent = nullptr)
        : QObject(parent)
    {
        connect(window, &QQuickWindow::frameSwapped, this, [this, origin]() {
            if (!m_captured.exchange(true))
                emit captured(origin.elapsed());
        }, Qt::DirectConnection);
    }

signals:
    void captured(qint64 elapsedMs);

private:
    std::atomic<bool> m_captured{false};
};

}  // namespace dettivo
