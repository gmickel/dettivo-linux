// The meetings list as `meetings.list` pages it (fn-34 R1, ADR 0038):
// one row per meeting with the week it belongs to, when it started, the
// title with its summary, the length, the speakers as swatches with their
// names (read from `meetings.get` for the rows that have any) and the
// chip that names its state. A query swaps the rows for `meetings.search`
// hits; a `meeting.state` transition refreshes the page; a deleted or
// discarded meeting leaves the list without a reload.
#pragma once

#include "daemon_link.h"

#include <QAbstractListModel>
#include <QDate>
#include <QJsonArray>
#include <QJsonObject>
#include <QSet>
#include <QString>
#include <QVariantList>

namespace dettivo {

class MeetingsModel : public QAbstractListModel {
    Q_OBJECT
    Q_PROPERTY(int count READ rowCount NOTIFY countChanged)
    Q_PROPERTY(bool loaded READ loaded NOTIFY countChanged)
    Q_PROPERTY(QString query READ query NOTIFY queryChanged)
    Q_PROPERTY(bool searching READ searching NOTIFY queryChanged)
    Q_PROPERTY(int hitCount READ hitCount NOTIFY queryChanged)
    Q_PROPERTY(QString error READ error NOTIFY queryChanged)
    Q_PROPERTY(QString qaState READ qaState WRITE setQaState NOTIFY qaStateChanged)

public:
    enum Role {
        IdRole = Qt::UserRole + 1,
        TitleRole,
        SummaryRole,
        WhenRole,
        DateRole,
        WeekRole,
        LengthRole,
        StatusRole,
        PartialRole,
        NotesRole,
        AnalysisRole,
        SpeakerCountRole,
        SpeakersRole,
        ChipRole,
        ChipKindRole,
        SnippetRole,
        MatchedFieldRole,
        RecoverableRole,
    };

    struct Item {
        QString id, title, summary, startedAt, when, date, week, length, status, analysisStatus, snippet, matchedField;
        QVariantList speakers;
        qint64 durationMs = 0;
        int speakerCount = 0;
        bool partial = false;
        bool hasNotes = false;
        bool recoverable = false;
    };

    explicit MeetingsModel(DaemonLink *link, QObject *parent = nullptr);

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
    /// Sets the query: empty returns to the list, anything else lists the
    /// hits of `meetings.search`.
    Q_INVOKABLE void search(const QString &query);
    /// The row of a meeting id, -1 when it is not listed.
    Q_INVOKABLE int rowOf(const QString &id) const;
    /// The meeting id at a row, empty when out of range.
    Q_INVOKABLE QString idAt(int row) const;
    /// Drops a meeting from the list (after `meetings.delete` or
    /// `meetings.discard` answered).
    Q_INVOKABLE void remove(const QString &id);
    /// Relabels a listed meeting (after `meetings.rename` answered).
    Q_INVOKABLE void retitle(const QString &id, const QString &title);

    bool loaded() const { return m_loaded; }
    QString query() const { return m_query; }
    bool searching() const { return !m_query.isEmpty(); }
    int hitCount() const { return searching() ? int(m_items.size()) : 0; }
    /// The daemon's refusal of the last request, empty when it answered.
    QString error() const { return m_error; }
    /// The `DETTIVO_E2E_MEETING_STATE` a render or a drive asked for; the
    /// route opens the matching dialog, tab or popover once.
    QString qaState() const { return m_qaState; }
    void setQaState(const QString &state);

    /// Replaces the rows with `meetings.list` items (tests, the sample).
    void applyItems(const QJsonArray &items, const QDate &today = QDate::currentDate());
    /// Replaces the rows with `meetings.search` hits (tests, the sample).
    void applyHits(const QString &query, const QJsonArray &hits, const QDate &today = QDate::currentDate());
    /// Fills a row's swatches from a `meetings.get` `speakers` array.
    void applySpeakers(const QString &id, const QJsonArray &speakers);
    /// Membership from `meetings.status.recoverable`; missing audio is
    /// decided by the daemon, never inferred from the row's lifecycle.
    void applyRecoverable(const QJsonArray &meetings);
    /// One row from a `meetings.list` item.
    static Item itemFrom(const QJsonObject &object, const QDate &today);
    /// `{name, colorIndex, speakerId}` per speaker, `You` first.
    static QVariantList speakersFrom(const QJsonArray &speakers);

public slots:
    void handleNotification(const QString &topic, const QJsonObject &payload);

signals:
    void countChanged();
    void queryChanged();
    void qaStateChanged();

private:
    void requestPage(const QString &cursor);
    void requestSearch(const QString &query);
    void requestSpeakers();
    void requestRecoverable();
    void setRows(QList<Item> rows);

    DaemonLink *m_link;
    QList<Item> m_items;
    QSet<QString> m_speakersRequested;
    QString m_nextCursor;
    QString m_query;
    QString m_error;
    QString m_qaState;
    bool m_loaded = false;
    bool m_fetching = false;
    quint64 m_recoveryGeneration = 0;
};

}  // namespace dettivo
