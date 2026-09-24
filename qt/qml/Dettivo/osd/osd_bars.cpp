#include "osd_bars.h"

#include <QQuickWindow>
#include <QSGRectangleNode>

#include <algorithm>
#include <cmath>

namespace dettivo {

namespace {

// The bars breathe around the level with a fixed, deterministic pattern
// per bar (no random source, so a render is reproducible for the visual
// diff): each bar has its own phase and rate.
constexpr qreal kDriftHz = 1.7;
constexpr qreal kDriftDepth = 0.35;
// How fast a bar reaches its target: the time constant in seconds.
constexpr qreal kEaseSeconds = 0.055;
constexpr qreal kMinHeightPx = 2.0;

}  // namespace

OsdBars::OsdBars(QQuickItem *parent) : QQuickItem(parent)
{
    setFlag(ItemHasContents, true);
    resetHeights();
    updateImplicitSize();
    m_clock.start();
}

void OsdBars::setLevel(qreal level)
{
    const qreal clamped = std::clamp(level, 0.0, 1.0);
    if (qFuzzyCompare(1.0 + clamped, 1.0 + m_level))
        return;
    m_level = clamped;
    emit levelChanged();
    update();
}

void OsdBars::setBarCount(int count)
{
    count = std::max(1, count);
    if (count == m_barCount)
        return;
    m_barCount = count;
    resetHeights();
    updateImplicitSize();
    emit barCountChanged();
    update();
}

void OsdBars::setColor(const QColor &color)
{
    if (color == m_color)
        return;
    m_color = color;
    emit colorChanged();
    update();
}

void OsdBars::setBarWidth(qreal width)
{
    if (qFuzzyCompare(width, m_barWidth))
        return;
    m_barWidth = width;
    updateImplicitSize();
    emit barWidthChanged();
    update();
}

void OsdBars::setGap(qreal gap)
{
    if (qFuzzyCompare(gap, m_gap))
        return;
    m_gap = gap;
    updateImplicitSize();
    emit gapChanged();
    update();
}

void OsdBars::setFloorLevel(qreal floor)
{
    floor = std::clamp(floor, 0.0, 1.0);
    if (qFuzzyCompare(1.0 + floor, 1.0 + m_floor))
        return;
    m_floor = floor;
    emit floorLevelChanged();
    update();
}

void OsdBars::setLive(bool live)
{
    if (live == m_live)
        return;
    m_live = live;
    emit liveChanged();
    update();
}

void OsdBars::setReducedMotion(bool reduced)
{
    if (reduced == m_reducedMotion)
        return;
    m_reducedMotion = reduced;
    emit reducedMotionChanged();
    update();
}

qreal OsdBars::barHeightAt(int index) const
{
    if (index < 0 || index >= m_heights.size())
        return 0.0;
    return m_heights.at(index);
}

qreal OsdBars::targetAt(int index) const
{
    if (index < 0 || index >= m_barCount)
        return 0.0;
    const qreal seconds = m_reducedMotion ? 0.0 : qreal(m_clock.nsecsElapsed()) / 1e9;
    return m_floor + (1.0 - m_floor) * std::clamp(m_level * shapeAt(index, seconds), 0.0, 1.0);
}

void OsdBars::updateImplicitSize()
{
    setImplicitWidth(m_barCount * m_barWidth + qreal(m_barCount - 1) * m_gap);
}

void OsdBars::resetHeights()
{
    m_heights.fill(m_floor, m_barCount);
    if (m_heights.size() != m_barCount)
        m_heights = QVector<qreal>(m_barCount, m_floor);
}

// A centre-weighted envelope (the baseline's bars are tallest around the
// middle) times a slow per-bar drift, so the bars read as a live signal
// rather than a meter.
qreal OsdBars::shapeAt(int index, qreal seconds) const
{
    const qreal centre = qreal(m_barCount - 1) / 2.0;
    const qreal distance = std::abs(qreal(index) - centre) / std::max(centre, 1.0);
    const qreal envelope = 0.45 + 0.55 * (1.0 - distance * distance);
    if (m_reducedMotion || !m_live)
        return envelope * 1.6;
    const qreal phase = qreal(index) * 1.9;
    const qreal rate = kDriftHz * (0.8 + 0.4 * qreal((index * 7) % 5) / 4.0);
    const qreal drift = 1.0 + kDriftDepth * std::sin(seconds * rate * 2.0 * M_PI + phase);
    return envelope * drift * 1.6;
}

// One rectangle node per bar under a plain parent node. Rectangle nodes
// are geometry nodes the batch renderer merges into a single draw, and the
// software adaptation draws them too, so the offscreen renders the visual
// diff uses show the same bars as the GPU path.
QSGNode *OsdBars::updatePaintNode(QSGNode *old, UpdatePaintNodeData *)
{
    QSGNode *root = old;
    if (root == nullptr)
        root = new QSGNode();
    QQuickWindow *win = window();
    if (win == nullptr)
        return root;

    const qint64 now = m_clock.nsecsElapsed();
    const qreal dt = m_lastFrameNs == 0 ? 0.0 : std::min(qreal(now - m_lastFrameNs) / 1e9, 0.1);
    m_lastFrameNs = now;
    if (m_heights.size() != m_barCount)
        resetHeights();

    // Frozen bars keep their last heights; reduced motion snaps to the
    // static shape; live bars ease toward a drifting target every frame.
    if (m_live) {
        for (int i = 0; i < m_barCount; ++i) {
            const qreal target = targetAt(i);
            if (m_reducedMotion) {
                m_heights[i] = target;
            } else {
                const qreal k = dt <= 0.0 ? 1.0 : 1.0 - std::exp(-dt / kEaseSeconds);
                m_heights[i] += (target - m_heights[i]) * k;
            }
        }
    }

    while (root->childCount() > m_barCount)
        delete root->lastChild();
    while (root->childCount() < m_barCount)
        root->appendChildNode(win->createRectangleNode());

    const qreal h = height();
    QSGNode *child = root->firstChild();
    for (int i = 0; i < m_barCount && child != nullptr; ++i, child = child->nextSibling()) {
        auto *bar = static_cast<QSGRectangleNode *>(child);
        const qreal barHeight = std::max(kMinHeightPx, m_heights.at(i) * h);
        const qreal x = qreal(i) * (m_barWidth + m_gap);
        bar->setRect(QRectF(x, (h - barHeight) / 2.0, m_barWidth, barHeight));
        bar->setColor(m_color);
    }
    m_frames.fetch_add(1);

    // Live bars without reduced motion ask for the next frame here, on the
    // render thread's schedule, so the animation runs at the display rate
    // without a timer on the GUI thread.
    if (m_live && !m_reducedMotion)
        update();
    return root;
}

}  // namespace dettivo
