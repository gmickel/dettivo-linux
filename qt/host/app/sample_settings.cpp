// The settings routes' sample (fn-26 R2): every key the routes edit with
// the artboards' values, the models table, the snippet panel, the agent
// hosts and the doctor report, so `--render` shows the same structure
// settings-models.png, settings-hotkeys.png and agents.png fix.
#include "agents_model.h"
#include "config_binding.h"
#include "doctor_model.h"
#include "hotkeys_setup_model.h"
#include "models_table.h"
#include "sample_data.h"
#include "settings_model.h"

#include <QJsonObject>
#include <QJsonValue>

namespace dettivo::sample {

namespace {

QJsonObject entry(const char *key, const QJsonValue &value, const char *source = "file")
{
    return {{QStringLiteral("key"), QLatin1String(key)}, {QStringLiteral("value"), value}, {QStringLiteral("source"), QLatin1String(source)}};
}

QJsonArray list(std::initializer_list<const char *> items)
{
    QJsonArray out;
    for (const char *item : items)
        out.append(QLatin1String(item));
    return out;
}

}  // namespace

QJsonArray settingsEntries()
{
    return {
        entry("daemon.log_level", QStringLiteral("info"), "default"),
        entry("daemon.shutdown_timeout_ms", 5000, "default"),
        entry("audio.input_device", QStringLiteral(""), "default"),
        entry("audio.level_interval_ms", 50, "default"),
        entry("dictation.language", QStringLiteral("en")),
        entry("dictation.vocabulary", list({"Dettivo", "Omarchy", "Hyprland"})),
        entry("dictation.max_duration_seconds", 300, "default"),
        entry("dictation.silence_peak_threshold", 0.01, "default"),
        entry("dictation.mode", QStringLiteral("enhanced")),
        entry("dictation.spoken_punctuation", true, "default"),
        entry("dictation.replacements", QJsonObject{{QStringLiteral("teh"), QStringLiteral("the")}}),
        entry("dictation.protect_tokens", true, "default"),
        entry("paths.data_dir", QStringLiteral("~/.local/share/dettivo"), "default"),
        entry("paths.models_dir", QStringLiteral("~/.local/share/dettivo/models"), "default"),
        entry("osd.enabled", true, "default"),
        entry("osd.host", QStringLiteral("auto"), "default"),
        entry("osd.position", QStringLiteral("top"), "default"),
        entry("osd.margin", 24, "default"),
        entry("osd.monitor", QStringLiteral("focused"), "default"),
        entry("osd.hide_after_ms", 1800, "default"),
        entry("osd.error_hide_after_ms", 4000, "default"),
        entry("osd.show_level", true, "default"),
        entry("osd.motion", QStringLiteral("full"), "default"),
        entry("omarchy.glyph", QStringLiteral("waveform"), "default"),
        entry("omarchy.level_meter", true, "default"),
        entry("omarchy.osd", QStringLiteral("panel"), "default"),
        entry("omarchy.open_shortcut", QStringLiteral("SUPER SHIFT, D"), "default"),
        entry("omarchy.history_items", 3, "default"),
        entry("hotkeys.hold", QStringLiteral("F9")),
        entry("hotkeys.toggle", QStringLiteral("SUPER CTRL, X")),
        entry("hotkeys.cancel", QStringLiteral("SUPER CTRL, Escape"), "default"),
        entry("hotkeys.reinsert", QStringLiteral("SUPER CTRL, V")),
        entry("hotkeys.backend", QStringLiteral("none")),
        entry("hotkeys.pause_media", true),
        entry("hotkeys.sounds", false, "default"),
        entry("hotkeys.evdev_devices", QJsonArray(), "default"),
        entry("speech.provider", QStringLiteral("parakeet")),
        entry("speech.model", QStringLiteral("parakeet-v3")),
        entry("speech.meeting_model", QStringLiteral("small")),
        entry("speech.parakeet_model_id", QStringLiteral("parakeet-v3"), "default"),
        entry("llm.provider", QStringLiteral("local")),
        entry("llm.model", QStringLiteral("qwen3-4b-instruct-2507"), "default"),
        entry("llm.analysis_model", QStringLiteral(""), "default"),
        entry("llm.ollama_url", QStringLiteral("http://127.0.0.1:11434"), "default"),
        entry("llm.ollama_model", QStringLiteral("qwen3:4b-instruct"), "default"),
        entry("llm.endpoint_url", QStringLiteral(""), "default"),
        entry("llm.endpoint_model", QStringLiteral(""), "default"),
        entry("llm.api_key_file", QStringLiteral(""), "default"),
        entry("llm.trusted_endpoints", QJsonArray(), "default"),
        entry("llm.timeout_ms", 8000, "default"),
        entry("llm.max_retries", 2, "default"),
        entry("llm.polish_experiment", QStringLiteral(""), "default"),
        entry("llm.experiments_dir", QStringLiteral(""), "default"),
        entry("engines.stt_idle_seconds", 300, "default"),
        entry("engines.llm_idle_seconds", 600, "default"),
        entry("engines.directory", QStringLiteral(""), "default"),
        entry("engines.whisper.backend", QStringLiteral("auto"), "default"),
        entry("engines.parakeet.backend", QStringLiteral("auto"), "default"),
        entry("engines.llm.backend", QStringLiteral("auto"), "default"),
        entry("engines.llm.context_length", 4096, "default"),
        entry("engines.llm.max_tokens", 1024, "default"),
        entry("engines.diarize.backend", "auto", "default"),
        entry("engines.diarize.threads", 0, "default"),
        entry("models.max_concurrent_downloads", 1, "default"),
        entry("models.verify_on_start", true, "default"),
        entry("models.catalogue_file", QStringLiteral(""), "default"),
        entry("polish.transforms", list({"fixGrammar", "removeFillers", "smartPunctuation"}), "default"),
        entry("polish.default_preset", QStringLiteral("generic"), "default"),
        entry("polish.default_style", QStringLiteral("asDictated"), "default"),
        entry("insert.backend", QStringLiteral("auto"), "default"),
        entry("insert.paste_keys", QJsonObject{{QStringLiteral("Code"), QStringLiteral("ctrl+v")}}),
        entry("insert.terminal_app_ids", list({"foot", "footclient", "com.mitchellh.ghostty", "ghostty", "Alacritty", "alacritty", "kitty"}), "default"),
        entry("insert.self_app_ids", list({"dettivo", "dettivo-app", "dettivo-osd", "dettivo-sheet"}), "default"),
        entry("insert.inter_key_delay_ms", 2, "default"),
        entry("insert.restore_clipboard", true, "default"),
        entry("insert.clipboard_restore_delay_ms", 300, "default"),
        entry("insert.undo_window_ms", 5000, "default"),
        entry("history.keep_audio", true, "default"),
        entry("history.audio_retention_days", 30, "default"),
        entry("history.max_items", 0, "default"),
        entry("history.artifacts", QStringLiteral("keep"), "default"),
        entry("history.db_path", QStringLiteral("~/.local/share/dettivo/dettivo.db"), "default"),
        entry("history.max_import_seconds", 14400, "default"),
        entry("transcribe.chunk_seconds", 30, "default"),
        entry("transcribe.overlap_seconds", 2, "default"),
        entry("transcribe.safety_margin_seconds", 5, "default"),
        entry("transcribe.silence_rms_floor", 0.0065, "default"),
        entry("transcribe.filler_filter", true, "default"),
        entry("audio.system_source", QStringLiteral("default_monitor"), "default"),
        entry("meetings.keep_audio", true, "default"),
        entry("meetings.artifacts", QStringLiteral("keep"), "default"),
        entry("meetings.checkpoint_interval_seconds", 15, "default"),
        entry("meetings.live", true, "default"),
        entry("meetings.live_window_ms", 3000, "default"),
        entry("meetings.live_tick_ms", 900, "default"),
        entry("meetings.live_overlap_ms", 450, "default"),
        entry("meetings.speech_floor_rms", 0.0065, "default"),
        entry("meetings.boundary_merge_gap_ms", 1200, "default"),
        entry("meetings.cross_source_padding_ms", 800, "default"),
        entry("meetings.diarization.enabled", true, "default"),
        entry("meetings.diarization.auto", true, "default"),
        entry("meetings.diarization.model", QStringLiteral("diarization"), "default"),
        entry("meetings.diarization.pause_ms", 250, "default"),
        entry("meetings.diarization.nearest_turn_ms", 10000, "default"),
        entry("meetings.diarization.min_speaker_share", 0.0, "default"),
        entry("meetings.diarization.max_speakers", 0, "default"),
        entry("meetings.diarization.clustering_threshold", 0.5, "default"),
        entry("meetings.delete_artifact_policy", QStringLiteral("all"), "default"),
        entry("meetings.analysis.auto", true, "default"),
        entry("meetings.analysis.timeout_ms", 60000, "default"),
        entry("meetings.analysis.chunk_chars", 12000, "default"),
        entry("meetings.analysis.provider", QStringLiteral(""), "default"),
        entry("transfer.max_upload_bytes", 1073741824, "default"),
        entry("ipc.auth_mode", QStringLiteral("peer"), "default"),
        entry("ipc.socket", QStringLiteral("$XDG_RUNTIME_DIR/dettivo/dettivo.sock"), "default"),
        entry("ipc.token_file", QStringLiteral("~/.config/dettivo/ipc.token"), "default"),
        entry("ipc.max_line_bytes", 1048576, "default"),
        entry("mcp.hardened", false, "default"),
        entry("mcp.max_message_bytes", 2000000, "default"),
        entry("rest.enabled", false, "default"),
        entry("rest.bind", QStringLiteral("127.0.0.1"), "default"),
        entry("rest.port", 45831, "default"),
        entry("rest.max_body_bytes", 52428800, "default"),
        entry("rest.request_timeout_ms", 30000, "default"),
        entry("qa.mode", true, "environment"),
    };
}

void applySettings(ConfigBinding *config, SettingsModel *settings, ModelsTable *models, HotkeysSetupModel *hotkeys,
                   AgentsModel *agents, DoctorModel *doctor)
{
    if (config != nullptr)
        config->applyEntries(settingsEntries());
    if (settings != nullptr)
        settings->applySample();
    if (models != nullptr)
        models->applySample();
    if (hotkeys != nullptr)
        hotkeys->applySample();
    if (agents != nullptr)
        agents->applySample();
    if (doctor != nullptr)
        doctor->applySample();
}

}  // namespace dettivo::sample
