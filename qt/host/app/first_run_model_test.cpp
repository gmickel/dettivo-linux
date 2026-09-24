// The first-run model without a daemon or a window (ADR 0024): the
// decision on provisioned and fresh machines, the Keys facts and the live
// press, the Models rows with the download stream and the Enhanced tick,
// the Try it allowance and result, and the sample the renders use.
#include "config_binding.h"
#include "fake_link.h"
#include "first_run_model.h"
#include "first_run_models.h"

#include <QFile>
#include <QJsonArray>
#include <QJsonDocument>
#include <QSignalSpy>
#include <QTest>

using namespace dettivo;
using dettivo::test::FakeLink;

class FirstRunTest : public QObject {
    Q_OBJECT

private slots:
    void completedFirstRunStaysDormantUntilReopened();
    void eligibilityAnswersAreReused();
    void firstRunIsNotRequiredOnAProvisionedMachine();
    void firstRunDecidesKeysAndConfirmsAPress();
    void shortcutActivationWaitsForSuccessAndRetriesUnsourced();
    void firstRunModelsPickWriteAndFollowTheDownload();
    void firstRunTryItArmsTheAllowanceAndReadsTheResult();
};

namespace {

/// The `result` of a contract fixture under crates/dettivo-proto/fixtures,
/// so a fake answer has the daemon's shape and nothing invented.
QJsonObject fixtureResult(const char *relative)
{
    QFile file(QStringLiteral(DETTIVO_CONTRACT_FIXTURES "/") + QLatin1String(relative));
    if (!file.open(QIODevice::ReadOnly))
        qFatal("fixture %s: %s", relative, qPrintable(file.errorString()));
    const QJsonObject doc = QJsonDocument::fromJson(file.readAll()).object();
    const QJsonObject result = doc.value(QStringLiteral("response")).toObject().value(QStringLiteral("result")).toObject();
    if (result.isEmpty())
        qFatal("fixture %s has no response.result", relative);
    return result;
}

QJsonObject modelRow(const char *provider, const char *id, const char *readiness, bool selected, const char *recommended, double size)
{
    QJsonObject row{{QStringLiteral("provider"), QLatin1String(provider)},
                    {QStringLiteral("id"), QLatin1String(id)},
                    {QStringLiteral("display_name"), QLatin1String(id)},
                    {QStringLiteral("kind"), QString(id).contains(QStringLiteral("vad")) ? QStringLiteral("vad") : QStringLiteral("stt")},
                    {QStringLiteral("readiness"), QLatin1String(readiness)},
                    {QStringLiteral("is_selected"), selected},
                    {QStringLiteral("is_default"), true},
                    {QStringLiteral("available"), true},
                    {QStringLiteral("size_bytes"), size},
                    {QStringLiteral("bytes_done"), QString(readiness) == QStringLiteral("ready") ? size : 0.0},
                    {QStringLiteral("bytes_total"), size},
                    {QStringLiteral("languages"), QJsonArray{QStringLiteral("en")}},
                    {QStringLiteral("error"), QJsonValue::Null},
                    {QStringLiteral("path"), QJsonValue::Null},
                    {QStringLiteral("license"), QStringLiteral("MIT")}};
    row.insert(QStringLiteral("recommended_for"), recommended == nullptr ? QJsonValue::Null : QJsonValue(QString::fromUtf8(recommended)));
    return row;
}

/// The answers a fresh Hyprland machine gives: no snippet, nothing bound,
/// one ready model (tiny.en) and the shortlist.
void answerFresh(FakeLink &link, bool sourced, bool bound, bool modelReady)
{
    link.answers.insert(QStringLiteral("system.capabilities"),
                        {{QStringLiteral("platform"), QJsonObject{{QStringLiteral("gpu"), QStringLiteral("vulkan")}, {QStringLiteral("compositor"), QStringLiteral("Hyprland")}}},
                         {QStringLiteral("llm"), QJsonObject{{QStringLiteral("methods"), QJsonArray{QStringLiteral("llm.providers.list")}}}}});
    link.answers.insert(QStringLiteral("speech.models.status"),
                        {{QStringLiteral("models_dir"), QStringLiteral("/home/u/.local/share/dettivo/models")},
                         {QStringLiteral("models"), QJsonArray{modelRow("whisper", "tiny.en", modelReady ? "ready" : "missing", false, nullptr, 77e6),
                                                                modelRow("whisper", "small.en", "missing", false, "English · light enough for CPU-only laptops", 466e6),
                                                                modelRow("parakeet", "parakeet-v3", "missing", true, "25 European languages · word timestamps · fastest on GPU", 940e6),
                                                                modelRow("whisper", "silero-vad", "ready", false, nullptr, 1e6)}}});
    link.answers.insert(QStringLiteral("hotkeys.status"),
                        {{QStringLiteral("backend"), QStringLiteral("none")},
                         {QStringLiteral("bound"), bound ? QJsonArray{QStringLiteral("push_to_talk")} : QJsonArray()},
                         {QStringLiteral("portal"), QJsonObject{{QStringLiteral("available"), false}, {QStringLiteral("reason"), QStringLiteral("no portal")}}},
                         {QStringLiteral("last_press_at"), QJsonValue::Null}});
    link.answers.insert(QStringLiteral("hotkeys.setup"),
                        {{QStringLiteral("compositor"), QStringLiteral("hyprland")},
                         {QStringLiteral("written"), sourced},
                         {QStringLiteral("sourced"), sourced},
                         {QStringLiteral("path"), QStringLiteral("/home/u/.config/hypr/dettivo.conf")},
                         {QStringLiteral("include_line"), QStringLiteral("source = ~/.config/hypr/dettivo.conf")},
                         {QStringLiteral("main_config"), QJsonObject{{QStringLiteral("path"), QStringLiteral("/home/u/.config/hypr/hyprland.conf")}, {QStringLiteral("exists"), true}}}});
    link.answers.insert(QStringLiteral("hotkeys.snippet"),
                        {{QStringLiteral("compositor"), QStringLiteral("hyprland")},
                         {QStringLiteral("text"), QStringLiteral("bind = , F9, exec, dettivo --quiet dictation start\n")},
                         {QStringLiteral("include_line"), QStringLiteral("source = ~/.config/hypr/dettivo.conf")},
                         {QStringLiteral("path"), QStringLiteral("/home/u/.config/hypr/dettivo.conf")},
                         {QStringLiteral("notes"), QJsonArray()}});
    link.answers.insert(QStringLiteral("audio.devices"),
                        {{QStringLiteral("default_source"), QStringLiteral("alsa_input.usb")},
                         {QStringLiteral("pinned"), QString()},
                         {QStringLiteral("pipewire"), true},
                         {QStringLiteral("devices"), QJsonArray{QJsonObject{{QStringLiteral("name"), QStringLiteral("alsa_input.usb")}, {QStringLiteral("description"), QStringLiteral("Arctis Nova")}}}}});
    link.answers.insert(QStringLiteral("speech.selection.get"),
                        {{QStringLiteral("dictation"), QJsonObject{{QStringLiteral("model_id"), QStringLiteral("parakeet-v3")}}}});
    link.answers.insert(QStringLiteral("llm.providers.list"),
                        {{QStringLiteral("providers"), QJsonArray{QJsonObject{{QStringLiteral("id"), QStringLiteral("local")}, {QStringLiteral("available"), false}, {QStringLiteral("detail"), QStringLiteral("the local engine arrives with its spec")}},
                                                               QJsonObject{{QStringLiteral("id"), QStringLiteral("ollama")}, {QStringLiteral("available"), false}, {QStringLiteral("model"), QStringLiteral("qwen3:4b-instruct")}}}}});
    link.answers.insert(QStringLiteral("config.get"),
                        {{QStringLiteral("entries"), QJsonArray{QJsonObject{{QStringLiteral("key"), QStringLiteral("llm.provider")}, {QStringLiteral("value"), QStringLiteral("auto")}, {QStringLiteral("source"), QStringLiteral("default")}}}}});
    link.answers.insert(QStringLiteral("config.path"), fixtureResult("config/path.json"));
    link.answers.insert(QStringLiteral("insert.allow_self_target"), {{QStringLiteral("enabled"), true}, {QStringLiteral("app_ids"), QJsonArray{QStringLiteral("dettivo"), QStringLiteral("dettivo-app")}}});
    link.answers.insert(QStringLiteral("speech.selection.set"), {{QStringLiteral("model"), QStringLiteral("small.en")}});
    link.answers.insert(QStringLiteral("speech.models.download"), modelRow("whisper", "small.en", "downloading", true, "English", 466e6));
    link.answers.insert(QStringLiteral("config.set"), {{QStringLiteral("key"), QStringLiteral("llm.provider")}});
}

}  // namespace

