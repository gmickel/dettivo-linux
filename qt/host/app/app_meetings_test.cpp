// The meetings host pieces without a daemon or a window (fn-34, ADR 0038):
// the list's weeks, chips and swatches over `meetings.list` and
// `meetings.get`, the live transcript's provisional tail kept fragment by
// fragment and the finals kept in time order (ADR 0061), and the live
// model's disclosure gate, start, levels per source, notes debounce and
// the stop acknowledged at once that lands on the detail. The detail,
// the actions and the formatting are app_meeting_detail_test.cpp.
#include "fake_link.h"
#include "live_segments_model.h"
#include "meeting_format.h"
#include "meeting_live_model.h"
#include "meetings_model.h"

#include <QJsonArray>
#include <QSignalSpy>
#include <QTest>

using namespace dettivo;
using dettivo::test::FakeLink;

namespace {

QJsonObject listItem(const QString &id, const QString &title, const QString &when, const QString &analysis, bool notes, bool partial, int speakers)
{
    return {{QStringLiteral("ref"), QJsonObject{{QStringLiteral("id"), id}, {QStringLiteral("kind"), QStringLiteral("meeting")}}},
            {QStringLiteral("title"), title},
            {QStringLiteral("summary"), QString()},
            {QStringLiteral("started_at"), when},
            {QStringLiteral("duration_seconds"), 2520},
            {QStringLiteral("status"), partial ? QStringLiteral("partial") : QStringLiteral("completed")},
            {QStringLiteral("is_partial"), partial},
            {QStringLiteral("has_notes"), notes},
            {QStringLiteral("analysis_status"), analysis},
            {QStringLiteral("speaker_count"), speakers}};
}

QJsonObject segmentEvent(const QString &id, const QString &source, int start, int end, const QString &text, bool provisional)
{
    return {{QStringLiteral("meeting_id"), QStringLiteral("m1")}, {QStringLiteral("segment_id"), id},
            {QStringLiteral("source"), source},              {QStringLiteral("start_ms"), start},
            {QStringLiteral("end_ms"), end},                 {QStringLiteral("text"), text},
            {QStringLiteral("provisional"), provisional},    {QStringLiteral("words"), QJsonArray()}};
}

}  // namespace

class AppMeetingsTest : public QObject {
    Q_OBJECT

private slots:
    void listGroupsByWeekChipsAndSwatches();
    void liveSegmentsKeepEveryFragmentOfTheTailAndFinalsInOrder();
    void liveModelGatesOnDisclosureStartsAndStops();
    void liveModelUsesRecordedEngineAndCompletesWithQueuedPasses();
};

