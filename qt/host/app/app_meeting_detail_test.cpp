// The meeting detail, the actions and the formatting without a daemon or
// a window (fn-34, ADR 0038; fn-63, ADR 0061): the detail's facts, talk
// shares and speaker rename, its title after `meetings.rename` and the
// post-processing stages the strip draws (queued, running, done, failed,
// pending, with the automatic speaker pass's chunks adopted), the notes
// draft that outlives a failed, unanswered or interrupted save (ADR
// 0048), the actions' upload import, rename, retitle and delete under the
// configured policy, and the formatting the screens read. The list and
// the live model are app_meetings_test.cpp.
#include "config_binding.h"
#include "fake_link.h"
#include "meeting_detail_model.h"
#include "meeting_format.h"
#include "meeting_live_model.h"
#include "meetings_actions.h"
#include "meetings_model.h"
#include "sample_data.h"

#include <QJsonArray>
#include <QSignalSpy>
#include <QTemporaryDir>
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

}  // namespace

class AppMeetingDetailTest : public QObject {
    Q_OBJECT

private slots:
    void detailReadsFactsSharesAndRenames();
    void detailReadsTheProcessingStagesAndTheTitle();
    void detailKeepsTheNewestAnswerWhenReadsReturnOutOfOrder();
    void notesStayDirtyUntilTheDaemonAnswers();
    void actionsImportUploadRenameAndDelete();
    void formatReadsAsTheScreensDo();
};

void AppMeetingDetailTest::detailReadsFactsSharesAndRenames()
{
    FakeLink link;
    link.answers.insert(QStringLiteral("meetings.get"), sample::meetingDetail());
    link.answers.insert(QStringLiteral("meetings.notes.set"), {{QStringLiteral("updated_at"), QStringLiteral("2026-02-13T16:05:00Z")}});
    MeetingDetailModel detail(&link);
    detail.load(sample::meetingDetailId());
    QVERIFY(detail.loaded());
    QCOMPARE(detail.title(), QStringLiteral("Dettivo Linux kickoff"));
    QVERIFY(detail.factsLine().contains(QStringLiteral("41 min · whisper small · diarized")));
    QCOMPARE(detail.speakers().size(), 3);
    const QVariantMap you = detail.speakerAt(0);
    QCOMPARE(you.value(QStringLiteral("talk")).toString(), QStringLiteral("24:02"));
    QCOMPARE(you.value(QStringLiteral("shareText")).toString(), QStringLiteral("58 %"));
    QCOMPARE(detail.segments().size(), 7);
    QCOMPARE(detail.segments().at(1).toMap().value(QStringLiteral("speaker")).toString(), QStringLiteral("Mara"));
    QCOMPARE(detail.segments().at(1).toMap().value(QStringLiteral("colorIndex")).toInt(), 1);
    QVERIFY(detail.hasPolished());
    QCOMPARE(detail.decisions().size(), 4);
    QCOMPARE(detail.actionItems().first(), QStringLiteral("You: write the decisions into the spec, open the PR"));
    QCOMPARE(detail.notesMeta(), QStringLiteral("2 sections · edited 11:31"));
    QCOMPARE(detail.audioFacts(), QStringLiteral("2 tracks · 41 min · kept"));
    QVERIFY(detail.analysisMeta().startsWith(QStringLiteral("qwen3-4b-instruct-2507")));

    detail.applyRename(QStringLiteral("speaker_00"), QStringLiteral("Marta"));
    QCOMPARE(detail.speakerAt(1).value(QStringLiteral("name")).toString(), QStringLiteral("Marta"));
    QCOMPARE(detail.segments().at(1).toMap().value(QStringLiteral("speaker")).toString(), QStringLiteral("Marta"));

    detail.setNotes(QStringLiteral("## Open"));
    detail.flushNotes();
    QCOMPARE(link.lastParams.value(QStringLiteral("meetings.notes.set")).value(QStringLiteral("source")).toString(), QStringLiteral("user"));
    QCOMPARE(detail.notesState(), QStringLiteral("Saved"));

    // A job on the meeting is followed and the row re-read at its end.
    link.calls.clear();
    detail.trackJob(QStringLiteral("job_analysis_1"), QStringLiteral("analyzing"));
    QCOMPARE(detail.analysisStatus(), QStringLiteral("running"));
    link.notify(QStringLiteral("job.progress"), {{QStringLiteral("job_id"), QStringLiteral("job_analysis_1")}, {QStringLiteral("stage"), QStringLiteral("done")}, {QStringLiteral("progress"), 1.0}});
    QCOMPARE(link.calls.count(QStringLiteral("meetings.get")), 1);
    QCOMPARE(detail.analysisStatus(), QStringLiteral("ready"));
    QCOMPARE(detail.progress(), -1.0);

    // The tab a meeting reopens on; a meeting without a speaker pass
    // shows its two sources with their talk time.
    detail.setLastTab(QStringLiteral("notes"));
    QCOMPARE(detail.lastTab(), QStringLiteral("notes"));
    detail.setLastTab(QStringLiteral("lobby"));
    QCOMPARE(detail.lastTab(), QStringLiteral("transcript"));
    QJsonObject bare = sample::meetingDetail();
    bare.remove(QStringLiteral("speakers"));
    bare.remove(QStringLiteral("diarization"));
    detail.apply(sample::meetingDetailId(), bare);
    QCOMPARE(detail.speakers().size(), 2);
    QCOMPARE(detail.speakerAt(0).value(QStringLiteral("name")).toString(), QStringLiteral("You"));
    QCOMPARE(detail.speakerAt(1).value(QStringLiteral("name")).toString(), QStringLiteral("Remote"));
    QVERIFY(detail.diarizationLine().contains(QStringLiteral("by source")));
}

