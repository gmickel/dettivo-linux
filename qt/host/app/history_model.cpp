#include "history_model.h"
#include "status_format.h"

#include <QDateTime>
#include <QJsonObject>
#include <QLocale>

namespace dettivo {

namespace {

const QString kAll = QStringLiteral("all");

QString dayHeadingFor(const QString &label, const QString &iso)
{
    if (label != QStringLiteral("Today") && label != QStringLiteral("Yesterday"))
        return label;
    const QDateTime when = QDateTime::fromString(iso, Qt::ISODate);
    if (!when.isValid())
        return label;
    const QDate day = when.toLocalTime().date();
    const QLocale locale = QLocale::c();
    return QStringLiteral("%1 · %2 %3 %4")
        .arg(label, locale.toString(day, QStringLiteral("ddd")))
        .arg(day.day())
        .arg(locale.toString(day, QStringLiteral("MMM")));
}

}  // namespace

HistoryModel::HistoryModel(DaemonLink *link, QObject *parent) : QAbstractListModel(parent), m_link(link)
{
    if (m_link != nullptr) {
        connect(m_link, &DaemonLink::notification, this, &HistoryModel::handleNotification);
        connect(m_link, &DaemonLink::connectedChanged, this, [this](bool connected) {
            if (connected)
                refresh();
        });
    }
}

int HistoryModel::rowCount(const QModelIndex &parent) const
{
    return parent.isValid() ? 0 : int(m_items.size());
}

QVariant HistoryModel::data(const QModelIndex &index, int role) const
{
    if (!index.isValid() || index.row() >= m_items.size())
        return {};
    const Item &item = m_items.at(index.row());
    switch (role) {
    case IdRole:
        return item.id;
    case KindRole:
        return item.kind;
    case TitleRole:
        return item.title;
    case TimeRole:
        return item.time;
    case DayRole:
        return item.day;
    case DayLabelRole:
        return item.dayLabel;
    case DayHeadingRole:
        return item.dayHeading;
    case DurationRole:
        return item.duration;
    case StatusRole:
        return item.status;
    case NewestDayRole:
        return item.newestDay;
    case AppRole:
        return item.app;
    case ModeRole:
        return item.mode;
    case SourceRole:
        return item.source;
    case MetaRole:
        return metaLine(item);
    case SnippetRole:
        return item.snippet;
    case MatchesRole:
        return item.matches;
    case ProgressRole:
        return item.progress;
    case StageRole:
        return item.stage;
    default:
        return {};
    }
}

QHash<int, QByteArray> HistoryModel::roleNames() const
{
    return {{IdRole, "itemId"},         {KindRole, "kind"},           {TitleRole, "title"},
            {TimeRole, "time"},         {DayRole, "day"},             {DayLabelRole, "dayLabel"},
            {DayHeadingRole, "dayHeading"}, {DurationRole, "duration"}, {StatusRole, "status"},
            {NewestDayRole, "newestDay"}, {AppRole, "app"},           {ModeRole, "mode"},
            {SourceRole, "source"},     {MetaRole, "meta"},           {SnippetRole, "snippet"},
            {MatchesRole, "matches"},   {ProgressRole, "progress"},   {StageRole, "stage"}};
}

bool HistoryModel::canFetchMore(const QModelIndex &parent) const
{
    return !parent.isValid() && !searching() && !m_nextCursor.isEmpty() && !m_fetching;
}

void HistoryModel::fetchMore(const QModelIndex &parent)
{
    if (canFetchMore(parent))
        requestPage(m_nextCursor);
}

void HistoryModel::refresh()
{
    if (searching())
        requestSearch(m_query);
    else
        requestPage(QString());
}

void HistoryModel::search(const QString &query)
{
    const QString trimmed = query.trimmed();
    if (trimmed == m_query)
        return;
    m_query = trimmed;
    m_error.clear();
    emit queryChanged();
    refresh();
}

int HistoryModel::rowOf(const QString &id) const
{
    for (int i = 0; i < m_items.size(); ++i) {
        if (m_items[i].id == id)
            return i;
    }
    return -1;
}

QString HistoryModel::idAt(int row) const
{
    return row >= 0 && row < m_items.size() ? m_items[row].id : QString();
}

void HistoryModel::remove(const QString &id)
{
    const int row = rowOf(id);
    if (row < 0)
        return;
    beginRemoveRows(QModelIndex(), row, row);
    m_items.removeAt(row);
    endRemoveRows();
    markNewestDay();
    emit countChanged();
    if (searching())
        emit queryChanged();
}

void HistoryModel::trackJob(const QString &jobId, const QString &itemId)
{
    m_jobs.insert(jobId, itemId);
    const int row = rowOf(itemId);
    if (row >= 0) {
        m_items[row].progress = 0;
        m_items[row].stage = QStringLiteral("queued");
        updateRow(row);
    }
}

void HistoryModel::requestPage(const QString &cursor)
{
    if (m_link == nullptr || !m_link->connected())
        return;
    QJsonObject params{{QStringLiteral("kinds"), QJsonArray{QStringLiteral("dictation"), QStringLiteral("meeting")}},
                       {QStringLiteral("limit"), kPageSize}};
    params.insert(QStringLiteral("cursor"), cursor.isEmpty() ? QJsonValue::Null : QJsonValue(cursor));
    m_fetching = true;
    m_link->call(QStringLiteral("transcripts.list"), params, [this, cursor](const QJsonObject &result, const QJsonObject &error) {
        m_fetching = false;
        if (!error.isEmpty() || searching())
            return;
        const QJsonArray items = result.value(QStringLiteral("items")).toArray();
        const QString next = result.value(QStringLiteral("next_cursor")).toString();
        const QDate today = QDate::currentDate();
        if (cursor.isEmpty()) {
            applyItems(items, today);
        } else if (!items.isEmpty()) {
            beginInsertRows(QModelIndex(), int(m_items.size()), int(m_items.size() + items.size() - 1));
            for (const QJsonValue &v : items)
                m_items.append(itemFrom(v.toObject(), today));
            endInsertRows();
            markNewestDay();
            emit countChanged();
        }
        m_nextCursor = next;
    });
}

void HistoryModel::requestSearch(const QString &query)
{
    if (m_link == nullptr || !m_link->connected())
        return;
    const QJsonObject params{{QStringLiteral("query"), query},
                             {QStringLiteral("kinds"), QJsonArray{QStringLiteral("dictation"), QStringLiteral("meeting")}},
                             {QStringLiteral("limit"), kSearchLimit}};
    m_link->call(QStringLiteral("transcripts.search"), params, [this, query](const QJsonObject &result, const QJsonObject &error) {
        if (query != m_query)
            return;
        if (!error.isEmpty()) {
            m_error = error.value(QStringLiteral("message")).toString();
            setRows({});
            emit queryChanged();
            return;
        }
        applyHits(query, result.value(QStringLiteral("items")).toArray());
    });
}

HistoryModel::Item HistoryModel::itemFrom(const QJsonObject &object, const QDate &today)
{
    Item item;
    const QJsonObject ref = object.value(QStringLiteral("ref")).toObject();
    item.id = ref.value(QStringLiteral("id")).toString();
    item.kind = ref.value(QStringLiteral("kind")).toString();
    item.title = object.value(QStringLiteral("title")).toString();
    item.startedAt = object.value(QStringLiteral("started_at")).toString();
    item.status = object.value(QStringLiteral("status")).toString();
    item.appId = object.value(QStringLiteral("app_id")).toString();
    item.app = format::appName(item.appId);
    item.mode = object.value(QStringLiteral("mode")).toString();
    item.source = object.value(QStringLiteral("source")).toString(QStringLiteral("dictation"));
    item.time = format::timeOfDay(item.startedAt);
    item.dayLabel = format::dayLabel(item.startedAt, today);
    item.dayHeading = dayHeadingFor(item.dayLabel, item.startedAt);
    const QDateTime when = QDateTime::fromString(item.startedAt, Qt::ISODate);
    item.day = when.isValid() ? when.toLocalTime().date().toString(Qt::ISODate) : QString();
    item.duration = format::duration(object.value(QStringLiteral("duration_seconds")).toDouble());
    // The meeting facts of the timeline (ADR 0038): absent on a dictation.
    item.speakerCount = object.value(QStringLiteral("speaker_count")).toInt(-1);
    item.analysisStatus = object.value(QStringLiteral("analysis_status")).toString();
    item.partial = object.value(QStringLiteral("is_partial")).toBool(false) || item.status == QStringLiteral("partial");
    return item;
}

QString HistoryModel::modeLabel(const QString &mode)
{
    if (mode == QStringLiteral("enhanced"))
        return tr("Enhanced");
    if (mode == QStringLiteral("deterministic_polish") || mode == QStringLiteral("polish"))
        return tr("Polish");
    if (mode == QStringLiteral("raw"))
        return tr("Raw");
    return mode;
}

QString HistoryModel::metaLine(const Item &item)
{
    if (item.kind == QStringLiteral("meeting")) {
        // `41 min · 3 speakers · analysed`: the chip's facts on the row.
        QStringList parts{item.duration};
        if (item.speakerCount == 1)
            parts.append(tr("1 speaker"));
        else if (item.speakerCount > 1)
            parts.append(tr("%1 speakers").arg(item.speakerCount));
        if (item.partial)
            parts.append(tr("partial"));
        else if (item.analysisStatus == QStringLiteral("ready"))
            parts.append(tr("analysed"));
        else if (item.status == QStringLiteral("transcribing") || item.status == QStringLiteral("recording"))
            parts.append(item.status);
        parts.removeAll(QString());
        return parts.join(QStringLiteral(" · "));
    }
    QStringList parts;
    if (!item.app.isEmpty())
        parts.append(item.app);
    if (item.source == QStringLiteral("audioImport"))
        parts.append(tr("Import"));
    else if (item.source == QStringLiteral("rerun"))
        parts.append(tr("Re-run"));
    if (!item.mode.isEmpty())
        parts.append(modeLabel(item.mode));
    if (item.status == QStringLiteral("transcribing"))
        parts.append(tr("transcribing"));
    else if (item.status == QStringLiteral("failed") || item.status == QStringLiteral("cancelled"))
        parts.append(item.status);
    else if (!item.duration.isEmpty())
        parts.append(item.duration);
    return parts.join(QStringLiteral(" · "));
}

void HistoryModel::markNewestDay()
{
    const QString newest = m_items.isEmpty() ? QString() : m_items.first().day;
    for (Item &item : m_items)
        item.newestDay = !newest.isEmpty() && item.day == newest;
}

void HistoryModel::setRows(QList<Item> rows)
{
    beginResetModel();
    m_items = std::move(rows);
    for (auto it = m_jobs.cbegin(); it != m_jobs.cend(); ++it) {
        const int row = rowOf(it.value());
        if (row >= 0 && m_items[row].progress < 0)
            m_items[row].progress = 0;
    }
    markNewestDay();
    m_loaded = true;
    endResetModel();
    emit countChanged();
}

void HistoryModel::applyItems(const QJsonArray &items, const QDate &today)
{
    QList<Item> rows;
    for (const QJsonValue &v : items)
        rows.append(itemFrom(v.toObject(), today));
    setRows(std::move(rows));
}

void HistoryModel::applyHits(const QString &query, const QJsonArray &hits, const QDate &today)
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
            row.insert(QStringLiteral("ref"), hit.value(QStringLiteral("ref")));
        Item item = itemFrom(row, today);
        item.snippet = hit.value(QStringLiteral("snippet")).toString();
        if (item.title.isEmpty())
            item.title = item.snippet;
        for (const QJsonValue &m : hit.value(QStringLiteral("matches")).toArray()) {
            const QJsonObject range = m.toObject();
            // One `[start, end]` pair per match; wrapped so the list gains
            // one element, not two.
            item.matches.append(QVariant(QVariantList{range.value(QStringLiteral("start")).toInt(),
                                                      range.value(QStringLiteral("end")).toInt()}));
        }
        rows.append(item);
    }
    setRows(std::move(rows));
    emit queryChanged();
}

