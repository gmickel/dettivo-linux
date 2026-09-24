// The history host pieces without a daemon or a window (fn-22): the list
// swaps to search hits with their painted ranges and back, follows a
// re-run's job on its row and drops a deleted item; the kind filter; the
// detail's facts from a `transcripts.get` answer; the actions' re-run,
// delete and export through a fake link into the QA export directory;
// and the player's bars and clock from a WAV.
#include "daemon_link.h"
#include "history_actions.h"
#include "history_detail_model.h"
#include "history_model.h"
#include "history_player.h"
#include <QAudioOutput>

#include <QDir>
#include <QJsonArray>
#include <QSignalSpy>
#include <QTemporaryDir>
#include <QTest>

#include <functional>

using namespace dettivo;

namespace {

class RuntimeEnvGuard {
public:
    explicit RuntimeEnvGuard(const QString &path)
        : m_value(qgetenv("XDG_RUNTIME_DIR")), m_present(qEnvironmentVariableIsSet("XDG_RUNTIME_DIR"))
    { qputenv("XDG_RUNTIME_DIR", path.toUtf8()); }
    ~RuntimeEnvGuard() {
        if (m_present) qputenv("XDG_RUNTIME_DIR", m_value);
        else qunsetenv("XDG_RUNTIME_DIR");
    }
private:
    QByteArray m_value;
    bool m_present;
};

class FakeLink : public DaemonLink {
public:
    bool connected() const override { return true; }
    void call(const QString &method, const QJsonObject &params, Reply reply) override
    {
        calls.append(method);
        lastParams.insert(method, params);
        if (pullHook && method == QStringLiteral("transfer.pull"))
            pullHook(params.value(QStringLiteral("seq")).toInt());
        if (!reply)
            return;
        if (errors.contains(method))
            reply({}, errors.value(method));
        else if (answers.contains(method))
            reply(answers.value(method), {});
    }
    void subscribe(const QStringList &) override {}
    void notify(const QString &topic, const QJsonObject &payload) { emit notification(topic, payload); }

    QStringList calls;
    QHash<QString, QJsonObject> lastParams;
    QHash<QString, QJsonObject> answers;
    QHash<QString, QJsonObject> errors;
    // Runs before every `transfer.pull` with its `seq`, so a test can
    // fail a later chunk.
    std::function<void(int)> pullHook;
};

QJsonObject ref(const QString &id)
{
    return {{QStringLiteral("kind"), QStringLiteral("dictation")}, {QStringLiteral("id"), id}};
}

QJsonObject row(const QString &id, const QString &title, const QString &when, const QString &source = QStringLiteral("dictation"))
{
    return {{QStringLiteral("ref"), ref(id)},           {QStringLiteral("title"), title},
            {QStringLiteral("started_at"), when},       {QStringLiteral("duration_seconds"), 4},
            {QStringLiteral("status"), QStringLiteral("completed")}, {QStringLiteral("app_id"), QStringLiteral("com.mitchellh.ghostty")},
            {QStringLiteral("mode"), QStringLiteral("raw")}, {QStringLiteral("source"), source}};
}

QString writeWav(const QTemporaryDir &dir)
{
    QByteArray pcm;
    for (int i = 0; i < 16000; ++i) {
        const qint16 sample = i < 8000 ? 16000 : 0;
        pcm.append(char(sample & 0xff));
        pcm.append(char((sample >> 8) & 0xff));
    }
    auto le32 = [](quint32 v) {
        QByteArray b(4, '\0');
        for (int i = 0; i < 4; ++i)
            b[i] = char((v >> (8 * i)) & 0xff);
        return b;
    };
    auto le16 = [](quint16 v) {
        QByteArray b(2, '\0');
        b[0] = char(v & 0xff);
        b[1] = char((v >> 8) & 0xff);
        return b;
    };
    QByteArray wav = "RIFF" + le32(quint32(36 + pcm.size())) + "WAVEfmt " + le32(16) + le16(1) + le16(1) + le32(16000)
        + le32(32000) + le16(2) + le16(16) + "data" + le32(quint32(pcm.size())) + pcm;
    const QString path = dir.filePath(QStringLiteral("take.wav"));
    QFile file(path);
    file.open(QIODevice::WriteOnly);
    file.write(wav);
    return path;
}

}  // namespace

class AppHistoryTest : public QObject {
    Q_OBJECT

private slots:
    void listSearchesPaintsFollowsJobsAndDrops();
    void filterKeepsOneKind();
    void detailReadsTheFacts();
    void actionsRerunDeleteAndExportThroughTheLink();
    void playerReadsBarsAndClockFromAWav();
    void playerStaysOffTheSoundServerUntilPlayed();
    void firstPlaybackKeepsThePreviewSeekPosition();
};