void AppMeetingsTest::listGroupsByWeekChipsAndSwatches()
{
    FakeLink link;
    const QDate today(2026, 2, 13);  // a Friday
    link.answers.insert(QStringLiteral("meetings.list"),
                        {{QStringLiteral("items"), QJsonArray{listItem(QStringLiteral("a"), QStringLiteral("Roadmap review"), QStringLiteral("2026-02-11T10:00:00Z"), QStringLiteral("ready"), true, false, 2),
                                                              listItem(QStringLiteral("b"), QStringLiteral("Design call"), QStringLiteral("2026-02-04T14:30:00Z"), QStringLiteral("none"), false, true, 0),
                                                              listItem(QStringLiteral("c"), QStringLiteral("Vendor intro"), QStringLiteral("2026-01-27T09:15:00Z"), QStringLiteral("none"), true, false, 0)}},
                         {QStringLiteral("next_cursor"), QJsonValue::Null}});
    link.answers.insert(QStringLiteral("meetings.get"),
                        {{QStringLiteral("speakers"), QJsonArray{QJsonObject{{QStringLiteral("speaker_id"), QStringLiteral("you")}, {QStringLiteral("name"), QStringLiteral("You")}, {QStringLiteral("color_index"), 0}, {QStringLiteral("talk_ms"), 9000}},
                                                                 QJsonObject{{QStringLiteral("speaker_id"), QStringLiteral("speaker_00")}, {QStringLiteral("name"), QStringLiteral("Mara")}, {QStringLiteral("color_index"), 1}, {QStringLiteral("talk_ms"), 7000}}}}});
    MeetingsModel meetings(&link);
    const QJsonArray items = link.answers.value(QStringLiteral("meetings.list")).value(QStringLiteral("items")).toArray();
    meetings.applyItems(items, today);
    QCOMPARE(meetings.rowCount(), 3);
    QCOMPARE(meetings.data(meetings.index(0), MeetingsModel::WeekRole).toString(), QStringLiteral("This week"));
    QCOMPARE(meetings.data(meetings.index(1), MeetingsModel::WeekRole).toString(), QStringLiteral("Last week"));
    QCOMPARE(meetings.data(meetings.index(2), MeetingsModel::WeekRole).toString(), QStringLiteral("Week of 26 Jan"));
    QCOMPARE(meetings.data(meetings.index(0), MeetingsModel::ChipRole).toString(), QStringLiteral("Analysed"));
    QCOMPARE(meetings.data(meetings.index(1), MeetingsModel::ChipRole).toString(), QStringLiteral("Partial"));
    QCOMPARE(meetings.data(meetings.index(1), MeetingsModel::ChipKindRole).toString(), QStringLiteral("partial"));
    QCOMPARE(meetings.data(meetings.index(2), MeetingsModel::ChipRole).toString(), QStringLiteral("Notes only"));
    QCOMPARE(meetings.data(meetings.index(0), MeetingsModel::LengthRole).toString(), QStringLiteral("42 min"));
    QCOMPARE(meetings.data(meetings.index(0), MeetingsModel::WhenRole).toString().left(3), QStringLiteral("Wed"));

    // The swatches come from meetings.get for the rows with speakers only.
    meetings.refresh();
    QCOMPARE(link.calls.count(QStringLiteral("meetings.get")), 1);
    QCOMPARE(link.lastParams.value(QStringLiteral("meetings.get")).value(QStringLiteral("meeting_id")).toString(), QStringLiteral("a"));
    const QVariantList swatches = meetings.data(meetings.index(0), MeetingsModel::SpeakersRole).toList();
    QCOMPARE(swatches.size(), 2);
    QCOMPARE(swatches.at(1).toMap().value(QStringLiteral("name")).toString(), QStringLiteral("Mara"));
    QCOMPARE(swatches.at(1).toMap().value(QStringLiteral("colorIndex")).toInt(), 1);

    // A transition refreshes; a removal drops the row without one.
    link.calls.clear();
    link.notify(QStringLiteral("meeting.state"), {{QStringLiteral("meeting_id"), QStringLiteral("b")}, {QStringLiteral("state"), QStringLiteral("completed")}});
    QCOMPARE(link.calls.count(QStringLiteral("meetings.list")), 1);
    meetings.remove(QStringLiteral("b"));
    QCOMPARE(meetings.rowCount(), 2);
    QCOMPARE(meetings.rowOf(QStringLiteral("c")), 1);

    // A search swaps the rows for the hits and back.
    link.answers.insert(QStringLiteral("meetings.search"),
                        {{QStringLiteral("items"), QJsonArray{QJsonObject{{QStringLiteral("ref"), QJsonObject{{QStringLiteral("id"), QStringLiteral("a")}, {QStringLiteral("kind"), QStringLiteral("meeting")}}},
                                                                          {QStringLiteral("title"), QStringLiteral("Roadmap review")},
                                                                          {QStringLiteral("started_at"), QStringLiteral("2026-02-11T10:00:00Z")},
                                                                          {QStringLiteral("snippet"), QStringLiteral("the budget")},
                                                                          {QStringLiteral("matched_field"), QStringLiteral("notes")}}}}});
    meetings.search(QStringLiteral("budget"));
    QVERIFY(meetings.searching());
    QCOMPARE(meetings.hitCount(), 1);
    QCOMPARE(meetings.data(meetings.index(0), MeetingsModel::MatchedFieldRole).toString(), QStringLiteral("notes"));
    meetings.search(QString());
    QCOMPARE(meetings.rowCount(), 3);
}