QString HistoryModel::newestDayLabel() const
{
    return m_items.isEmpty() ? tr("Today") : m_items.first().dayLabel;
}

void HistoryModel::updateRow(int row)
{
    const QModelIndex index = this->index(row);
    emit dataChanged(index, index, {ProgressRole, StageRole, MetaRole, StatusRole});
}

void HistoryModel::handleNotification(const QString &topic, const QJsonObject &payload)
{
    if (topic == QStringLiteral("dictation.state")) {
        if (payload.value(QStringLiteral("state")).toString() == QStringLiteral("idle")
            && payload.value(QStringLiteral("previous_state")).toString() == QStringLiteral("inserting"))
            refresh();
        return;
    }
    if (topic != QStringLiteral("job.progress"))
        return;
    const QString jobId = payload.value(QStringLiteral("job_id")).toString();
    const QString stage = payload.value(QStringLiteral("stage")).toString();
    const bool ended = stage == QStringLiteral("done") || stage == QStringLiteral("failed")
        || stage == QStringLiteral("cancelled");
    const QString itemId = m_jobs.value(jobId);
    if (itemId.isEmpty()) {
        // A job the command line started: its row appears or settles on
        // the next page read.
        if (ended)
            refresh();
        return;
    }
    const int row = rowOf(itemId);
    if (row >= 0) {
        m_items[row].progress = ended ? -1 : qBound(0.0, payload.value(QStringLiteral("progress")).toDouble(), 1.0);
        m_items[row].stage = stage;
        if (ended)
            m_items[row].status = stage == QStringLiteral("done") ? QStringLiteral("completed") : stage;
        updateRow(row);
    }
    if (ended) {
        m_jobs.remove(jobId);
        refresh();
    }
}

