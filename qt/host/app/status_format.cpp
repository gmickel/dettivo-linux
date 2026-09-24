#include "status_format.h"

#include <QFileInfo>
#include <QLocale>
#include <QRegularExpression>
#include <QStringList>

#include <cmath>

namespace dettivo::format {

namespace {

QString modifierName(const QString &raw)
{
    const QString m = raw.toUpper();
    if (m == QStringLiteral("SUPER") || m == QStringLiteral("LOGO") || m == QStringLiteral("WIN")
        || m == QStringLiteral("MOD4"))
        return QStringLiteral("Super");
    if (m == QStringLiteral("CTRL") || m == QStringLiteral("CONTROL"))
        return QStringLiteral("Ctrl");
    if (m == QStringLiteral("ALT"))
        return QStringLiteral("Alt");
    if (m == QStringLiteral("SHIFT"))
        return QStringLiteral("Shift");
    return raw;
}

QString keyName(const QString &raw)
{
    const QString k = raw.trimmed();
    if (k.size() == 1)
        return k.toUpper();
    if (k.compare(QStringLiteral("escape"), Qt::CaseInsensitive) == 0 || k.compare(QStringLiteral("esc"), Qt::CaseInsensitive) == 0)
        return QStringLiteral("Esc");
    if (k.compare(QStringLiteral("return"), Qt::CaseInsensitive) == 0 || k.compare(QStringLiteral("enter"), Qt::CaseInsensitive) == 0)
        return QStringLiteral("Enter");
    if (k.startsWith(QLatin1Char('f'), Qt::CaseInsensitive) && k.size() <= 3 && k.mid(1).toInt() > 0)
        return k.toUpper();
    QString out = k;
    out[0] = out[0].toUpper();
    return out;
}

}  // namespace

QString chord(const QString &configured)
{
    QString text = configured.trimmed();
    if (text.isEmpty())
        return QString();
    QStringList modifiers;
    QString key;
    if (text.contains(QLatin1Char(','))) {
        const int comma = int(text.indexOf(QLatin1Char(',')));
        const QStringList mods = text.left(comma).split(QRegularExpression(QStringLiteral("[\\s+]+")), Qt::SkipEmptyParts);
        for (const QString &m : mods)
            modifiers.append(modifierName(m));
        key = text.mid(comma + 1).trimmed();
    } else {
        const QStringList parts = text.split(QRegularExpression(QStringLiteral("[\\s+]+")), Qt::SkipEmptyParts);
        for (int i = 0; i < parts.size(); ++i) {
            if (i + 1 == parts.size())
                key = parts[i];
            else
                modifiers.append(modifierName(parts[i]));
        }
    }
    if (key.isEmpty())
        return modifiers.join(QLatin1Char('+'));
    modifiers.append(keyName(key));
    return modifiers.join(QLatin1Char('+'));
}

QString appName(const QString &appId)
{
    const QString id = appId.trimmed();
    if (id.isEmpty())
        return QString();
    const int dot = int(id.lastIndexOf(QLatin1Char('.')));
    return dot >= 0 ? id.mid(dot + 1) : id;
}

QString duration(double seconds)
{
    const double s = std::max(0.0, seconds);
    if (s < 60.0)
        return QStringLiteral("%1 s").arg(int(std::lround(s)));
    const int minutes = int(std::lround(s / 60.0));
    if (minutes < 60)
        return QStringLiteral("%1 min").arg(minutes);
    return QStringLiteral("%1 h %2 min").arg(minutes / 60).arg(minutes % 60);
}

QString modelName(const QString &idOrPath)
{
    QString name = idOrPath.trimmed();
    if (name.isEmpty())
        return QString();
    if (name.contains(QLatin1Char('/')))
        name = QFileInfo(name).fileName();
    if (name.endsWith(QStringLiteral(".bin")) || name.endsWith(QStringLiteral(".gguf")))
        name = name.left(int(name.lastIndexOf(QLatin1Char('.'))));
    if (name.startsWith(QStringLiteral("ggml-")))
        name = name.mid(5);
    return name;
}

QString homePath(const QString &path, const QProcessEnvironment &env)
{
    if (path.isEmpty())
        return path;
    const QString home = env.value(QStringLiteral("HOME"));
    if (!home.isEmpty() && (path == home || path.startsWith(home + QLatin1Char('/'))))
        return QStringLiteral("~") + path.mid(home.size());
    for (const char *var : {"XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_STATE_HOME"}) {
        const QString base = env.value(QLatin1String(var));
        if (!base.isEmpty() && (path == base || path.startsWith(base + QLatin1Char('/'))))
            return QStringLiteral("$") + QLatin1String(var) + path.mid(base.size());
    }
    return path;
}

QString clock(const QDateTime &when)
{
    const QLocale locale = QLocale::c();
    return QStringLiteral("%1 %2 %3 · %4")
        .arg(locale.toString(when, QStringLiteral("ddd")))
        .arg(when.date().day())
        .arg(locale.toString(when, QStringLiteral("MMM")), locale.toString(when, QStringLiteral("HH:mm")));
}

QString timeOfDay(const QString &iso)
{
    const QDateTime when = QDateTime::fromString(iso, Qt::ISODate);
    if (!when.isValid())
        return QString();
    return QLocale::c().toString(when.toLocalTime(), QStringLiteral("HH:mm"));
}

QString dayLabel(const QString &iso, const QDate &today)
{
    const QDateTime when = QDateTime::fromString(iso, Qt::ISODate);
    if (!when.isValid())
        return QString();
    const QDate day = when.toLocalTime().date();
    if (day == today)
        return QStringLiteral("Today");
    if (day == today.addDays(-1))
        return QStringLiteral("Yesterday");
    const QLocale locale = QLocale::c();
    return QStringLiteral("%1 %2 %3")
        .arg(locale.toString(day, QStringLiteral("ddd")))
        .arg(day.day())
        .arg(locale.toString(day, QStringLiteral("MMM")));
}

}  // namespace dettivo::format