void AppMeetingsTest::liveSegmentsKeepEveryFragmentOfTheTailAndFinalsInOrder()
{
    LiveSegmentsModel segments;
    segments.setStartedAt(QStringLiteral("2026-02-13T16:00:00Z"));
    // One window's tail is several fragments; every one stays on screen
    // (the dogfood of fn-63 lost all but the last).
    segments.apply(segmentEvent(QStringLiteral("remote-p1"), QStringLiteral("remote"), 0, 900, QStringLiteral("ask not"), true));
    segments.apply(segmentEvent(QStringLiteral("remote-p2"), QStringLiteral("remote"), 1000, 1800, QStringLiteral("what your country"), true));
    segments.apply(segmentEvent(QStringLiteral("remote-p3"), QStringLiteral("remote"), 1900, 2600, QStringLiteral("can do"), true));
    QCOMPARE(segments.rowCount(), 3);
    QCOMPARE(segments.tailCount(QStringLiteral("remote")), 3);
    QCOMPARE(segments.data(segments.index(0), LiveSegmentsModel::TextRole).toString(), QStringLiteral("ask not"));
    QCOMPARE(segments.data(segments.index(2), LiveSegmentsModel::TextRole).toString(), QStringLiteral("can do"));
    QVERIFY(segments.data(segments.index(0), LiveSegmentsModel::ProvisionalRole).toBool());
    // The next window re-announces its tail from p1: the old fragments go,
    // a shorter tail stays shorter, and a repeated id replaces its fragment.
    segments.apply(segmentEvent(QStringLiteral("remote-p1"), QStringLiteral("remote"), 0, 1800, QStringLiteral("ask not what"), true));
    QCOMPARE(segments.rowCount(), 1);
    segments.apply(segmentEvent(QStringLiteral("remote-p2"), QStringLiteral("remote"), 1900, 2800, QStringLiteral("your country can"), true));
    segments.apply(segmentEvent(QStringLiteral("remote-p2"), QStringLiteral("remote"), 1900, 2900, QStringLiteral("your country can do"), true));
    QCOMPARE(segments.rowCount(), 2);
    QCOMPARE(segments.data(segments.index(1), LiveSegmentsModel::TextRole).toString(), QStringLiteral("your country can do"));
    // The other source keeps its own tail.
    segments.apply(segmentEvent(QStringLiteral("you-p1"), QStringLiteral("you"), 4000, 5000, QStringLiteral("yes"), true));
    QCOMPARE(segments.rowCount(), 3);
    // A final retires the fragments of its source it hardened out of and
    // leaves the other source's tail alone.
    segments.apply(segmentEvent(QStringLiteral("remote-1"), QStringLiteral("remote"), 0, 4200, QStringLiteral("ask not what your country can do"), false));
    QCOMPARE(segments.finalCount(), 1);
    QCOMPARE(segments.tailCount(QStringLiteral("remote")), 0);
    QCOMPARE(segments.rowCount(), 2);  // the final, then the microphone's tail
    QCOMPARE(segments.data(segments.index(0), LiveSegmentsModel::LabelRole).toString(), QStringLiteral("Remote"));
    QCOMPARE(segments.data(segments.index(1), LiveSegmentsModel::LabelRole).toString(), QStringLiteral("You"));
    // A fragment past the final's end is the tail the next window keeps.
    segments.apply(segmentEvent(QStringLiteral("remote-p1"), QStringLiteral("remote"), 4300, 5100, QStringLiteral("for you"), true));
    QCOMPARE(segments.rowCount(), 3);
    segments.apply(segmentEvent(QStringLiteral("remote-3"), QStringLiteral("remote"), 3000, 4250, QStringLiteral("late"), false));
    QCOMPARE(segments.tailCount(QStringLiteral("remote")), 1);
    segments.apply(segmentEvent(QStringLiteral("remote-4"), QStringLiteral("remote"), 4300, 5100, QStringLiteral("for you"), false));
    QCOMPARE(segments.tailCount(QStringLiteral("remote")), 0);
    QCOMPARE(segments.finalCount(), 3);
    QJsonObject gap = segmentEvent(QStringLiteral("you-1"), QStringLiteral("you"), 4000, 5000, QStringLiteral("yes"), false);
    gap.insert(QStringLiteral("gap_before_ms"), 1500);
    segments.apply(gap);
    QCOMPARE(segments.finalCount(), 4);
    QCOMPARE(segments.gapCount(), 1);
    QCOMPARE(segments.rowCount(), 4);
    // A later final that starts earlier lands before the one already there.
    segments.apply(segmentEvent(QStringLiteral("remote-2"), QStringLiteral("remote"), 2000, 2900, QStringLiteral("early"), false));
    QCOMPARE(segments.data(segments.index(1), LiveSegmentsModel::TextRole).toString(), QStringLiteral("early"));
    QCOMPARE(segments.data(segments.index(0), LiveSegmentsModel::TimeRole).toString().size(), 5);
    segments.clear();
    QCOMPARE(segments.rowCount(), 0);
}

