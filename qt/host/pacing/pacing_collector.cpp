#include "pacing_collector.h"

#include <chrono>

#include <QColor>
#include <QCoreApplication>
#include <QDateTime>
#include <QJsonDocument>
#include <QQmlEngine>
#include <QQuickWindow>
#include <QSaveFile>
#include <QScreen>
#include <QTimer>

#include <algorithm>
#include <numeric>

namespace dettivo {

qint64 PacingCollector::nowNs()
{
    return std::chrono::duration_cast<std::chrono::nanoseconds>(
               std::chrono::steady_clock::now().time_since_epoch())
        .count();
}

qint64 PacingCollector::elapsedNs() const
{
    return nowNs() - m_startNs.load();
}

PacingCollector::PacingCollector(QQuickWindow *window, int seconds, QObject *parent, bool retainFrames)
    : QObject(parent), m_window(window), m_seconds(seconds), m_retainFrames(retainFrames)
{
}

void PacingCollector::noteSwap(double atMs)
{
    QJsonObject applied;
    {
        std::lock_guard<std::mutex> guard(m_lock);
        if (m_retainFrames)
            m_swapsMs.append(atMs);
        if (m_synchronizedThemeChange >= 0) {
            applied = m_themeChanges.at(m_synchronizedThemeChange).toObject();
            applied.insert(QStringLiteral("frame_after_ms"), atMs - applied.value(QStringLiteral("at_ms")).toDouble());
            m_themeChanges[m_synchronizedThemeChange] = applied;
            if (m_pendingThemeChange == m_synchronizedThemeChange)
                m_pendingThemeChange = -1;
            m_synchronizedThemeChange = -1;
        }
    }
    if (!applied.isEmpty())
        emit themeApplied(applied);
}

void PacingCollector::start()
{
    // A steady-clock origin held in an atomic: the render-thread handlers
    // below read it without touching any state the GUI thread mutates.
    m_startNs.store(nowNs());
    m_startUnixMs = double(QDateTime::currentMSecsSinceEpoch());
    connect(m_window, &QQuickWindow::beforeSynchronizing, this, [this]() {
        std::lock_guard<std::mutex> guard(m_lock);
        m_synchronizedThemeChange = m_pendingThemeChange;
    }, Qt::DirectConnection);
    // frameSwapped is emitted on the render thread right after the swap;
    // the timestamp is taken there so GUI-thread scheduling never shows up
    // as a swap interval.
    connect(m_window, &QQuickWindow::frameSwapped, this,
            [this]() { noteSwap(double(elapsedNs()) / 1e6); }, Qt::DirectConnection);
    // beforeRendering and afterRendering run on the render thread; the
    // direct connection keeps the timing there and a lock guards the list.
    connect(m_window, &QQuickWindow::beforeRendering, this,
            [this]() { m_renderStartNs.store(elapsedNs()); }, Qt::DirectConnection);
    connect(m_window, &QQuickWindow::afterRendering, this, [this]() {
        if (!m_retainFrames)
            return;
        const qint64 start = m_renderStartNs.load();
        if (start == 0)
            return;
        const double ms = double(elapsedNs() - start) / 1e6;
        std::lock_guard<std::mutex> guard(m_lock);
        m_rendersMs.append(ms);
    }, Qt::DirectConnection);
    if (m_seconds > 0)
        QTimer::singleShot(m_seconds * 1000, this, [this]() { emit finished(summary()); });
}

QJsonObject PacingCollector::summary() const
{
    QVector<double> renders;
    QVector<double> swaps;
    QJsonArray changes;
    {
        std::lock_guard<std::mutex> guard(m_lock);
        renders = m_rendersMs;
        swaps = m_swapsMs;
        changes = m_themeChanges;
    }
    const double hz = m_window->screen() != nullptr ? m_window->screen()->refreshRate() : 60.0;
    const double seconds = m_seconds > 0 ? double(m_seconds) : double(elapsedNs()) / 1e9;
    QJsonObject out = summarise(swaps, renders, hz, seconds);
    out.insert(QStringLiteral("surface"), QCoreApplication::applicationName());
    out.insert(QStringLiteral("started_unix_ms"), m_startUnixMs);
    out.insert(QStringLiteral("theme_changes"), changes);
    // Every swap timestamp, so a drive can judge the phases in which the
    // surface animated and leave its idle stretches alone.
    QJsonArray swapList;
    for (double ms : swaps)
        swapList.append(ms);
    out.insert(QStringLiteral("swaps_ms"), swapList);
    return out;
}

bool PacingCollector::writeTo(const QString &path) const
{
    QSaveFile file(path);
    if (!file.open(QIODevice::WriteOnly))
        return false;
    file.write(QJsonDocument(summary()).toJson(QJsonDocument::Indented));
    return file.commit();
}

void PacingCollector::watchTheme(QQmlEngine *engine)
{
    QObject *theme = engine != nullptr
        ? engine->singletonInstance<QObject *>(QStringLiteral("Dettivo"), QStringLiteral("ThemeBackend"))
        : nullptr;
    if (theme == nullptr)
        return;
    connect(theme, SIGNAL(themeChanged()), this, SLOT(onThemeChanged()));
}

void PacingCollector::onThemeChanged()
{
    auto *theme = sender();
    if (theme == nullptr)
        return;
    const QJsonObject change{
        {QStringLiteral("at_ms"), double(elapsedNs()) / 1e6},
        {QStringLiteral("applied_unix_ms"), double(QDateTime::currentMSecsSinceEpoch())},
        {QStringLiteral("source"), theme->property("source").toString()},
        {QStringLiteral("theme_dir"), theme->property("themeDir").toString()},
        {QStringLiteral("background"), theme->property("colorBackground").value<QColor>().name()},
        {QStringLiteral("accent"), theme->property("colorAccent").value<QColor>().name()},
    };
    std::lock_guard<std::mutex> guard(m_lock);
    if (!m_retainFrames) {
        QJsonArray active;
        if (m_synchronizedThemeChange >= 0) {
            active.append(m_themeChanges.at(m_synchronizedThemeChange));
            m_synchronizedThemeChange = 0;
        }
        m_themeChanges = active;
    }
    m_themeChanges.append(change);
    m_pendingThemeChange = int(m_themeChanges.size()) - 1;
    // A window that is not on screen swaps nothing; the change is still
    // recorded, with no frame after it.
    m_window->update();
}

PacingCollector *PacingCollector::attachFromEnvironment(QQuickWindow *window, QQmlEngine *engine)
{
    const QString path = QString::fromLocal8Bit(qgetenv("DETTIVO_QA_PACING"));
    if (path.isEmpty() || window == nullptr)
        return nullptr;
    auto *collector = new PacingCollector(window, 0, window);
    collector->watchTheme(engine);
    collector->start();
    auto *flush = new QTimer(collector);
    connect(flush, &QTimer::timeout, collector, [collector, path]() { collector->writeTo(path); });
    flush->start(1000);
    connect(qApp, &QCoreApplication::aboutToQuit, collector, [collector, path]() { collector->writeTo(path); });
    return collector;
}

QJsonObject PacingCollector::summarise(const QVector<double> &swapsMs, const QVector<double> &rendersMs,
                                       double refreshHz, double seconds)
{
    const double period = refreshHz > 0 ? 1000.0 / refreshHz : 1000.0 / 60.0;
    QJsonArray dropped;
    double maxInterval = 0;
    for (int i = 1; i < swapsMs.size(); ++i) {
        const double interval = swapsMs.at(i) - swapsMs.at(i - 1);
        maxInterval = std::max(maxInterval, interval);
        // A vblank went by without a swap: the interval covers two full
        // periods or more. A late frame callback that still swaps inside
        // the next period is jitter, reported through max_interval_ms.
        const int missed = int(std::floor(interval / period)) - 1;
        if (missed >= 1) {
            dropped.append(QJsonObject{{QStringLiteral("frame"), i},
                                       {QStringLiteral("at_ms"), swapsMs.at(i)},
                                       {QStringLiteral("interval_ms"), interval},
                                       {QStringLiteral("missed"), missed}});
        }
    }
    QVector<double> sorted = rendersMs;
    std::sort(sorted.begin(), sorted.end());
    const double maxRender = sorted.isEmpty() ? 0.0 : sorted.last();
    const double meanRender = sorted.isEmpty()
        ? 0.0
        : std::accumulate(sorted.begin(), sorted.end(), 0.0) / double(sorted.size());
    const double p99 = sorted.isEmpty() ? 0.0 : sorted.at(std::min(sorted.size() - 1, qsizetype(double(sorted.size()) * 0.99)));
    return QJsonObject{{QStringLiteral("seconds"), seconds},
                       {QStringLiteral("refresh_hz"), refreshHz},
                       {QStringLiteral("period_ms"), period},
                       {QStringLiteral("frames"), swapsMs.size()},
                       {QStringLiteral("expected_frames"), qRound(refreshHz * seconds)},
                       {QStringLiteral("dropped_frames"), dropped.size()},
                       {QStringLiteral("dropped"), dropped},
                       {QStringLiteral("max_interval_ms"), maxInterval},
                       {QStringLiteral("max_interval_periods"), period > 0 ? maxInterval / period : 0.0},
                       {QStringLiteral("render_max_ms"), maxRender},
                       {QStringLiteral("render_mean_ms"), meanRender},
                       {QStringLiteral("render_p99_ms"), p99},
                       {QStringLiteral("renders"), rendersMs.size()}};
}

}  // namespace dettivo