void AppHistoryTest::listSearchesPaintsFollowsJobsAndDrops()
{
    FakeLink link;
    link.answers.insert(QStringLiteral("transcripts.list"),
                        {{QStringLiteral("items"), QJsonArray{row(QStringLiteral("a"), QStringLiteral("API gateway"), QStringLiteral("2026-02-13T10:00:00Z")),
                                                              row(QStringLiteral("b"), QStringLiteral("Vulkan build"), QStringLiteral("2026-02-12T10:00:00Z"))}},
                         {QStringLiteral("next_cursor"), QJsonValue::Null}});
    link.answers.insert(QStringLiteral("transcripts.search"),
                        {{QStringLiteral("items"), QJsonArray{QJsonObject{{QStringLiteral("ref"), ref(QStringLiteral("a"))},
                                                                          {QStringLiteral("snippet"), QStringLiteral("the API gateway…")},
                                                                          {QStringLiteral("score"), QJsonValue::Null},
                                                                          {QStringLiteral("matches"), QJsonArray{QJsonObject{{QStringLiteral("start"), 4}, {QStringLiteral("end"), 7}}}},
                                                                          {QStringLiteral("item"), row(QStringLiteral("a"), QStringLiteral("API gateway"), QStringLiteral("2026-02-13T10:00:00Z"))}}}}});
    HistoryModel history(&link);
    history.refresh();
    QCOMPARE(history.rowCount(), 2);
    QCOMPARE(history.data(history.index(0), HistoryModel::MetaRole).toString(), QStringLiteral("ghostty · Raw · 4 s"));
    QCOMPARE(history.data(history.index(0), HistoryModel::DayHeadingRole).toString(), QStringLiteral("Fri 13 Feb"));

    history.search(QStringLiteral("api"));
    QCOMPARE(link.lastParams.value(QStringLiteral("transcripts.search")).value(QStringLiteral("query")).toString(), QStringLiteral("api"));
    QVERIFY(history.searching());
    QCOMPARE(history.hitCount(), 1);
    QCOMPARE(history.rowCount(), 1);
    QCOMPARE(history.data(history.index(0), HistoryModel::SnippetRole).toString(), QStringLiteral("the API gateway…"));
    const QVariantList matches = history.data(history.index(0), HistoryModel::MatchesRole).toList();
    QCOMPARE(matches.size(), 1);
    QCOMPARE(matches.first().toList().at(1).toInt(), 7);
    history.search(QString());
    QVERIFY(!history.searching());
    QCOMPARE(history.rowCount(), 2);

    history.trackJob(QStringLiteral("job_rerun_1"), QStringLiteral("b"));
    QCOMPARE(history.data(history.index(1), HistoryModel::ProgressRole).toDouble(), 0.0);
    link.notify(QStringLiteral("job.progress"), {{QStringLiteral("job_id"), QStringLiteral("job_rerun_1")}, {QStringLiteral("progress"), 0.5}, {QStringLiteral("stage"), QStringLiteral("transcribing")}});
    QCOMPARE(history.data(history.index(1), HistoryModel::ProgressRole).toDouble(), 0.5);
    QCOMPARE(history.data(history.index(1), HistoryModel::StageRole).toString(), QStringLiteral("transcribing"));
    const int listsBefore = int(link.calls.count(QStringLiteral("transcripts.list")));
    link.notify(QStringLiteral("job.progress"), {{QStringLiteral("job_id"), QStringLiteral("job_rerun_1")}, {QStringLiteral("progress"), 1.0}, {QStringLiteral("stage"), QStringLiteral("done")}});
    QCOMPARE(int(link.calls.count(QStringLiteral("transcripts.list"))), listsBefore + 1);

    history.remove(QStringLiteral("a"));
    QCOMPARE(history.rowCount(), 1);
    QCOMPARE(history.idAt(0), QStringLiteral("b"));
    QCOMPARE(history.rowOf(QStringLiteral("a")), -1);
}

