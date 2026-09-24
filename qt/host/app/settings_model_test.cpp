// The settings routes' host pieces without a daemon or a window (fn-26,
// ADR 0033): the key table names every section's keys once and every
// schema key has an editor or a reason, the editor reads values with
// their sources through the fake link, writes through config.set and
// config.unset, keeps the daemon's refusal per key, locks a row an
// environment variable set, renders the TOML block, refreshes on
// config.changed; the models table orders the rows and follows a
// download; the hotkeys panel reads the snippet and the check; the agents
// hosts and the doctor report run through a fake command runner.
#include "agents_model.h"
#include "cli_runner.h"
#include "config_binding.h"
#include "doctor_model.h"
#include "fake_link.h"
#include "hotkeys_setup_model.h"
#include "models_table.h"
#include "sample_data.h"
#include "settings_keys.h"
#include "settings_model.h"
#include "status_model.h"

#include <QCoreApplication>
#include <QJsonArray>
#include <QJsonDocument>
#include <QSignalSpy>
#include <QTemporaryDir>
#include <QTest>

using namespace dettivo;
using dettivo::test::FakeLink;

namespace {

/// A command runner with canned answers per first argument after the flags.
class FakeRunner : public CliRunner {
public:
    void run(const QStringList &args, Done done) override
    {
        calls.append(args.join(QLatin1Char(' ')));
        const QString joined = args.join(QLatin1Char(' '));
        for (auto it = answers.cbegin(); it != answers.cend(); ++it) {
            if (joined.contains(it.key())) {
                done(it.value().first, it.value().second, QString());
                return;
            }
        }
        done(1, QString(), QStringLiteral("no canned answer"));
    }
    QString program() const override { return QStringLiteral("/usr/bin/dettivo"); }

    QStringList calls;
    QHash<QString, QPair<int, QString>> answers;
};

QJsonObject registryAnswer()
{
    return {{QStringLiteral("keys"),
             QJsonArray{QJsonObject{{QStringLiteral("key"), QStringLiteral("hotkeys.hold")},
                                    {QStringLiteral("section"), QStringLiteral("hotkeys")},
                                    {QStringLiteral("kind"), QStringLiteral("text")},
                                    {QStringLiteral("default"), QStringLiteral("F9")},
                                    {QStringLiteral("doc"), QStringLiteral("Hold to talk: press starts, release stops.")}},
                        QJsonObject{{QStringLiteral("key"), QStringLiteral("qa.mode")},
                                    {QStringLiteral("section"), QStringLiteral("qa")},
                                    {QStringLiteral("kind"), QStringLiteral("boolean")},
                                    {QStringLiteral("default"), false},
                                    {QStringLiteral("doc"), QStringLiteral("QA mode.")}}}}};
}

}  // namespace

class SettingsModelTest : public QObject {
    Q_OBJECT

private slots:
    void keyTableCoversEverySectionOnce();
    void editorReadsWritesAndKeepsTheRefusal();
    void environmentLocksARowAndTheBlockReadsLikeTheFile();
    void aFileChangeRefreshesAndNotices();
    void modelsTableOrdersRowsAndFollowsADownload();
    void warmModelIdentityDoesNotMatchAShorterVariant();
    void diarizationBackendReportsCudaAndCpuFallback();
    void configurationChangesRefreshTheVisibleHealthState();
    void dictationFailureIsVisibleAndClearsOnRetry();
    void agentSummaryUsesLiveFactsAndKeepsConfiguredHostsOnRefresh();
    void hotkeysPanelReadsSnippetAndCheck();
    void agentsHostsRunTheCommandLine();
    void doctorRunsAndCopies();
    void sampleCarriesTheArtboards();
};

