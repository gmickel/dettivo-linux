#include "meetings_model.h"
#include "meeting_format.h"

#include <QVariantMap>

namespace dettivo {

namespace {

const QStringList kRefreshStates = {QStringLiteral("recording"), QStringLiteral("completed"), QStringLiteral("partial"),
                                    QStringLiteral("cancelled"), QStringLiteral("failed"),    QStringLiteral("stopped")};

}  // namespace

MeetingsModel::MeetingsModel(DaemonLink *link, QObject *parent) : QAbstractListModel(parent), m_link(link)
{
    if (m_link != nullptr) {
        connect(m_link, &DaemonLink::notification, this, &MeetingsModel::handleNotification);
        connect(m_link, &DaemonLink::connectedChanged, this, [this](bool connected) {
            ++m_recoveryGeneration;
            applyRecoverable({});
            if (connected)
                refresh();
        });
    }
}

int MeetingsModel::rowCount(const QModelIndex &parent) const
{
    return parent.isValid() ? 0 : int(m_items.size());
}

QVariant MeetingsModel::data(const QModelIndex &index, int role) const
{
    if (!index.isValid() || index.row() >= m_items.size())
        return {};
    const Item &item = m_items.at(index.row());
    switch (role) {
    case IdRole:
        return item.id;
    case TitleRole:
        return item.title;
    case SummaryRole:
        return item.summary;
    case WhenRole:
        return item.when;
    case DateRole:
        return item.date;
    case WeekRole:
        return item.week;
    case LengthRole:
        return item.length;
    case StatusRole:
        return item.status;
    case PartialRole:
        return item.partial;
    case NotesRole:
        return item.hasNotes;
    case AnalysisRole:
        return item.analysisStatus;
    case SpeakerCountRole:
        return item.speakerCount;
    case SpeakersRole:
        return item.speakers;
    case ChipRole:
        return meeting_format::chip(item.status, item.partial, item.analysisStatus, item.hasNotes, item.durationMs > 0);
    case ChipKindRole:
        return meeting_format::chipKind(item.status, item.partial, item.analysisStatus, item.hasNotes);
    case SnippetRole:
        return item.snippet;
    case MatchedFieldRole:
        return item.matchedField;
    case RecoverableRole:
        return item.recoverable;
    default:
        return {};
    }
}

QHash<int, QByteArray> MeetingsModel::roleNames() const
{
    return {{IdRole, "meetingId"},     {TitleRole, "title"},           {SummaryRole, "summary"},
            {WhenRole, "when"},        {DateRole, "date"},             {WeekRole, "week"},
            {LengthRole, "length"},    {StatusRole, "status"},         {PartialRole, "partial"},
            {NotesRole, "hasNotes"},   {AnalysisRole, "analysisStatus"}, {SpeakerCountRole, "speakerCount"},
            {SpeakersRole, "speakers"}, {ChipRole, "chip"},            {ChipKindRole, "chipKind"},
            {SnippetRole, "snippet"},  {MatchedFieldRole, "matchedField"}, {RecoverableRole, "recoverable"}};
}

bool MeetingsModel::canFetchMore(const QModelIndex &parent) const
{
    return !parent.isValid() && !searching() && !m_nextCursor.isEmpty() && !m_fetching;
}

void MeetingsModel::fetchMore(const QModelIndex &parent)
{
    if (canFetchMore(parent))
        requestPage(m_nextCursor);
}

void MeetingsModel::refresh()
{
    if (searching())
        requestSearch(m_query);
    else
        requestPage(QString());
}

void MeetingsModel::search(const QString &query)
{
    const QString trimmed = query.trimmed();
    if (trimmed == m_query)
        return;
    m_query = trimmed;
    m_error.clear();
    emit queryChanged();
    refresh();
}

int MeetingsModel::rowOf(const QString &id) const
{
    for (int i = 0; i < m_items.size(); ++i) {
        if (m_items[i].id == id)
            return i;
    }
    return -1;
}

QString MeetingsModel::idAt(int row) const
{
    return row >= 0 && row < m_items.size() ? m_items[row].id : QString();
}

void MeetingsModel::remove(const QString &id)
{
    const int row = rowOf(id);
    if (row < 0)
        return;
    beginRemoveRows(QModelIndex(), row, row);
    m_items.removeAt(row);
    endRemoveRows();
    emit countChanged();
    if (searching())
        emit queryChanged();
}

void MeetingsModel::retitle(const QString &id, const QString &title)
{
    const int row = rowOf(id);
    if (row < 0 || m_items[row].title == title)
        return;
    m_items[row].title = title;
    emit dataChanged(index(row), index(row), {TitleRole});
}

void MeetingsModel::setQaState(const QString &state)
{
    if (state == m_qaState)
        return;
    m_qaState = state;
    emit qaStateChanged();
}

void MeetingsModel::requestPage(const QString &cursor)
{
    if (m_link == nullptr || !m_link->connected())
        return;
    QJsonObject params{{QStringLiteral("limit"), kPageSize}};
    params.insert(QStringLiteral("cursor"), cursor.isEmpty() ? QJsonValue::Null : QJsonValue(cursor));
    m_fetching = true;
    m_link->call(QStringLiteral("meetings.list"), params, [this, cursor](const QJsonObject &result, const QJsonObject &error) {
        m_fetching = false;
        if (!error.isEmpty() || searching())
            return;
        const QJsonArray items = result.value(QStringLiteral("items")).toArray();
        const QDate today = QDate::currentDate();
        if (cursor.isEmpty()) {
            applyItems(items, today);
        } else if (!items.isEmpty()) {
            beginInsertRows(QModelIndex(), int(m_items.size()), int(m_items.size() + items.size() - 1));
            for (const QJsonValue &v : items)
                m_items.append(itemFrom(v.toObject(), today));
            endInsertRows();
            emit countChanged();
        }
        m_nextCursor = result.value(QStringLiteral("next_cursor")).toString();
        requestSpeakers();
        requestRecoverable();
    });
}

void MeetingsModel::requestSearch(const QString &query)
{
    if (m_link == nullptr || !m_link->connected())
        return;
    const QJsonObject params{{QStringLiteral("query"), query}, {QStringLiteral("limit"), kSearchLimit}};
    m_link->call(QStringLiteral("meetings.search"), params, [this, query](const QJsonObject &result, const QJsonObject &error) {
        if (query != m_query)
            return;
        if (!error.isEmpty()) {
            m_error = error.value(QStringLiteral("message")).toString();
            setRows({});
            emit queryChanged();
            return;
        }
        applyHits(query, result.value(QStringLiteral("items")).toArray());
        requestSpeakers();
        requestRecoverable();
    });
}

// The swatches come from `meetings.get`, one call per listed meeting
// that has speakers and has not been asked yet; a row without any keeps
// its count alone.
void MeetingsModel::requestSpeakers()
{
    if (m_link == nullptr || !m_link->connected())
        return;
    for (const Item &item : m_items) {
        if (item.speakerCount <= 0 || !item.speakers.isEmpty() || m_speakersRequested.contains(item.id))
            continue;
        m_speakersRequested.insert(item.id);
        const QString id = item.id;
        m_link->call(QStringLiteral("meetings.get"), {{QStringLiteral("meeting_id"), id}},
                     [this, id](const QJsonObject &result, const QJsonObject &error) {
                         m_speakersRequested.remove(id);
                         if (error.isEmpty())
                             applySpeakers(id, result.value(QStringLiteral("speakers")).toArray());
                     });
    }
}

MeetingsModel::Item MeetingsModel::itemFrom(const QJsonObject &object, const QDate &today)
{
    Item item;
    item.id = object.value(QStringLiteral("ref")).toObject().value(QStringLiteral("id")).toString();
    item.title = object.value(QStringLiteral("title")).toString();
    item.summary = object.value(QStringLiteral("summary")).toString();
    item.startedAt = object.value(QStringLiteral("started_at")).toString();
    item.status = object.value(QStringLiteral("status")).toString();
    item.analysisStatus = object.value(QStringLiteral("analysis_status")).toString(QStringLiteral("none"));
    item.partial = object.value(QStringLiteral("is_partial")).toBool(false) || item.status == QStringLiteral("partial");
    item.hasNotes = object.value(QStringLiteral("has_notes")).toBool(false);
    item.speakerCount = object.value(QStringLiteral("speaker_count")).toInt(0);
    item.durationMs = qint64(object.value(QStringLiteral("duration_seconds")).toDouble() * 1000.0);
    item.when = meeting_format::weekdayClock(item.startedAt);
    item.date = meeting_format::shortDate(item.startedAt);
    item.week = meeting_format::weekHeading(item.startedAt, today);
    item.length = meeting_format::minutes(item.durationMs);
    return item;
}

QVariantList MeetingsModel::speakersFrom(const QJsonArray &speakers)
{
    QVariantList out;
    for (const QJsonValue &v : speakers) {
        const QJsonObject speaker = v.toObject();
        out.append(QVariantMap{{QStringLiteral("name"), speaker.value(QStringLiteral("name")).toString()},
                               {QStringLiteral("colorIndex"), speaker.value(QStringLiteral("color_index")).toInt(0)},
                               {QStringLiteral("speakerId"), speaker.value(QStringLiteral("speaker_id")).toString()}});
    }
    return out;
}

void MeetingsModel::applySpeakers(const QString &id, const QJsonArray &speakers)
{
    const int row = rowOf(id);
    if (row < 0)
        return;
    m_items[row].speakers = speakersFrom(speakers);
    if (m_items[row].speakerCount < m_items[row].speakers.size())
        m_items[row].speakerCount = int(m_items[row].speakers.size());
    const QModelIndex index = this->index(row);
    emit dataChanged(index, index, {SpeakersRole, SpeakerCountRole});
}

void MeetingsModel::setRows(QList<Item> rows)
{
    ++m_recoveryGeneration;
    beginResetModel();
    // Swatches already read survive a page reload.
    for (Item &row : rows) {
        const int before = rowOf(row.id);
        if (before >= 0 && row.speakers.isEmpty())
            row.speakers = m_items[before].speakers;
    }
    m_items = std::move(rows);
    m_loaded = true;
    endResetModel();
    emit countChanged();
}

void MeetingsModel::requestRecoverable()
{
    const quint64 generation = ++m_recoveryGeneration;
    applyRecoverable({});
    if (m_link == nullptr || !m_link->connected() || m_items.isEmpty())
        return;
    m_link->call(QStringLiteral("meetings.status"), {{QStringLiteral("meeting_id"), m_items.first().id}},
                 [this, generation](const QJsonObject &result, const QJsonObject &error) {
        if (generation == m_recoveryGeneration && error.isEmpty())
            applyRecoverable(result.value(QStringLiteral("recoverable")).toArray());
    });
}

void MeetingsModel::applyRecoverable(const QJsonArray &meetings)
{
    QSet<QString> ids;
    for (const auto &meeting : meetings)
        ids.insert(meeting.toObject().value(QStringLiteral("ref")).toObject().value(QStringLiteral("id")).toString());
    for (int row = 0; row < m_items.size(); ++row) {
        const bool recoverable = ids.contains(m_items[row].id);
        if (m_items[row].recoverable == recoverable)
            continue;
        m_items[row].recoverable = recoverable;
        emit dataChanged(index(row), index(row), {RecoverableRole});
    }
}

void MeetingsModel::applyItems(const QJsonArray &items, const QDate &today)
{
    QList<Item> rows;
    for (const QJsonValue &v : items)
        rows.append(itemFrom(v.toObject(), today));
    setRows(std::move(rows));
}

void MeetingsModel::applyHits(const QString &query, const QJsonArray &hits, const QDate &today)
{
    if (query != m_query) {
        m_query = query;
        m_error.clear();
    }
    QList<Item> rows;
    for (const QJsonValue &v : hits) {
        const QJsonObject hit = v.toObject();
        QJsonObject row = hit.value(QStringLiteral("item")).toObject();
        if (row.isEmpty())
            row = hit;
        if (!row.contains(QStringLiteral("ref")))
            row.insert(QStringLiteral("ref"), hit.value(QStringLiteral("ref")));
        Item item = itemFrom(row, today);
        item.snippet = hit.value(QStringLiteral("snippet")).toString();
        item.matchedField = hit.value(QStringLiteral("matched_field")).toString();
        if (item.title.isEmpty())
            item.title = hit.value(QStringLiteral("title")).toString(item.snippet);
        rows.append(item);
    }
    setRows(std::move(rows));
    emit queryChanged();
}

void MeetingsModel::handleNotification(const QString &topic, const QJsonObject &payload)
{
    if (topic == QStringLiteral("job.progress")) {
        const QString stage = payload.value(QStringLiteral("stage")).toString();
        if (stage == QStringLiteral("done") || stage == QStringLiteral("failed") || stage == QStringLiteral("cancelled"))
            refresh();
        return;
    }
    if (topic != QStringLiteral("meeting.state"))
        return;
    const QString state = payload.value(QStringLiteral("state")).toString();
    // The page is re-read on every transition that changes a row's chip:
    // a start, an end, a recovery, and the speaker and analysis passes.
    if (kRefreshStates.contains(state) || payload.contains(QStringLiteral("analysis_status"))
        || payload.contains(QStringLiteral("diarization_status")))
        refresh();
}

}  // namespace dettivo