void AppHistoryTest::filterKeepsOneKind()
{
    HistoryModel history(nullptr);
    history.applyItems({row(QStringLiteral("a"), QStringLiteral("One"), QStringLiteral("2026-02-13T10:00:00Z")),
                        row(QStringLiteral("b"), QStringLiteral("Two"), QStringLiteral("2026-02-13T09:00:00Z"), QStringLiteral("audioImport"))});
    HistoryFilterModel filtered(&history);
    QCOMPARE(filtered.rowCount(), 2);
    filtered.setFilter(QStringLiteral("import"));
    QCOMPARE(filtered.rowCount(), 1);
    QCOMPARE(filtered.sourceRow(0), 1);
    QCOMPARE(filtered.rowOf(QStringLiteral("b")), 0);
    filtered.setFilter(QStringLiteral("meeting"));
    QCOMPARE(filtered.rowCount(), 0);
    filtered.setFilter(QStringLiteral("dictation"));
    QCOMPARE(filtered.rowCount(), 1);
    QCOMPARE(filtered.rowOf(QStringLiteral("a")), 0);
}

void AppHistoryTest::detailReadsTheFacts()
{
    FakeLink link;
    const QJsonObject facts{{QStringLiteral("created_at"), QStringLiteral("2026-02-13T16:00:00Z")},
                            {QStringLiteral("app_id"), QStringLiteral("com.mitchellh.ghostty")},
                            {QStringLiteral("app_name"), QStringLiteral("Ghostty")},
                            {QStringLiteral("source"), QStringLiteral("dictation")},
                            {QStringLiteral("status"), QStringLiteral("completed")},
                            {QStringLiteral("provider"), QStringLiteral("whisper")},
                            {QStringLiteral("model"), QStringLiteral("large-v3-turbo")},
                            {QStringLiteral("language"), QStringLiteral("en")},
                            {QStringLiteral("duration_seconds"), 30},
                            {QStringLiteral("title"), QStringLiteral("Dictation")},
                            {QStringLiteral("audio"), QJsonObject{{QStringLiteral("retained"), false}, {QStringLiteral("reason"), QStringLiteral("expired")}}},
                            {QStringLiteral("insertion"), QJsonObject{{QStringLiteral("outcome"), QStringLiteral("inserted")},
                                                                      {QStringLiteral("method"), QStringLiteral("paste")},
                                                                      {QStringLiteral("backend"), QJsonObject{{QStringLiteral("name"), QStringLiteral("virtual_keyboard")}}}}},
                            {QStringLiteral("timings"), QJsonObject{{QStringLiteral("capture_ms"), 90}, {QStringLiteral("transcribe_ms"), 810}, {QStringLiteral("insert_ms"), 20}, {QStringLiteral("stop_to_insert_ms"), 920}}}};
    link.answers.insert(QStringLiteral("transcripts.get"),
                        {{QStringLiteral("ref"), ref(QStringLiteral("x"))}, {QStringLiteral("text_raw"), QStringLiteral("raw words")},
                         {QStringLiteral("text_polish"), QStringLiteral("Raw words.")}, {QStringLiteral("mode"), QStringLiteral("enhanced")},
                         {QStringLiteral("facts"), facts}});
    HistoryDetailModel detail(&link);
    detail.load(QStringLiteral("x"));
    QVERIFY(detail.loaded());
    QCOMPARE(detail.enhancedText(), QStringLiteral("Raw words."));
    QCOMPARE(detail.rawText(), QStringLiteral("raw words"));
    QCOMPARE(detail.modeLabel(), QStringLiteral("Enhanced"));
    QCOMPARE(detail.outcomeLabel(), QStringLiteral("Inserted"));
    QCOMPARE(detail.rawMeta(), QStringLiteral("large-v3-turbo · 0.8 s"));
    QVERIFY(detail.whenLine().endsWith(QStringLiteral("inserted into ghostty")));
    QVERIFY(!detail.audioRetained());
    QCOMPARE(detail.audioReason(), QStringLiteral("Audio expired."));
    QVERIFY(!detail.canRerun());
    QCOMPARE(detail.rerunBlockedReason(), QStringLiteral("Audio expired."));
    const QVariantList grid = detail.facts();
    QCOMPARE(grid.size(), 8);
    QCOMPARE(grid.at(1).toMap().value(QStringLiteral("label")).toString(), QStringLiteral("stop to insert"));
    QCOMPARE(grid.at(1).toMap().value(QStringLiteral("value")).toString(), QStringLiteral("0.9 s"));
    QCOMPARE(grid.at(3).toMap().value(QStringLiteral("value")).toString(), QStringLiteral("virtual keyboard"));
    QCOMPARE(grid.at(4).toMap().value(QStringLiteral("value")).toString(), QStringLiteral("whisper · large-v3-turbo"));

    link.errors.insert(QStringLiteral("transcripts.get"), {{QStringLiteral("message"), QStringLiteral("no dictation y")}});
    detail.load(QStringLiteral("y"));
    QVERIFY(!detail.loaded());
    QCOMPARE(detail.error(), QStringLiteral("no dictation y"));
    detail.load(QString());
    QVERIFY(detail.itemId().isEmpty());
}