void SettingsModelTest::keyTableCoversEverySectionOnce()
{
    QCOMPARE(settings::sections().size(), 9);
    QStringList seen;
    for (const QString &section : settings::sections()) {
        const QStringList keys = settings::keysFor(section);
        QVERIFY2(!keys.isEmpty(), qPrintable(section));
        for (const QString &key : keys) {
            QVERIFY2(!seen.contains(section + key), qPrintable(section + QLatin1Char(' ') + key));
            seen.append(section + key);
            QVERIFY2(settings::reasonNotEditable(key).isEmpty(), qPrintable(key));
        }
    }
    QVERIFY(settings::editableKeys().contains(QStringLiteral("engines.whisper.backend")));
    QVERIFY(!settings::reasonNotEditable(QStringLiteral("polish.rules")).isEmpty());
    QCOMPARE(settings::tableOf(QStringLiteral("engines.whisper.backend")), QStringLiteral("engines.whisper"));
    QCOMPARE(settings::leafOf(QStringLiteral("engines.whisper.backend")), QStringLiteral("backend"));
    QCOMPARE(settings::environmentVariable(QStringLiteral("ipc.socket")), QStringLiteral("DETTIVO_IPC_SOCKET"));
    QVERIFY(settings::environmentVariable(QStringLiteral("hotkeys.hold")).isEmpty());
}

void SettingsModelTest::editorReadsWritesAndKeepsTheRefusal()
{
    FakeLink link;
    link.answers.insert(QStringLiteral("config.keys"), registryAnswer());
    link.answers.insert(QStringLiteral("config.path"), {{QStringLiteral("config"), QStringLiteral("/home/u/.config/dettivo/config.toml")}});
    link.answers.insert(QStringLiteral("config.get"),
                        {{QStringLiteral("entries"), QJsonArray{QJsonObject{{QStringLiteral("key"), QStringLiteral("hotkeys.hold")},
                                                                            {QStringLiteral("value"), QStringLiteral("F9")},
                                                                            {QStringLiteral("source"), QStringLiteral("default")}}}}});
    ConfigBinding config(&link);
    SettingsModel model(&link, &config);
    model.start();
    config.refresh();
    QVERIFY(model.registryLoaded());
    QCOMPARE(model.configPath(), QStringLiteral("/home/u/.config/dettivo/config.toml"));
    QCOMPARE(model.text(QStringLiteral("hotkeys.hold")), QStringLiteral("F9"));
    QCOMPARE(model.source(QStringLiteral("hotkeys.hold")), QStringLiteral("default"));
    QCOMPARE(model.doc(QStringLiteral("hotkeys.hold")), QStringLiteral("Hold to talk: press starts, release stops."));
    QCOMPARE(model.kind(QStringLiteral("hotkeys.hold")), QStringLiteral("text"));
    QCOMPARE(model.defaultText(QStringLiteral("hotkeys.hold")), QStringLiteral("\"F9\""));
    QCOMPARE(model.defaultText(QStringLiteral("qa.mode")), QStringLiteral("false"));

    link.answers.insert(QStringLiteral("config.set"), {});
    model.set(QStringLiteral("hotkeys.hold"), QStringLiteral("F10"));
    QCOMPARE(link.calls.last(), QStringLiteral("config.get"));
    QCOMPARE(link.lastParams.value(QStringLiteral("config.set")).value(QStringLiteral("value")).toString(), QStringLiteral("F10"));

    QSignalSpy errors(&model, &SettingsModel::errorChanged);
    link.errors.insert(QStringLiteral("config.set"), {{QStringLiteral("message"), QStringLiteral("hotkeys.hold: expected a string, got 5")}});
    model.set(QStringLiteral("hotkeys.hold"), 5);
    QCOMPARE(errors.size(), 1);
    QCOMPARE(model.error(QStringLiteral("hotkeys.hold")), QStringLiteral("hotkeys.hold: expected a string, got 5"));
    link.errors.remove(QStringLiteral("config.set"));
    model.set(QStringLiteral("hotkeys.hold"), QStringLiteral("F9"));
    QVERIFY(model.error(QStringLiteral("hotkeys.hold")).isEmpty());
    QCOMPARE(errors.size(), 2);

    link.answers.insert(QStringLiteral("config.unset"), {});
    model.unset(QStringLiteral("hotkeys.hold"));
    QCOMPARE(link.lastParams.value(QStringLiteral("config.unset")).value(QStringLiteral("key")).toString(), QStringLiteral("hotkeys.hold"));
    QCOMPARE(link.calls.last(), QStringLiteral("config.get"));
    link.errors.insert(QStringLiteral("config.unset"), {{QStringLiteral("message"), QStringLiteral("reset refused")}});
    model.unset(QStringLiteral("hotkeys.hold"));
    QCOMPARE(model.error(QStringLiteral("hotkeys.hold")), QStringLiteral("reset refused"));
    SettingsModel offline(nullptr, nullptr);
    offline.unset(QStringLiteral("paths.data_dir"));
    QVERIFY(!offline.error(QStringLiteral("paths.data_dir")).isEmpty());
}

