// The live transcript of a recording meeting (fn-34 R2, ADR 0038, ADR
// 0061): the final segments of both sources interleaved by start on the
// meeting clock, then the provisional tail of each source, every fragment
// of it. A window's `<source>-p1` opens a fresh tail for its source,
// later `-pN` fragments extend it, and a final retires the fragments it
// hardened out of. Every row carries its wall-clock time, its source
// label and colour, its text, whether it is still provisional, and the
// capture gap before it when one was journaled.
#pragma once

#include <QAbstractListModel>
#include <QJsonArray>
#include <QJsonObject>
#include <QList>
#include <QString>

namespace dettivo {

class LiveSegmentsModel : public QAbstractListModel {
    Q_OBJECT
    Q_PROPERTY(int count READ rowCount NOTIFY countChanged)
    Q_PROPERTY(int finalCount READ finalCount NOTIFY countChanged)
    Q_PROPERTY(int gapCount READ gapCount NOTIFY countChanged)
    Q_PROPERTY(QString startedAt READ startedAt WRITE setStartedAt NOTIFY startedAtChanged)

public:
    enum Role {
        SegmentIdRole = Qt::UserRole + 1,
        SourceRole,
        LabelRole,
        ColorIndexRole,
        TimeRole,
        TextRole,
        ProvisionalRole,
        GapBeforeRole,
        StartMsRole,
    };

    struct Segment {
        QString id, source, text;
        qint64 startMs = 0;
        qint64 endMs = 0;
        qint64 gapBeforeMs = 0;
        bool provisional = false;
    };

    explicit LiveSegmentsModel(QObject *parent = nullptr);

    int rowCount(const QModelIndex &parent = QModelIndex()) const override;
    QVariant data(const QModelIndex &index, int role) const override;
    QHash<int, QByteArray> roleNames() const override;

    /// Final segments so far.
    int finalCount() const { return int(m_finals.size()); }
    /// Segments that followed a capture gap.
    int gapCount() const;
    /// The meeting's start (ISO 8601), the base of every row's clock.
    QString startedAt() const { return m_startedAt; }
    void setStartedAt(const QString &iso);

    /// Folds one `meeting.segment` payload in: a provisional `-p1`
    /// replaces the whole tail of its source, a later `-pN` replaces the
    /// fragment with its id or extends the tail, and a final is appended
    /// for good and drops the fragments of its source that start before
    /// its end.
    Q_INVOKABLE void apply(const QJsonObject &payload);
    /// Provisional fragments of `source` still on screen.
    int tailCount(const QString &source) const;
    /// Replaces everything with a list of payloads (tests, the sample, a
    /// checkpoint's tail).
    void applyAll(const QJsonArray &payloads);
    /// Empties the transcript.
    void clear();
    /// The rows as `meeting.segment`-shaped objects, finals first.
    QJsonArray toJson() const;

signals:
    void countChanged();
    void startedAtChanged();

private:
    static Segment segmentFrom(const QJsonObject &payload);
    /// `N` of a `<source>-pN` id; 0 for any other id.
    static int fragmentIndex(const QString &id);
    const Segment &at(int row) const;
    void rebuild();

    QString m_startedAt;
    QList<Segment> m_finals;
    QList<Segment> m_tail;  // the provisional fragments, per source in window order
};

}  // namespace dettivo
