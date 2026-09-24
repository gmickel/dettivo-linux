#include "app_journal.h"

#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QJsonDocument>

namespace dettivo {

AppJournal::AppJournal(const QString &stateDir, bool enabled)
    : m_path(stateDir + QStringLiteral("/qa/app.jsonl")), m_enabled(enabled)
{
}

void AppJournal::record(const QString &event, const QJsonObject &fields)
{
    if (!m_enabled)
        return;
    QJsonObject line = fields;
    line.insert(QStringLiteral("event"), event);
    QDir().mkpath(QFileInfo(m_path).absolutePath());
    QFile file(m_path);
    if (!file.open(QIODevice::WriteOnly | QIODevice::Append))
        return;
    file.write(QJsonDocument(line).toJson(QJsonDocument::Compact) + '\n');
}

qint64 AppJournal::residentKb()
{
    QFile status(QStringLiteral("/proc/self/status"));
    if (!status.open(QIODevice::ReadOnly))
        return -1;
    // procfs reports no size, so the file is read whole rather than to atEnd().
    const QList<QByteArray> lines = status.readAll().split('\n');
    for (const QByteArray &line : lines) {
        if (line.startsWith("VmRSS:")) {
            const QList<QByteArray> parts = line.simplified().split(' ');
            bool ok = false;
            const qint64 value = parts.size() == 3 && parts[2] == "kB" ? parts[1].toLongLong(&ok) : -1;
            return ok && value >= 0 ? value : -1;
        }
    }
    return -1;
}

}  // namespace dettivo