void AppMeetingDetailTest::detailReadsTheProcessingStagesAndTheTitle()
{
    FakeLink link;
    link.answers.insert(QStringLiteral("meetings.get"), sample::meetingDetail());
    MeetingDetailModel detail(&link);
    QSignalSpy stages(&detail, &MeetingDetailModel::stagesChanged);
    detail.load(sample::meetingDetailId());
    QVERIFY(stages.size() > 0);
    // The analysed sample: every stage done, nothing left to wait for.
    QCOMPARE(detail.stages().size(), 3);
    QCOMPARE(detail.stages().at(0).toMap().value(QStringLiteral("state")).toString(), QStringLiteral("done"));
    QCOMPARE(detail.stages().at(1).toMap().value(QStringLiteral("state")).toString(), QStringLiteral("done"));
    QCOMPARE(detail.stages().at(2).toMap().value(QStringLiteral("state")).toString(), QStringLiteral("done"));
    QCOMPARE(detail.processingState(), QStringLiteral("done"));
    QCOMPARE(detail.processingLine(), QStringLiteral("Processing complete"));
    QCOMPARE(detail.stages().at(1).toMap().value(QStringLiteral("detail")).toString(), QStringLiteral("3 speakers"));
    QVERIFY(detail.factsLine().contains(QStringLiteral("analysed")));

    // The pass leaves segments without a speaker_id labelled by source;
    // the ready stage counts them and the outcome stays on screen.
    {
        QJsonObject partlyAssigned = sample::meetingDetail();
        QJsonArray segments = partlyAssigned.value(QStringLiteral("segments")).toArray();
        for (int i = 0; i < segments.size(); ++i) {
            QJsonObject s = segments.at(i).toObject();
            if (i % 3 == 1) {
                s.remove(QStringLiteral("speaker_id"));
                s.remove(QStringLiteral("speaker"));
                s.remove(QStringLiteral("speaker_confidence"));
            }
            segments[i] = s;
        }
        partlyAssigned.insert(QStringLiteral("segments"), segments);
        detail.apply(sample::meetingDetailId(), partlyAssigned);
        QCOMPARE(detail.stages().at(1).toMap().value(QStringLiteral("state")).toString(), QStringLiteral("done"));
        QCOMPARE(detail.stages().at(1).toMap().value(QStringLiteral("detail")).toString(), QStringLiteral("3 speakers · 2 segments unassigned"));
        QCOMPARE(detail.processingState(), QStringLiteral("done"));
        // A pass switched off is not a failure and not a gap.
        partlyAssigned.insert(QStringLiteral("analysis_status"), QStringLiteral("none"));
        partlyAssigned.remove(QStringLiteral("analysis"));
        detail.apply(sample::meetingDetailId(), partlyAssigned);
        QCOMPARE(detail.stages().at(2).toMap().value(QStringLiteral("state")).toString(), QStringLiteral("skipped"));
        QCOMPARE(detail.processingState(), QStringLiteral("done"));
        QCOMPARE(detail.processingLine(), QStringLiteral("Processing complete · analysis not run"));
        // A wanted pass without a model is incomplete, and says so.
        partlyAssigned.insert(QStringLiteral("diarization"), QJsonObject{{QStringLiteral("status"), QStringLiteral("unavailable")}, {QStringLiteral("error"), QStringLiteral("run dettivo speech download --provider diarize --model diarization")}});
        partlyAssigned.remove(QStringLiteral("speakers"));
        detail.apply(sample::meetingDetailId(), partlyAssigned);
        QCOMPARE(detail.stages().at(1).toMap().value(QStringLiteral("state")).toString(), QStringLiteral("unavailable"));
        QCOMPARE(detail.processingState(), QStringLiteral("incomplete"));
        QCOMPARE(detail.processingLine(), QStringLiteral("Processing incomplete · speakers skipped, no speaker model downloaded"));
        detail.apply(sample::meetingDetailId(), sample::meetingDetail());
    }

    // Fresh from the stop: the transcript is on the row, both passes are
    // planned; the strip says what is ready and what comes next.
    QJsonObject row = sample::meetingDetail();
    row.remove(QStringLiteral("speakers"));
    row.insert(QStringLiteral("diarization"), QJsonObject{{QStringLiteral("status"), QStringLiteral("queued")}});
    row.insert(QStringLiteral("analysis_status"), QStringLiteral("queued"));
    row.remove(QStringLiteral("analysis"));
    link.answers.insert(QStringLiteral("meetings.get"), row);
    link.notify(QStringLiteral("meeting.state"), {{QStringLiteral("meeting_id"), sample::meetingDetailId()}, {QStringLiteral("state"), QStringLiteral("completed")}});
    QCOMPARE(detail.stages().at(1).toMap().value(QStringLiteral("state")).toString(), QStringLiteral("queued"));
    QCOMPARE(detail.stages().at(2).toMap().value(QStringLiteral("state")).toString(), QStringLiteral("queued"));
    QCOMPARE(detail.processingState(), QStringLiteral("running"));
    QCOMPARE(detail.processingLine(), QStringLiteral("Transcript ready · speakers queued"));
    QVERIFY(detail.diarizationLine().contains(QStringLiteral("starts next")));

    // The automatic speaker pass reports under a job the screen never
    // asked for; `meetings.status` names this meeting's job, and only
    // that job's chunks are followed (another meeting's pass is not).
    row.insert(QStringLiteral("diarization"), QJsonObject{{QStringLiteral("status"), QStringLiteral("running")}});
    link.answers.insert(QStringLiteral("meetings.get"), row);
    link.answers.insert(QStringLiteral("meetings.status"), {{QStringLiteral("status"), QStringLiteral("completed")}, {QStringLiteral("job"), QJsonObject{{QStringLiteral("job_id"), QStringLiteral("job_diarize_7")}, {QStringLiteral("state"), QStringLiteral("running")}}}});
    link.notify(QStringLiteral("meeting.state"), {{QStringLiteral("meeting_id"), sample::meetingDetailId()}, {QStringLiteral("state"), QStringLiteral("completed")}, {QStringLiteral("diarization_status"), QStringLiteral("running")}});
    QCOMPARE(link.lastParams.value(QStringLiteral("meetings.status")).value(QStringLiteral("meeting_id")).toString(), sample::meetingDetailId());
    QCOMPARE(detail.stages().at(1).toMap().value(QStringLiteral("state")).toString(), QStringLiteral("running"));
    QCOMPARE(detail.stages().at(1).toMap().value(QStringLiteral("progress")).toDouble(), -1.0);
    link.notify(QStringLiteral("job.progress"), {{QStringLiteral("job_id"), QStringLiteral("job_diarize_9")}, {QStringLiteral("stage"), QStringLiteral("diarizing")}, {QStringLiteral("progress"), 0.9}});
    QCOMPARE(detail.stages().at(1).toMap().value(QStringLiteral("progress")).toDouble(), -1.0);
    link.notify(QStringLiteral("job.progress"), {{QStringLiteral("job_id"), QStringLiteral("job_diarize_7")}, {QStringLiteral("stage"), QStringLiteral("diarizing")}, {QStringLiteral("progress"), 0.5}});
    QCOMPARE(detail.stages().at(1).toMap().value(QStringLiteral("progress")).toDouble(), 0.5);
    QCOMPARE(detail.processingLine(), QStringLiteral("Transcript ready · speakers running"));

    // Speakers ready, the analysis failed: settled, and not a success.
    row = sample::meetingDetail();
    row.remove(QStringLiteral("analysis"));
    row.insert(QStringLiteral("analysis_status"), QStringLiteral("failed"));
    row.insert(QStringLiteral("analysis_error"), QStringLiteral("invalid_output: no JSON object"));
    link.answers.insert(QStringLiteral("meetings.get"), row);
    link.notify(QStringLiteral("job.progress"), {{QStringLiteral("job_id"), QStringLiteral("job_diarize_7")}, {QStringLiteral("stage"), QStringLiteral("done")}, {QStringLiteral("progress"), 1.0}});
    QCOMPARE(detail.stages().at(1).toMap().value(QStringLiteral("state")).toString(), QStringLiteral("done"));
    QCOMPARE(detail.stages().at(2).toMap().value(QStringLiteral("state")).toString(), QStringLiteral("failed"));
    QCOMPARE(detail.stages().at(2).toMap().value(QStringLiteral("detail")).toString(), QStringLiteral("invalid_output: no JSON object"));
    QCOMPARE(detail.processingState(), QStringLiteral("failed"));
    QCOMPARE(detail.processingLine(), QStringLiteral("Processing finished · analysis failed"));

    // A meeting still transcribing: the passes wait for the transcript.
    row.insert(QStringLiteral("status"), QStringLiteral("transcribing"));
    row.insert(QStringLiteral("chunks_completed"), 18);
    row.insert(QStringLiteral("chunks_total"), 22);
    row.remove(QStringLiteral("diarization"));
    row.insert(QStringLiteral("analysis_status"), QStringLiteral("none"));
    detail.apply(sample::meetingDetailId(), row);
    QCOMPARE(detail.stages().at(0).toMap().value(QStringLiteral("state")).toString(), QStringLiteral("running"));
    QCOMPARE(detail.stages().at(0).toMap().value(QStringLiteral("detail")).toString(), QStringLiteral("18 of 22 chunks"));
    QCOMPARE(detail.stages().at(1).toMap().value(QStringLiteral("state")).toString(), QStringLiteral("pending"));
    QCOMPARE(detail.processingLine(), QStringLiteral("Transcript running"));

    // The title as the daemon stored it after a rename.
    QSignalSpy changed(&detail, &MeetingDetailModel::changed);
    detail.applyTitle(QStringLiteral("Kickoff, renamed"));
    QCOMPARE(detail.title(), QStringLiteral("Kickoff, renamed"));
    QCOMPARE(changed.size(), 1);
}