void FirstRunTest::completedFirstRunStaysDormantUntilReopened()
{
    FakeLink link;
    answerFresh(link, true, false, true);
    ConfigBinding config(&link);
    FirstRunModel firstRun(&link, &config);
    firstRun.markComplete();
    firstRun.start();
    link.setConnected(false);
    link.setConnected(true);
    config.applyEntries({});
    QVERIFY2(link.calls.isEmpty(), qPrintable(link.calls.join(", ")));
    firstRun.openAt(QStringLiteral("keys"));
    QVERIFY(link.calls.contains(QStringLiteral("hotkeys.snippet")));
    link.calls.clear();
    link.setConnected(false);
    link.setConnected(true);
    QCOMPARE(link.calls.count(QStringLiteral("hotkeys.status")), 1);
    QCOMPARE(link.calls.count(QStringLiteral("speech.models.status")), 1);
    firstRun.finish();
    link.calls.clear();
    QTest::qWait(1100);
    QVERIFY2(link.calls.isEmpty(), qPrintable(link.calls.join(", ")));
}

void FirstRunTest::eligibilityAnswersAreReused()
{
    FakeLink link;
    answerFresh(link, false, false, true);
    FirstRunModel firstRun(&link, nullptr);
    firstRun.start();
    firstRun.start();
    QVERIFY(firstRun.required());
    for (const QString &method : {QStringLiteral("speech.models.status"), QStringLiteral("hotkeys.status"), QStringLiteral("hotkeys.setup")})
        QCOMPARE(link.calls.count(method), 1);
}

