#include "fake_link.h"
#include "meetings_actions.h"

#include <QCryptographicHash>
#include <QFile>
#include <QJsonDocument>
#include <QJsonArray>
#include <QSignalSpy>
#include <QTemporaryDir>
#include <QTest>
#include <QTimer>

using namespace dettivo;
using dettivo::test::FakeLink;

class UploadLink : public FakeLink {
public:
    UploadLink()
    {
        answers.insert("transfer.begin", {{"transfer_id", "upload"}, {"chunk_max_bytes", 1048576}});
        answers.insert("config.get", {{"entries", QJsonArray{QJsonObject{{"key", "ipc.max_line_bytes"}, {"value", 1048576}}}}});
        answers.insert("transfer.chunk", {{"accepted", true}});
        answers.insert("transfer.commit", {{"committed", true}});
        answers.insert("transcripts.import", {{"ref", QJsonObject{{"id", "meeting"}}}, {"job", QJsonObject{{"job_id", "job"}}}});
    }
    qint64 requestBytes(const QString &method, const QJsonObject &params) const override
    {
        return encodeRequest(method, params, QStringLiteral("18446744073709551615"), QString(tokenBytes, 'a')).size();
    }
    void call(const QString &method, const QJsonObject &params, Reply reply) override
    {
        if ((method == "transcripts.import" && holdImport) || (method == "transfer.begin" && holdBegin)) {
            calls.append(method);
            held = std::move(reply);
            return;
        }
        if (method == "transfer.chunk") {
            const qint64 limit = answers["config.get"].value("entries").toArray().first().toObject().value("value").toInteger();
            if (requestBytes(method, params) > limit) {
                calls.append(method);
                reply({}, {{"message", "IPC line too large"}});
                return;
            }
            const QByteArray bytes = QByteArray::fromBase64(params.value("data_b64").toString().toLatin1());
            maxChunk = qMax(maxChunk, bytes.size());
            hash.addData(bytes);
            sequences.append(params.value("seq").toInt());
            if (onChunk)
                onChunk();
            if (holdChunk) {
                calls.append(method);
                held = std::move(reply);
                return;
            }
        }
        FakeLink::call(method, params, std::move(reply));
    }
    QCryptographicHash hash{QCryptographicHash::Sha256};
    QList<int> sequences;
    qsizetype maxChunk = 0;
    int tokenBytes = 64;
    std::function<void()> onChunk;
    bool holdChunk = false, holdImport = false, holdBegin = false;
    Reply held;
};

class MeetingsUploadTest : public QObject {
    Q_OBJECT
private slots:
    void largeUploadIsAsynchronousBoundedAndOrdered();
    void changedSourceCancels_data();
    void changedSourceCancels();
    void transferRefusalCancels();
    void encodedChunksFitTheDefaultIpcLine_data();
    void encodedChunksFitTheDefaultIpcLine();
    void destructionCancelsPendingUpload();
    void cancelButtonStopsTheActiveTransfer_data();
    void cancelButtonStopsTheActiveTransfer();
    void cancellationDuringImportCancelsOnlyItsAcceptedMeeting_data();
    void cancellationDuringImportCancelsOnlyItsAcceptedMeeting();
    void readFailureIsNotAnEmptySuccessfulUpload();
};

static void writeAudio(const QString &path, qint64 bytes)
{
    QFile file(path);
    QVERIFY(file.open(QIODevice::WriteOnly));
    const QByteArray block(1024 * 1024, 'x');
    while (bytes > 0) {
        const qint64 count = qMin(bytes, qint64(block.size()));
        QCOMPARE(file.write(block.constData(), count), count);
        bytes -= count;
    }
}

void MeetingsUploadTest::largeUploadIsAsynchronousBoundedAndOrdered()
{
    QTemporaryDir dir;
    const QString path = dir.filePath("large.wav");
    writeAudio(path, 16 * MeetingsActions::kChunkBytes + 7);
    UploadLink link;
    MeetingsActions actions(&link, nullptr);
    QSignalSpy imported(&actions, &MeetingsActions::importStarted);
    QSignalSpy failed(&actions, &MeetingsActions::failed);
    int ticks = 0;
    QTimer timer;
    connect(&timer, &QTimer::timeout, [&]() { ++ticks; });
    timer.start(0);
    actions.importFile(path, "whisper", "tiny.en", "en", true, false);
    QVERIFY(actions.busy());
    QVERIFY(!link.calls.contains("transfer.begin"));
    QTRY_COMPARE(imported.size(), 1);
    QCOMPARE(failed.size(), 0);
    QVERIFY(ticks > 0);
    QVERIFY(link.maxChunk > 0 && link.maxChunk < MeetingsActions::kChunkBytes);
    QCOMPARE(link.sequences.size(), (16 * MeetingsActions::kChunkBytes + 7 + link.maxChunk - 1) / link.maxChunk);
    for (int i = 0; i < link.sequences.size(); ++i)
        QCOMPARE(link.sequences[i], i + 1);
    QCOMPARE(link.lastParams["transfer.commit"].value("sha256").toString(), QString::fromLatin1(link.hash.result().toHex()));
}