// Every transition reads the row again; two reads of one meeting can
// answer backwards, and the older answer must never put a running pass
// back over the terminal state the newer one reported.
void AppMeetingDetailTest::detailKeepsTheNewestAnswerWhenReadsReturnOutOfOrder()
{
    FakeLink link;
    QJsonObject running = sample::meetingDetail();
    running.remove(QStringLiteral("speakers"));
    running.insert(QStringLiteral("diarization"), QJsonObject{{QStringLiteral("status"), QStringLiteral("running")}});
    running.insert(QStringLiteral("analysis_status"), QStringLiteral("queued"));
    link.answers.insert(QStringLiteral("meetings.get"), running);
    MeetingDetailModel detail(&link);
    detail.load(sample::meetingDetailId());
    QCOMPARE(detail.diarizationStatus(), QStringLiteral("running"));

    link.defer = true;
    link.notify(QStringLiteral("meeting.state"), {{QStringLiteral("meeting_id"), sample::meetingDetailId()}, {QStringLiteral("state"), QStringLiteral("completed")}, {QStringLiteral("diarization_status"), QStringLiteral("running")}});
    link.notify(QStringLiteral("meeting.state"), {{QStringLiteral("meeting_id"), sample::meetingDetailId()}, {QStringLiteral("state"), QStringLiteral("completed")}, {QStringLiteral("diarization_status"), QStringLiteral("ready")}});
    QCOMPARE(link.pending.size(), 2);
    // The newer read answers first with the settled row.
    link.answers.insert(QStringLiteral("meetings.get"), sample::meetingDetail());
    link.answerLastPending();
    QCOMPARE(detail.diarizationStatus(), QStringLiteral("ready"));
    QCOMPARE(detail.processingState(), QStringLiteral("done"));
    // The older read answers last with the row as it was: dropped.
    link.answers.insert(QStringLiteral("meetings.get"), running);
    link.answerPending();
    QCOMPARE(detail.diarizationStatus(), QStringLiteral("ready"));
    QCOMPARE(detail.processingState(), QStringLiteral("done"));
    QCOMPARE(detail.speakers().size(), 3);

    // A switch to another meeting drops what the first one still owes.
    detail.reload();
    detail.load(QStringLiteral("other"));
    link.answers.insert(QStringLiteral("meetings.get"), running);
    link.answerPending();
    QCOMPARE(detail.meetingId(), QStringLiteral("other"));
    QCOMPARE(detail.diarizationStatus(), QStringLiteral("running"));
    link.defer = false;
}