void AppHistoryTest::actionsRerunDeleteAndExportThroughTheLink()
{
    QTemporaryDir dir;
    FakeLink link;
    HistoryActions actions(&link, dir.path());
    QSignalSpy failed(&actions, &HistoryActions::failed);
    QSignalSpy started(&actions, &HistoryActions::rerunStarted);
    QSignalSpy removed(&actions, &HistoryActions::removed);
    QSignalSpy exported(&actions, &HistoryActions::exported);

    link.errors.insert(QStringLiteral("transcripts.rerun"), {{QStringLiteral("message"), QStringLiteral("dictation x has no retained audio")}});
    actions.rerun(QStringLiteral("x"), QString(), QString());
    QCOMPARE(failed.count(), 1);
    QCOMPARE(failed.first().at(1).toString(), QStringLiteral("dictation x has no retained audio"));
    link.errors.clear();
    link.answers.insert(QStringLiteral("transcripts.rerun"),
                        {{QStringLiteral("ref"), ref(QStringLiteral("new"))}, {QStringLiteral("rerun_of"), ref(QStringLiteral("x"))},
                         {QStringLiteral("job"), QJsonObject{{QStringLiteral("job_id"), QStringLiteral("job_rerun_1")}}}});
    actions.rerun(QStringLiteral("x"), QStringLiteral("whisper"), QStringLiteral("tiny.en"));
    QCOMPARE(started.count(), 1);
    QCOMPARE(started.first().at(1).toString(), QStringLiteral("new"));
    QCOMPARE(started.first().at(2).toString(), QStringLiteral("job_rerun_1"));
    QCOMPARE(link.lastParams.value(QStringLiteral("transcripts.rerun")).value(QStringLiteral("model")).toString(), QStringLiteral("tiny.en"));

    link.answers.insert(QStringLiteral("transcripts.delete"), {{QStringLiteral("deleted"), true}});
    actions.remove(QStringLiteral("x"));
    QCOMPARE(removed.count(), 1);

    link.answers.insert(QStringLiteral("transfer.begin"), {{QStringLiteral("transfer_id"), QStringLiteral("xfer_1")}});
    link.answers.insert(QStringLiteral("transcripts.export"), {{QStringLiteral("filename"), QStringLiteral("dictation-x.json")}});
    link.answers.insert(QStringLiteral("transfer.pull"), {{QStringLiteral("seq"), 1}, {QStringLiteral("data_b64"), QString::fromLatin1(QByteArray("{\"items\":[]}\n").toBase64())}, {QStringLiteral("eof"), true}});
    actions.exportItems(QStringLiteral("x"), QStringLiteral("json"), QStringLiteral("item"));
    QCOMPARE(exported.count(), 1);
    const QString path = exported.first().at(0).toString();
    QCOMPARE(path, dir.filePath(QStringLiteral("dictation-x.json")));
    QFile file(path);
    QVERIFY(file.open(QIODevice::ReadOnly));
    QCOMPARE(file.readAll(), QByteArray("{\"items\":[]}\n"));
    QCOMPARE(link.lastParams.value(QStringLiteral("transcripts.export")).value(QStringLiteral("scope")).toString(), QStringLiteral("item"));
    QCOMPARE(link.lastParams.value(QStringLiteral("transfer.begin")).value(QStringLiteral("content_type")).toString(), QStringLiteral("application/json"));
    QVERIFY(!actions.busy());

    // qt-hosts/F3 (fn-43): a transfer that fails leaves the previous
    // export untouched, whether the first chunk or a later one fails;
    // the file changes only once the whole transfer landed.
    const auto sentinel = [&]() {
        QFile f(path);
        return f.open(QIODevice::ReadOnly) ? f.readAll() : QByteArray();
    };
    QCOMPARE(sentinel(), QByteArray("{\"items\":[]}\n"));
    link.errors.insert(QStringLiteral("transfer.pull"), {{QStringLiteral("message"), QStringLiteral("the daemon went away")}});
    actions.exportItems(QStringLiteral("x"), QStringLiteral("json"), QStringLiteral("item"));
    QCOMPARE(failed.count(), 2);
    QCOMPARE(sentinel(), QByteArray("{\"items\":[]}\n"));
    QVERIFY(!actions.busy());
    link.errors.clear();
    // The first chunk lands, the second fails.
    link.answers.insert(QStringLiteral("transfer.pull"), {{QStringLiteral("seq"), 1}, {QStringLiteral("data_b64"), QString::fromLatin1(QByteArray("partial").toBase64())}, {QStringLiteral("eof"), false}});
    link.pullHook = [&](int seq) {
        if (seq == 2)
            link.errors.insert(QStringLiteral("transfer.pull"), {{QStringLiteral("message"), QStringLiteral("chunk lost")}});
    };
    actions.exportItems(QStringLiteral("x"), QStringLiteral("json"), QStringLiteral("item"));
    QCOMPARE(failed.count(), 3);
    QCOMPARE(failed.last().at(1).toString(), QStringLiteral("chunk lost"));
    QCOMPARE(sentinel(), QByteArray("{\"items\":[]}\n"));
    QVERIFY(!actions.busy());
}

