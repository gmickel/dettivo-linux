#include "config_binding.h"
#include "fake_link.h"
#include "hotkeys_setup_model.h"
#include "models_table.h"
#include "settings_model.h"

#include <QTest>
#include <QJsonArray>

using namespace dettivo;
using dettivo::test::FakeLink;

class StartupWorkTest : public QObject {
    Q_OBJECT
private slots:
    void hiddenSettingsDoNotReadOnConnect();
    void unobservedModelRuntimeStaysUnknown();
    void unobservedDownloadsAreNotReportedAsIdle();
    void downloadCountUsesValidatedRawCatalogue_data();
    void downloadCountUsesValidatedRawCatalogue();
};

void StartupWorkTest::hiddenSettingsDoNotReadOnConnect()
{
    FakeLink link;
    ConfigBinding config(nullptr);
    SettingsModel settings(&link, &config);
    ModelsTable models(&link);
    QCOMPARE(models.headline(), QStringLiteral("Backend not checked"));
    link.answers["system.capabilities"] = {{"platform", QJsonObject{{"gpu", "vulkan"}}}};
    HotkeysSetupModel hotkeys(&link, &config);
    link.setConnected(false);
    link.setConnected(true);
    QVERIFY2(link.calls.isEmpty(), qPrintable(link.calls.join(", ")));
    settings.start();
    models.start();
    QCOMPARE(models.headline(), QStringLiteral("Vulkan"));
    hotkeys.start();
    QVERIFY(link.calls.contains(QStringLiteral("config.keys")));
    QCOMPARE(link.calls.count(QStringLiteral("speech.models.status")), 1);
    QCOMPARE(link.calls.count(QStringLiteral("hotkeys.status")), 1);
    link.calls.clear();
    link.setConnected(false);
    link.setConnected(true);
    QCOMPARE(link.calls.count(QStringLiteral("speech.models.status")), 1);
    QCOMPARE(link.calls.count(QStringLiteral("hotkeys.status")), 1);
    settings.setActive(false);
    models.setActive(false);
    hotkeys.setActive(false);
    link.calls.clear();
    link.setConnected(false);
    link.setConnected(true);
    config.applyEntries({});
    link.notify(QStringLiteral("engine.state"), {});
    QVERIFY2(link.calls.isEmpty(), qPrintable(link.calls.join(", ")));
    models.setActive(true);
    QCOMPARE(link.calls.count(QStringLiteral("speech.models.status")), 1);
    models.setActive(true);
    QCOMPARE(link.calls.count(QStringLiteral("speech.models.status")), 1);

}

void StartupWorkTest::unobservedDownloadsAreNotReportedAsIdle()
{
    FakeLink link;
    ModelsTable models(&link);
    QCOMPARE(models.activeDownloads(), -1);
    link.defer = true;
    const QJsonObject downloading{{"id", "tiny.en"}, {"provider", "whisper"}, {"readiness", "downloading"}};
    link.answers["speech.models.status"] = {{"models", QJsonArray{downloading}}};
    link.errors["llm.models.status"] = {{"message", "unavailable"}};
    models.ensureDownloadStatus();
    models.ensureDownloadStatus();
    QCOMPARE(link.calls.size(), 2);
    QCOMPARE(models.activeDownloads(), -1);
    link.answerPending();
    QCOMPARE(models.activeDownloads(), -1);
    link.errors.clear();
    link.answers["llm.models.status"] = {{"models", QJsonArray{}}};
    models.ensureDownloadStatus();
    link.answerPending();
    QCOMPARE(models.activeDownloads(), 1);
    models.ensureDownloadStatus();
    QCOMPARE(link.calls.size(), 4);
    link.notify("model.download", {{"provider", "whisper"}, {"model", "tiny.en"}, {"state", "done"}});
    QCOMPARE(models.activeDownloads(), -1);
    QJsonObject ready = downloading;
    ready["readiness"] = "ready";
    link.answers["speech.models.status"] = {{"models", QJsonArray{ready}}};
    models.ensureDownloadStatus();
    link.answerPending();
    QCOMPARE(models.activeDownloads(), 0);
    models.ensureDownloadStatus();
    QCOMPARE(link.calls.size(), 6);
    link.setConnected(false);
    link.setConnected(true);
    QCOMPARE(models.activeDownloads(), -1);
    QCOMPARE(link.calls.size(), 6);
    models.ensureDownloadStatus();
    link.answers.remove("llm.models.status");
    link.answerPending();
    QCOMPARE(models.activeDownloads(), -1);
    for (const auto &malformed : {QJsonObject{}, QJsonObject{{"models", QJsonObject{}}}}) {
        link.setConnected(false);
        link.setConnected(true);
        link.answers["llm.models.status"] = malformed;
        models.ensureDownloadStatus();
        link.answerPending();
        QCOMPARE(models.activeDownloads(), -1);
    }
}

