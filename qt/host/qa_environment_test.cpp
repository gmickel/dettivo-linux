// Every documented QA variable is honoured by the hosts, bad values and
// unknown reserved names are rejected by name, release builds refuse QA
// mode unless allowed, and docs/qa.md names every variable this parser
// knows (R5).
#include "qa_environment.h"

#include <QFile>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QTemporaryDir>
#include <QTest>
#include <QSet>

using dettivo::QaEnvironment;

namespace {

QProcessEnvironment env(std::initializer_list<std::pair<const char *, const char *>> pairs)
{
    QProcessEnvironment e;
    for (const auto &[k, v] : pairs)
        e.insert(QString::fromLatin1(k), QString::fromLatin1(v));
    return e;
}

}  // namespace

class QaEnvironmentTest : public QObject {
    Q_OBJECT

private slots:
    void offByDefaultAndEverySwitchTurnsItOn();
    void releaseBuildsRefuseUnlessAllowed();
    void unknownReservedNamesAreRejectedByName();
    void hooksNeedQaMode();
    void everyHookParsesAndBadValuesNameTheVariable();
    void docsTableNamesEveryVariable();
    void sharedConformance_data();
    void sharedConformance();
};

void QaEnvironmentTest::offByDefaultAndEverySwitchTurnsItOn()
{
    QString error;
    auto qa = QaEnvironment::parse(env({}), false, &error);
    QVERIFY(qa.has_value());
    QVERIFY(!qa->enabled);
    for (const char *sw : {"DETTIVO_QA_MODE", "DETTIVO_QA", "DETTIVO_MOCK_MODE"}) {
        auto on = QaEnvironment::parse(env({{sw, "1"}}), false, &error);
        QVERIFY2(on.has_value() && on->enabled, sw);
    }
}

void QaEnvironmentTest::releaseBuildsRefuseUnlessAllowed()
{
    QString error;
    QVERIFY(!QaEnvironment::parse(env({{"DETTIVO_QA_MODE", "1"}}), true, &error).has_value());
    QVERIFY(error.startsWith(QStringLiteral("DETTIVO_QA_MODE:")));
    auto allowed = QaEnvironment::parse(
        env({{"DETTIVO_QA_MODE", "1"}, {"DETTIVO_QA_ALLOW_RELEASE", "1"}}), true, &error);
    QVERIFY(allowed.has_value() && allowed->enabled && allowed->allowRelease);
}

void QaEnvironmentTest::unknownReservedNamesAreRejectedByName()
{
    for (const char *name : {"DETTIVO_MOCK_CAMERA", "DETTIVO_E2E_JUMP", "DETTIVO_QA_TURBO"}) {
        QString error;
        QVERIFY(!QaEnvironment::parse(env({{"DETTIVO_QA_MODE", "1"}, {name, "1"}}), false, &error)
                     .has_value());
        QVERIFY2(error.startsWith(QString::fromLatin1(name) + QStringLiteral(":")), qPrintable(error));
    }
    QString error;
    QVERIFY(QaEnvironment::parse(env({{"DETTIVO_IPC_SOCKET", "/x"}}), false, &error).has_value());
}

void QaEnvironmentTest::hooksNeedQaMode()
{
    QString error;
    QVERIFY(!QaEnvironment::parse(env({{"DETTIVO_E2E_SEED", "1"}}), false, &error).has_value());
    QVERIFY(error.startsWith(QStringLiteral("DETTIVO_E2E_SEED:")));
    QVERIFY(error.contains(QStringLiteral("DETTIVO_QA_MODE")));
}

