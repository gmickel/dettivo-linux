// The designed sample data stays stable across the app's baseline surfaces.
#include "config_binding.h"
#include "engines_model.h"
#include "fake_link.h"
#include "history_model.h"
#include "meeting_live_model.h"
#include "meetings_model.h"
#include "sample_data.h"
#include "status_model.h"

#include <QDateTime>
#include <QTest>
#include <QTimeZone>

using namespace dettivo;
using dettivo::test::FakeLink;

class AppSampleDataTest : public QObject {
    Q_OBJECT

private slots:
    void sampleDataCarriesTheBaseline();
    void sampleDatesMatchTheArtboard();
    void sampleMeetingsKeepRelativeLabelsAndDisclosureTime();
};

void AppSampleDataTest::sampleDataCarriesTheBaseline()
{
    FakeLink link;
    ConfigBinding config(&link);
    StatusModel status(&link, &config);
    EnginesModel engines(&link);
    HistoryModel history(&link);
    NewestDayModel today(&history);
    sample::apply(&status, &engines, &history, &config);
    QCOMPARE(history.rowCount(), 10);
    QCOMPARE(today.rowCount(), 6);
    QCOMPARE(history.data(history.index(0), HistoryModel::MetaRole).toString(), QStringLiteral("ghostty · Enhanced · 4 s"));
    QCOMPARE(history.data(history.index(6), HistoryModel::DayLabelRole).toString(), QStringLiteral("Yesterday"));
    QCOMPARE(history.newestDayLabel(), QStringLiteral("Today"));
    QCOMPARE(history.data(history.index(3), HistoryModel::KindRole).toString(), QStringLiteral("meeting"));
    QCOMPARE(history.data(history.index(3), HistoryModel::DurationRole).toString(), QStringLiteral("41 min"));
    QCOMPARE(engines.engines().first().key, QStringLiteral("parakeet"));
    QCOMPARE(status.holdChord(), QStringLiteral("F9"));
    QCOMPARE(status.targetApp(), QStringLiteral("ghostty"));
    QCOMPARE(status.clock(), QStringLiteral("Wed 3 Sep · 13:42"));
    QCOMPARE(status.daemonState(), QStringLiteral("connected"));
}

void AppSampleDataTest::sampleDatesMatchTheArtboard()
{
    const QDate artboardDate(2025, 9, 3); // Wednesday 3 September in the approved source artboard.
    const auto localDate = [](const QJsonValue &value) {
        return QDateTime::fromString(value.toString(), Qt::ISODateWithMs).toLocalTime().date();
    };
    const QJsonArray history = sample::historyItems();
    QCOMPARE(localDate(history.first().toObject().value(QStringLiteral("started_at"))), artboardDate);
    QCOMPARE(localDate(history.last().toObject().value(QStringLiteral("started_at"))), artboardDate.addDays(-2));
    const QJsonObject facts = sample::historyDetail().value(QStringLiteral("facts")).toObject();
    QCOMPARE(localDate(facts.value(QStringLiteral("created_at"))), artboardDate);
    QCOMPARE(localDate(sample::meetingsItems().first().toObject().value(QStringLiteral("started_at"))), artboardDate);
    QCOMPARE(localDate(sample::meetingDetail().value(QStringLiteral("started_at"))), artboardDate);
}

void AppSampleDataTest::sampleMeetingsKeepRelativeLabelsAndDisclosureTime()
{
    FakeLink link;
    MeetingsModel meetings(&link);
    MeetingLiveModel live(&link);
    sample::applyMeetings(QStringLiteral("live"), &meetings, &live, nullptr, nullptr);
    QCOMPARE(meetings.data(meetings.index(0), MeetingsModel::WeekRole).toString(), QStringLiteral("This week"));
    QCOMPARE(meetings.data(meetings.index(5), MeetingsModel::WeekRole).toString(), QStringLiteral("Last week"));
    QCOMPARE(live.disclosureAt(), QStringLiteral("11:04"));

    // Real disclosures still compare with the real local day by default.
    const QDateTime now(QDate::currentDate(), QTime(11, 4), QTimeZone::LocalTime);
    live.applyDisclosure({{QStringLiteral("acknowledged"), true},
                          {QStringLiteral("acknowledged_at"), now.toUTC().toString(Qt::ISODateWithMs)}});
    QCOMPARE(live.disclosureAt(), QStringLiteral("11:04"));
    live.applyDisclosure({{QStringLiteral("acknowledged"), true},
                          {QStringLiteral("acknowledged_at"), now.addDays(-1).toUTC().toString(Qt::ISODateWithMs)}});
    QVERIFY(live.disclosureAt() != QStringLiteral("11:04"));
}

QTEST_GUILESS_MAIN(AppSampleDataTest)
#include "app_sample_data_test.moc"