void FirstRunTest::firstRunIsNotRequiredOnAProvisionedMachine()
{
    // A sourced snippet and a ready model: never shown, and the completion
    // is recorded so the check never runs again (ADR 0024).
    FakeLink link;
    answerFresh(link, true, false, true);
    ConfigBinding config(&link);
    FirstRunModel firstRun(&link, &config);
    QSignalSpy completed(&firstRun, &FirstRunModel::completed);
    firstRun.start();
    QVERIFY(firstRun.decided());
    QVERIFY(!firstRun.required());
    QCOMPARE(completed.count(), 1);
    QVERIFY(completed.first().first().toString().endsWith(QLatin1Char('Z')));

    // Bound actions on a daemon backend count like a sourced snippet.
    FakeLink bound;
    answerFresh(bound, false, true, true);
    FirstRunModel viaPortal(&bound, nullptr);
    viaPortal.start();
    QVERIFY(!viaPortal.required());

    // A ready model alone is not enough, nor is a snippet without a model;
    // a ready VAD model never counts as a speech model.
    FakeLink noKeys;
    answerFresh(noKeys, false, false, true);
    FirstRunModel needsKeys(&noKeys, nullptr);
    needsKeys.start();
    QVERIFY(needsKeys.decided() && needsKeys.required());
    FakeLink noModel;
    answerFresh(noModel, true, false, false);
    FirstRunModel needsModel(&noModel, nullptr);
    needsModel.start();
    QVERIFY(needsModel.required());

    // DETTIVO_E2E_COMPLETE and a recorded completion skip the check.
    FakeLink skipped;
    answerFresh(skipped, false, false, false);
    FirstRunModel done(&skipped, nullptr);
    done.markComplete();
    done.start();
    QVERIFY(done.decided() && !done.required());
    QVERIFY(!skipped.calls.contains(QStringLiteral("hotkeys.setup")) || skipped.calls.count(QStringLiteral("hotkeys.setup")) == 1);
}