void SettingsModelTest::environmentLocksARowAndTheBlockReadsLikeTheFile()
{
    FakeLink link;
    ConfigBinding config(&link);
    SettingsModel model(&link, &config);
    config.applyEntries({QJsonObject{{QStringLiteral("key"), QStringLiteral("ipc.socket")},
                                     {QStringLiteral("value"), QStringLiteral("/run/user/7/dettivo/dettivo.sock")},
                                     {QStringLiteral("source"), QStringLiteral("environment")}},
                         QJsonObject{{QStringLiteral("key"), QStringLiteral("ipc.auth_mode")},
                                     {QStringLiteral("value"), QStringLiteral("peer")},
                                     {QStringLiteral("source"), QStringLiteral("default")}},
                         QJsonObject{{QStringLiteral("key"), QStringLiteral("mcp.hardened")}, {QStringLiteral("value"), true}, {QStringLiteral("source"), QStringLiteral("file")}},
                         QJsonObject{{QStringLiteral("key"), QStringLiteral("insert.paste_keys")},
                                     {QStringLiteral("value"), QJsonObject{{QStringLiteral("foot"), QStringLiteral("ctrl+shift+v")}}},
                                     {QStringLiteral("source"), QStringLiteral("file")}},
                         QJsonObject{{QStringLiteral("key"), QStringLiteral("insert.terminal_app_ids")},
                                     {QStringLiteral("value"), QJsonArray{QStringLiteral("foot"), QStringLiteral("kitty")}},
                                     {QStringLiteral("source"), QStringLiteral("file")}}});
    QCOMPARE(model.lockedBy(QStringLiteral("ipc.socket")), QStringLiteral("DETTIVO_IPC_SOCKET"));
    QVERIFY(model.lockedBy(QStringLiteral("ipc.auth_mode")).isEmpty());
    QCOMPARE(model.text(QStringLiteral("insert.paste_keys")), QStringLiteral("foot=ctrl+shift+v"));
    QCOMPARE(model.text(QStringLiteral("insert.terminal_app_ids")), QStringLiteral("foot, kitty"));
    QCOMPARE(SettingsModel::tomlValue(QVariantList{QStringLiteral("a"), QStringLiteral("b")}), QStringLiteral("[\"a\", \"b\"]"));
    QCOMPARE(SettingsModel::tomlValue(QVariantMap{{QStringLiteral("foot"), QStringLiteral("ctrl+v")}}), QStringLiteral("{ \"foot\" = \"ctrl+v\" }"));
    QCOMPARE(SettingsModel::tomlValue(0.01), QStringLiteral("0.01"));
    const QString block = model.writesBlock(QStringLiteral("agents"));
    QVERIFY2(block.startsWith(QStringLiteral("[ipc]")), qPrintable(block));
    QVERIFY2(block.contains(QStringLiteral("auth_mode = \"peer\"    socket = \"/run/user/7/dettivo/dettivo.sock\"")), qPrintable(block));
    QVERIFY2(block.contains(QStringLiteral("\n[mcp]")), qPrintable(block));
    QVERIFY2(block.contains(QStringLiteral("hardened = true")), qPrintable(block));
    QVERIFY2(block.contains(QStringLiteral("max_line_bytes = …")), qPrintable(block));
    QCOMPARE(model.keysFor(QStringLiteral("diagnostics")), QStringList{QStringLiteral("qa.mode")});
}

