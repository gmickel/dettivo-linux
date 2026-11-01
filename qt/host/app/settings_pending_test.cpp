#include "config_binding.h"
#include "fake_link.h"
#include "settings_model.h"

#include <QJSEngine>
#include <QJsonArray>
#include <QTest>

using namespace dettivo;
using dettivo::test::FakeLink;

class SettingsPendingTest : public QObject {
    Q_OBJECT
private slots:
    // QML hands a JavaScript array to set() as a QJSValue, which
    // QJsonValue::fromVariant turns into null; the first term of an empty
    // list must still reach config.set as a list.
    void sendsAJavaScriptListAsAList()
    {
        FakeLink link;
        ConfigBinding config(&link);
        SettingsModel model(&link, &config);
        link.answers.insert("config.set", {});
        link.defer = true;
        QJSEngine engine;
        model.set(QStringLiteral("dictation.vocabulary"), QVariant::fromValue(engine.evaluate("[].concat(['SapienXT'])")));
        QCOMPARE(link.lastParams.value("config.set").value("value").toArray(), (QJsonArray{"SapienXT"}));
    }

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

    // The daemon answers a write before the config.changed it raised: the
    // route's own write is no change on disk, a hand edit is, and a hand
    // edit clears a refusal that no longer describes the file.
    void ownWritesAreNotFileChanges()
    {
        const QString vocabulary = QStringLiteral("dictation.vocabulary");
        const auto entry = [](const QString &key, const QJsonValue &value) {
            return QJsonObject{{"key", key}, {"value", value}, {"source", "file"}};
        };
        const auto changed = [](const QString &key) { return QJsonObject{{"ok", true}, {"keys", QJsonArray{key}}}; };
        FakeLink link;
        ConfigBinding config(&link);
        SettingsModel model(&link, &config);
        config.applyEntries({entry(vocabulary, QJsonArray{"Dettivo"}), entry("hotkeys.hold", "F9")});
        const QJsonObject added = entry(vocabulary, QJsonArray{"Dettivo", "Omarchy"});
        link.answers.insert("config.set", added);
        link.answers.insert("config.get", {{"entries", QJsonArray{added, entry("hotkeys.hold", "F9")}}});
        model.set(vocabulary, QVariantList{"Dettivo", "Omarchy"});
        link.notify("config.changed", changed(vocabulary));
        QVERIFY2(model.notice().isEmpty(), qPrintable(model.notice()));

        // A field's write: its refresh may still be in flight when the event lands.
        link.defer = true;
        link.answers.insert("config.set", entry("hotkeys.hold", "F10"));
        link.answers.insert("config.get", {{"entries", QJsonArray{added, entry("hotkeys.hold", "F10")}}});
        model.set("hotkeys.hold", "F10");
        link.answerPending();
        link.notify("config.changed", changed("hotkeys.hold"));
        link.answerPending();
        QVERIFY2(model.notice().isEmpty(), qPrintable(model.notice()));
        link.defer = false;

        link.errors.insert("config.set", {{"message", "\"Omarchy\" has 17 sounds_like spellings"}});
        model.set(vocabulary, QVariantList{"Dettivo", "Omarchy"});
        QVERIFY(!model.error(vocabulary).isEmpty());
        link.answers.insert("config.get", {{"entries", QJsonArray{entry(vocabulary, QJsonArray{"Dettivo"})}}});
        link.notify("config.changed", changed(vocabulary));
        QVERIFY2(model.error(vocabulary).isEmpty(), qPrintable(model.error(vocabulary)));
        QVERIFY2(model.notice().contains("1 value(s) refreshed"), qPrintable(model.notice()));
    }
};

QTEST_GUILESS_MAIN(SettingsPendingTest)
#include "settings_pending_test.moc"