void AppMeetingDetailTest::notesStayDirtyUntilTheDaemonAnswers()
{
    FakeLink link;
    QJsonObject row = sample::meetingDetail();
    row.insert(QStringLiteral("notes"), QStringLiteral("row notes"));
    link.answers.insert(QStringLiteral("meetings.get"), row);
    link.answers.insert(QStringLiteral("meetings.notes.set"), {{QStringLiteral("updated_at"), QStringLiteral("2026-02-13T16:05:00Z")}});
    MeetingDetailModel detail(&link);
    QSignalSpy failed(&detail, &MeetingDetailModel::failed);
    detail.load(sample::meetingDetailId());
    QCOMPARE(detail.notes(), QStringLiteral("row notes"));

    // A refused save keeps the draft; the next flush sends it again.
    link.errors.insert(QStringLiteral("meetings.notes.set"), {{QStringLiteral("message"), QStringLiteral("disk full")}});
    detail.setNotes(QStringLiteral("## Open"));
    detail.flushNotes();
    QCOMPARE(link.calls.count(QStringLiteral("meetings.notes.set")), 1);
    QCOMPARE(detail.notesState(), QStringLiteral("Not saved: disk full"));
    QCOMPARE(failed.size(), 1);
    link.errors.remove(QStringLiteral("meetings.notes.set"));
    detail.flushNotes();
    QCOMPARE(link.calls.count(QStringLiteral("meetings.notes.set")), 2);
    QCOMPARE(link.lastParams.value(QStringLiteral("meetings.notes.set")).value(QStringLiteral("markdown")).toString(), QStringLiteral("## Open"));
    QCOMPARE(detail.notesState(), QStringLiteral("Saved"));

    // A reload while a save is outstanding keeps the draft, an edit
    // during the save goes out after its answer, and the late answer
    // never marks the newer text saved.
    link.defer = true;
    detail.setNotes(QStringLiteral("## Open\n- a"));
    detail.flushNotes();
    link.defer = false;
    link.notify(QStringLiteral("meeting.state"), {{QStringLiteral("meeting_id"), sample::meetingDetailId()}, {QStringLiteral("state"), QStringLiteral("completed")}});
    QCOMPARE(detail.notes(), QStringLiteral("## Open\n- a"));
    detail.setNotes(QStringLiteral("## Open\n- a\n- b"));
    detail.flushNotes();
    QCOMPARE(link.calls.count(QStringLiteral("meetings.notes.set")), 3);
    QCOMPARE(detail.notesState(), QStringLiteral("Saving"));
    link.answerPending();
    QCOMPARE(link.calls.count(QStringLiteral("meetings.notes.set")), 4);
    QCOMPARE(link.lastParams.value(QStringLiteral("meetings.notes.set")).value(QStringLiteral("markdown")).toString(), QStringLiteral("## Open\n- a\n- b"));
    QCOMPARE(detail.notesState(), QStringLiteral("Saved"));

    // A draft that cannot be sent when the meeting is left waits for the
    // meeting, and goes out when the link is back.
    link.setConnected(false);
    detail.setNotes(QStringLiteral("## Parked"));
    detail.flushNotes();
    QCOMPARE(detail.notesState(), QStringLiteral("Not saved: the daemon is not connected"));
    detail.load(QStringLiteral("other"));
    QVERIFY(detail.notes().isEmpty());
    link.setConnected(true);
    QCOMPARE(link.lastParams.value(QStringLiteral("meetings.notes.set")).value(QStringLiteral("meeting_id")).toString(), sample::meetingDetailId());
    QCOMPARE(link.lastParams.value(QStringLiteral("meetings.notes.set")).value(QStringLiteral("markdown")).toString(), QStringLiteral("## Parked"));
    QVERIFY(!detail.notesState().startsWith(QStringLiteral("Not saved")));

    // The live model shares the mechanism: a refused save is retried.
    link.answers.insert(QStringLiteral("meetings.disclosure.get"), {{QStringLiteral("acknowledged"), true}});
    link.answers.insert(QStringLiteral("meetings.start"), {{QStringLiteral("ref"), QJsonObject{{QStringLiteral("id"), QStringLiteral("m1")}}}});
    MeetingLiveModel live(&link);
    live.start(QString(), false, QString(), QString(), 0, false);
    link.errors.insert(QStringLiteral("meetings.notes.set"), {{QStringLiteral("message"), QStringLiteral("busy")}});
    live.setNotes(QStringLiteral("## Live"));
    live.flushNotes();
    QCOMPARE(live.notesState(), QStringLiteral("Not saved: busy"));
    link.errors.remove(QStringLiteral("meetings.notes.set"));
    live.flushNotes();
    QCOMPARE(link.lastParams.value(QStringLiteral("meetings.notes.set")).value(QStringLiteral("source")).toString(), QStringLiteral("live"));
    QCOMPARE(live.notesState(), QStringLiteral("Saved"));
}