void SettingsModelTest::aFileChangeRefreshesAndNotices()
{
    FakeLink link;
    link.answers.insert(QStringLiteral("config.get"),
                        {{QStringLiteral("entries"), QJsonArray{QJsonObject{{QStringLiteral("key"), QStringLiteral("hotkeys.hold")},
                                                                            {QStringLiteral("value"), QStringLiteral("F10")},
                                                                            {QStringLiteral("source"), QStringLiteral("file")}}}}});
    ConfigBinding config(&link);
    SettingsModel model(&link, &config);
    QSignalSpy changed(&model, &SettingsModel::fileChanged);
    link.notify(QStringLiteral("config.changed"),
                {{QStringLiteral("path"), QStringLiteral("/c.toml")}, {QStringLiteral("ok"), true}, {QStringLiteral("keys"), QJsonArray{QStringLiteral("hotkeys.hold")}}});
    QCOMPARE(changed.size(), 1);
    QCOMPARE(model.text(QStringLiteral("hotkeys.hold")), QStringLiteral("F10"));
    QVERIFY2(model.notice().contains(QStringLiteral("refreshed")), qPrintable(model.notice()));
    link.notify(QStringLiteral("config.changed"), {{QStringLiteral("path"), QStringLiteral("/c.toml")}, {QStringLiteral("ok"), false}, {QStringLiteral("keys"), QJsonArray()}});
    QVERIFY2(model.notice().contains(QStringLiteral("does not parse")), qPrintable(model.notice()));
    QVERIFY(model.notice().contains(QStringLiteral("last valid settings")));
}

void SettingsModelTest::configurationChangesRefreshTheVisibleHealthState()
{
    FakeLink link;
    ConfigBinding config(&link);
    StatusModel status(&link, &config);
    link.answers.insert(QStringLiteral("system.health"), {{QStringLiteral("ok"), true}});
    status.start();
    QVERIFY(status.healthOk());
    link.answers.insert(QStringLiteral("system.health"), {{QStringLiteral("ok"), false}});
    link.notify(QStringLiteral("config.changed"), {{QStringLiteral("ok"), false}});
    QVERIFY(!status.healthOk());
    link.answers.insert(QStringLiteral("system.health"), {{QStringLiteral("ok"), true}});
    link.notify(QStringLiteral("config.changed"), {{QStringLiteral("ok"), true}});
    QVERIFY(status.healthOk());
}

void SettingsModelTest::dictationFailureIsVisibleAndClearsOnRetry()
{
    FakeLink link;
    StatusModel status(&link, nullptr);
    link.errors.insert(QStringLiteral("dictation.start"), {{QStringLiteral("message"), QStringLiteral("Selected model is not downloaded.")}});
    status.startDictation();
    QCOMPARE(status.dictationError(), QStringLiteral("Selected model is not downloaded."));
    link.errors.clear();
    status.startDictation();
    QVERIFY(status.dictationError().isEmpty());
    link.notify(QStringLiteral("dictation.state"), {{QStringLiteral("state"), QStringLiteral("failed")}, {QStringLiteral("reason"), QStringLiteral("Microphone disconnected.")}});
    QCOMPARE(status.dictationError(), QStringLiteral("Microphone disconnected."));
    link.notify(QStringLiteral("dictation.state"), {{QStringLiteral("state"), QStringLiteral("recording")}});
    QVERIFY(status.dictationError().isEmpty());
}

void SettingsModelTest::agentSummaryUsesLiveFactsAndKeepsConfiguredHostsOnRefresh()
{
    FakeLink link;
    StatusModel status(&link, nullptr);
    QVERIFY(status.mcpHosts().isEmpty());
    link.answers.insert(QStringLiteral("system.version"), {{QStringLiteral("api_version"), QStringLiteral("1.0.0")}, {QStringLiteral("app_version"), QStringLiteral("0.1.0")}});
    link.answers.insert(QStringLiteral("system.capabilities"), {{QStringLiteral("rest"), QJsonObject{{QStringLiteral("enabled"), true}, {QStringLiteral("port"), 41731}}}});
    status.start();
    QCOMPARE(status.version(), QStringLiteral("1.0.0"));
    QVERIFY(status.restState().contains(QStringLiteral("41731")));
    status.setMcpHosts({});
    QCOMPARE(status.mcpHosts(), QStringLiteral("none configured"));
    status.setMcpHosts({QStringLiteral("codex")});
    status.refresh();
    QCOMPARE(status.mcpHosts(), QStringLiteral("codex"));
    link.answers.insert(QStringLiteral("system.capabilities"), {{QStringLiteral("rest"), QJsonObject{{QStringLiteral("enabled"), false}}}});
    status.refresh();
    QCOMPARE(status.restState(), QStringLiteral("off"));
}

