#include "live_segments_model.h"
#include "meeting_format.h"

namespace dettivo {

LiveSegmentsModel::LiveSegmentsModel(QObject *parent) : QAbstractListModel(parent) {}

int LiveSegmentsModel::rowCount(const QModelIndex &parent) const
{
    return parent.isValid() ? 0 : int(m_finals.size() + m_tail.size());
}

const LiveSegmentsModel::Segment &LiveSegmentsModel::at(int row) const
{
    return row < m_finals.size() ? m_finals.at(row) : m_tail.at(row - int(m_finals.size()));
}

QVariant LiveSegmentsModel::data(const QModelIndex &index, int role) const
{
    if (!index.isValid() || index.row() >= rowCount())
        return {};
    const Segment &segment = at(index.row());
    switch (role) {
    case SegmentIdRole:
        return segment.id;
    case SourceRole:
        return segment.source;
    case LabelRole:
        return meeting_format::sourceLabel(segment.source);
    case ColorIndexRole:
        return segment.source == QStringLiteral("you") ? 0 : 1;
    case TimeRole:
        return meeting_format::clockAt(m_startedAt, segment.startMs);
    case TextRole:
        return segment.text;
    case ProvisionalRole:
        return segment.provisional;
    case GapBeforeRole:
        return segment.gapBeforeMs;
    case StartMsRole:
        return segment.startMs;
    default:
        return {};
    }
}

QHash<int, QByteArray> LiveSegmentsModel::roleNames() const
{
    return {{SegmentIdRole, "segmentId"}, {SourceRole, "source"},       {LabelRole, "label"},
            {ColorIndexRole, "colorIndex"}, {TimeRole, "time"},          {TextRole, "text"},
            {ProvisionalRole, "provisional"}, {GapBeforeRole, "gapBefore"}, {StartMsRole, "startMs"}};
}

int LiveSegmentsModel::gapCount() const
{
    int gaps = 0;
    for (const Segment &s : m_finals)
        gaps += s.gapBeforeMs > 0 ? 1 : 0;
    return gaps;
}

void LiveSegmentsModel::setStartedAt(const QString &iso)
{
    if (iso == m_startedAt)
        return;
    m_startedAt = iso;
    emit startedAtChanged();
    if (rowCount() > 0)
        emit dataChanged(index(0), index(rowCount() - 1), {TimeRole});
}

LiveSegmentsModel::Segment LiveSegmentsModel::segmentFrom(const QJsonObject &payload)
{
    Segment s;
    s.id = payload.value(QStringLiteral("segment_id")).toString();
    s.source = payload.value(QStringLiteral("source")).toString(QStringLiteral("remote"));
    s.text = payload.value(QStringLiteral("text")).toString();
    s.startMs = qint64(payload.value(QStringLiteral("start_ms")).toDouble());
    s.endMs = qint64(payload.value(QStringLiteral("end_ms")).toDouble());
    s.gapBeforeMs = qint64(payload.value(QStringLiteral("gap_before_ms")).toDouble(0));
    s.provisional = payload.value(QStringLiteral("provisional")).toBool(false);
    return s;
}

int LiveSegmentsModel::fragmentIndex(const QString &id)
{
    const int at = id.lastIndexOf(QStringLiteral("-p"));
    if (at < 0)
        return 0;
    bool ok = false;
    const int n = id.mid(at + 2).toInt(&ok);
    return ok && n > 0 ? n : 0;
}

int LiveSegmentsModel::tailCount(const QString &source) const
{
    int n = 0;
    for (const Segment &s : m_tail)
        n += s.source == source ? 1 : 0;
    return n;
}

// The merger emits a window's tail as `<source>-p1`, `-p2`, ... and the
// daemon publishes every fragment; keeping only the newest one lost
// the sentences before it (fn-63).
void LiveSegmentsModel::apply(const QJsonObject &payload)
{
    const Segment segment = segmentFrom(payload);
    if (segment.provisional) {
        const int index = fragmentIndex(segment.id);
        if (index <= 1) {
            // A fresh tail (or an id without a fragment number, which
            // stands for the whole tail) replaces the source's tail.
            m_tail.removeIf([&segment](const Segment &s) { return s.source == segment.source; });
        } else {
            m_tail.removeIf([&segment](const Segment &s) { return s.source == segment.source && s.id == segment.id; });
        }
        if (!segment.text.trimmed().isEmpty())
            m_tail.append(segment);
    } else {
        // The fragments the final hardened out of end here; whatever
        // starts after it is the tail the next window re-announces.
        m_tail.removeIf([&segment](const Segment &s) { return s.source == segment.source && s.startMs < segment.endMs; });
        // Finals arrive in time order per source; the two sources
        // interleave by start, the other side first on a tie.
        int at = int(m_finals.size());
        while (at > 0 && m_finals[at - 1].startMs > segment.startMs)
            --at;
        m_finals.insert(at, segment);
    }
    rebuild();
}

void LiveSegmentsModel::applyAll(const QJsonArray &payloads)
{
    beginResetModel();
    m_finals.clear();
    m_tail.clear();
    endResetModel();
    for (const QJsonValue &v : payloads)
        apply(v.toObject());
    emit countChanged();
}

void LiveSegmentsModel::clear()
{
    beginResetModel();
    m_finals.clear();
    m_tail.clear();
    endResetModel();
    emit countChanged();
}

QJsonArray LiveSegmentsModel::toJson() const
{
    QJsonArray out;
    auto push = [&out](const Segment &s) {
        QJsonObject o{{QStringLiteral("segment_id"), s.id},   {QStringLiteral("source"), s.source},
                      {QStringLiteral("text"), s.text},       {QStringLiteral("start_ms"), double(s.startMs)},
                      {QStringLiteral("end_ms"), double(s.endMs)}, {QStringLiteral("provisional"), s.provisional}};
        if (s.gapBeforeMs > 0)
            o.insert(QStringLiteral("gap_before_ms"), double(s.gapBeforeMs));
        out.append(o);
    };
    for (const Segment &s : m_finals)
        push(s);
    for (const Segment &s : m_tail)
        push(s);
    return out;
}

// A window lands every tick; the rows are few hundred at most and the
// view keeps its end in sight, so a reset costs less than tracking moves.
void LiveSegmentsModel::rebuild()
{
    beginResetModel();
    endResetModel();
    emit countChanged();
}

}  // namespace dettivo