void FirstRunTest::firstRunDecidesKeysAndConfirmsAPress()
{
    FakeLink link;
    answerFresh(link, false, false, true);
    ConfigBinding config(&link);
    config.applyEntries({QJsonObject{{QStringLiteral("key"), QStringLiteral("hotkeys.hold")}, {QStringLiteral("value"), QStringLiteral("F8")}},
                         QJsonObject{{QStringLiteral("key"), QStringLiteral("hotkeys.toggle")}, {QStringLiteral("value"), QStringLiteral("SUPER CTRL, X")}},
                         QJsonObject{{QStringLiteral("key"), QStringLiteral("hotkeys.cancel")}, {QStringLiteral("value"), QStringLiteral("SUPER CTRL, Escape")}},
                         QJsonObject{{QStringLiteral("key"), QStringLiteral("hotkeys.reinsert")}, {QStringLiteral("value"), QStringLiteral("SUPER CTRL SHIFT, X")}}});
    FirstRunModel firstRun(&link, &config);
    firstRun.start();
    QVERIFY(firstRun.required());
    QCOMPARE(firstRun.step(), QStringLiteral("keys"));
    QCOMPARE(firstRun.compositor(), QStringLiteral("Hyprland"));
    QVERIFY(firstRun.snippetSupported());
    QCOMPARE(firstRun.snippetText(), QStringLiteral("bind = , F9, exec, dettivo --quiet dictation start\n"));
    QCOMPARE(firstRun.includeLine(), QStringLiteral("source = ~/.config/hypr/dettivo.conf"));
    QVERIFY(firstRun.snippetPath().endsWith(QStringLiteral("hypr/dettivo.conf")));
    QVERIFY(!firstRun.snippetWritten());
    const QVariantList bindings = firstRun.bindings();
    QCOMPARE(bindings.size(), 4);
    QCOMPARE(bindings.first().toMap().value(QStringLiteral("keys")).toStringList(), QStringList{QStringLiteral("F8")});
    QCOMPARE(bindings.at(3).toMap().value(QStringLiteral("keys")).toStringList(), (QStringList{QStringLiteral("Super"), QStringLiteral("Ctrl"), QStringLiteral("Shift"), QStringLiteral("X")}));
    QCOMPARE(firstRun.holdKey(), QStringLiteral("F8"));
    // The path is the fixture's `config` field, the one the daemon answers.
    QVERIFY2(firstRun.configPath().endsWith(QStringLiteral("/.config/dettivo/config.toml")), qPrintable(firstRun.configPath()));

    // Continue writes the snippet through hotkeys.setup { write: true }.
    link.answers[QStringLiteral("hotkeys.setup")].insert(QStringLiteral("written"), true);
    firstRun.next();
    QCOMPARE(link.lastParams.value(QStringLiteral("hotkeys.setup")).value(QStringLiteral("write")).toBool(), true);
    QVERIFY(firstRun.snippetWritten());
    QCOMPARE(firstRun.step(), QStringLiteral("models"));
    firstRun.back();
    QCOMPARE(firstRun.step(), QStringLiteral("keys"));

    // A recording transition is the live confirmation on the compositor path.
    QVERIFY(!firstRun.pressed());
    link.notify(QStringLiteral("dictation.state"), {{QStringLiteral("state"), QStringLiteral("recording")}, {QStringLiteral("previous_state"), QStringLiteral("idle")}});
    QVERIFY(firstRun.pressed());
    QCOMPARE(firstRun.pressLine(), QStringLiteral("F8 held · listening · Arctis Nova"));
    firstRun.skip();
    QCOMPARE(firstRun.step(), QStringLiteral("models"));

    // An unknown compositor: no snippet, the portal path is shown.
    FakeLink gnome;
    answerFresh(gnome, false, false, true);
    gnome.answers.remove(QStringLiteral("hotkeys.snippet"));
    gnome.errors.insert(QStringLiteral("hotkeys.snippet"), {{QStringLiteral("message"), QStringLiteral("unknown compositor \"gnome\"; supported: hyprland, sway, niri")}});
    FirstRunModel portal(&gnome, nullptr);
    portal.start();
    QVERIFY(!portal.snippetSupported());
    QVERIFY(portal.portalLine().contains(QStringLiteral("no portal")));
    portal.writeSnippet();
    QCOMPARE(gnome.calls.count(QStringLiteral("hotkeys.setup")), 1);
}