void SettingsModelTest::warmModelIdentityDoesNotMatchAShorterVariant()
{
    ModelsTable table(nullptr);
    QJsonArray models;
    for (const QString &id : {QStringLiteral("tiny"), QStringLiteral("tiny.en")}) {
        models.append(QJsonObject{{QStringLiteral("provider"), QStringLiteral("whisper")},
                                  {QStringLiteral("id"), id},
                                  {QStringLiteral("readiness"), QStringLiteral("ready")},
                                  {QStringLiteral("path"), QStringLiteral("/models/whisper/%1/ggml-%1.bin").arg(id)}});
    }
    table.applySpeech({{QStringLiteral("models"), models}});
    table.applyEngines({QJsonObject{{QStringLiteral("binary"), QStringLiteral("dettivo-engine-whisper")},
                                    {QStringLiteral("running"), true},
                                    {QStringLiteral("model"), QStringLiteral("/models/whisper/tiny.en/ggml-tiny.en.bin")},
                                    {QStringLiteral("backend"), QStringLiteral("cpu")}}});
    QVERIFY(!table.rows()[0].warm);
    QVERIFY(table.rows()[1].warm);
    QCOMPARE(table.rows()[0].state, QStringLiteral("idle"));
}

void SettingsModelTest::diarizationBackendReportsCudaAndCpuFallback()
{
    ModelsTable table(nullptr);
    table.applySpeech({{QStringLiteral("models"), QJsonArray{QJsonObject{
        {QStringLiteral("provider"), QStringLiteral("diarize")},
        {QStringLiteral("id"), QStringLiteral("diarization")},
        {QStringLiteral("readiness"), QStringLiteral("ready")}}}}});
    for (const QString &backend : {QStringLiteral("cuda"), QStringLiteral("cpu")}) {
        table.applyEngines({QJsonObject{
            {QStringLiteral("binary"), QStringLiteral("dettivo-engine-diarize")},
            {QStringLiteral("backend"), backend}}});
        QCOMPARE(table.rows().size(), 1);
        QCOMPARE(table.rows()[0].backend, backend.toUpper());
    }
}