void QaEnvironmentTest::everyHookParsesAndBadValuesNameTheVariable()
{
    QTemporaryDir dir;
    const QString wav = dir.filePath("mic.wav");
    {
        QFile f(wav);
        QVERIFY(f.open(QIODevice::WriteOnly));
        f.write("RIFF");
    }
    QString error;
    auto qa = QaEnvironment::parse(env({{"DETTIVO_QA_MODE", "1"},
                                        {"DETTIVO_MOCK_MIC", qPrintable(wav)},
                                        {"DETTIVO_MOCK_SYSTEM_AUDIO", qPrintable(wav)},
                                        {"DETTIVO_MOCK_INSERT", "1"},
                                        {"DETTIVO_MOCK_INPUT", "1"},
                                        {"DETTIVO_MOCK_A11Y", "1"},
                                        {"DETTIVO_MOCK_LLM", "fixture:/tmp/llm"},
                                        {"DETTIVO_MOCK_ENGINE", "whisper=fixture:/tmp/w,llm=fixture:/tmp/l"},
                                        {"DETTIVO_FORCE_CPU", "1"},
                                        {"DETTIVO_MODEL_SERVER", "http://127.0.0.1:8080"},
                                        {"DETTIVO_E2E_COMPLETE", "1"},
                                        {"DETTIVO_E2E_STEP", "models"},
                                        {"DETTIVO_E2E_SEED", "1"},
                                        {"DETTIVO_E2E_OPEN", "settings.audio"},
                                        {"DETTIVO_E2E_ROUTE", "devices"},
                                        {"DETTIVO_E2E_EXPORT_DIR", "/tmp/exports"},
                                        {"DETTIVO_E2E_OSD_STATE", "inserted"},
                                        {"DETTIVO_E2E_BAR_STATE", "panel-meeting"},
                                        {"DETTIVO_E2E_MEETING_STATE", "detail-notes"},
                                        {"DETTIVO_E2E_STATE", "engine-crashed"},
                                        {"DETTIVO_QA_PACING", "/tmp/pacing.json"},
                                        {"DETTIVO_QA_PLANT", "default_font"},
                                        {"DETTIVO_QA_CANARY", "1"}}),
                                   false, &error);
    QVERIFY2(qa.has_value(), qPrintable(error));
    QVERIFY(qa->e2eComplete && qa->e2eSeed);
    QCOMPARE(qa->e2eStep, QStringLiteral("models"));
    QCOMPARE(qa->e2eOpen, QStringLiteral("settings.audio"));
    QCOMPARE(qa->e2eRoute, QStringLiteral("devices"));
    QCOMPARE(qa->e2eExportDir, QStringLiteral("/tmp/exports"));
    QCOMPARE(qa->e2eOsdState, QStringLiteral("inserted"));
    QCOMPARE(qa->e2eBarState, QStringLiteral("panel-meeting"));
    QCOMPARE(qa->e2eMeetingState, QStringLiteral("detail-notes"));
    QCOMPARE(qa->e2eState, QStringLiteral("engine-crashed"));
    QCOMPARE(qa->qaPlant, QStringLiteral("default_font"));

    const std::pair<const char *, const char *> bad[] = {
        {"DETTIVO_MOCK_MIC", "/nonexistent/x.wav"}, {"DETTIVO_MOCK_LLM", "parrot"},
        {"DETTIVO_MOCK_ENGINE", "whisper"},         {"DETTIVO_MODEL_SERVER", "ftp://x"},
        {"DETTIVO_E2E_STEP", "finish"},             {"DETTIVO_E2E_OPEN", "garage"},
        {"DETTIVO_E2E_OSD_STATE", "sleeping"}, {"DETTIVO_QA_PLANT", "serif_label"},
        {"DETTIVO_E2E_BAR_STATE", "panel-asleep"}, {"DETTIVO_E2E_MEETING_STATE", "lobby"},
        {"DETTIVO_E2E_STATE", "engine-sulking"},
    };
    for (const auto &[var, value] : bad) {
        QString e;
        QVERIFY2(!QaEnvironment::parse(env({{"DETTIVO_QA_MODE", "1"}, {var, value}}), false, &e).has_value(),
                 value);
        QVERIFY2(e.startsWith(QString::fromLatin1(var) + QStringLiteral(":")), qPrintable(e));
    }
}

void QaEnvironmentTest::docsTableNamesEveryVariable()
{
    QFile docs(QStringLiteral(DETTIVO_REPO_ROOT "/docs/qa.md"));
    QVERIFY2(docs.open(QIODevice::ReadOnly), "docs/qa.md");
    const QString text = QString::fromUtf8(docs.readAll());
    const QStringList names = QaEnvironment::knownVariables();
    QCOMPARE(names.size(), 27);
    for (const QString &name : names)
        QVERIFY2(text.contains(QStringLiteral("`") + name), qPrintable(name));
}

void QaEnvironmentTest::sharedConformance_data()
{
    QTest::addColumn<QJsonObject>("testCase");
    QFile file(QStringLiteral(DETTIVO_REPO_ROOT "/crates/dettivo-core/fixtures/qa_environment_cases.json"));
    QVERIFY(file.open(QIODevice::ReadOnly));
    const auto cases = QJsonDocument::fromJson(file.readAll()).array();
    QVERIFY(!cases.isEmpty());
    QSet<QString> tested;
    for (const auto &value : cases) {
        const auto c = value.toObject();
        for (const auto &key : c.value("env").toObject().keys())
            tested.insert(key);
        QTest::newRow(qPrintable(c.value("name").toString())) << c;
    }
    for (const auto &key : QaEnvironment::knownVariables())
        QVERIFY2(tested.contains(key), qPrintable(key));
}

void QaEnvironmentTest::sharedConformance()
{
    QFETCH(QJsonObject, testCase);
    QTemporaryDir dir;
    QFile file(dir.filePath("input.wav"));
    QVERIFY(file.open(QIODevice::WriteOnly));
    file.write("RIFF");
    file.close();
    QProcessEnvironment environment;
    const auto vars = testCase.value("env").toObject();
    for (auto it = vars.begin(); it != vars.end(); ++it) {
        QString value = it.value().toString();
        value.replace("$FILE", file.fileName());
        value.replace("$MISSING", dir.filePath("absent.wav"));
        environment.insert(it.key(), value);
    }
    QString error;
    const auto result = QaEnvironment::parse(environment, testCase.value("release").toBool(), &error);
    QCOMPARE(result.has_value(), testCase.value("accepted").toBool());
    if (!result)
        QVERIFY2(error.startsWith(testCase.value("error_variable").toString() + ":"), qPrintable(error));
}

QTEST_GUILESS_MAIN(QaEnvironmentTest)
#include "qa_environment_test.moc"
