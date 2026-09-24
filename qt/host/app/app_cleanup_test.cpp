#include "fake_link.h"
#include "history_actions.h"
#include "history_player.h"
#include "meeting_live_model.h"
#include "meeting_format.h"
#include "meetings_actions.h"
#include "meetings_model.h"

#include <QFile>
#include <QJsonArray>
#include <QJsonDocument>
#include <QSignalSpy>
#include <QTemporaryDir>
#include <QTest>
#include <QtEndian>

using namespace dettivo;
using dettivo::test::FakeLink;

namespace {
QByteArray le32(quint32 value)
{
    QByteArray bytes(4, '\0');
    qToLittleEndian(value, bytes.data());
    return bytes;
}
QByteArray wave(const QByteArray &chunks)
{
    return "RIFF" + le32(quint32(chunks.size() + 4)) + "WAVE" + chunks;
}
QByteArray format()
{
    return "fmt " + le32(16) + QByteArray::fromHex("01000100803e0000007d000002001000");
}
}

class AppCleanupTest : public QObject {
    Q_OBJECT
private slots:
    void boundedWavMetadata_data()
    {
        QTest::addColumn<QByteArray>("bytes");
        QTest::addColumn<qint64>("duration");
        const QByteArray data = "data" + le32(32000) + QByteArray(32000, '\0');
        QTest::newRow("padded-unknown") << wave("JUNK" + le32(3) + QByteArray("abc\0", 4) + format() + data) << qint64(1000);
        QTest::newRow("truncated-header") << QByteArray("RIFF") << qint64(-1);
        QTest::newRow("truncated-data") << wave(format() + "data" + le32(32000) + QByteArray(2, '\0')) << qint64(-1);
        QTest::newRow("short-format") << wave("fmt " + le32(4) + QByteArray(4, '\0') + data) << qint64(-1);
        QTest::newRow("oversized") << wave("JUNK" + le32(0xffffffff) + format() + data) << qint64(-1);
        QTest::newRow("zero-progress") << wave("JUNK" + le32(0xfffffff8) + format() + data) << qint64(-1);
    }
    void boundedWavMetadata()
    {
        QFETCH(QByteArray, bytes);
        QFETCH(qint64, duration);
        QTemporaryDir dir;
        QFile file(dir.filePath("audio.wav"));
        QVERIFY(file.open(QIODevice::WriteOnly));
        QCOMPARE(file.write(bytes), bytes.size());
        file.close();
        MeetingsActions actions(nullptr, nullptr);
        QCOMPARE(actions.probeFile(file.fileName()).value("durationMs").toLongLong(), duration);
        QVariantList peaks;
        int rate = 0;
        qint64 measured = -1;
        QString error;
        QCOMPARE(HistoryPlayer::readWav(file.fileName(), &peaks, &rate, &measured, &error), duration >= 0);
        if (duration >= 0)
            QCOMPARE(measured, duration);
    }
    void emptyNotesOverrideIsPresent_data()
    {
        QTest::addColumn<QString>("notes");
        QTest::newRow("empty") << QStringLiteral("");
        QTest::newRow("whitespace") << QStringLiteral(" \n");
        QTest::newRow("text") << QStringLiteral("edited notes");
    }
    void emptyNotesOverrideIsPresent()
    {
        QFETCH(QString, notes);
        FakeLink link;
        link.answers.insert("transfer.begin", {{"transfer_id", "t1"}});
        HistoryActions actions(&link, {});
        actions.exportMeeting("m1", "md", false, notes);
        const auto params = link.lastParams.value("transcripts.export");
        QVERIFY(params.contains("notes_override"));
        QCOMPARE(params.value("notes_override").toString(), notes);
    }
    void absentNotesOverrideUsesSavedNotes()
    {
        FakeLink link;
        link.answers.insert("transfer.begin", {{"transfer_id", "t1"}});
        HistoryActions actions(&link, {});
        actions.exportMeeting("m1", "md", false);
        QVERIFY(!link.lastParams.value("transcripts.export").contains("notes_override"));
    }
    void discoversRecordingAfterSubscriptionAndReconcilesOverflow()
    {
        FakeLink link;
        link.setConnected(false);
        MeetingLiveModel live(&link);
        link.answers.insert("meetings.list", {{"items", QJsonArray{QJsonObject{{"ref", QJsonObject{{"id", "m1"}}}, {"status", "recording"}}}}});
        link.answers.insert("meetings.status", {{"status", "recording"}, {"capture", QJsonObject{{"duration_ms", 1000}}}});
        link.setConnected(true);
        QVERIFY(!live.active());
        QMetaObject::invokeMethod(&link, "subscriptionReady", Qt::DirectConnection);
        QVERIFY(live.recording());
        QCOMPARE(live.meetingId(), QStringLiteral("m1"));
        link.setConnected(false);
        link.setConnected(true);
        QMetaObject::invokeMethod(&link, "subscriptionReady", Qt::DirectConnection);
        QVERIFY(live.recording());
        QSignalSpy completed(&live, &MeetingLiveModel::completed);
        link.answers.insert("meetings.status", {{"status", "completed"}});
        link.answers.insert("meetings.list", {{"items", QJsonArray()}});
        emit link.overflow(1);
        QVERIFY(!live.active());
        QCOMPARE(completed.size(), 1);
    }
    void discoveryPagesAndDiscardsAnObsoleteResponse()
    {
        FakeLink link;
        link.defer = true;
        MeetingLiveModel live(&link);
        emit link.subscriptionReady();
        link.answers.insert("meetings.list", {{"items", QJsonArray()}, {"next_cursor", "older"}});
        link.answerPending();
        QCOMPARE(link.lastParams.value("meetings.list").value("cursor").toString(), QStringLiteral("older"));
        link.answers.insert("meetings.list", {{"items", QJsonArray{QJsonObject{{"ref", QJsonObject{{"id", "m1"}}}, {"status", "recording"}}}}});
        link.answerPending();
        QCOMPARE(link.lastParams.value("meetings.status").value("meeting_id").toString(), QStringLiteral("m1"));
        link.answers.insert("meetings.status", {{"status", "recording"}});
        link.answerPending();
        QVERIFY(live.recording());
        emit link.overflow(1);
        link.notify("meeting.state", {{"meeting_id", "m1"}, {"state", "completed"}});
        QVERIFY(!live.active());
        link.answerPending();
        QVERIFY(!live.active());
    }
    void completionDuringInitialAttachDoesNotResurrectRecording()
    {
        FakeLink link;
        link.defer = true;
        MeetingLiveModel live(&link);
        live.attach("m1");
        link.notify("meeting.state", {{"meeting_id", "m1"}, {"state", "completed"}});
        link.answers.insert("meetings.status", {{"status", "recording"}});
        link.answerPending();
        QVERIFY(!live.active());
    }
    void recoveryAvailabilityComesFromTheDaemon()
    {
        FakeLink link;
        MeetingsModel model(&link);
        const QJsonObject capture{{"ref", QJsonObject{{"id", "capture"}}}, {"status", "failed"}, {"has_notes", true}};
        const QJsonObject imported{{"ref", QJsonObject{{"id", "import"}}}, {"status", "cancelled"}, {"has_notes", true}};
        const QJsonObject missing{{"ref", QJsonObject{{"id", "missing"}}}, {"status", "failed"}};
        link.answers.insert("meetings.list", {{"items", QJsonArray{capture, imported, missing}}});
        link.answers.insert("meetings.status", {{"recoverable", QJsonArray{capture, imported}}});
        model.refresh();
        const int role = model.roleNames().key("recoverable", -1);
        QVERIFY(model.data(model.index(0), role).toBool());
        QVERIFY(model.data(model.index(1), role).toBool());
        QVERIFY(!model.data(model.index(2), role).toBool());
        link.errors.insert("meetings.status", {{"message", "cannot validate audio"}});
        model.refresh();
        QVERIFY(!model.data(model.index(0), role).toBool());
        QVERIFY(!model.data(model.index(1), role).toBool());
        QVERIFY(model.data(model.index(0), MeetingsModel::NotesRole).toBool());
        QCOMPARE(model.idAt(0), QStringLiteral("capture"));
        link.errors.remove("meetings.status");
        link.answers.insert("meetings.search", {{"items", QJsonArray{QJsonObject{{"ref", QJsonObject{{"id", "import"}}}, {"snippet", "saved notes"}}}}});
        model.search("notes");
        QCOMPARE(model.idAt(0), QStringLiteral("import"));
        QVERIFY(model.data(model.index(0), role).toBool());
    }

