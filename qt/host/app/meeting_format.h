// The text the meetings screens derive from the daemon's facts (fn-34,
// ADR 0038): the week a meeting belongs to, its weekday and clock, the
// length in minutes, the elapsed display of a live meeting, the meeting
// clock of a segment as wall time, a speaker's talk time and share, and
// the chip that names a meeting's state.
#pragma once

#include <QDate>
#include <QString>

namespace dettivo::meeting_format {

/// `This week`, `Last week` or `Week of 25 Aug` for an ISO timestamp
/// relative to `today` (weeks start on Monday).
QString weekHeading(const QString &iso, const QDate &today);

/// `Wed 11:05` in local time; empty when unreadable.
QString weekdayClock(const QString &iso);

/// `3 Sep` in local time; empty when unreadable.
QString shortDate(const QString &iso);

/// `Wed 3 Sep` in local time; empty when unreadable.
QString weekdayDate(const QString &iso);

/// `11:05 to 11:46` from a start and an end; the start alone when the end
/// is missing.
QString span(const QString &startIso, const QString &endIso);

/// `41 min` (`< 1 min` under a minute, `1 h 12 min` above an hour).
QString minutes(qint64 ms);

/// `00:23:41` for the live elapsed display.
QString elapsed(qint64 ms);

/// `21:04`: the wall clock at `startIso` plus `offsetMs`; the offset as
/// `mm:ss` when the start is unreadable.
QString clockAt(const QString &startIso, qint64 offsetMs);

/// `24:02` (`1:02:03` above an hour) for a speaker's talk time.
QString talk(qint64 ms);

/// `58 %` for a share in 0..1.
QString percent(double share);

/// The chip of a list row: `Analysed`, `Partial`, `Notes only`, `Recording`,
/// `Transcribing`, `Failed`, `Transcript` or `Empty` from the row's state.
QString chip(const QString &status, bool isPartial, const QString &analysisStatus, bool hasNotes, bool hasTranscript);

/// `analysed`, `partial`, `notes`, `live`, `failed` or `plain`: the tone a
/// chip is drawn in.
QString chipKind(const QString &status, bool isPartial, const QString &analysisStatus, bool hasNotes);

/// The label of a source: `You` for the microphone, `Remote` for the
/// system track.
QString sourceLabel(const QString &source);

}  // namespace dettivo::meeting_format
