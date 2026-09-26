#include "settings_keys.h"

#include <QHash>

namespace dettivo::settings {

namespace {

struct RouteKey {
    const char *section;
    const char *key;
};

// Every editable key, once per section that shows it. The lint reads the
// lines between the markers; keep one key per line.
// routes:start
constexpr RouteKey kRouteKeys[] = {
    {"general", "daemon.log_level"},
    {"general", "daemon.shutdown_timeout_ms"},
    {"general", "audio.input_device"},
    {"general", "audio.level_interval_ms"},
    {"general", "dictation.language"},
    {"vocabulary", "dictation.vocabulary"},
    {"general", "dictation.max_duration_seconds"},
    {"general", "dictation.silence_peak_threshold"},
    {"general", "paths.data_dir"},
    {"general", "paths.models_dir"},
    {"general", "osd.enabled"},
    {"general", "osd.host"},
    {"general", "osd.position"},
    {"general", "osd.margin"},
    {"general", "osd.monitor"},
    {"general", "osd.hide_after_ms"},
    {"general", "osd.error_hide_after_ms"},
    {"general", "osd.show_level"},
    {"general", "osd.motion"},
    {"general", "omarchy.glyph"},
    {"general", "omarchy.level_meter"},
    {"general", "omarchy.osd"},
    {"general", "omarchy.open_shortcut"},
    {"general", "omarchy.history_items"},
    {"hotkeys", "hotkeys.hold"},
    {"hotkeys", "hotkeys.toggle"},
    {"hotkeys", "hotkeys.cancel"},
    {"hotkeys", "hotkeys.reinsert"},
    {"hotkeys", "hotkeys.backend"},
    {"hotkeys", "hotkeys.pause_media"},
    {"hotkeys", "hotkeys.sounds"},
    {"hotkeys", "hotkeys.evdev_devices"},
    {"models", "speech.provider"},
    {"models", "speech.model"},
    {"models", "speech.meeting_model"},
    {"models", "speech.parakeet_model_id"},
    {"models", "llm.model"},
    {"models", "engines.stt_idle_seconds"},
    {"models", "engines.llm_idle_seconds"},
    {"models", "engines.directory"},
    {"models", "engines.whisper.backend"},
    {"models", "engines.parakeet.backend"},
    {"models", "engines.llm.backend"},
    {"models", "engines.llm.context_length"},
    {"models", "engines.llm.max_tokens"},
    {"models", "engines.diarize.backend"},
    {"models", "engines.diarize.threads"},
    {"models", "models.max_concurrent_downloads"},
    {"models", "models.verify_on_start"},
    {"models", "models.catalogue_file"},
    {"polish", "dictation.mode"},
    {"polish", "dictation.spoken_punctuation"},
    {"polish", "dictation.replacements"},
    {"polish", "dictation.protect_tokens"},
    {"polish", "polish.transforms"},
    {"polish", "polish.default_preset"},
    {"polish", "polish.default_style"},
    {"polish", "llm.provider"},
    {"polish", "llm.ollama_url"},
    {"polish", "llm.ollama_model"},
    {"polish", "llm.endpoint_url"},
    {"polish", "llm.endpoint_model"},
    {"polish", "llm.api_key_file"},
    {"polish", "llm.trusted_endpoints"},
    {"polish", "llm.timeout_ms"},
    {"polish", "llm.max_retries"},
    {"polish", "llm.polish_experiment"},
    {"polish", "llm.experiments_dir"},
    {"insertion", "insert.backend"},
    {"insertion", "insert.paste_keys"},
    {"insertion", "insert.terminal_app_ids"},
    {"insertion", "insert.self_app_ids"},
    {"insertion", "insert.inter_key_delay_ms"},
    {"insertion", "insert.restore_clipboard"},
    {"insertion", "insert.clipboard_restore_delay_ms"},
    {"insertion", "insert.undo_window_ms"},
    {"meetings", "speech.meeting_model"},
    {"meetings", "llm.analysis_model"},
    {"meetings", "history.keep_audio"},
    {"meetings", "history.audio_retention_days"},
    {"meetings", "history.max_items"},
    {"meetings", "history.artifacts"},
    {"meetings", "history.db_path"},
    {"meetings", "history.max_import_seconds"},
    {"meetings", "transcribe.chunk_seconds"},
    {"meetings", "transcribe.overlap_seconds"},
    {"meetings", "transcribe.safety_margin_seconds"},
    {"meetings", "transcribe.silence_rms_floor"},
    {"meetings", "transcribe.filler_filter"},
    {"meetings", "audio.system_source"},
    {"meetings", "meetings.keep_audio"},
    {"meetings", "meetings.artifacts"},
    {"meetings", "meetings.checkpoint_interval_seconds"},
    {"meetings", "meetings.live"},
    {"meetings", "meetings.live_window_ms"},
    {"meetings", "meetings.live_tick_ms"},
    {"meetings", "meetings.live_overlap_ms"},
    {"meetings", "meetings.speech_floor_rms"},
    {"meetings", "meetings.boundary_merge_gap_ms"},
    {"meetings", "meetings.cross_source_padding_ms"},
    {"meetings", "meetings.diarization.enabled"},
    {"meetings", "meetings.diarization.auto"},
    {"meetings", "meetings.diarization.model"},
    {"meetings", "meetings.diarization.pause_ms"},
    {"meetings", "meetings.diarization.nearest_turn_ms"},
    {"meetings", "meetings.diarization.min_speaker_share"},
    {"meetings", "meetings.diarization.max_speakers"},
    {"meetings", "meetings.diarization.clustering_threshold"},
    {"meetings", "meetings.delete_artifact_policy"},
    {"meetings", "meetings.analysis.auto"},
    {"meetings", "meetings.analysis.timeout_ms"},
    {"meetings", "meetings.analysis.chunk_chars"},
    {"meetings", "meetings.analysis.provider"},
    {"meetings", "transfer.max_upload_bytes"},
    {"agents", "ipc.auth_mode"},
    {"agents", "ipc.socket"},
    {"agents", "ipc.token_file"},
    {"agents", "ipc.max_line_bytes"},
    {"agents", "mcp.hardened"},
    {"agents", "mcp.max_message_bytes"},
    {"agents", "rest.enabled"},
    {"agents", "rest.bind"},
    {"agents", "rest.port"},
    {"agents", "rest.max_body_bytes"},
    {"agents", "rest.request_timeout_ms"},
    {"diagnostics", "qa.mode"},
};
// routes:end

struct Excluded {
    const char *key;
    const char *reason;
};

// Keys with no editor and why; the lint accepts a key here as covered.
// excluded:start
constexpr Excluded kExcluded[] = {
    {"polish.rules", "an array of tables edited through `dettivo polish rules` and the Polish route's rule list"},
    {"polish.apps", "a table of tables edited through `dettivo polish apps` and the Polish route's app list"},
    {"polish.presets", "a table of tables with nested lists; edited in the file"},
};
// excluded:end

struct EnvironmentKey {
    const char *key;
    const char *variable;
};

constexpr EnvironmentKey kEnvironment[] = {
    {"ipc.socket", "DETTIVO_IPC_SOCKET"},
    {"paths.data_dir", "DETTIVO_DATA_DIR"},
    {"qa.mode", "DETTIVO_QA"},
};

}  // namespace

QStringList sections()
{
    return {QStringLiteral("general"), QStringLiteral("vocabulary"),   QStringLiteral("hotkeys"),  QStringLiteral("models"), QStringLiteral("polish"),
            QStringLiteral("insertion"), QStringLiteral("meetings"), QStringLiteral("agents"), QStringLiteral("diagnostics")};
}

QStringList keysFor(const QString &section)
{
    QStringList out;
    for (const RouteKey &entry : kRouteKeys) {
        if (section == QLatin1String(entry.section))
            out.append(QLatin1String(entry.key));
    }
    return out;
}

QStringList editableKeys()
{
    QStringList out;
    for (const RouteKey &entry : kRouteKeys) {
        const QString key = QLatin1String(entry.key);
        if (!out.contains(key))
            out.append(key);
    }
    return out;
}

QString reasonNotEditable(const QString &key)
{
    for (const Excluded &entry : kExcluded) {
        if (key == QLatin1String(entry.key))
            return QLatin1String(entry.reason);
    }
    return {};
}

QString tableOf(const QString &key)
{
    const int dot = key.lastIndexOf(QLatin1Char('.'));
    return dot < 0 ? key : key.left(dot);
}

QString leafOf(const QString &key)
{
    const int dot = key.lastIndexOf(QLatin1Char('.'));
    return dot < 0 ? key : key.mid(dot + 1);
}

QString environmentVariable(const QString &key)
{
    for (const EnvironmentKey &entry : kEnvironment) {
        if (key == QLatin1String(entry.key))
            return QLatin1String(entry.variable);
    }
    return {};
}

}  // namespace dettivo::settings