void SettingsModelTest::modelsTableOrdersRowsAndFollowsADownload()
{
    FakeLink link;
    ModelsTable table(&link);
    auto model = [](const char *provider, const char *id, const char *readiness, bool selected, double total, double done) {
        return QJsonObject{{QStringLiteral("provider"), QLatin1String(provider)},
                           {QStringLiteral("id"), QLatin1String(id)},
                           {QStringLiteral("display_name"), QLatin1String(id)},
                           {QStringLiteral("readiness"), QLatin1String(readiness)},
                           {QStringLiteral("is_selected"), selected},
                           {QStringLiteral("available"), true},
                           {QStringLiteral("kind"), QLatin1String(provider) == QLatin1String("llm") ? QStringLiteral("llm") : QStringLiteral("stt")},
                           {QStringLiteral("languages"), QJsonArray{QStringLiteral("en")}},
                           {QStringLiteral("license"), QStringLiteral("MIT")},
                           {QStringLiteral("size_bytes"), total},
                           {QStringLiteral("bytes_total"), total},
                           {QStringLiteral("bytes_done"), done},
                           {QStringLiteral("error"), QJsonValue::Null}};
    };
    table.applySpeech({{QStringLiteral("models_dir"), QStringLiteral("/home/u/.local/share/dettivo/models")},
                       {QStringLiteral("models"), QJsonArray{model("whisper", "tiny.en", "missing", false, 7.7e7, 0),
                                                             model("parakeet", "parakeet-v3", "ready", true, 6.4e8, 6.4e8)}}});
    table.applyLlm({{QStringLiteral("models"), QJsonArray{model("llm", "qwen3-1.7b", "downloading", true, 1.1e9, 5.5e8)}}});
    table.applyEngines({QJsonObject{{QStringLiteral("binary"), QStringLiteral("dettivo-engine-parakeet")},
                                    {QStringLiteral("running"), true},
                                    {QStringLiteral("model"), QStringLiteral("/m/parakeet/parakeet-v3/model.bin")},
                                    {QStringLiteral("backend"), QStringLiteral("vulkan")}}});
    QCOMPARE(table.rowCount(), 3);
    const QList<ModelsTable::Row> rows = table.rows();
    QCOMPARE(rows[0].id, QStringLiteral("parakeet-v3"));
    QCOMPARE(rows[0].state, QStringLiteral("warm"));
    QCOMPARE(rows[0].backend, QStringLiteral("Vulkan"));
    QCOMPARE(rows[1].id, QStringLiteral("tiny.en"));
    QCOMPARE(rows[1].state, QStringLiteral("not downloaded"));
    QCOMPARE(rows[1].size, QStringLiteral("77 MB"));
    QCOMPARE(rows[2].id, QStringLiteral("qwen3-1.7b"));
    QCOMPARE(rows[2].state, QStringLiteral("50 %"));
    QCOMPARE(rows[2].size, QStringLiteral("1.1 GB"));

    QCOMPARE(table.speechChoices().size(), 2);
    QCOMPARE(table.llmChoices().size(), 1);
    auto sideload = model("llm", "experiment", "ready", false, 1, 1);
    sideload.insert(QStringLiteral("source"), QStringLiteral("sideload"));
    table.applyLlm({{QStringLiteral("models"), QJsonArray{sideload}}});
    QVERIFY(table.llmChoices().isEmpty());
    table.applyLlm({{QStringLiteral("models"), QJsonArray{model("llm", "qwen3-1.7b", "downloading", true, 1.1e9, 5.5e8)}}});
    QVERIFY(table.headline().contains(QStringLiteral("~/.local/share/dettivo/models")) || table.headline().contains(QStringLiteral("models")));

    link.notify(QStringLiteral("model.download"),
                {{QStringLiteral("provider"), QStringLiteral("llm")}, {QStringLiteral("model"), QStringLiteral("qwen3-1.7b")},
                 {QStringLiteral("state"), QStringLiteral("running")}, {QStringLiteral("bytes_done"), 8.25e8}, {QStringLiteral("bytes_total"), 1.1e9}});
    QCOMPARE(table.rows()[2].state, QStringLiteral("75 %"));

    table.download(QStringLiteral("whisper"), QStringLiteral("tiny.en"));
    QCOMPARE(link.calls.last(), QStringLiteral("speech.models.download"));
    table.download(QStringLiteral("llm"), QStringLiteral("qwen3-1.7b"));
    QCOMPARE(link.calls.last(), QStringLiteral("llm.models.download"));
    QVERIFY(!link.lastParams.value(QStringLiteral("llm.models.download")).contains(QStringLiteral("provider")));
    table.remove(QStringLiteral("whisper"), QStringLiteral("tiny.en"));
    QCOMPARE(link.calls.last(), QStringLiteral("speech.models.delete"));
    table.select(QStringLiteral("whisper"), QStringLiteral("tiny.en"));
    QCOMPARE(link.calls.last(), QStringLiteral("speech.selection.set"));

    // qt-hosts/F13 (fn-43): a refused action keeps its reason through
    // the refresh that follows it, until the next attempt or a success.
    QSignalSpy refused(&table, &ModelsTable::refused);
    link.answers.insert(QStringLiteral("speech.models.status"), QJsonObject{{QStringLiteral("models"), QJsonArray()}});
    link.answers.insert(QStringLiteral("llm.models.status"), QJsonObject{{QStringLiteral("models"), QJsonArray()}});
    link.errors.insert(QStringLiteral("speech.models.download"), {{QStringLiteral("message"), QStringLiteral("no space left")}});
    table.download(QStringLiteral("whisper"), QStringLiteral("tiny.en"));
    QCOMPARE(table.lastRefusal(), QStringLiteral("no space left"));
    QCOMPARE(refused.count(), 1);
    table.refresh();
    QCOMPARE(table.lastRefusal(), QStringLiteral("no space left"));
    link.errors.clear();
    link.answers.insert(QStringLiteral("speech.models.download"), QJsonObject{});
    table.download(QStringLiteral("whisper"), QStringLiteral("tiny.en"));
    QVERIFY2(table.lastRefusal().isEmpty(), qPrintable(table.lastRefusal()));
}