void MeetingsUploadTest::changedSourceCancels_data()
{
    QTest::addColumn<QString>("change");
    for (const auto *change : {"truncate", "rewrite", "replace", "remove"})
        QTest::newRow(change) << QString::fromLatin1(change);
}

void MeetingsUploadTest::changedSourceCancels()
{
    QFETCH(QString, change);
    QTemporaryDir dir;
    const QString path = dir.filePath("mutable.wav");
    writeAudio(path, 2 * MeetingsActions::kChunkBytes);
    UploadLink link;
    bool changed = false;
    link.onChunk = [&]() {
        if (changed)
            return;
        changed = true;
        if (change == "replace" || change == "remove") {
            QVERIFY(QFile::remove(path));
            if (change == "replace")
                writeAudio(path, 2 * MeetingsActions::kChunkBytes);
        } else {
            QFile file(path);
            QVERIFY(file.open(QIODevice::ReadWrite));
            if (change == "truncate")
                QVERIFY(file.resize(10));
            else
                QCOMPARE(file.write("y"), 1);
        }
    };
    MeetingsActions actions(&link, nullptr);
    QSignalSpy failed(&actions, &MeetingsActions::failed);
    actions.importFile(path, {}, {}, {}, false, false);
    QTRY_COMPARE(failed.size(), 1);
    QVERIFY(link.calls.contains("transfer.cancel"));
    QVERIFY(!link.calls.contains("transfer.commit"));
    QVERIFY(!link.calls.contains("transcripts.import"));
    QVERIFY(!actions.busy());
}

void MeetingsUploadTest::encodedChunksFitTheDefaultIpcLine_data()
{
    QTest::addColumn<int>("lineBytes");
    QTest::addColumn<int>("rawBytes");
    QTest::addColumn<int>("tokenBytes");
    QTest::addColumn<bool>("fits");
    QTest::newRow("default") << 1048576 << 1048576 << 64 << true;
    QTest::newRow("small-line-large-auth") << 8192 << 1048576 << 1024 << true;
    QTest::newRow("small-transfer-cap") << 1048576 << 4096 << 64 << true;
    QTest::newRow("no-envelope-space") << 1024 << 1048576 << 2048 << false;
}

void MeetingsUploadTest::encodedChunksFitTheDefaultIpcLine()
{
    QFETCH(int, lineBytes);
    QFETCH(int, rawBytes);
    QFETCH(int, tokenBytes);
    QFETCH(bool, fits);
    QTemporaryDir dir;
    const QString path = dir.filePath("line-limit.wav");
    writeAudio(path, lineBytes == 1048576 && rawBytes == 1048576 ? 2 * MeetingsActions::kChunkBytes : 32768);
    UploadLink link;
    link.tokenBytes = tokenBytes;
    link.answers["config.get"] = {{"entries", QJsonArray{QJsonObject{{"key", "ipc.max_line_bytes"}, {"value", lineBytes}}}}};
    link.answers["transfer.begin"] = {{"transfer_id", "upload"}, {"chunk_max_bytes", rawBytes}};
    qint64 largest = 0;
    link.onChunk = [&]() {
        const QJsonObject params{{"transfer_id", "upload"}, {"seq", 1},
            {"data_b64", QString::fromLatin1(QByteArray(link.maxChunk, 'x').toBase64())}};
        largest = qMax(largest, link.requestBytes("transfer.chunk", params));
    };
    MeetingsActions actions(&link, nullptr);
    QSignalSpy imported(&actions, &MeetingsActions::importStarted);
    QSignalSpy failed(&actions, &MeetingsActions::failed);
    actions.importFile(path, {}, {}, {}, false, false);
    if (fits) {
        QTRY_COMPARE(imported.size(), 1);
        QCOMPARE(failed.size(), 0);
        QVERIFY(largest > 0 && largest <= lineBytes);
        QVERIFY(link.maxChunk <= rawBytes);
    } else {
        QTRY_COMPARE(failed.size(), 1);
        QVERIFY(link.calls.contains("transfer.cancel"));
        QVERIFY(!link.calls.contains("transfer.chunk"));
        QVERIFY(!link.calls.contains("transcripts.import"));
    }
}

void MeetingsUploadTest::transferRefusalCancels()
{
    QTemporaryDir dir;
    const QString path = dir.filePath("refused.wav");
    writeAudio(path, 5);
    UploadLink link;
    link.errors.insert("transfer.chunk", {{"message", "refused"}});
    MeetingsActions actions(&link, nullptr);
    QSignalSpy failed(&actions, &MeetingsActions::failed);
    actions.importFile(path, {}, {}, {}, false, false);
    QTRY_COMPARE(failed.size(), 1);
    QVERIFY(link.calls.contains("transfer.cancel"));
    QVERIFY(!link.calls.contains("transcripts.import"));
}

