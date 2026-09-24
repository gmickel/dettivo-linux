// Frame pacing evidence for every Qt surface (fn-12 R5, fn-20 R4): every
// swapped frame's timestamp and every render's duration on the render
// thread, summarised as JSON: frames, the refresh rate, dropped frames (a
// swap interval of two refresh periods or more, so a vblank passed without
// a frame) with their timestamps, the render cost per frame, and every
// theme change with the moment the first frame after it swapped.
//
// Two ways to run it: `dettivo-osd --pacing <seconds>` collects for a fixed
// time and prints the summary; `DETTIVO_QA_PACING=<file>` in any Qt host
// collects for the process's life and writes the summary into the file,
// once a second and at exit, so a drive that ends the process still finds
// the evidence.
#pragma once

#include <atomic>
#include <QJsonArray>
#include <QJsonObject>
#include <QObject>
#include <QString>
#include <QVector>

#include <mutex>

class QQmlEngine;
class QQuickWindow;

namespace dettivo {

class PacingCollector : public QObject {
    Q_OBJECT

public:
    /// A collector over `window`; `seconds` of 0 means until the process ends.
    /// `retainFrames=false` emits theme timing without retaining frame history.
    PacingCollector(QQuickWindow *window, int seconds, QObject *parent = nullptr, bool retainFrames = true);

    /// Starts counting; `finished` fires after `seconds` with the summary
    /// when a duration was given.
    void start();

    /// The summary so far.
    QJsonObject summary() const;

    /// Writes the summary to `path` atomically.
    bool writeTo(const QString &path) const;

    /// Records a theme change (the palette that applied, when, and the
    /// delay to the next swapped frame once it lands); `engine` is where
    /// the Dettivo theme singleton lives.
    void watchTheme(QQmlEngine *engine);

    /// When `DETTIVO_QA_PACING` names a file: a collector over `window`
    /// that writes there once a second and when the application quits.
    /// Returns nullptr when the variable is unset.
    static PacingCollector *attachFromEnvironment(QQuickWindow *window, QQmlEngine *engine);

    /// The summary from swap timestamps (ms), render costs (ms), the
    /// refresh rate and the seconds observed, for the tests.
    static QJsonObject summarise(const QVector<double> &swapsMs, const QVector<double> &rendersMs,
                                 double refreshHz, double seconds);

signals:
    void finished(const QJsonObject &summary);
    void themeApplied(const QJsonObject &change);

private slots:
    void onThemeChanged();

private:
    static qint64 nowNs();
    qint64 elapsedNs() const;
    void noteSwap(double atMs);
    QQuickWindow *m_window;
    int m_seconds;
    bool m_retainFrames;
    std::atomic<qint64> m_startNs{0};
    double m_startUnixMs = 0.0;
    mutable std::mutex m_lock;
    QVector<double> m_swapsMs;
    QVector<double> m_rendersMs;
    QJsonArray m_themeChanges;
    int m_pendingThemeChange = -1;
    int m_synchronizedThemeChange = -1;
    std::atomic<qint64> m_renderStartNs{0};
};

}  // namespace dettivo