    void cancelAndRecoverKeepTheOriginalMeetingReference()
    {
        QFile file(QFINDTESTDATA("../../../crates/dettivo-proto/fixtures/meetings/cancel.json"));
        QVERIFY(file.open(QIODevice::ReadOnly));
        const auto fixture = QJsonDocument::fromJson(file.readAll()).object();
        const auto params = fixture.value("request").toObject().value("params").toObject();
        const auto result = fixture.value("response").toObject().value("result").toObject();
        const QString id = params.value("meeting_id").toString();
        QVERIFY(!id.isEmpty());
        FakeLink link;
        MeetingsActions actions(&link, nullptr);
        QSignalSpy failed(&actions, &MeetingsActions::failed);
        QSignalSpy cancelled(&actions, &MeetingsActions::cancelled);
        link.answers.insert("meetings.cancel", result);
        actions.cancel(id);
        QCOMPARE(link.lastParams.value("meetings.cancel"), params);
        QCOMPARE(cancelled.size(), 1);
        QCOMPARE(cancelled.first().at(0).toString(), id);
        auto unfinished = result;
        auto job = result.value("job").toObject();
        job.insert("state", "running");
        unfinished.insert("job", job);
        link.answers.insert("meetings.cancel", unfinished);
        actions.cancel(id);
        QCOMPARE(cancelled.size(), 1);
        QCOMPARE(failed.size(), 1);
        auto wrongMeeting = result;
        wrongMeeting.insert("ref", QJsonObject{{"id", "another-meeting"}, {"kind", "meeting"}});
        link.answers.insert("meetings.cancel", wrongMeeting);
        actions.cancel(id);
        QCOMPARE(cancelled.size(), 1);
        QCOMPARE(failed.size(), 2);
        link.errors.insert("meetings.cancel", {{"message", "not running"}});
        actions.cancel(id);
        QCOMPARE(failed.size(), 3);
        QCOMPARE(failed.first().at(0).toString(), QStringLiteral("cancel"));
        QFile recoveryFile(QFINDTESTDATA("../../../crates/dettivo-proto/fixtures/meetings/recover.json"));
        QVERIFY(recoveryFile.open(QIODevice::ReadOnly));
        const auto recoveryFixture = QJsonDocument::fromJson(recoveryFile.readAll()).object();
        const auto recoveryParams = recoveryFixture.value("request").toObject().value("params").toObject();
        QCOMPARE(recoveryParams, params);
        link.answers.insert("meetings.recover", recoveryFixture.value("response").toObject().value("result").toObject());
        QSignalSpy recovered(&actions, &MeetingsActions::recovered);
        actions.recover(id);
        QCOMPARE(link.lastParams.value("meetings.recover"), recoveryParams);
        QCOMPARE(recovered.size(), 1);
        QCOMPARE(recovered.first().at(0).toString(), id);
        QVERIFY(!link.calls.contains("meetings.notes.set"));
        QVERIFY(!link.calls.contains("transcripts.import"));
    }
    void terminalImportProgressRefreshesRecoveryOnTheSameScreen_data()
    {
        QTest::addColumn<QString>("stage");
        QTest::newRow("cancelled") << QStringLiteral("cancelled");
        QTest::newRow("failed") << QStringLiteral("failed");
        QTest::newRow("done") << QStringLiteral("done");
    }
    void terminalImportProgressRefreshesRecoveryOnTheSameScreen()
    {
        QFETCH(QString, stage);
        FakeLink link;
        MeetingsModel model(&link);
        QJsonObject row{{"ref", QJsonObject{{"id", "import"}, {"kind", "meeting"}}}, {"status", "transcribing"}, {"has_notes", true}};
        link.answers.insert("meetings.list", {{"items", QJsonArray{row}}});
        link.answers.insert("meetings.status", {{"recoverable", QJsonArray()}});
        model.refresh();
        QCOMPARE(model.data(model.index(0), MeetingsModel::StatusRole).toString(), QStringLiteral("transcribing"));
        QVERIFY(!model.data(model.index(0), MeetingsModel::RecoverableRole).toBool());
        const int reads = link.calls.count("meetings.list");
        link.notify("job.progress", {{"job_id", "job_import"}, {"stage", "transcribing"}, {"progress", 0.5}});
        QCOMPARE(link.calls.count("meetings.list"), reads);
        const bool recoverable = stage != QStringLiteral("done");
        row.insert("status", recoverable ? stage : QStringLiteral("completed"));
        link.answers.insert("meetings.list", {{"items", QJsonArray{row}}});
        link.answers.insert("meetings.status", {{"recoverable", recoverable ? QJsonArray{row} : QJsonArray()}});
        link.notify("job.progress", {{"job_id", "job_import"}, {"stage", stage}, {"progress", 1.0}});
        QCOMPARE(link.calls.count("meetings.list"), reads + 1);
        QCOMPARE(model.data(model.index(0), MeetingsModel::StatusRole).toString(), row.value("status").toString());
        QCOMPARE(model.data(model.index(0), MeetingsModel::RecoverableRole).toBool(), recoverable);
        QVERIFY(model.data(model.index(0), MeetingsModel::NotesRole).toBool());
        QCOMPARE(model.idAt(0), QStringLiteral("import"));
    }
    void recoverableTerminalChipsNameTheActualState_data()
    {
        QTest::addColumn<QString>("status");
        QTest::addColumn<QString>("label");
        QTest::addColumn<bool>("partial");
        QTest::newRow("cancelled-import") << QStringLiteral("cancelled") << QStringLiteral("Cancelled") << false;
        QTest::newRow("cancelled-recovered-capture") << QStringLiteral("cancelled") << QStringLiteral("Cancelled") << true;
        QTest::newRow("stopped-capture") << QStringLiteral("stopped") << QStringLiteral("Stopped") << false;
        QTest::newRow("stopped-recovered-capture") << QStringLiteral("stopped") << QStringLiteral("Stopped") << true;
    }
    void recoverableTerminalChipsNameTheActualState()
    {
        QFETCH(QString, status);
        QFETCH(QString, label);
        QFETCH(bool, partial);
        QCOMPARE(meeting_format::chip(status, partial, "none", true, true), label);
        QCOMPARE(meeting_format::chipKind(status, partial, "none", true), QStringLiteral("plain"));
    }
};

QTEST_MAIN(AppCleanupTest)
#include "app_cleanup_test.moc"