void MeetingsUploadTest::destructionCancelsPendingUpload()
{
    QTemporaryDir dir;
    const QString path = dir.filePath("cancelled.wav");
    writeAudio(path, 2 * MeetingsActions::kChunkBytes);
    UploadLink link;
    link.holdChunk = true;
    auto *actions = new MeetingsActions(&link, nullptr);
    actions->importFile(path, {}, {}, {}, false, false);
    QTRY_VERIFY(bool(link.held));
    delete actions;
    QVERIFY(link.calls.contains("transfer.cancel"));
    link.held({}, {});
    QTest::qWait(50);
    QVERIFY(!link.calls.contains("transfer.commit"));
}

void MeetingsUploadTest::cancelButtonStopsTheActiveTransfer_data()
{
    QTest::addColumn<bool>("beforeBegin");
    QTest::newRow("begin-pending") << true;
    QTest::newRow("chunk-pending") << false;
}

void MeetingsUploadTest::cancelButtonStopsTheActiveTransfer()
{
    QFETCH(bool, beforeBegin);
    QTemporaryDir dir;
    const QString path = dir.filePath("cancelled.wav");
    writeAudio(path, 2 * MeetingsActions::kChunkBytes);
    UploadLink link;
    link.holdChunk = !beforeBegin;
    link.holdBegin = beforeBegin;
    MeetingsActions actions(&link, nullptr);
    actions.importFile(path, {}, {}, {}, false, false);
    QTRY_VERIFY(bool(link.held));
    actions.cancelImport();
    QVERIFY(!actions.busy());
    link.held({{"transfer_id", "upload"}, {"chunk_max_bytes", 1048576}}, {});
    QVERIFY(link.calls.contains("transfer.cancel"));
    QTest::qWait(50);
    QVERIFY(!link.calls.contains("transfer.commit"));
    QVERIFY(!link.calls.contains("transcripts.import"));
}

void MeetingsUploadTest::cancellationDuringImportCancelsOnlyItsAcceptedMeeting_data()
{
    QTest::addColumn<bool>("destroy");
    QTest::addColumn<bool>("confirmed");
    QTest::newRow("cancel-confirmed") << false << true;
    QTest::newRow("cancel-unconfirmed") << false << false;
    QTest::newRow("destroy") << true << true;
}

void MeetingsUploadTest::cancellationDuringImportCancelsOnlyItsAcceptedMeeting()
{
    QFETCH(bool, destroy);
    QFETCH(bool, confirmed);
    QTemporaryDir dir;
    const QString path = dir.filePath("pending.wav");
    writeAudio(path, 5);
    UploadLink link;
    link.holdImport = true;
    link.answers.insert("meetings.cancel", {{"ref", QJsonObject{{"kind", "meeting"}, {"id", "owned"}}},
        {"job", QJsonObject{{"state", confirmed ? "cancelled" : "running"}}}});
    auto actions = std::make_unique<MeetingsActions>(&link, nullptr);
    QSignalSpy cancelled(actions.get(), &MeetingsActions::cancelled);
    QSignalSpy failed(actions.get(), &MeetingsActions::failed);
    actions->importFile(path, {}, {}, {}, false, false);
    QTRY_VERIFY(bool(link.held));
    if (destroy)
        actions.reset();
    else
        actions->cancelImport();
    link.held({{"ref", QJsonObject{{"kind", "meeting"}, {"id", "owned"}}},
        {"job", QJsonObject{{"job_id", "owned-job"}}}}, {});
    QVERIFY(link.calls.contains("meetings.cancel"));
    QCOMPARE(link.lastParams["meetings.cancel"].value("meeting_id").toString(), QString("owned"));
    if (!destroy) {
        QCOMPARE(cancelled.size(), confirmed ? 1 : 0);
        QCOMPARE(failed.size(), confirmed ? 0 : 1);
    }
}

void MeetingsUploadTest::readFailureIsNotAnEmptySuccessfulUpload()
{
    QTemporaryDir dir;
    const QString path = dir.filePath("unreadable.wav");
    QVERIFY(QFile::link("/proc/self/mem", path));
    UploadLink link;
    MeetingsActions actions(&link, nullptr);
    QSignalSpy failed(&actions, &MeetingsActions::failed);
    actions.importFile(path, {}, {}, {}, false, false);
    QTRY_COMPARE(failed.size(), 1);
    QVERIFY(!link.calls.contains("transcripts.import"));
}

QTEST_GUILESS_MAIN(MeetingsUploadTest)
#include "meetings_upload_test.moc"