void FirstRunTest::shortcutActivationWaitsForSuccessAndRetriesUnsourced()
{
    FakeLink link;
    answerFresh(link, false, false, true);
    link.answers[QStringLiteral("hotkeys.setup")].insert(QStringLiteral("written"), true);
    link.answers[QStringLiteral("hotkeys.snippet")].insert(QStringLiteral("path"), QStringLiteral("/tmp/hypr/dettivo.lua"));
    FirstRunModel firstRun(&link, nullptr);
    firstRun.start();
    QVERIFY(firstRun.snippetWritten());
    QVERIFY(!firstRun.snippetSourced());
    link.defer = true;
    firstRun.next();
    QCOMPARE(firstRun.step(), QStringLiteral("keys"));
    QCOMPARE(link.lastParams.value(QStringLiteral("hotkeys.setup")).value(QStringLiteral("write")).toBool(), true);
    const auto calls = link.calls.count(QStringLiteral("hotkeys.setup"));
    firstRun.next();
    QCOMPARE(link.calls.count(QStringLiteral("hotkeys.setup")), calls);
    link.errors.insert(QStringLiteral("hotkeys.setup"), {{QStringLiteral("message"), QStringLiteral("Reload failed; retry shortcut setup.")}});
    link.answerPending();
    QCOMPARE(firstRun.step(), QStringLiteral("keys"));
    QVERIFY(firstRun.keysError().contains(QStringLiteral("retry shortcut setup")));
    link.errors.remove(QStringLiteral("hotkeys.setup"));
    link.answers[QStringLiteral("hotkeys.setup")].insert(QStringLiteral("sourced"), true);
    firstRun.next();
    QCOMPARE(firstRun.step(), QStringLiteral("keys"));
    link.answerPending();
    QCOMPARE(firstRun.step(), QStringLiteral("models"));
    QVERIFY(firstRun.keysError().isEmpty());
}