void AppMeetingsTest::liveModelGatesOnDisclosureStartsAndStops()
{
    FakeLink link;
    link.answers.insert(QStringLiteral("meetings.disclosure.get"),
                        {{QStringLiteral("acknowledged"), false}, {QStringLiteral("acknowledged_at"), QJsonValue::Null}, {QStringLiteral("message"), QStringLiteral("May be recorded.")}});
    link.answers.insert(QStringLiteral("meetings.start"),
                        {{QStringLiteral("ref"), QJsonObject{{QStringLiteral("id"), QStringLiteral("m1")}, {QStringLiteral("kind"), QStringLiteral("meeting")}}},
                         {QStringLiteral("job"), QJsonObject{{QStringLiteral("job_id"), QStringLiteral("job_meeting_1")}}}});
    link.answers.insert(QStringLiteral("meetings.stop"),
                        {{QStringLiteral("ref"), QJsonObject{{QStringLiteral("id"), QStringLiteral("m1")}}},
                         {QStringLiteral("job"), QJsonObject{{QStringLiteral("job_id"), QStringLiteral("job_meeting_1")}, {QStringLiteral("message"), QStringLiteral("transcribing")}}}});
    link.answers.insert(QStringLiteral("meetings.notes.set"), {{QStringLiteral("updated_at"), QStringLiteral("2026-02-13T16:05:00Z")}});
    MeetingLiveModel live(&link);
    QSignalSpy gate(&live, &MeetingLiveModel::disclosureRequired);
    QSignalSpy started(&live, &MeetingLiveModel::started);
    QSignalSpy completed(&live, &MeetingLiveModel::completed);
    live.start(QStringLiteral("Kickoff"), true, QStringLiteral("whisper"), QStringLiteral("small"), 3, true, true);
    QCOMPARE(gate.size(), 1);
    QCOMPARE(gate.first().first().toString(), QStringLiteral("May be recorded."));
    QVERIFY(!link.calls.contains(QStringLiteral("meetings.start")));
    QCOMPARE(live.pendingParams().value(QStringLiteral("expected_speakers")).toInt(), 3);
    QCOMPARE(live.pendingParams().value(QStringLiteral("title")).toString(), QStringLiteral("Kickoff"));

    // Not now drops the start; a second start asks again and Acknowledge
    // and start puts the acknowledgement on the request.
    live.dismissStart();
    QVERIFY(!live.starting());
    live.start(QString(), false, QString(), QString(), 0, false);
    QCOMPARE(gate.size(), 2);
    live.acknowledgeAndStart();
    const QJsonObject params = link.lastParams.value(QStringLiteral("meetings.start"));
    QVERIFY(params.value(QStringLiteral("acknowledge_meeting_disclosure")).toBool());
    QVERIFY(!params.value(QStringLiteral("capture")).toObject().value(QStringLiteral("system_audio")).toBool());
    QVERIFY(!params.contains(QStringLiteral("title")));
    // An explicit choice is sent; an untouched one is left to the
    // configuration's defaults, never sent as an override.
    QCOMPARE(params.value(QStringLiteral("analyze")).toBool(true), false);
    QVERIFY2(!params.contains(QStringLiteral("diarize")), "the rail offers no diarize choice");
    QCOMPARE(started.size(), 1);
    QCOMPARE(live.meetingId(), QStringLiteral("m1"));
    QVERIFY(live.recording());
    QVERIFY(live.disclosureAcknowledged());

    // Levels arrive per source; segments for this meeting only.
    link.notify(QStringLiteral("audio.level"), {{QStringLiteral("rms"), 0.2}, {QStringLiteral("peak"), 0.5}, {QStringLiteral("source"), QStringLiteral("system")}});
    QVERIFY(live.systemLevel() > 0.5);
    QCOMPARE(live.micLevel(), 0.0);
    link.notify(QStringLiteral("meeting.segment"), segmentEvent(QStringLiteral("you-p1"), QStringLiteral("you"), 0, 900, QStringLiteral("hello"), true));
    QJsonObject other = segmentEvent(QStringLiteral("you-p1"), QStringLiteral("you"), 0, 900, QStringLiteral("elsewhere"), true);
    other.insert(QStringLiteral("meeting_id"), QStringLiteral("m2"));
    link.notify(QStringLiteral("meeting.segment"), other);
    QCOMPARE(live.segments()->rowCount(), 1);

    // Notes write after the debounce with source live; Stop flushes them.
    live.setNotes(QStringLiteral("## Decisions"));
    QVERIFY(!link.calls.contains(QStringLiteral("meetings.notes.set")));
    QCOMPARE(live.notesState(), QStringLiteral("Saving"));
    // The stop is acknowledged before the daemon answers: `finishing`
    // holds, the levels drop, and a second Stop has nothing to press.
    link.defer = true;
    live.stop();
    QVERIFY(live.finishing());
    QVERIFY(!live.recording());
    QCOMPARE(live.systemLevel(), 0.0);
    link.defer = false;
    link.answerPending();
    QCOMPARE(link.lastParams.value(QStringLiteral("meetings.notes.set")).value(QStringLiteral("source")).toString(), QStringLiteral("live"));
    QCOMPARE(live.notesState(), QStringLiteral("Saved"));
    QCOMPARE(live.state(), QStringLiteral("stopping"));
    link.notify(QStringLiteral("meeting.state"), {{QStringLiteral("meeting_id"), QStringLiteral("m1")}, {QStringLiteral("state"), QStringLiteral("transcribing")}});
    QVERIFY(live.finishing());
    link.notify(QStringLiteral("job.progress"), {{QStringLiteral("job_id"), QStringLiteral("job_meeting_1")}, {QStringLiteral("stage"), QStringLiteral("transcribing")}, {QStringLiteral("chunks_done"), 2}, {QStringLiteral("chunks_total"), 5}});
    QCOMPARE(live.chunksDone(), 2);
    QCOMPARE(live.chunksTotal(), 5);
    link.notify(QStringLiteral("meeting.state"), {{QStringLiteral("meeting_id"), QStringLiteral("m1")}, {QStringLiteral("state"), QStringLiteral("completed")}});
    QCOMPARE(completed.size(), 1);
    QCOMPARE(completed.first().first().toString(), QStringLiteral("m1"));
    QVERIFY(!live.active());

    // A refused stop releases the acknowledgement and names the reason.
    QSignalSpy failed(&live, &MeetingLiveModel::failed);
    link.answers[QStringLiteral("meetings.disclosure.get")].insert(QStringLiteral("acknowledged"), true);
    live.start(QString(), true, QString(), QString(), 0);
    QVERIFY(live.recording());
    link.errors.insert(QStringLiteral("meetings.stop"), {{QStringLiteral("message"), QStringLiteral("no meeting is recording")}});
    live.stop();
    QVERIFY(!live.finishing());
    QVERIFY(live.recording());
    QCOMPARE(failed.size(), 1);
    QCOMPARE(failed.first().first().toString(), QStringLiteral("stop"));
    QCOMPARE(live.error(), QStringLiteral("no meeting is recording"));
    link.errors.remove(QStringLiteral("meetings.stop"));
    link.notify(QStringLiteral("meeting.state"), {{QStringLiteral("meeting_id"), QStringLiteral("m1")}, {QStringLiteral("state"), QStringLiteral("cancelled")}});
    QVERIFY(!live.active());
    failed.clear();

    // A refused start names the gate.
    link.errors.insert(QStringLiteral("meetings.start"),
                       {{QStringLiteral("message"), QStringLiteral("Meeting disclosure acknowledgement required")},
                        {QStringLiteral("data"), QJsonObject{{QStringLiteral("details"), QJsonObject{{QStringLiteral("kind"), QStringLiteral("sessionActive")}}}}}});
    live.start(QString(), true, QString(), QString(), 0);
    QVERIFY(!link.lastParams.value(QStringLiteral("meetings.start")).contains(QStringLiteral("analyze")));
    QCOMPARE(failed.size(), 1);
    QVERIFY(live.error().contains(QStringLiteral("sessionActive")));
}