NewestDayModel::NewestDayModel(HistoryModel *source, QObject *parent) : QSortFilterProxyModel(parent)
{
    setSourceModel(source);
    connect(this, &QAbstractItemModel::rowsInserted, this, &NewestDayModel::countChanged);
    connect(this, &QAbstractItemModel::rowsRemoved, this, &NewestDayModel::countChanged);
    connect(this, &QAbstractItemModel::modelReset, this, &NewestDayModel::countChanged);
    connect(source, &HistoryModel::countChanged, this, [this]() {
        // Qt 6.10 brackets a filter change so selections survive it;
        // the 6.8 floor (ADR 0013) has invalidateRowsFilter() for the job.
#if QT_VERSION >= QT_VERSION_CHECK(6, 10, 0)
        beginFilterChange();
        endFilterChange(QSortFilterProxyModel::Direction::Rows);
#else
        invalidateRowsFilter();
#endif
        emit countChanged();
    });
}

bool NewestDayModel::filterAcceptsRow(int row, const QModelIndex &parent) const
{
    const QModelIndex index = sourceModel()->index(row, 0, parent);
    return sourceModel()->data(index, HistoryModel::NewestDayRole).toBool();
}

HistoryFilterModel::HistoryFilterModel(HistoryModel *source, QObject *parent) : QSortFilterProxyModel(parent)
{
    setSourceModel(source);
    connect(this, &QAbstractItemModel::rowsInserted, this, &HistoryFilterModel::countChanged);
    connect(this, &QAbstractItemModel::rowsRemoved, this, &HistoryFilterModel::countChanged);
    connect(this, &QAbstractItemModel::modelReset, this, &HistoryFilterModel::countChanged);
}