void AppMeetingDetailTest::actionsImportUploadRenameAndDelete()
{
    FakeLink link;
    ConfigBinding config(&link);
    config.applyEntries({QJsonObject{{QStringLiteral("key"), QStringLiteral("meetings.delete_artifact_policy")}, {QStringLiteral("value"), QStringLiteral("transcript_only")}, {QStringLiteral("source"), QStringLiteral("file")}}});
    link.answers.insert(QStringLiteral("config.get"), {{QStringLiteral("entries"), QJsonArray{QJsonObject{{QStringLiteral("key"), QStringLiteral("ipc.max_line_bytes")}, {QStringLiteral("value"), 1048576}}}}});
    link.answers.insert(QStringLiteral("transfer.begin"), {{QStringLiteral("transfer_id"), QStringLiteral("xfer_1")}, {QStringLiteral("chunk_max_bytes"), 1048576}});
    link.answers.insert(QStringLiteral("transfer.chunk"), {{QStringLiteral("accepted"), true}});
    link.answers.insert(QStringLiteral("transfer.commit"), {{QStringLiteral("committed"), true}});
    link.answers.insert(QStringLiteral("transcripts.import"),
                        {{QStringLiteral("ref"), QJsonObject{{QStringLiteral("id"), QStringLiteral("m9")}, {QStringLiteral("kind"), QStringLiteral("meeting")}}},
                         {QStringLiteral("job"), QJsonObject{{QStringLiteral("job_id"), QStringLiteral("job_import_1")}, {QStringLiteral("message"), QStringLiteral("transcribing")}}},
                         {QStringLiteral("is_partial"), false}});
    link.answers.insert(QStringLiteral("meetings.speakers.rename"),
                        {{QStringLiteral("speaker"), QJsonObject{{QStringLiteral("name"), QStringLiteral("Gordon")}, {QStringLiteral("speaker_id"), QStringLiteral("you")}}}, {QStringLiteral("segments_updated"), 3}});
    link.answers.insert(QStringLiteral("meetings.delete"), {{QStringLiteral("deleted"), true}});
    MeetingsActions actions(&link, &config);
    QCOMPARE(actions.deletePolicy(), QStringLiteral("transcript_only"));

    QTemporaryDir dir;
    const QString wav = dir.filePath(QStringLiteral("call.wav"));
    {
        QByteArray pcm(32000, '\0');
        QByteArray header = "RIFF";
        auto le32 = [](quint32 v) { QByteArray b(4, '\0'); for (int i = 0; i < 4; ++i) b[i] = char((v >> (8 * i)) & 0xff); return b; };
        auto le16 = [](quint16 v) { QByteArray b(2, '\0'); b[0] = char(v & 0xff); b[1] = char((v >> 8) & 0xff); return b; };
        header += le32(quint32(36 + pcm.size())) + "WAVEfmt " + le32(16) + le16(1) + le16(1) + le32(16000) + le32(32000) + le16(2) + le16(16) + "data" + le32(quint32(pcm.size()));
        QFile file(wav);
        QVERIFY(file.open(QIODevice::WriteOnly));
        file.write(header + pcm);
    }
    const QVariantMap facts = actions.probeFile(wav);
    QVERIFY(facts.value(QStringLiteral("ok")).toBool());
    QCOMPARE(facts.value(QStringLiteral("durationMs")).toLongLong(), 1000);
    QVERIFY(!actions.probeFile(dir.filePath(QStringLiteral("missing.wav"))).value(QStringLiteral("ok")).toBool());

    QSignalSpy imported(&actions, &MeetingsActions::importStarted);
    actions.importFile(wav, QStringLiteral("whisper"), QString(), QStringLiteral("en"), true, false);
    QTRY_COMPARE(imported.size(), 1);
    QCOMPARE(link.calls.count(QStringLiteral("transfer.chunk")), 1);
    const QJsonObject commit = link.lastParams.value(QStringLiteral("transfer.commit"));
    QCOMPARE(commit.value(QStringLiteral("total_chunks")).toInt(), 1);
    QCOMPARE(commit.value(QStringLiteral("sha256")).toString().size(), 64);
    const QJsonObject import = link.lastParams.value(QStringLiteral("transcripts.import"));
    QCOMPARE(import.value(QStringLiteral("target_kind")).toString(), QStringLiteral("meeting"));
    QCOMPARE(import.value(QStringLiteral("filename")).toString(), QStringLiteral("call.wav"));
    QCOMPARE(import.value(QStringLiteral("provider")).toString(), QStringLiteral("whisper"));
    QVERIFY(import.value(QStringLiteral("diarize")).toBool());
    QVERIFY(!import.value(QStringLiteral("analyze")).toBool());
    QVERIFY(import.value(QStringLiteral("acknowledge_meeting_disclosure")).toBool());
    QVERIFY(actions.importing());
    QSignalSpy ended(&actions, &MeetingsActions::importEnded);
    link.notify(QStringLiteral("job.progress"), {{QStringLiteral("job_id"), QStringLiteral("job_import_1")}, {QStringLiteral("stage"), QStringLiteral("transcribing")}, {QStringLiteral("progress"), 0.5}});
    QCOMPARE(actions.importProgress(), 0.5);
    link.notify(QStringLiteral("job.progress"), {{QStringLiteral("job_id"), QStringLiteral("job_import_1")}, {QStringLiteral("stage"), QStringLiteral("done")}, {QStringLiteral("progress"), 1.0}});
    QCOMPARE(ended.size(), 1);
    QVERIFY(!actions.importing());

    QSignalSpy renamed(&actions, &MeetingsActions::renamed);
    actions.rename(QStringLiteral("m1"), QStringLiteral("you"), QStringLiteral(" Gordon "));
    QCOMPARE(renamed.size(), 1);
    QCOMPARE(link.lastParams.value(QStringLiteral("meetings.speakers.rename")).value(QStringLiteral("name")).toString(), QStringLiteral("Gordon"));
    QCOMPARE(renamed.first().at(3).toInt(), 3);

    // The title goes to `meetings.rename` trimmed; an empty one is
    // refused on the spot; the list relabels its row without a reload.
    link.answers.insert(QStringLiteral("meetings.rename"),
                        {{QStringLiteral("ref"), QJsonObject{{QStringLiteral("id"), QStringLiteral("m1")}, {QStringLiteral("kind"), QStringLiteral("meeting")}}},
                         {QStringLiteral("title"), QStringLiteral("Roadmap review 2")}, {QStringLiteral("title_source"), QStringLiteral("manual")}});
    QSignalSpy retitled(&actions, &MeetingsActions::retitled);
    QSignalSpy refused(&actions, &MeetingsActions::failed);
    actions.retitle(QStringLiteral("m1"), QStringLiteral("  Roadmap review 2 "));
    QCOMPARE(retitled.size(), 1);
    QCOMPARE(link.lastParams.value(QStringLiteral("meetings.rename")).value(QStringLiteral("title")).toString(), QStringLiteral("Roadmap review 2"));
    QCOMPARE(retitled.first().at(1).toString(), QStringLiteral("Roadmap review 2"));
    link.calls.clear();
    actions.retitle(QStringLiteral("m1"), QStringLiteral("   "));
    QCOMPARE(refused.size(), 1);
    QCOMPARE(refused.first().first().toString(), QStringLiteral("retitle"));
    QVERIFY(!link.calls.contains(QStringLiteral("meetings.rename")));
    MeetingsModel meetings(&link);
    meetings.applyItems({listItem(QStringLiteral("m1"), QStringLiteral("Roadmap review"), QStringLiteral("2026-02-11T10:00:00Z"), QStringLiteral("ready"), true, false, 2)}, QDate(2026, 2, 13));
    QSignalSpy relabelled(&meetings, &MeetingsModel::dataChanged);
    meetings.retitle(QStringLiteral("m1"), QStringLiteral("Roadmap review 2"));
    QCOMPARE(meetings.data(meetings.index(0), MeetingsModel::TitleRole).toString(), QStringLiteral("Roadmap review 2"));
    QCOMPARE(relabelled.size(), 1);

    QSignalSpy removed(&actions, &MeetingsActions::removed);
    actions.remove(QStringLiteral("m1"), QString());
    QCOMPARE(removed.size(), 1);
    QCOMPARE(link.lastParams.value(QStringLiteral("meetings.delete")).value(QStringLiteral("artifact_policy")).toString(), QStringLiteral("transcript_only"));

    // The engine menu: a model of the selected provider writes the
    // meeting model; another provider goes through the selection.
    link.answers.insert(QStringLiteral("config.set"), {{QStringLiteral("key"), QStringLiteral("speech.meeting_model")}});
    link.answers.insert(QStringLiteral("speech.selection.set"), {{QStringLiteral("dictation"), QJsonObject{}}});
    actions.applyProviders({{QStringLiteral("providers"), QJsonArray{QJsonObject{{QStringLiteral("id"), QStringLiteral("whisper")}, {QStringLiteral("display_name"), QStringLiteral("Whisper")}, {QStringLiteral("supports_meetings"), true}, {QStringLiteral("models"), QJsonArray{}}}}}},
                           {{QStringLiteral("dictation"), QJsonObject{{QStringLiteral("provider_id"), QStringLiteral("whisper")}}}});
    actions.pickEngine(QStringLiteral("whisper"), QStringLiteral("small"));
    QCOMPARE(link.lastParams.value(QStringLiteral("config.set")).value(QStringLiteral("key")).toString(), QStringLiteral("speech.meeting_model"));
    QCOMPARE(actions.selectedModel(), QStringLiteral("small"));
    // The daemon on Parakeet (dictation-only, not offered): the picker
    // shows Whisper; picking it only changes the meeting model.
    link.calls.clear();
    actions.applyProviders({{QStringLiteral("providers"), QJsonArray{QJsonObject{{QStringLiteral("id"), QStringLiteral("whisper")}, {QStringLiteral("display_name"), QStringLiteral("Whisper")}, {QStringLiteral("supports_meetings"), true}, {QStringLiteral("models"), QJsonArray{}}},
                                                                     QJsonObject{{QStringLiteral("id"), QStringLiteral("parakeet")}, {QStringLiteral("display_name"), QStringLiteral("Parakeet")}, {QStringLiteral("supports_meetings"), false}, {QStringLiteral("models"), QJsonArray{}}}}}},
                           {{QStringLiteral("dictation"), QJsonObject{{QStringLiteral("provider_id"), QStringLiteral("parakeet")}}}});
    QCOMPARE(actions.selectedProvider(), QStringLiteral("whisper"));
    actions.pickEngine(QStringLiteral("whisper"), QStringLiteral("small"));
    QVERIFY(!link.calls.contains(QStringLiteral("speech.selection.set")));
    QCOMPARE(link.lastParams.value(QStringLiteral("config.set")).value(QStringLiteral("key")).toString(), QStringLiteral("speech.meeting_model"));
    QCOMPARE(actions.selectedProvider(), QStringLiteral("whisper"));

    QSignalSpy failed(&actions, &MeetingsActions::failed);
    link.errors.insert(QStringLiteral("meetings.delete"),
                       {{QStringLiteral("message"), QStringLiteral("a job is still using the meeting")},
                        {QStringLiteral("data"), QJsonObject{{QStringLiteral("details"), QJsonObject{{QStringLiteral("kind"), QStringLiteral("jobRunning")}}}}}});
    actions.remove(QStringLiteral("m1"), QStringLiteral("all"));
    QCOMPARE(failed.size(), 1);
    QCOMPARE(failed.first().at(1).toString(), QStringLiteral("a job is still using the meeting (jobRunning)"));
}