void AppMeetingsTest::liveModelUsesRecordedEngineAndCompletesWithQueuedPasses()
{
    FakeLink link;
    link.answers.insert("meetings.status", {{"status", "recording"}, {"capture", QJsonObject{{"duration_ms", 60000}}}});
    link.answers.insert("meetings.get", {{"stt_provider_id", "whisper"}, {"stt_model_id", "large-v3-turbo"}});
    MeetingLiveModel live(&link);
    live.attach("m1");
    QCOMPARE(live.engineLabel(), QStringLiteral("whisper large-v3-turbo"));
    QSignalSpy completed(&live, &MeetingLiveModel::completed);
    link.notify("meeting.state", {{"meeting_id", "m1"}, {"state", "transcribing"}});
    link.notify("meeting.state", {{"meeting_id", "m1"}, {"state", "completed"},
                                 {"previous_state", "transcribing"}, {"analysis_status", "queued"}, {"diarization_status", "queued"}});
    QCOMPARE(completed.size(), 1);
    QVERIFY(!live.active());
    QVERIFY(live.engineLabel().isEmpty());
    link.notify("meeting.state", {{"meeting_id", "m1"}, {"state", "completed"}, {"analysis_status", "ready"}});
    QCOMPARE(completed.size(), 1);

    // Late metadata from a prior meeting must not relabel its replacement.
    link.defer = true;
    live.attach("m2");
    link.answerPending();
    live.attach("m3");
    link.answerLastPending();
    link.answers["meetings.get"]["stt_model_id"] = "small";
    link.answerLastPending();
    QCOMPARE(live.engineLabel(), QStringLiteral("whisper small"));
    link.answers["meetings.get"]["stt_model_id"] = "tiny";
    link.answerPending();
    QCOMPARE(live.engineLabel(), QStringLiteral("whisper small"));
}

QTEST_GUILESS_MAIN(AppMeetingsTest)
#include "app_meetings_test.moc"