void HistoryFilterModel::setFilter(const QString &filter)
{
    const QString next = filter.isEmpty() ? kAll : filter;
    if (next == m_filter)
        return;
    m_filter = next;
#if QT_VERSION >= QT_VERSION_CHECK(6, 10, 0)
    beginFilterChange();
    endFilterChange(QSortFilterProxyModel::Direction::Rows);
#else
    invalidateRowsFilter();
#endif
    emit filterChanged();
    emit countChanged();
}

int HistoryFilterModel::sourceRow(int row) const
{
    const QModelIndex index = mapToSource(this->index(row, 0));
    return index.isValid() ? index.row() : -1;
}

int HistoryFilterModel::rowOf(const QString &id) const
{
    for (int i = 0; i < rowCount(); ++i) {
        if (data(index(i, 0), HistoryModel::IdRole).toString() == id)
            return i;
    }
    return -1;
}

bool HistoryFilterModel::filterAcceptsRow(int row, const QModelIndex &parent) const
{
    if (m_filter == kAll)
        return true;
    const QModelIndex index = sourceModel()->index(row, 0, parent);
    const QString kind = sourceModel()->data(index, HistoryModel::KindRole).toString();
    const QString source = sourceModel()->data(index, HistoryModel::SourceRole).toString();
    if (m_filter == QStringLiteral("meeting"))
        return kind == QStringLiteral("meeting");
    if (m_filter == QStringLiteral("import"))
        return source == QStringLiteral("audioImport");
    return kind == QStringLiteral("dictation") && source != QStringLiteral("audioImport");
}

}  // namespace dettivo
