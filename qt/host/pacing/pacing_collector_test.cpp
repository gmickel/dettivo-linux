// The pacing summary names every dropped frame with its timestamp and
// reports the render cost, so the evidence file says exactly where a
// frame was missed rather than only that one was.
#include "pacing_collector.h"
#include "first_frame_timer.h"
#include <thread>

#include <QJsonArray>
#include <QQuickWindow>
#include <QSignalSpy>
#include <QTest>

using dettivo::PacingCollector;

class TestTheme : public QObject {
    Q_OBJECT
signals:
    void themeChanged();
};

class PacingTest : public QObject {
    Q_OBJECT

private slots:
    void firstFrameTimestampExcludesGuiDeliveryDelay()
    {
        QQuickWindow window;
        QElapsedTimer origin;
        origin.start();
        dettivo::FirstFrameTimer timer(&window, origin);
        qint64 observed = -1;
        int deliveries = 0;
        connect(&timer, &dettivo::FirstFrameTimer::captured, this, [&](qint64 at) {
            observed = at;
            ++deliveries;
        });
        std::thread render([&]() {
            emit window.frameSwapped();
            emit window.frameSwapped();
        });
        render.join();
        const qint64 afterSwap = origin.elapsed();
        QTest::qSleep(100);
        QCOMPARE(observed, qint64(-1));
        QTRY_COMPARE(deliveries, 1);
        QVERIFY2(observed <= afterSwap, "timestamp must be captured before GUI delivery");
    }

    void themeMustReachSynchronizationBeforeItsSwap()
    {
        QQuickWindow window;
        PacingCollector collector(&window, 0);
        TestTheme theme;
        QObject::connect(&theme, SIGNAL(themeChanged()), &collector, SLOT(onThemeChanged()));
        collector.start();
        emit window.beforeSynchronizing();
        emit theme.themeChanged();
        emit window.frameSwapped();
        auto changes = collector.summary().value("theme_changes").toArray();
        QVERIFY(!changes.at(0).toObject().contains("frame_after_ms"));
        emit window.beforeSynchronizing();
        emit window.frameSwapped();
        changes = collector.summary().value("theme_changes").toArray();
        QVERIFY(changes.at(0).toObject().contains("frame_after_ms"));
    }

    void consecutiveThemesCreditOnlyTheSynchronizedGeneration()
    {
        QQuickWindow window;
        PacingCollector collector(&window, 0);
        TestTheme theme;
        QObject::connect(&theme, SIGNAL(themeChanged()), &collector, SLOT(onThemeChanged()));
        collector.start();
        emit theme.themeChanged();
        emit window.beforeSynchronizing();
        emit theme.themeChanged();
        emit window.frameSwapped();
        auto changes = collector.summary().value("theme_changes").toArray();
        QVERIFY(changes.at(0).toObject().contains("frame_after_ms"));
        QVERIFY(!changes.at(1).toObject().contains("frame_after_ms"));
        emit window.beforeSynchronizing();
        emit window.frameSwapped();
        changes = collector.summary().value("theme_changes").toArray();
        QVERIFY(changes.at(1).toObject().contains("frame_after_ms"));
        emit theme.themeChanged();
        QVERIFY(!collector.summary().value("theme_changes").toArray().at(2).toObject().contains("frame_after_ms"));
    }

    void journalOnlyCollectorRetainsNoFrameHistory()
    {
        QQuickWindow window;
        PacingCollector collector(&window, 0, nullptr, false);
        TestTheme theme;
        QObject::connect(&theme, SIGNAL(themeChanged()), &collector, SLOT(onThemeChanged()));
        QSignalSpy applied(&collector, &PacingCollector::themeApplied);
        collector.start();
        for (int i = 0; i < 10; ++i) {
            emit theme.themeChanged();
            emit window.beforeSynchronizing();
            emit window.beforeRendering();
            emit window.afterRendering();
            emit window.frameSwapped();
        }
        QCOMPARE(applied.size(), 10);
        const auto summary = collector.summary();
        QCOMPARE(summary.value("frames").toInt(), 0);
        QCOMPARE(summary.value("renders").toInt(), 0);
        QCOMPARE(summary.value("theme_changes").toArray().size(), 1);
    }

    void droppedFramesAreNamedWithTimestamps()
    {
        // 60 Hz: 16.67 ms per frame; one gap of 50 ms drops two frames,
        // while a 31 ms interval (under two periods) is jitter.
        QVector<double> swaps;
        double t = 0;
        for (int i = 0; i < 10; ++i) {
            swaps.append(t);
            t += (i == 4) ? 50.0 : (i == 7 ? 31.0 : 16.67);
        }
        const QVector<double> renders{0.3, 0.4, 0.2, 0.9, 0.35, 0.3, 0.3, 0.3, 0.3, 0.3};
        const QJsonObject s = PacingCollector::summarise(swaps, renders, 60.0, 1);
        QCOMPARE(s.value(QStringLiteral("frames")).toInt(), 10);
        QCOMPARE(s.value(QStringLiteral("dropped_frames")).toInt(), 1);
        const QJsonObject drop = s.value(QStringLiteral("dropped")).toArray().first().toObject();
        QCOMPARE(drop.value(QStringLiteral("frame")).toInt(), 5);
        QVERIFY(qFuzzyCompare(drop.value(QStringLiteral("at_ms")).toDouble(), 16.67 * 4 + 50.0));
        QVERIFY(s.value(QStringLiteral("max_interval_ms")).toDouble() > 49.0);
        QCOMPARE(drop.value(QStringLiteral("missed")).toInt(), 2);
        QCOMPARE(s.value(QStringLiteral("render_max_ms")).toDouble(), 0.9);
        QVERIFY(s.value(QStringLiteral("render_mean_ms")).toDouble() < 0.4);
        QCOMPARE(s.value(QStringLiteral("expected_frames")).toInt(), 60);
        // A fractional refresh rate rounds to the nearest frame count.
        const QJsonObject ntsc = PacingCollector::summarise(swaps, renders, 59.94, 10);
        QCOMPARE(ntsc.value(QStringLiteral("expected_frames")).toInt(), 599);
    }

    void aSteadyRunDropsNothing()
    {
        QVector<double> swaps;
        for (int i = 0; i < 240; ++i)
            swaps.append(i * (1000.0 / 240.0));
        const QJsonObject s = PacingCollector::summarise(swaps, {0.2}, 240.0, 1);
        QCOMPARE(s.value(QStringLiteral("dropped_frames")).toInt(), 0);
        QVERIFY(s.value(QStringLiteral("max_interval_ms")).toDouble() < 5.0);
    }
};

QTEST_MAIN(PacingTest)
#include "pacing_collector_test.moc"