void AppHistoryTest::playerReadsBarsAndClockFromAWav()
{
    QTemporaryDir dir;
    const QString path = writeWav(dir);
    QVariantList peaks;
    int rate = 0;
    qint64 duration = 0;
    QString error;
    QVERIFY2(HistoryPlayer::readWav(path, &peaks, &rate, &duration, &error), qPrintable(error));
    QCOMPARE(rate, 16000);
    QCOMPARE(duration, 1000);
    QCOMPARE(peaks.size(), HistoryPlayer::kBars);
    QVERIFY(peaks.first().toDouble() > 0.4);
    QCOMPARE(peaks.last().toDouble(), 0.0);
    QCOMPARE(HistoryPlayer::clockLabel(1000, 4200), QStringLiteral("0:01 / 0:04"));
    QCOMPARE(HistoryPlayer::clockLabel(0, 61000), QStringLiteral("0:00 / 1:01"));
    QVERIFY(!HistoryPlayer::readWav(dir.filePath(QStringLiteral("missing.wav")), &peaks, &rate, &duration, &error));
    QVERIFY(!error.isEmpty());
}

// Opening the audio output is what connects the process to the sound
// server; libpulse then creates `$XDG_RUNTIME_DIR/pulse` and locks the
// runtime dir to 0700. The smoke tests point XDG_RUNTIME_DIR into the
// build tree, and in CI that dir is root's, so a player that opened its
// output on construction left a directory the runner could not read.
void AppHistoryTest::playerStaysOffTheSoundServerUntilPlayed()
{
    QTemporaryDir dir;
    const QString runtime = dir.filePath(QStringLiteral("runtime"));
    QVERIFY(QDir().mkpath(runtime));
    const RuntimeEnvGuard environment(runtime);
    HistoryPlayer player;
    QVERIFY(player.findChildren<QMediaPlayer *>().isEmpty());
    player.setSource(writeWav(dir));
    QVERIFY(player.ready());
    player.seek(0.5);
    QCOMPARE(player.position(), player.duration() / 2);
    QVERIFY(player.findChildren<QMediaPlayer *>().isEmpty());
    player.pause();
    player.applySample({0.2, 0.5}, 4000, 1000);
    player.play();
    QVERIFY(player.findChildren<QMediaPlayer *>().isEmpty());
    player.setSource(QString());
    QVERIFY(!player.ready());
    QVERIFY(QDir(runtime).entryList(QDir::AllEntries | QDir::NoDotAndDotDot).isEmpty());
}

void AppHistoryTest::firstPlaybackKeepsThePreviewSeekPosition()
{
    QTemporaryDir dir;
    const RuntimeEnvGuard environment(dir.path());
    HistoryPlayer player;
    player.setSource(writeWav(dir));
    player.seek(0.5);
    const qint64 expected = player.position();
    player.play();
    auto *backend = player.findChild<QMediaPlayer *>();
    auto *output = player.findChild<QAudioOutput *>();
    QVERIFY(backend != nullptr);
    QVERIFY(output != nullptr);
    output->setMuted(true);
    player.pause();
    QTRY_VERIFY_WITH_TIMEOUT(backend->mediaStatus() == QMediaPlayer::LoadedMedia
                            || backend->mediaStatus() == QMediaPlayer::BufferedMedia, 3000);
    QTRY_COMPARE_WITH_TIMEOUT(backend->position(), expected, 3000);
    QTRY_COMPARE_WITH_TIMEOUT(player.position(), expected, 3000);
    player.setSource(QString());
    QVERIFY(!player.ready());
    QVERIFY(backend->source().isEmpty());
}

QTEST_GUILESS_MAIN(AppHistoryTest)
#include "app_history_test.moc"
