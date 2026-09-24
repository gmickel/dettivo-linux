// The recording pill's level bars: one scene-graph item, fed by
// `audio.level` and interpolated per frame on the render thread. No
// canvas, no painted item, no per-bar QML items: the bars are rectangle
// nodes under one parent node that the batch renderer draws in one go,
// which is what keeps the pill under a millisecond a frame (fn-12 R5).
// Exposed to QML as `OsdBars` in the Dettivo module.
#pragma once

#include <QColor>
#include <QElapsedTimer>
#include <QQuickItem>
#include <QVector>

#include <atomic>

namespace dettivo {

class OsdBars : public QQuickItem {
    Q_OBJECT
    QML_ELEMENT

    /// The level the bars follow, 0 to 1 (the `rms` of `audio.level`).
    Q_PROPERTY(qreal level READ level WRITE setLevel NOTIFY levelChanged)
    /// How many bars.
    Q_PROPERTY(int barCount READ barCount WRITE setBarCount NOTIFY barCountChanged)
    /// The bar colour; the caller dims it for the transcribing state.
    Q_PROPERTY(QColor color READ color WRITE setColor NOTIFY colorChanged)
    /// Bar width and gap in pixels (the module's waveform tokens).
    Q_PROPERTY(qreal barWidth READ barWidth WRITE setBarWidth NOTIFY barWidthChanged)
    Q_PROPERTY(qreal gap READ gap WRITE setGap NOTIFY gapChanged)
    /// The height fraction an idle bar keeps, so silence still reads as bars.
    Q_PROPERTY(qreal floorLevel READ floorLevel WRITE setFloorLevel NOTIFY floorLevelChanged)
    /// Live bars move every frame (listening); frozen bars hold their last
    /// heights (transcribing).
    Q_PROPERTY(bool live READ live WRITE setLive NOTIFY liveChanged)
    /// Reduced motion: bars sit at a static height for the level, no drift.
    Q_PROPERTY(bool reducedMotion READ reducedMotion WRITE setReducedMotion NOTIFY reducedMotionChanged)
    /// Frames rendered so far (the pacing evidence and the tests read it).
    Q_PROPERTY(int frames READ frames)

public:
    explicit OsdBars(QQuickItem *parent = nullptr);

    qreal level() const { return m_level; }
    void setLevel(qreal level);
    int barCount() const { return m_barCount; }
    void setBarCount(int count);
    QColor color() const { return m_color; }
    void setColor(const QColor &color);
    qreal barWidth() const { return m_barWidth; }
    void setBarWidth(qreal width);
    qreal gap() const { return m_gap; }
    void setGap(qreal gap);
    qreal floorLevel() const { return m_floor; }
    void setFloorLevel(qreal floor);
    bool live() const { return m_live; }
    void setLive(bool live);
    bool reducedMotion() const { return m_reducedMotion; }
    void setReducedMotion(bool reduced);
    int frames() const { return m_frames.load(); }

    /// The height fraction (0 to 1) bar `index` currently draws.
    Q_INVOKABLE qreal barHeightAt(int index) const;
    /// The height fraction bar `index` is heading for at the current level.
    Q_INVOKABLE qreal targetAt(int index) const;

signals:
    void levelChanged();
    void barCountChanged();
    void colorChanged();
    void barWidthChanged();
    void gapChanged();
    void floorLevelChanged();
    void liveChanged();
    void reducedMotionChanged();

protected:
    QSGNode *updatePaintNode(QSGNode *old, UpdatePaintNodeData *) override;

private:
    void updateImplicitSize();
    void resetHeights();
    qreal shapeAt(int index, qreal seconds) const;

    qreal m_level = 0.0;
    int m_barCount = 14;
    QColor m_color = QColor(Qt::white);
    qreal m_barWidth = 3.0;
    qreal m_gap = 2.0;
    qreal m_floor = 0.12;
    bool m_live = false;
    bool m_reducedMotion = false;
    // Render-side state: the heights the last frame drew and the clock the
    // drift runs on. Only touched while the GUI thread is blocked in
    // updatePaintNode, so no lock is needed.
    QVector<qreal> m_heights;
    QElapsedTimer m_clock;
    qint64 m_lastFrameNs = 0;
    std::atomic<int> m_frames{0};
};

}  // namespace dettivo