void StartupWorkTest::downloadCountUsesValidatedRawCatalogue_data()
{
    QTest::addColumn<QJsonArray>("rows");
    QTest::addColumn<int>("expected");
    QTest::newRow("non-object") << QJsonArray{QJsonValue::Null} << -1;
    QTest::newRow("missing-readiness") << QJsonArray{QJsonObject{}} << -1;
    QTest::newRow("non-string-readiness") << QJsonArray{QJsonObject{{"readiness", 1}}} << -1;
    QTest::newRow("unknown-readiness") << QJsonArray{QJsonObject{{"readiness", "maybe"}}} << -1;
    QTest::newRow("hidden-downloads") << QJsonArray{
        QJsonObject{{"id", "vad"}, {"kind", "vad"}, {"readiness", "downloading"}},
        QJsonObject{{"id", "unavailable"}, {"available", false}, {"readiness", "downloading"}}} << 2;
    QTest::newRow("mixed-invalid") << QJsonArray{QJsonObject{{"readiness", "downloading"}}, QJsonObject{}} << -1;
    QJsonArray idle;
    for (const auto *state : {"ready", "unverified", "partial", "missing", "quarantined"})
        idle.append(QJsonObject{{"readiness", state}});
    QTest::newRow("known-idle-states") << idle << 0;
}

void StartupWorkTest::downloadCountUsesValidatedRawCatalogue()
{
    QFETCH(QJsonArray, rows);
    QFETCH(int, expected);
    for (bool speech : {false, true}) {
        ModelsTable models(nullptr);
        models.applySpeech({{"models", speech ? rows : QJsonArray{}}});
        models.applyLlm({{"models", speech ? QJsonArray{} : rows}});
        QCOMPARE(models.activeDownloads(), expected);
    }
    ModelsTable combined(nullptr);
    combined.applySpeech({{"models", rows}});
    combined.applyLlm({{"models", rows}});
    QCOMPARE(combined.activeDownloads(), expected < 0 ? -1 : 2 * expected);
    if (expected == 2)
        QCOMPARE(combined.rowCount(), 0);
}

void StartupWorkTest::unobservedModelRuntimeStaysUnknown()
{
    ModelsTable models(nullptr);
    models.applySpeech({{"models", QJsonArray{QJsonObject{{"provider", "whisper"}, {"id", "tiny.en"}, {"readiness", "ready"}}}}});
    QCOMPARE(models.rowCount(), 1);
    QCOMPARE(models.data(models.index(0, 0), ModelsTable::BackendRole).toString(), QStringLiteral("not checked"));
    QCOMPARE(models.data(models.index(0, 0), ModelsTable::StateRole).toString(), QStringLiteral("not checked"));
    models.applyEngines({QJsonObject{{"binary", "dettivo-engine-whisper"}, {"running", false}, {"backend", "cpu"}}});
    QCOMPARE(models.data(models.index(0, 0), ModelsTable::BackendRole).toString(), QStringLiteral("CPU"));
    QCOMPARE(models.data(models.index(0, 0), ModelsTable::StateRole).toString(), QStringLiteral("idle"));
}

QTEST_GUILESS_MAIN(StartupWorkTest)
#include "startup_work_test.moc"