void FirstRunTest::firstRunModelsPickWriteAndFollowTheDownload()
{
    FakeLink link;
    answerFresh(link, false, false, false);
    FirstRunModel firstRun(&link, nullptr);
    firstRun.start();
    auto *models = qobject_cast<FirstRunModels *>(firstRun.models());
    QVERIFY(models);
    // The shortlist: recommended rows and the selected one; no VAD, and
    // tiny.en (neither recommended nor ready nor selected) stays out.
    QCOMPARE(models->rowCount(), 2);
    QCOMPARE(models->rows().first().id, QStringLiteral("small.en"));
    QCOMPARE(models->rows().at(1).id, QStringLiteral("parakeet-v3"));
    QVERIFY(models->rows().at(1).selected && models->rows().at(1).recommended);
    QCOMPARE(models->rows().first().detail, QStringLiteral("English · light enough for CPU-only laptops"));
    QCOMPARE(models->rows().first().size, QStringLiteral("466 MB"));
    QCOMPARE(models->rows().first().state, QStringLiteral("get"));
    QVERIFY(!models->ready());
    QCOMPARE(models->tierLine(), QStringLiteral("Vulkan will run them."));
    QCOMPARE(FirstRunModels::defaultFor(QStringLiteral("vulkan")), QStringLiteral("parakeet/parakeet-v3"));
    QCOMPARE(FirstRunModels::defaultFor(QString()), QStringLiteral("whisper/small.en"));
    QVERIFY(models->modelsDir().endsWith(QStringLiteral("dettivo/models")));

    // A pick writes [speech] and starts the download.
    models->select(QStringLiteral("whisper"), QStringLiteral("small.en"));
    QCOMPARE(link.lastParams.value(QStringLiteral("speech.selection.set")).value(QStringLiteral("model")).toString(), QStringLiteral("small.en"));
    QCOMPARE(link.lastParams.value(QStringLiteral("speech.models.download")).value(QStringLiteral("model")).toString(), QStringLiteral("small.en"));

    // Progress arrives on the stream; done makes the step ready.
    link.answers[QStringLiteral("speech.models.status")] = {{QStringLiteral("models_dir"), QStringLiteral("/x")},
                                                            {QStringLiteral("models"), QJsonArray{modelRow("whisper", "small.en", "downloading", true, "English", 466e6)}}};
    models->refresh();
    link.notify(QStringLiteral("model.download"), {{QStringLiteral("provider"), QStringLiteral("whisper")}, {QStringLiteral("model"), QStringLiteral("small.en")}, {QStringLiteral("state"), QStringLiteral("running")}, {QStringLiteral("bytes_done"), 233e6}, {QStringLiteral("bytes_total"), 466e6}, {QStringLiteral("error"), QJsonValue::Null}});
    QVERIFY(models->downloading());
    QCOMPARE(models->rows().first().state, QStringLiteral("50 %"));
    QCOMPARE(models->downloadFraction(), 0.5);
    QVERIFY(models->downloadLine().startsWith(QStringLiteral("233 of 466 MB")));
    // A failure: the event carries the reason and the daemon's row keeps it.
    QJsonObject failedRow = modelRow("whisper", "small.en", "partial", true, "English", 466e6);
    failedRow.insert(QStringLiteral("error"), QStringLiteral("connection reset"));
    link.answers[QStringLiteral("speech.models.status")] = {{QStringLiteral("models_dir"), QStringLiteral("/x")}, {QStringLiteral("models"), QJsonArray{failedRow}}};
    link.notify(QStringLiteral("model.download"), {{QStringLiteral("provider"), QStringLiteral("whisper")}, {QStringLiteral("model"), QStringLiteral("small.en")}, {QStringLiteral("state"), QStringLiteral("failed")}, {QStringLiteral("bytes_done"), 233e6}, {QStringLiteral("bytes_total"), 466e6}, {QStringLiteral("error"), QStringLiteral("connection reset")}});
    QCOMPARE(models->rows().first().state, QStringLiteral("failed"));
    QCOMPARE(models->rows().first().error, QStringLiteral("connection reset"));
    link.answers[QStringLiteral("speech.models.status")] = {{QStringLiteral("models_dir"), QStringLiteral("/x")},
                                                            {QStringLiteral("models"), QJsonArray{modelRow("whisper", "small.en", "ready", true, "English", 466e6)}}};
    link.notify(QStringLiteral("model.download"), {{QStringLiteral("provider"), QStringLiteral("whisper")}, {QStringLiteral("model"), QStringLiteral("small.en")}, {QStringLiteral("state"), QStringLiteral("done")}, {QStringLiteral("bytes_done"), 466e6}, {QStringLiteral("bytes_total"), 466e6}, {QStringLiteral("error"), QJsonValue::Null}});
    QVERIFY(models->ready());
    QCOMPARE(models->rows().first().state, QStringLiteral("ready"));

    // The Enhanced tick writes [llm] provider; the local download waits
    // for the daemon to declare llm.models.download.
    QCOMPARE(models->enhanced(), QStringLiteral("none"));
    QVERIFY(!models->ollamaAvailable());
    QCOMPARE(models->localState(), QStringLiteral("soon"));
    models->setEnhanced(QStringLiteral("ollama"));
    QCOMPARE(link.lastParams.value(QStringLiteral("config.set")).value(QStringLiteral("value")).toString(), QStringLiteral("ollama"));
    QCOMPARE(models->enhanced(), QStringLiteral("ollama"));
    models->setEnhanced(QStringLiteral("none"));
    QCOMPARE(link.lastParams.value(QStringLiteral("config.set")).value(QStringLiteral("value")).toString(), QStringLiteral("auto"));
    QVERIFY(!link.calls.contains(QStringLiteral("llm.models.download")));
    // The capabilities arrive after the providers: the row is recomputed
    // from what it already knows, without another refresh.
    models->setLlmMethods({QStringLiteral("llm.models.download")});
    QCOMPARE(models->localState(), QStringLiteral("get"));
    // Local: the status names [llm] model, the download asks for that id,
    // and a refusal stays on the row.
    link.answers.insert(QStringLiteral("llm.models.status"),
                        {{QStringLiteral("selected"), QStringLiteral("qwen3-4b-instruct-2507")},
                         {QStringLiteral("models"), QJsonArray{QJsonObject{{QStringLiteral("id"), QStringLiteral("qwen3-4b-instruct-2507")}, {QStringLiteral("provider"), QStringLiteral("llm")}, {QStringLiteral("readiness"), QStringLiteral("missing")}}}}});
    link.errors.insert(QStringLiteral("llm.models.download"), {{QStringLiteral("message"), QStringLiteral("no space left on device")}});
    models->setEnhanced(QStringLiteral("local"));
    QVERIFY(link.calls.contains(QStringLiteral("llm.models.download")));
    QCOMPARE(link.lastParams.value(QStringLiteral("llm.models.download")).value(QStringLiteral("model")).toString(), QStringLiteral("qwen3-4b-instruct-2507"));
    QCOMPARE(models->localState(), QStringLiteral("failed"));
    QCOMPARE(models->localLine(), QStringLiteral("no space left on device"));
    // Accepted: the row follows the download's events to ready.
    link.errors.remove(QStringLiteral("llm.models.download"));
    link.answers.insert(QStringLiteral("llm.models.download"), {{QStringLiteral("id"), QStringLiteral("qwen3-4b-instruct-2507")}});
    models->setEnhanced(QStringLiteral("local"));
    QCOMPARE(models->localState(), QStringLiteral("0 %"));
    link.notify(QStringLiteral("model.download"), {{QStringLiteral("provider"), QStringLiteral("llm")}, {QStringLiteral("model"), QStringLiteral("qwen3-4b-instruct-2507")}, {QStringLiteral("state"), QStringLiteral("running")}, {QStringLiteral("bytes_done"), 1.25e9}, {QStringLiteral("bytes_total"), 2.5e9}, {QStringLiteral("error"), QJsonValue::Null}});
    QCOMPARE(models->localState(), QStringLiteral("50 %"));
    link.notify(QStringLiteral("model.download"), {{QStringLiteral("provider"), QStringLiteral("llm")}, {QStringLiteral("model"), QStringLiteral("qwen3-4b-instruct-2507")}, {QStringLiteral("state"), QStringLiteral("done")}, {QStringLiteral("bytes_done"), 2.5e9}, {QStringLiteral("bytes_total"), 2.5e9}, {QStringLiteral("error"), QJsonValue::Null}});
    QCOMPARE(models->localState(), QStringLiteral("ready"));
}

