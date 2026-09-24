// The history list as `transcripts.list` pages it (fn-17 R2, fn-22 R1):
// one row per dictation or meeting with the time, the text, the meta
// line (app, mode, duration) and the day it belongs to, `Today` first. A
// query swaps the rows for `transcripts.search` hits with the painted
// ranges; a re-run the app started shows its job's progress on its row;
// a deleted item leaves the list without a reload. `NewestDayModel` is
// the slice Home shows and `HistoryFilterModel` the kind filter.
#pragma once

#include "daemon_link.h"

#include <QAbstractListModel>
#include <QDate>
#include <QHash>
#include <QJsonArray>
#include <QSortFilterProxyModel>
#include <QString>
#include <QVariantList>

namespace dettivo {

class HistoryModel : public QAbstractListModel {
    Q_OBJECT
    Q_PROPERTY(int count READ rowCount NOTIFY countChanged)
    Q_PROPERTY(QString newestDayLabel READ newestDayLabel NOTIFY countChanged)
    Q_PROPERTY(bool loaded READ loaded NOTIFY countChanged)
    Q_PROPERTY(QString query READ query NOTIFY queryChanged)
    Q_PROPERTY(bool searching READ searching NOTIFY queryChanged)
    Q_PROPERTY(int hitCount READ hitCount NOTIFY queryChanged)
    Q_PROPERTY(QString error READ error NOTIFY queryChanged)

public:
    enum Role {
        IdRole = Qt::UserRole + 1,
        KindRole,
        TitleRole,
        TimeRole,
        DayRole,
        DayLabelRole,
        DayHeadingRole,
        DurationRole,
        StatusRole,
        NewestDayRole,
        AppRole,
        ModeRole,
        SourceRole,
        MetaRole,
        SnippetRole,
        MatchesRole,
        ProgressRole,
        StageRole,
    };

    struct Item {
        QString id, kind, title, startedAt, time, day, dayLabel, dayHeading, duration, status;
        QString appId, app, mode, source, snippet, stage, analysisStatus;
        QVariantList matches;
        double progress = -1;
        int speakerCount = -1;
        bool newestDay = false;
        bool partial = false;
    };

    explicit HistoryModel(DaemonLink *link, QObject *parent = nullptr);

    int rowCount(const QModelIndex &parent = QModelIndex()) const override;
    QVariant data(const QModelIndex &index, int role) const override;
    QHash<int, QByteArray> roleNames() const override;
    bool canFetchMore(const QModelIndex &parent) const override;
    void fetchMore(const QModelIndex &parent) override;

    /// Rows per page.
    static constexpr int kPageSize = 20;
    /// Hits per search.
    static constexpr int kSearchLimit = 100;

    /// Reads the first page again (or the search again while a query is set).
    Q_INVOKABLE void refresh();
    /// Sets the query: empty returns to the timeline, anything else lists
    /// the hits of `transcripts.search` with their painted ranges.
    Q_INVOKABLE void search(const QString &query);
    /// The row of an item id, -1 when it is not listed.
    Q_INVOKABLE int rowOf(const QString &id) const;
    /// The item id at a row, empty when out of range.
    Q_INVOKABLE QString idAt(int row) const;
    /// Drops an item from the list (after `transcripts.delete` answered).
    Q_INVOKABLE void remove(const QString &id);
    /// Follows a re-run job on its new item: the row shows the progress
    /// hairline and the stage until the job ends, then the page reloads.
    Q_INVOKABLE void trackJob(const QString &jobId, const QString &itemId);

    /// The label of the newest day (`Today` when it is today).
    QString newestDayLabel() const;
    /// True once a page has been read (or applied).
    bool loaded() const { return m_loaded; }
    QString query() const { return m_query; }
    bool searching() const { return !m_query.isEmpty(); }
    /// Hits of the current query; 0 outside a search.
    int hitCount() const { return searching() ? int(m_items.size()) : 0; }
    /// The daemon's refusal of the last request, empty when it answered.
    QString error() const { return m_error; }
    /// Replaces the rows with `transcripts.list` items (tests, the sample).
    void applyItems(const QJsonArray &items, const QDate &today = QDate::currentDate());
    /// Replaces the rows with `transcripts.search` hits (tests, the sample).
    void applyHits(const QString &query, const QJsonArray &hits, const QDate &today = QDate::currentDate());
    /// One row from a `transcripts.list` item.
    static Item itemFrom(const QJsonObject &object, const QDate &today);
    /// The meta line of a row: `ghostty · Enhanced · 4 s`.
    static QString metaLine(const Item &item);
    /// `Raw`, `Polish` or `Enhanced` for a mode id.
    static QString modeLabel(const QString &mode);

public slots:
    void handleNotification(const QString &topic, const QJsonObject &payload);

signals:
    void countChanged();
    void queryChanged();

private:
    void requestPage(const QString &cursor);
    void requestSearch(const QString &query);
    void markNewestDay();
    void setRows(QList<Item> rows);
    void updateRow(int row);

    DaemonLink *m_link;
    QList<Item> m_items;
    QHash<QString, QString> m_jobs;  // job id -> item id
    QString m_nextCursor;
    QString m_query;
    QString m_error;
    bool m_loaded = false;
    bool m_fetching = false;
};

/// The rows of the newest day, for Home's Today list.
class NewestDayModel : public QSortFilterProxyModel {
    Q_OBJECT
    Q_PROPERTY(int count READ rowCount NOTIFY countChanged)

public:
    explicit NewestDayModel(HistoryModel *source, QObject *parent = nullptr);

signals:
    void countChanged();

protected:
    bool filterAcceptsRow(int row, const QModelIndex &parent) const override;
};

/// The rows of one kind (`all`, `dictation`, `meeting`, `import`), the
/// list's filter chips.
class HistoryFilterModel : public QSortFilterProxyModel {
    Q_OBJECT
    Q_PROPERTY(int count READ rowCount NOTIFY countChanged)
    Q_PROPERTY(QString filter READ filter WRITE setFilter NOTIFY filterChanged)

public:
    explicit HistoryFilterModel(HistoryModel *source, QObject *parent = nullptr);

    QString filter() const { return m_filter; }
    void setFilter(const QString &filter);
    /// The source row of a proxy row, -1 when out of range.
    Q_INVOKABLE int sourceRow(int row) const;
    /// The proxy row of an item id, -1 when it is not shown.
    Q_INVOKABLE int rowOf(const QString &id) const;

signals:
    void countChanged();
    void filterChanged();

protected:
    bool filterAcceptsRow(int row, const QModelIndex &parent) const override;

private:
    QString m_filter = QStringLiteral("all");
};

}  // namespace dettivo