void AppMeetingDetailTest::formatReadsAsTheScreensDo()
{
    QCOMPARE(meeting_format::minutes(41 * 60000), QStringLiteral("41 min"));
    QCOMPARE(meeting_format::minutes(64 * 60000), QStringLiteral("64 min"));
    QCOMPARE(meeting_format::minutes(125 * 60000), QStringLiteral("2 h 5 min"));
    QCOMPARE(meeting_format::minutes(30000), QStringLiteral("< 1 min"));
    QCOMPARE(meeting_format::elapsed(23 * 60000 + 41000), QStringLiteral("00:23:41"));
    QCOMPARE(meeting_format::talk(1442000), QStringLiteral("24:02"));
    QCOMPARE(meeting_format::talk(3723000), QStringLiteral("1:02:03"));
    QCOMPARE(meeting_format::percent(0.583), QStringLiteral("58 %"));
    QCOMPARE(meeting_format::clockAt(QString(), 65000), QStringLiteral("01:05"));
    QCOMPARE(meeting_format::weekHeading(QStringLiteral("2026-02-11T10:00:00Z"), QDate(2026, 2, 13)), QStringLiteral("This week"));
    QCOMPARE(meeting_format::weekHeading(QStringLiteral("2026-02-08T10:00:00Z"), QDate(2026, 2, 9)), QStringLiteral("Last week"));
    QCOMPARE(meeting_format::chip(QStringLiteral("completed"), false, QStringLiteral("none"), false, true), QStringLiteral("Transcript"));
    QCOMPARE(meeting_format::chip(QStringLiteral("recording"), false, QStringLiteral("none"), false, false), QStringLiteral("Recording"));
    QCOMPARE(meeting_format::chipKind(QStringLiteral("failed"), false, QStringLiteral("none"), false), QStringLiteral("failed"));
    QCOMPARE(meeting_format::sourceLabel(QStringLiteral("you")), QStringLiteral("You"));
    QCOMPARE(meeting_format::sourceLabel(QStringLiteral("remote")), QStringLiteral("Remote"));
}

QTEST_GUILESS_MAIN(AppMeetingDetailTest)
#include "app_meeting_detail_test.moc"
