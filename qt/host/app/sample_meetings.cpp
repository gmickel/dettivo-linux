// The meetings baselines' facts with no daemon (fn-34 R4, ADR 0038): the
// six rows over two weeks of meetings-list.png with their swatches and
// chips, the live meeting of meeting-live.png with its meters, elapsed
// clock, eight segments (the last provisional) and notes, and the
// analysed kickoff of meeting-detail.png with three speakers, seven
// segments, notes and an analysis, so the visual check renders the
// structure the artboards fix.
#include "meeting_detail_model.h"
#include "meeting_live_model.h"
#include "meetings_actions.h"
#include "meetings_model.h"
#include "sample_data.h"

#include <QDate>
#include <QDateTime>
#include <QTimeZone>

namespace dettivo::sample {

namespace {

QString meetingId(const char *suffix)
{
    return QStringLiteral("5a3p1e00-0000-4000-8000-00000000ee") + QLatin1String(suffix);
}

QString isoAt(int daysBefore, int hour, int minute)
{
    const QDateTime when(referenceDate().addDays(-daysBefore), QTime(hour, minute), QTimeZone::LocalTime);
    return when.toUTC().toString(Qt::ISODateWithMs);
}

/// The reference day at a clock time, for facts the rail shows as a time alone.
QString todayAt(int hour, int minute)
{
    const QDateTime when(referenceDate(), QTime(hour, minute), QTimeZone::LocalTime);
    return when.toUTC().toString(Qt::ISODateWithMs);
}

QJsonObject row(const char *suffix, int daysBefore, int hour, int minute, const char *title, const char *summary, int minutes,
                int speakers, const char *analysis, bool notes, bool partial)
{
    return {{QStringLiteral("ref"), QJsonObject{{QStringLiteral("id"), meetingId(suffix)}, {QStringLiteral("kind"), QStringLiteral("meeting")}}},
            {QStringLiteral("title"), QString::fromUtf8(title)},
            {QStringLiteral("summary"), QString::fromUtf8(summary)},
            {QStringLiteral("started_at"), isoAt(daysBefore, hour, minute)},
            {QStringLiteral("duration_seconds"), minutes * 60},
            {QStringLiteral("status"), partial ? QStringLiteral("partial") : QStringLiteral("completed")},
            {QStringLiteral("is_partial"), partial},
            {QStringLiteral("has_notes"), notes},
            {QStringLiteral("analysis_status"), QLatin1String(analysis)},
            {QStringLiteral("speaker_count"), speakers}};
}

QJsonObject speaker(const char *id, const char *name, int color, int talkMs)
{
    return {{QStringLiteral("speaker_id"), QLatin1String(id)},
            {QStringLiteral("name"), QLatin1String(name)},
            {QStringLiteral("color_index"), color},
            {QStringLiteral("talk_ms"), talkMs}};
}

QJsonObject segment(int startMs, int endMs, const char *text, const char *speaker, const char *speakerId, bool you)
{
    QJsonObject s{{QStringLiteral("start_ms"), startMs},
                  {QStringLiteral("end_ms"), endMs},
                  {QStringLiteral("text"), QString::fromUtf8(text)},
                  {QStringLiteral("polished_text"), QString::fromUtf8(text)},
                  {QStringLiteral("source_type"), you ? QStringLiteral("microphone") : QStringLiteral("system")}};
    if (speaker != nullptr) {
        s.insert(QStringLiteral("speaker"), QString::fromUtf8(speaker));
        s.insert(QStringLiteral("speaker_id"), QLatin1String(speakerId));
    }
    return s;
}

QJsonObject liveSegment(const char *id, const char *source, int startMs, int endMs, const char *text, bool provisional)
{
    return {{QStringLiteral("meeting_id"), meetingId("01")}, {QStringLiteral("segment_id"), QLatin1String(id)},
            {QStringLiteral("source"), QLatin1String(source)}, {QStringLiteral("start_ms"), startMs},
            {QStringLiteral("end_ms"), endMs},                {QStringLiteral("text"), QString::fromUtf8(text)},
            {QStringLiteral("provisional"), provisional},     {QStringLiteral("words"), QJsonArray()}};
}

constexpr auto kNotes = "## Decisions\n- Engine processes, one protocol, Vulkan default\n- parakeet.cpp for Parakeet, S-13 proves timestamps\n- Diarization stays Sherpa-ONNX, own process\n- 4B default, tuned 1.7B optional, same gates\n\n## Open\n- Bar widget timer placement";

}  // namespace

QJsonArray meetingsItems()
{
    return {row("01", 0, 11, 5, "Dettivo Linux kickoff", "Engine processes, one protocol, Vulkan default · 4 decisions · 3 actions", 41, 3, "ready", true, false),
            row("02", 1, 9, 14, "Weekly sync with Tobias", "Release gate, CUDA package timing · 2 decisions", 28, 2, "ready", false, false),
            row("03", 2, 16, 0, "Customer call · Nordwind", "", 52, 2, "none", false, true),
            row("04", 5, 14, 30, "Omarchy plugin review", "Bar widget states, panel contents · 3 decisions", 19, 2, "ready", false, false),
            row("05", 6, 10, 0, "Imported · interview-raw.m4a", "Import · diarized into 2 speakers", 64, 2, "none", true, false),
            row("06", 7, 15, 15, "Design review · Studio direction", "Type, tokens, OSD · 5 decisions · 2 actions", 33, 2, "ready", false, false)};
}

QJsonArray meetingSpeakers(const QString &id)
{
    if (id == meetingId("01"))
        return {speaker("you", "You", 0, 1442000), speaker("speaker_00", "Mara", 1, 670000), speaker("speaker_01", "Tobias", 2, 368000)};
    if (id == meetingId("02"))
        return {speaker("you", "You", 0, 900000), speaker("speaker_00", "Tobias", 1, 780000)};
    if (id == meetingId("03"))
        return {speaker("you", "You", 0, 1500000), speaker("remote", "Remote", 1, 1600000)};
    if (id == meetingId("05"))
        return {speaker("speaker_00", "Speaker 1", 0, 2000000), speaker("speaker_01", "Speaker 2", 1, 1800000)};
    return {speaker("you", "You", 0, 600000), speaker("speaker_00", "Mara", 1, 500000)};
}

QString meetingDetailId()
{
    return meetingId("01");
}

QString meetingNotesOnlyId()
{
    return meetingId("05");
}

QJsonObject meetingDetail()
{
    const QString started = isoAt(0, 11, 5);
    const QDateTime end(referenceDate(), QTime(11, 46), QTimeZone::LocalTime);
    const QJsonArray segments{
        segment(0, 8000, "Let's lock the engine process model today. One protocol, four engines, Vulkan by default.", "You", "you", true),
        segment(14 * 60000, 14 * 60000 + 8000, "Fine by me. Does parakeet.cpp give us the timestamps the merger needs, or is that still the spike?", "Mara", "speaker_00", false),
        segment(26 * 60000, 26 * 60000 + 9000, "Still the spike. Word timestamps are there; precision against the golden alignment is what S-13 has to prove.", "You", "you", true),
        segment(27 * 60000 + 47000, 27 * 60000 + 51000, "And diarization stays on ONNX?", "Tobias", "speaker_01", false),
        segment(28 * 60000 + 50000, 28 * 60000 + 56000, "Yes, in its own process. CPU is fine for a post-meeting pass.", "You", "you", true),
        segment(35 * 60000 + 15000, 35 * 60000 + 24000, "Then the decision for today is the 4B default and the tuned 1.7B as optional. Same gates as the Mac.", "Mara", "speaker_00", false),
        segment(37 * 60000 + 36000, 37 * 60000 + 44000, "Agreed. I'll write that into the spec and open the PR after the call.", "You", "you", true)};
    const QJsonObject analysis{
        {QStringLiteral("summary"), QStringLiteral("Kickoff for the Linux port. The engine process architecture was agreed, with parakeet.cpp for Parakeet pending the timestamp spike, diarization staying on Sherpa-ONNX, and the Mac's polish model strategy carried over unchanged.")},
        {QStringLiteral("decisions"), QJsonArray{QStringLiteral("Engine processes with one protocol, Vulkan by default"), QStringLiteral("parakeet.cpp for Parakeet; S-13 proves timestamps"),
                                                 QStringLiteral("Diarization stays Sherpa-ONNX in its own process"), QStringLiteral("Qwen3 4B default, tuned 1.7B optional, same gates")}},
        {QStringLiteral("action_items"), QJsonArray{QJsonObject{{QStringLiteral("text"), QStringLiteral("write the decisions into the spec, open the PR")}, {QStringLiteral("owner"), QStringLiteral("You")}},
                                                    QJsonObject{{QStringLiteral("text"), QStringLiteral("place the bar widget timer")}, {QStringLiteral("owner"), QStringLiteral("Mara")}},
                                                    QJsonObject{{QStringLiteral("text"), QStringLiteral("CUDA package timing after the Vulkan baseline")}, {QStringLiteral("owner"), QStringLiteral("Tobias")}}}}};
    return {{QStringLiteral("ref"), QJsonObject{{QStringLiteral("id"), meetingDetailId()}, {QStringLiteral("kind"), QStringLiteral("meeting")}}},
            {QStringLiteral("title"), QStringLiteral("Dettivo Linux kickoff")},
            {QStringLiteral("status"), QStringLiteral("completed")},
            {QStringLiteral("started_at"), started},
            {QStringLiteral("ended_at"), end.toUTC().toString(Qt::ISODateWithMs)},
            {QStringLiteral("duration_seconds"), 41 * 60},
            {QStringLiteral("language"), QStringLiteral("en")},
            {QStringLiteral("stt_provider_id"), QStringLiteral("whisper")},
            {QStringLiteral("stt_model_id"), QStringLiteral("small")},
            {QStringLiteral("system_audio"), true},
            {QStringLiteral("microphone_takes"), 1},
            {QStringLiteral("audio_kept"), true},
            {QStringLiteral("segments"), segments},
            {QStringLiteral("speakers"), meetingSpeakers(meetingDetailId())},
            {QStringLiteral("diarization"), QJsonObject{{QStringLiteral("status"), QStringLiteral("ready")}, {QStringLiteral("engine"), QStringLiteral("dettivo-engine-diarize")}, {QStringLiteral("coverage"), 0.96}}},
            {QStringLiteral("notes"), QLatin1String(kNotes)},
            {QStringLiteral("notes_source"), QStringLiteral("user")},
            {QStringLiteral("notes_updated_at"), isoAt(0, 11, 31)},
            {QStringLiteral("analysis"), analysis},
            {QStringLiteral("analysis_status"), QStringLiteral("ready")},
            {QStringLiteral("analysis_model"), QStringLiteral("qwen3-4b-instruct-2507")},
            {QStringLiteral("analyzed_at"), isoAt(0, 11, 47)}};
}

QJsonObject liveFacts()
{
    return {{QStringLiteral("meeting_id"), meetingId("01")},
            {QStringLiteral("title"), QStringLiteral("Dettivo Linux kickoff")},
            {QStringLiteral("started_at"), isoAt(0, 21, 3)},
            {QStringLiteral("engine"), QStringLiteral("whisper small")},
            {QStringLiteral("mic_device"), QStringLiteral("Arctis Nova")},
            {QStringLiteral("system_device"), QStringLiteral("chromium · meet.google.com")},
            {QStringLiteral("elapsed_ms"), 23 * 60000 + 41000},
            {QStringLiteral("mic_level"), 0.62},
            {QStringLiteral("mic_peak"), 0.8},
            {QStringLiteral("system_level"), 0.28},
            {QStringLiteral("system_peak"), 0.45},
            {QStringLiteral("notes"), QLatin1String(kNotes)},
            {QStringLiteral("state"), QStringLiteral("recording")}};
}

QJsonArray liveSegments()
{
    return {liveSegment("you-1", "you", 60000, 68000, "Let's lock the engine process model today. One protocol, four engines, Vulkan by default.", false),
            liveSegment("remote-1", "remote", 16 * 60000, 16 * 60000 + 8000, "Fine by me. Does parakeet.cpp give us the timestamps the merger needs, or is that still the spike?", false),
            liveSegment("you-2", "you", 28 * 60000, 28 * 60000 + 9000, "Still the spike. Word timestamps are there; precision against the golden alignment is what S-13 has to prove.", false),
            liveSegment("remote-2", "remote", 49 * 60000, 49 * 60000 + 4000, "And diarization stays on ONNX?", false),
            liveSegment("you-3", "you", 52 * 60000, 52 * 60000 + 6000, "Yes, in its own process. CPU is fine for a post-meeting pass.", false),
            liveSegment("remote-3", "remote", 77 * 60000, 77 * 60000 + 9000, "Then the decision for today is the 4B default and the tuned 1.7B as optional. Same gates as the Mac.", false),
            liveSegment("you-4", "you", 98 * 60000, 98 * 60000 + 8000, "Agreed. I'll write that into the spec and open the PR after the call.", false),
            liveSegment("remote-p1", "remote", 153 * 60000, 153 * 60000 + 5000, "one more thing on the bar widget, the timer should", true)};
}

void applyMeetings(const QString &state, MeetingsModel *meetings, MeetingLiveModel *live, MeetingDetailModel *detail,
                   MeetingsActions *actions)
{
    if (meetings != nullptr) {
        meetings->setQaState(state);
        if (state == QStringLiteral("list-empty")) {
            meetings->applyItems(QJsonArray(), referenceDate());
        } else {
            meetings->applyItems(meetingsItems(), referenceDate());
            QJsonArray recoverable;
            for (const QJsonValue &v : meetingsItems()) {
                const QString id = v.toObject().value(QStringLiteral("ref")).toObject().value(QStringLiteral("id")).toString();
                meetings->applySpeakers(id, meetingSpeakers(id));
                if (v.toObject().value(QStringLiteral("is_partial")).toBool())
                    recoverable.append(v);
            }
            meetings->applyRecoverable(recoverable);
        }
    }
    if (live != nullptr) {
        live->applyDisclosure({{QStringLiteral("acknowledged"), true},
                               {QStringLiteral("acknowledged_at"), todayAt(11, 4)},
                               {QStringLiteral("message"), QStringLiteral("This meeting may be recorded and transcribed by Dettivo. Please inform all participants and comply with local laws and company policy before recording.")}}, referenceDate());
        live->applyDevices({{QStringLiteral("default_source"), QStringLiteral("mic")},
                            {QStringLiteral("default_sink"), QStringLiteral("sink")},
                            {QStringLiteral("pipewire"), true},
                            {QStringLiteral("devices"), QJsonArray{QJsonObject{{QStringLiteral("name"), QStringLiteral("mic")}, {QStringLiteral("description"), QStringLiteral("Arctis Nova")}},
                                                                   QJsonObject{{QStringLiteral("name"), QStringLiteral("sink")}, {QStringLiteral("description"), QStringLiteral("Arctis Nova")}}}}});
        if (state == QStringLiteral("live"))
            live->applySample(liveFacts(), liveSegments());
    }
    if (detail != nullptr && (state.startsWith(QStringLiteral("detail")) || state == QStringLiteral("rename")))
        detail->apply(meetingDetailId(), meetingDetail());
    if (actions != nullptr) {
        actions->applyProviders(providers(), selection());
        actions->applySuggestions({QJsonObject{{QStringLiteral("name"), QStringLiteral("Mara")}, {QStringLiteral("uses"), 4}},
                                   QJsonObject{{QStringLiteral("name"), QStringLiteral("Tobias")}, {QStringLiteral("uses"), 3}},
                                   QJsonObject{{QStringLiteral("name"), QStringLiteral("Nordwind")}, {QStringLiteral("uses"), 1}}});
    }
}

}  // namespace dettivo::sample
