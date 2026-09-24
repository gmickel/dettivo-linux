#include "sample_data.h"
#include "config_binding.h"
#include "engines_model.h"
#include "history_actions.h"
#include "history_detail_model.h"
#include "history_model.h"
#include "history_player.h"
#include "status_model.h"

#include <QDate>
#include <QDateTime>
#include <QTimeZone>

#include <cstring>

namespace dettivo::sample {

namespace {

QString sampleId(const char *suffix)
{
    return QStringLiteral("5a3p1e00-0000-4000-8000-0000000000") + QLatin1String(suffix);
}

QJsonObject item(const char *suffix, int daysAgo, int hour, int minute, const char *kind, const char *title, double seconds,
                 const char *app = "com.mitchellh.ghostty", const char *mode = "enhanced")
{
    const QDateTime when(referenceDate().addDays(-daysAgo), QTime(hour, minute), QTimeZone::LocalTime);
    QJsonObject row{{QStringLiteral("ref"), QJsonObject{{QStringLiteral("id"), sampleId(suffix)}, {QStringLiteral("kind"), QLatin1String(kind)}}},
                    {QStringLiteral("title"), QLatin1String(title)},
                    {QStringLiteral("started_at"), when.toUTC().toString(Qt::ISODate)},
                    {QStringLiteral("duration_seconds"), seconds},
                    {QStringLiteral("status"), QStringLiteral("completed")}};
    if (std::strcmp(kind, "dictation") == 0) {
        row.insert(QStringLiteral("app_id"), QLatin1String(app));
        row.insert(QStringLiteral("mode"), QLatin1String(mode));
        row.insert(QStringLiteral("source"), QStringLiteral("dictation"));
    }
    return row;
}

QJsonObject engine(const char *binary, bool running, const char *model, const char *backend)
{
    return {{QStringLiteral("binary"), QLatin1String(binary)},
            {QStringLiteral("running"), running},
            {QStringLiteral("model"), model == nullptr ? QJsonValue::Null : QJsonValue(QLatin1String(model))},
            {QStringLiteral("backend"), backend == nullptr ? QJsonValue::Null : QJsonValue(QLatin1String(backend))},
            {QStringLiteral("path"), QStringLiteral("/usr/bin/") + QLatin1String(binary)},
            {QStringLiteral("crashes"), 0},
            {QStringLiteral("degraded"), false},
            {QStringLiteral("reason"), QJsonValue::Null}};
}

QJsonObject model(const char *id, const char *label, bool downloaded)
{
    return {{QStringLiteral("id"), QLatin1String(id)},
            {QStringLiteral("label"), QLatin1String(label)},
            {QStringLiteral("is_downloaded"), downloaded}};
}

}  // namespace

QDate referenceDate()
{
    return QDate(2025, 9, 3);
}

QJsonObject status()
{
    return {{QStringLiteral("hold"), QStringLiteral("F9")},
            {QStringLiteral("toggle"), QStringLiteral("Super+Ctrl+X")},
            {QStringLiteral("target"), QStringLiteral("ghostty")},
            {QStringLiteral("socket_mode"), QStringLiteral("peer")},
            {QStringLiteral("gpu"), QStringLiteral("vulkan")},
            {QStringLiteral("compositor"), QStringLiteral("Hyprland")},
            {QStringLiteral("input"), QStringLiteral("Arctis Nova")},
            {QStringLiteral("rest"), QStringLiteral("off")},
            {QStringLiteral("mcp"), QStringLiteral("claude-code · codex")},
            {QStringLiteral("last_call"), QStringLiteral("get_latest_transcript · 13:13")},
            {QStringLiteral("clock"), QStringLiteral("Wed 3 Sep · 13:42")}};
}

QJsonArray historyItems()
{
    return {item("01", 0, 13, 12, "dictation", "Add a regression test for the merger overlap case before we ship.", 4),
            item("02", 0, 12, 58, "dictation", "Reply to Mara: yes, keep the Vulkan build as the default package.", 6, "org.chromium.Chromium"),
            item("03", 0, 12, 40, "dictation", "Summarise the diarization thresholds in the spec.", 3, "cursor", "raw"),
            item("04", 0, 11, 5, "meeting", "Dettivo Linux kickoff", 41 * 60),
            item("05", 0, 9, 31, "dictation", "Open a spike spec for parakeet.cpp timestamps against the golden alignment.", 8),
            item("06", 0, 9, 2, "dictation", "Morning notes: engine processes, one protocol, Vulkan first, CUDA later as a package.", 14),
            item("07", 1, 17, 22, "dictation", "The merger should prefer the earlier boundary when both chunks agree.", 7, "foot", "deterministic_polish"),
            item("08", 1, 16, 48, "dictation", "Push the branch and open a draft PR with the benchmark table.", 5),
            item("09", 1, 9, 14, "meeting", "Weekly sync with Tobias", 28 * 60),
            item("10", 2, 15, 30, "dictation", "Rename the merger config to LiveMeetingTranscriber before the release.", 4, "cursor")};
}

QString detailId()
{
    return sampleId("01");
}

QJsonObject historyDetail()
{
    const QDateTime when(referenceDate(), QTime(13, 12), QTimeZone::LocalTime);
    const QJsonObject insertion{
        {QStringLiteral("outcome"), QStringLiteral("inserted")},
        {QStringLiteral("method"), QStringLiteral("paste")},
        {QStringLiteral("backend"), QJsonObject{{QStringLiteral("name"), QStringLiteral("virtual_keyboard")},
                                                {QStringLiteral("latency_ms"), 35},
                                                {QStringLiteral("undo_supported"), true}}}};
    const QJsonObject facts{{QStringLiteral("created_at"), when.toUTC().toString(Qt::ISODate)},
                            {QStringLiteral("app_id"), QStringLiteral("com.mitchellh.ghostty")},
                            {QStringLiteral("app_name"), QStringLiteral("Ghostty")},
                            {QStringLiteral("source"), QStringLiteral("dictation")},
                            {QStringLiteral("status"), QStringLiteral("completed")},
                            {QStringLiteral("provider"), QStringLiteral("parakeet")},
                            {QStringLiteral("model"), QStringLiteral("parakeet-v3-int8")},
                            {QStringLiteral("language"), QStringLiteral("en")},
                            {QStringLiteral("duration_seconds"), 4},
                            {QStringLiteral("title"), QStringLiteral("Add a regression test for the merger overlap case")},
                            // `sample` is the take the player already shows (HistoryPlayer::applySample).
                            {QStringLiteral("audio"), QJsonObject{{QStringLiteral("retained"), true}, {QStringLiteral("path"), QStringLiteral("sample")}}},
                            {QStringLiteral("insertion"), insertion},
                            {QStringLiteral("timings"), QJsonObject{{QStringLiteral("capture_ms"), 120},
                                                                    {QStringLiteral("transcribe_ms"), 400},
                                                                    {QStringLiteral("insert_ms"), 35},
                                                                    {QStringLiteral("stop_to_insert_ms"), 900}}}};
    return {{QStringLiteral("ref"), QJsonObject{{QStringLiteral("kind"), QStringLiteral("dictation")}, {QStringLiteral("id"), detailId()}}},
            {QStringLiteral("text_raw"), QStringLiteral("um add a regression test for the the merger overlap case before we ship")},
            {QStringLiteral("text_polish"), QStringLiteral("Add a regression test for the merger overlap case before we ship.")},
            {QStringLiteral("segments"), QJsonArray()},
            {QStringLiteral("mode"), QStringLiteral("enhanced")},
            {QStringLiteral("facts"), facts}};
}

QJsonObject providers()
{
    return {{QStringLiteral("providers"),
             QJsonArray{QJsonObject{{QStringLiteral("id"), QStringLiteral("parakeet")},
                                    {QStringLiteral("display_name"), QStringLiteral("Parakeet")},
                                    {QStringLiteral("supports_meetings"), false},
                                    {QStringLiteral("models"), QJsonArray{model("parakeet-v3-int8", "Parakeet v3 (int8)", true)}}},
                        QJsonObject{{QStringLiteral("id"), QStringLiteral("whisper")},
                                    {QStringLiteral("display_name"), QStringLiteral("Whisper")},
                                    {QStringLiteral("supports_meetings"), true},
                                    {QStringLiteral("models"), QJsonArray{model("large-v3-turbo", "Large v3 Turbo", true),
                                                                          model("small", "Small", true),
                                                                          model("tiny.en", "Tiny (English)", false)}}}}}};
}

QJsonArray engines()
{
    return {engine("dettivo-engine-parakeet", true, "parakeet-v3-int8", "vulkan"),
            engine("dettivo-engine-whisper", false, "ggml-small.bin", nullptr),
            engine("dettivo-engine-llm", false, "qwen3-4b-instruct", nullptr),
            engine("dettivo-engine-diarize", false, nullptr, nullptr)};
}

QJsonObject selection()
{
    return {{QStringLiteral("dictation"), QJsonObject{{QStringLiteral("is_parakeet"), true},
                                                      {QStringLiteral("model_id"), QStringLiteral("parakeet-v3-int8")},
                                                      {QStringLiteral("provider_id"), QStringLiteral("parakeet")}}}};
}

QJsonArray configEntries()
{
    auto entry = [](const char *key, const QJsonValue &value) {
        return QJsonObject{{QStringLiteral("key"), QLatin1String(key)},
                           {QStringLiteral("value"), value},
                           {QStringLiteral("source"), QStringLiteral("default")}};
    };
    return {entry("hotkeys.hold", QStringLiteral("F9")), entry("hotkeys.toggle", QStringLiteral("SUPER CTRL, X")),
            entry("dictation.mode", QStringLiteral("raw")), entry("audio.input_device", QString())};
}

void apply(StatusModel *statusModel, EnginesModel *enginesModel, HistoryModel *history, ConfigBinding *config,
           HistoryDetailModel *detail, HistoryPlayer *player, HistoryActions *actions)
{
    if (config != nullptr)
        config->applyEntries(configEntries());
    if (enginesModel != nullptr) {
        enginesModel->applySelection(selection());
        enginesModel->applyEngines(engines());
    }
    if (history != nullptr)
        history->applyItems(historyItems(), referenceDate());
    if (statusModel != nullptr)
        statusModel->applySample(status());
    if (detail != nullptr)
        detail->apply(detailId(), historyDetail());
    if (player != nullptr) {
        // A take of speech: bursts of words with breaths between, a
        // quarter played, as history.png draws it.
        QVariantList peaks;
        for (int i = 0; i < HistoryPlayer::kBars; ++i) {
            const double word = (i % 9 < 6) ? 0.35 + 0.55 * ((i * 7) % 5) / 4.0 : 0.08;
            peaks.append(word);
        }
        player->applySample(peaks, 4200, 1000);
    }
    if (actions != nullptr)
        actions->applyProviders(providers(), selection());
}

}  // namespace dettivo::sample
