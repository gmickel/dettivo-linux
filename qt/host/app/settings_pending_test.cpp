#include "config_binding.h"
#include "fake_link.h"
#include "settings_model.h"

#include <QTest>

using namespace dettivo;
using dettivo::test::FakeLink;

class SettingsPendingTest : public QObject {
    Q_OBJECT
private slots:
    void waitsForOwnRefreshAndRecovers_data()
    {
        QTest::addColumn<QString>("key");
        QTest::newRow("vocabulary") << QStringLiteral("dictation.vocabulary");
        QTest::newRow("transforms") << QStringLiteral("polish.transforms");
    }
    void waitsForOwnRefreshAndRecovers()
    {
        QFETCH(QString, key);
        FakeLink link;
        ConfigBinding config(&link);
        SettingsModel model(&link, &config);
        const QJsonArray initial{QJsonObject{{"key", key}, {"value", QJsonArray{"original"}}}};
        const QJsonArray updated{QJsonObject{{"key", key}, {"value", QJsonArray{"original", "added"}}}};
        config.applyEntries(initial);
        link.answers.insert("config.set", {});
        link.answers.insert("config.unset", {});
        link.answers.insert("config.get", {{"entries", updated}});
        link.defer = true;
        model.set(key, QVariantList{"original", "added"});
        QVERIFY(model.pending(key));
        model.set(key, QVariantList{"lost"});
        model.unset(key);
        QCOMPARE(link.calls.size(), 1);
        // Notifications and unrelated snapshots cannot unlock the editor.
        config.applyEntries(initial);
        QVERIFY(model.pending(key));
        link.answerPending();
        QVERIFY(model.pending(key));
        QCOMPARE(link.calls.last(), "config.get");
        link.answerPending();
        QVERIFY(!model.pending(key));
        QCOMPARE(model.value(key).toList(), (QVariantList{"original", "added"}));
        // An unchanged reset snapshot still completes.
        model.unset(key);
        QVERIFY(model.pending(key));
        link.answerPending();
        link.answerPending();
        QVERIFY(!model.pending(key));
        // Refusal releases the lock and leaves the confirmed list intact.
        link.errors.insert("config.set", {{"message", "refused"}});
        model.set(key, QVariantList{"rejected"});
        link.answerPending();
        QVERIFY(!model.pending(key));
        QCOMPARE(model.error(key), "refused");
        QCOMPARE(model.value(key).toList(), (QVariantList{"original", "added"}));
        link.errors.clear();
        link.answers.insert("config.set", {{"key", key}, {"value", QJsonArray{"acknowledged"}}});
        model.set(key, QVariantList{"original", "added"});
        QVERIFY(model.error(key).isEmpty());
        link.answerPending();
        link.errors.insert("config.get", {{"message", "connection closed"}});
        link.answerPending();
        QVERIFY(!model.pending(key));
        QCOMPARE(model.error(key), "connection closed");
        QCOMPARE(model.value(key).toList(), (QVariantList{"acknowledged"}));
        link.errors.clear();
        model.set(key, QVariantList{"original", "added"});
        link.answerPending();
        link.answerPending();
        QVERIFY(!model.pending(key));
        QVERIFY(model.error(key).isEmpty());
        link.setConnected(false);
        model.unset(key);
        QVERIFY(!model.pending(key));
        QVERIFY(!model.error(key).isEmpty());
    }
};

QTEST_GUILESS_MAIN(SettingsPendingTest)
#include "settings_pending_test.moc"
