#include "meeting_format.h"

#include <QCoreApplication>
#include <QDateTime>
#include <QLocale>

namespace dettivo::meeting_format {

namespace {

QDateTime local(const QString &iso)
{
    const QDateTime when = QDateTime::fromString(iso, Qt::ISODateWithMs);
    if (when.isValid())
        return when.toLocalTime();
    const QDateTime plain = QDateTime::fromString(iso, Qt::ISODate);
    return plain.isValid() ? plain.toLocalTime() : QDateTime();
}

QString tr(const char *text)
{
    return QCoreApplication::translate("meeting_format", text);
}

QString two(qint64 value)
{
    return QStringLiteral("%1").arg(value, 2, 10, QLatin1Char('0'));
}

}  // namespace

QString weekHeading(const QString &iso, const QDate &today)
{
    const QDateTime when = local(iso);
    if (!when.isValid())
        return QString();
    const QDate day = when.date();
    const QDate thisMonday = today.addDays(1 - today.dayOfWeek());
    const QDate monday = day.addDays(1 - day.dayOfWeek());
    if (monday == thisMonday)
        return tr("This week");
    if (monday == thisMonday.addDays(-7))
        return tr("Last week");
    const QLocale locale = QLocale::c();
    return tr("Week of %1 %2").arg(monday.day()).arg(locale.toString(monday, QStringLiteral("MMM")));
}

QString weekdayClock(const QString &iso)
{
    const QDateTime when = local(iso);
    if (!when.isValid())
        return QString();
    return QStringLiteral("%1 %2").arg(QLocale::c().toString(when.date(), QStringLiteral("ddd")), when.toString(QStringLiteral("HH:mm")));
}

QString shortDate(const QString &iso)
{
    const QDateTime when = local(iso);
    if (!when.isValid())
        return QString();
    return QStringLiteral("%1 %2").arg(when.date().day()).arg(QLocale::c().toString(when.date(), QStringLiteral("MMM")));
}

QString weekdayDate(const QString &iso)
{
    const QDateTime when = local(iso);
    if (!when.isValid())
        return QString();
    return QStringLiteral("%1 %2").arg(QLocale::c().toString(when.date(), QStringLiteral("ddd")), shortDate(iso));
}

QString span(const QString &startIso, const QString &endIso)
{
    const QDateTime start = local(startIso);
    const QDateTime end = local(endIso);
    if (!start.isValid())
        return QString();
    const QString from = start.toString(QStringLiteral("HH:mm"));
    if (!end.isValid())
        return from;
    return tr("%1 to %2").arg(from, end.toString(QStringLiteral("HH:mm")));
}

QString minutes(qint64 ms)
{
    const qint64 mins = ms / 60000;
    if (ms < 60000)
        return tr("< 1 min");
    if (mins < 100)
        return tr("%1 min").arg(mins);
    return tr("%1 h %2 min").arg(mins / 60).arg(mins % 60);
}

QString elapsed(qint64 ms)
{
    const qint64 seconds = qMax<qint64>(0, ms / 1000);
    return QStringLiteral("%1:%2:%3").arg(two(seconds / 3600), two((seconds / 60) % 60), two(seconds % 60));
}

QString clockAt(const QString &startIso, qint64 offsetMs)
{
    const QDateTime start = local(startIso);
    if (!start.isValid()) {
        const qint64 seconds = offsetMs / 1000;
        return QStringLiteral("%1:%2").arg(two(seconds / 60), two(seconds % 60));
    }
    return start.addMSecs(offsetMs).toString(QStringLiteral("HH:mm"));
}

QString talk(qint64 ms)
{
    const qint64 seconds = qMax<qint64>(0, ms / 1000);
    if (seconds >= 3600)
        return QStringLiteral("%1:%2:%3").arg(seconds / 3600).arg(two((seconds / 60) % 60), two(seconds % 60));
    return QStringLiteral("%1:%2").arg(seconds / 60).arg(two(seconds % 60));
}

QString percent(double share)
{
    return QStringLiteral("%1 %").arg(qRound(qBound(0.0, share, 1.0) * 100));
}

QString chip(const QString &status, bool isPartial, const QString &analysisStatus, bool hasNotes, bool hasTranscript)
{
    if (status == QStringLiteral("cancelled"))
        return tr("Cancelled");
    if (status == QStringLiteral("stopped"))
        return tr("Stopped");
    if (isPartial || status == QStringLiteral("partial"))
        return tr("Partial");
    if (status == QStringLiteral("recording") || status == QStringLiteral("stopping"))
        return tr("Recording");
    if (status == QStringLiteral("transcribing"))
        return tr("Transcribing");
    if (status == QStringLiteral("failed"))
        return tr("Failed");
    if (analysisStatus == QStringLiteral("ready"))
        return tr("Analysed");
    if (analysisStatus == QStringLiteral("running"))
        return tr("Analysing");
    if (analysisStatus == QStringLiteral("queued"))
        return tr("Analysis queued");
    if (hasNotes)
        return tr("Notes only");
    return hasTranscript ? tr("Transcript") : tr("Empty");
}

QString chipKind(const QString &status, bool isPartial, const QString &analysisStatus, bool hasNotes)
{
    if (status == QStringLiteral("cancelled") || status == QStringLiteral("stopped"))
        return QStringLiteral("plain");
    if (isPartial || status == QStringLiteral("partial") || status == QStringLiteral("failed"))
        return status == QStringLiteral("failed") ? QStringLiteral("failed") : QStringLiteral("partial");
    if (status == QStringLiteral("recording") || status == QStringLiteral("stopping")
        || status == QStringLiteral("transcribing"))
        return QStringLiteral("live");
    if (analysisStatus == QStringLiteral("ready"))
        return QStringLiteral("analysed");
    if (hasNotes)
        return QStringLiteral("notes");
    return QStringLiteral("plain");
}

QString sourceLabel(const QString &source)
{
    if (source == QStringLiteral("you") || source == QStringLiteral("microphone"))
        return tr("You");
    return tr("Remote");
}

}  // namespace dettivo::meeting_format