void SettingsModelTest::hotkeysPanelReadsSnippetAndCheck()
{
    FakeLink link;
    ConfigBinding config(&link);
    link.answers.insert(QStringLiteral("hotkeys.snippet"),
                        {{QStringLiteral("compositor"), QStringLiteral("hyprland")},
                         {QStringLiteral("text"), QStringLiteral("bind = , F9, exec, dettivo dictation start\n")},
                         {QStringLiteral("path"), QStringLiteral("/home/u/.config/hypr/dettivo.conf")}});
    link.answers.insert(QStringLiteral("hotkeys.setup"),
                        {{QStringLiteral("written"), true},
                         {QStringLiteral("sourced"), false},
                         {QStringLiteral("include_line"), QStringLiteral("source = ~/.config/hypr/dettivo.conf")},
                         {QStringLiteral("main_config"), QJsonObject{{QStringLiteral("path"), QStringLiteral("/home/u/.config/hypr/hyprland.conf")}}},
                         {QStringLiteral("path"), QStringLiteral("/home/u/.config/hypr/dettivo.conf")}});
    link.answers.insert(QStringLiteral("hotkeys.status"), {{QStringLiteral("backend"), QStringLiteral("none")}});
    HotkeysSetupModel panel(&link, &config);
    panel.start();
    QVERIFY(panel.supported());
    QCOMPARE(panel.compositor(), QStringLiteral("hyprland"));
    QVERIFY(panel.snippetText().startsWith(QStringLiteral("bind = , F9")));
    QVERIFY(panel.written());
    QVERIFY(!panel.sourced());
    QVERIFY2(panel.statusLine().contains(QStringLiteral("source = ~/.config/hypr/dettivo.conf")), qPrintable(panel.statusLine()));
    panel.rewrite();
    QCOMPARE(link.lastParams.value(QStringLiteral("hotkeys.setup")).value(QStringLiteral("write")).toBool(), true);
    QCOMPARE(panel.outcome(), QStringLiteral("Snippet written."));
    link.errors.insert(QStringLiteral("hotkeys.snippet"), {{QStringLiteral("message"), QStringLiteral("no snippet for gnome")}});
    panel.refresh();
    QVERIFY(!panel.supported());
    QVERIFY(panel.statusLine().contains(QStringLiteral("portal")));
}