void FirstRunTest::firstRunTryItArmsTheAllowanceAndReadsTheResult()
{
    FakeLink link;
    answerFresh(link, false, false, true);
    FirstRunModel firstRun(&link, nullptr);
    firstRun.openAt(QStringLiteral("try"));
    firstRun.start();
    QCOMPARE(firstRun.step(), QStringLiteral("try"));
    QCOMPARE(firstRun.engineLine(), QStringLiteral("parakeet-v3"));
    QVERIFY(firstRun.micAvailable());
    firstRun.setAllowance(true);
    QVERIFY(firstRun.allowanceArmed());
    QCOMPARE(link.lastParams.value(QStringLiteral("insert.allow_self_target")).value(QStringLiteral("enabled")).toBool(), true);
    link.notify(QStringLiteral("dictation.state"), {{QStringLiteral("state"), QStringLiteral("recording")}, {QStringLiteral("previous_state"), QStringLiteral("idle")}});
    QCOMPARE(firstRun.dictationState(), QStringLiteral("recording"));
    link.notify(QStringLiteral("dictation.state"),
                {{QStringLiteral("state"), QStringLiteral("idle")},
                 {QStringLiteral("previous_state"), QStringLiteral("inserting")},
                 {QStringLiteral("mode"), QStringLiteral("raw")},
                 {QStringLiteral("insertion"), QJsonObject{{QStringLiteral("outcome"), QStringLiteral("inserted")}, {QStringLiteral("backend"), QStringLiteral("virtual_keyboard")}}},
                 {QStringLiteral("timings"), QJsonObject{{QStringLiteral("capture_ms"), 120}, {QStringLiteral("transcribe_ms"), 540}, {QStringLiteral("insert_ms"), 140}}}});
    QVERIFY(firstRun.resultKnown());
    QCOMPARE(firstRun.insertedVia(), QStringLiteral("virtual keyboard"));
    QCOMPARE(firstRun.resultOutcome(), QStringLiteral("inserted"));
    QCOMPARE(firstRun.stopToInsert(), QStringLiteral("0.8 s"));
    QCOMPARE(firstRun.modeLine(), QStringLiteral("Raw"));
    link.notify(QStringLiteral("dictation.state"),
                {{QStringLiteral("state"), QStringLiteral("idle")},
                 {QStringLiteral("previous_state"), QStringLiteral("inserting")},
                 {QStringLiteral("insertion"), QJsonObject{{QStringLiteral("outcome"), QStringLiteral("failed")}, {QStringLiteral("reason"), QStringLiteral("target_is_self")}}},
                 {QStringLiteral("timings"), QJsonObject{{QStringLiteral("capture_ms"), 0}, {QStringLiteral("transcribe_ms"), 500}, {QStringLiteral("insert_ms"), 0}}}});
    QCOMPARE(firstRun.insertedVia(), QStringLiteral("not inserted"));
    QCOMPARE(firstRun.resultOutcome(), QStringLiteral("failed"));
    QCOMPARE(firstRun.resultReason(), QStringLiteral("target_is_self"));

    // Done disarms the allowance, records the completion and leaves.
    QSignalSpy completed(&firstRun, &FirstRunModel::completed);
    firstRun.finish();
    QCOMPARE(link.lastParams.value(QStringLiteral("insert.allow_self_target")).value(QStringLiteral("enabled")).toBool(), false);
    QCOMPARE(completed.count(), 1);
    QVERIFY(firstRun.decided() && !firstRun.required());

    // No microphone: the step says so and Done still works.
    FakeLink silent;
    answerFresh(silent, false, false, true);
    silent.answers[QStringLiteral("audio.devices")] = {{QStringLiteral("default_source"), QJsonValue::Null}, {QStringLiteral("pinned"), QString()}, {QStringLiteral("pipewire"), true}, {QStringLiteral("devices"), QJsonArray()}};
    FirstRunModel quiet(&silent, nullptr);
    quiet.openAt(QStringLiteral("try"));
    quiet.start();
    QVERIFY(!quiet.micAvailable());
    QVERIFY(quiet.micLine().contains(QStringLiteral("No microphone")));

    // The sample carries the baselines' facts for the renders.
    FirstRunModel sample(nullptr, nullptr);
    sample.applySample(QStringLiteral("models"));
    QCOMPARE(sample.step(), QStringLiteral("models"));
    auto *rows = qobject_cast<FirstRunModels *>(sample.models());
    QCOMPARE(rows->rowCount(), 3);
    QCOMPARE(rows->rows().first().state, QStringLiteral("62 %"));
    QVERIFY(rows->downloading());
    QCOMPARE(rows->enhanced(), QStringLiteral("local"));
    QCOMPARE(sample.pressLine(), QStringLiteral("F9 held · listening · Arctis Nova"));
}

QTEST_GUILESS_MAIN(FirstRunTest)
#include "first_run_model_test.moc"