void SettingsModelTest::agentsHostsRunTheCommandLine()
{
    QTemporaryDir dir;
    const QString claude = dir.filePath(QStringLiteral(".mcp.json"));
    QFile file(claude);
    QVERIFY(file.open(QIODevice::WriteOnly));
    file.write("{\"mcpServers\": {\"dettivo\": {\"command\": \"dettivo\"}}}");
    file.close();
    FakeRunner runner;
    auto answer = [&](const QString &host, const QString &path) {
        runner.answers.insert(QStringLiteral("--host ") + host,
                              {0, QString::fromUtf8(QJsonDocument(QJsonObject{{QStringLiteral("host"), host},
                                                                             {QStringLiteral("name"), QStringLiteral("dettivo")},
                                                                             {QStringLiteral("path"), path},
                                                                             {QStringLiteral("text"), QStringLiteral("claude mcp add dettivo")}})
                                                        .toJson())});
    };
    answer(QStringLiteral("claude-code"), claude);
    answer(QStringLiteral("codex"), dir.filePath(QStringLiteral("config.toml")));
    answer(QStringLiteral("cursor"), dir.filePath(QStringLiteral("mcp.json")));
    answer(QStringLiteral("claude-desktop"), dir.filePath(QStringLiteral("claude.json")));
    runner.answers.insert(QStringLiteral("mcp check"), {0, QStringLiteral("{\"tools\": 19, \"resource_templates\": 5}")});
    AgentsModel agents(&runner);
    QCOMPARE(agents.hosts()[0].toMap().value(QStringLiteral("state")).toString(), QStringLiteral("not checked"));
    agents.refresh();
    QVERIFY(!agents.busy());
    const QVariantList hosts = agents.hosts();
    QCOMPARE(hosts.size(), 4);
    QCOMPARE(hosts[0].toMap().value(QStringLiteral("name")).toString(), QStringLiteral("Claude Code"));
    QCOMPARE(hosts[0].toMap().value(QStringLiteral("configured")).toBool(), true);
    QCOMPARE(hosts[1].toMap().value(QStringLiteral("configured")).toBool(), false);
    QCOMPARE(agents.entryText(), QStringLiteral("claude mcp add dettivo"));
    QVERIFY(agents.toolsLine().startsWith(QStringLiteral("19 tools · 5 resource templates")));
    QVERIFY(AgentsModel::holdsEntry(QStringLiteral("codex"), QStringLiteral("[mcp_servers.dettivo]\ncommand = \"dettivo\"\n"), QStringLiteral("dettivo")));
    QVERIFY(!AgentsModel::holdsEntry(QStringLiteral("cursor"), QStringLiteral("{}"), QStringLiteral("dettivo")));
    agents.writeHost(QStringLiteral("cursor"));
    QVERIFY2(runner.calls.contains(QStringLiteral("--json mcp config --host cursor --write")), qPrintable(runner.calls.join(QStringLiteral(" | "))));
}

void SettingsModelTest::doctorRunsAndCopies()
{
    FakeRunner runner;
    runner.answers.insert(QStringLiteral("doctor"), {1, QStringLiteral("socket    /run/s.sock (present)\nconfig    INVALID\n")});
    DoctorModel doctor(&runner);
    QCOMPARE(doctor.verdict(), QStringLiteral("Not run yet."));
    doctor.run();
    QVERIFY(!doctor.running());
    QVERIFY(!doctor.healthy());
    QCOMPARE(doctor.lines().size(), 2);
    QVERIFY(doctor.verdict().contains(QStringLiteral("problems")));
    QCOMPARE(doctor.command(), QStringLiteral("dettivo doctor"));
    doctor.applyReport(0, QStringLiteral("all good"));
    QVERIFY(doctor.healthy());
    QCOMPARE(doctor.verdict(), QStringLiteral("Everything answers."));
}

void SettingsModelTest::sampleCarriesTheArtboards()
{
    FakeLink link;
    ConfigBinding config(&link);
    SettingsModel settings(&link, &config);
    ModelsTable models(&link);
    HotkeysSetupModel hotkeys(&link, &config);
    AgentsModel agents(nullptr);
    DoctorModel doctor(nullptr);
    sample::applySettings(&config, &settings, &models, &hotkeys, &agents, &doctor);
    // Every editable key has a sample value, so a render shows no blank row.
    for (const QString &key : settings::editableKeys())
        QVERIFY2(settings.value(key).isValid(), qPrintable(key));
    QCOMPARE(settings.text(QStringLiteral("speech.model")), QStringLiteral("parakeet-v3"));
    QCOMPARE(settings.lockedBy(QStringLiteral("qa.mode")), QStringLiteral("DETTIVO_QA"));
    QCOMPARE(models.rowCount(), 6);
    QCOMPARE(models.rows()[0].state, QStringLiteral("warm"));
    QVERIFY(hotkeys.sourced());
    QCOMPARE(agents.hosts().size(), 4);
    QVERIFY(doctor.healthy());
    QVERIFY(doctor.lines().size() > 10);
}

QTEST_GUILESS_MAIN(SettingsModelTest)
#include "settings_model_test.moc"
