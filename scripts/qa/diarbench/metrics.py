"""Every number the diarization bench reports, defined once (docs/diarization-bench.md).

A recording scored with one engine gives one dict of additive counts (stages.score).
A split pools its recordings by summing those counts, and every metric in `METRICS`
is a ratio of summed counts, so pooling weights each recording by its own size and a
bootstrap over files only has to resample count dicts. Nothing here reads audio or
text: the inputs are times, speaker labels and word counts.

Lines are the transcript segments the assignment stage returned: start, end, source
and speaker (None when unlabelled). Reference units are what a person said: one AMI
word each, or one hand-labelled transcript line with its word count (fn-72 labels).
"""
from bisect import bisect_right
from collections import Counter
from dataclasses import dataclass
import random
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import diarization_score  # noqa: E402

SHORT_TURN_WORDS = 3
BOOTSTRAP_RESAMPLES = 1000
BOOTSTRAP_SEED = 67


@dataclass(frozen=True)
class Metric:
    name: str
    label: str
    numerator: tuple
    denominator: tuple
    percent: bool = True

    def value(self, counts):
        below = sum(counts.get(k, 0) for k in self.denominator)
        if not below:
            return None
        above = sum(counts.get(k, 0) for k in self.numerator)
        return (100 if self.percent else 1) * above / below


# The headline first, then its parts. Lower is better for every metric but the last.
METRICS = [
    Metric("attribution_error", "Attribution error (headline)", ("words_wrong", "words_unlabelled"), ("words",)),
    Metric("wrong", "  wrong speaker", ("words_wrong",), ("words",)),
    Metric("unlabelled", "  unlabelled", ("words_unlabelled",), ("words",)),
    Metric("short_turn_error", "Short-turn error (<=3 words)", ("short_wrong", "short_unlabelled"), ("short_words",)),
    Metric("der", "DER of labelled lines", ("der_missed_s", "der_false_alarm_s", "der_confusion_s"), ("der_reference_s",)),
    Metric("der_missed", "  missed", ("der_missed_s",), ("der_reference_s",)),
    Metric("der_false_alarm", "  false alarm", ("der_false_alarm_s",), ("der_reference_s",)),
    Metric("der_confusion", "  confusion", ("der_confusion_s",), ("der_reference_s",)),
    Metric("engine_der", "Engine DER (turns)", ("engine_missed_s", "engine_false_alarm_s", "engine_confusion_s"),
           ("engine_reference_s",)),
    Metric("engine_missed", "  missed", ("engine_missed_s",), ("engine_reference_s",)),
    Metric("engine_false_alarm", "  false alarm", ("engine_false_alarm_s",), ("engine_reference_s",)),
    Metric("engine_confusion", "  confusion", ("engine_confusion_s",), ("engine_reference_s",)),
    Metric("lines_right", "Lines, right speaker", ("lines_right",), ("lines_reference",)),
    Metric("lines_wrong", "Lines, wrong speaker", ("lines_wrong",), ("lines_reference",)),
    Metric("lines_unlabelled", "Lines, unlabelled", ("lines_reference_unlabelled",), ("lines_reference",)),
    Metric("remote_unlabelled", "Remote lines unlabelled", ("remote_unlabelled",), ("remote_lines",)),
    Metric("mix_side_confusion", "Local/remote proxy (mix)", ("mix_confused",), ("mix_single",)),
    Metric("realtime_factor", "Engine speed (x realtime)", ("audio_s",), ("engine_wall_s",), percent=False),
]
BY_NAME = {m.name: m for m in METRICS}


def mapping(pairs):
    """The one-to-one hypothesis->reference label map that maximises the summed
    weight of (hypothesis, reference, weight) triples."""
    counts = Counter()
    for hyp, ref, weight in pairs:
        counts[hyp, ref] += weight
    hyps, refs = sorted({h for h, _ in counts}), sorted({r for _, r in counts})
    matrix = [[counts[h, r] for r in refs] for h in hyps]
    return {hyps[i]: refs[j] for i, j in diarization_score.assignment_pairs(matrix)}


class Lines:
    """Spans indexed for 'which spans overlap this one', per source."""

    def __init__(self, lines):
        self.by_source = {}
        for line in sorted(lines, key=lambda x: (x["start_ms"], x["end_ms"])):
            for source in {None, line.get("source")}:
                self.by_source.setdefault(source, []).append(line)
        self.starts = {s: [x["start_ms"] for x in v] for s, v in self.by_source.items()}
        self.longest = max((x["end_ms"] - x["start_ms"] for x in lines), default=0)
        self.sourced = any(x.get("source") for x in lines)

    def overlapping(self, start, end, source=None):
        """Spans of `source` (any when None) sharing time with [start, end), or containing
        `start` when the span is empty, with the shared milliseconds, earliest first (so a
        tie goes to the span that started first)."""
        spans, starts = self.by_source.get(source, []), self.starts.get(source, [])
        i = bisect_right(starts, max(end, start + 1)) - 1
        found = []
        while i >= 0 and spans[i]["start_ms"] >= start - self.longest:
            span = spans[i]
            if end > start:
                shared = min(end, span["end_ms"]) - max(start, span["start_ms"])
                if shared > 0:
                    found.append((span, shared))
            elif span["start_ms"] <= start < span["end_ms"]:
                found.append((span, 0))
            i -= 1
        return found[::-1]

    def holding(self, start, end, source=None):
        """The span with the most overlap with [start, end); None when none overlaps."""
        best = max(self.overlapping(start, end, source), key=lambda pair: pair[1], default=None)
        return best and best[0]


def short_units(units):
    """Indexes of units in reference turns of SHORT_TURN_WORDS words or fewer: a turn is a
    run of consecutive units (by start) with one speaker."""
    order = sorted(range(len(units)), key=lambda i: (units[i]["start_ms"], units[i]["end_ms"]))
    short, run = set(), []

    def close():
        if run and sum(units[i]["words"] for i in run) <= SHORT_TURN_WORDS:
            short.update(run)

    for i in order:
        if run and units[run[-1]]["speaker"] != units[i]["speaker"]:
            close()
            run = []
        run.append(i)
    close()
    return short


def attribution(lines, units):
    """Word counts: a reference unit's words count against the line holding it. The line
    is right when its speaker maps to the unit's speaker under the best one-to-one map,
    wrong when it maps elsewhere, unlabelled when it has no speaker. Units no line holds
    are `words_uncovered` and stay out of the headline, since no speaker choice fixes them."""
    index = Lines(lines)
    held = [index.holding(u["start_ms"], u["end_ms"], u.get("source")) for u in units]
    labels = mapping((line["speaker"], u["speaker"], u["words"]) for u, line in zip(units, held)
                     if line is not None and line["speaker"] is not None)
    short = short_units(units)
    out = Counter()
    for i, (u, line) in enumerate(zip(units, held)):
        if line is None:
            out["words_uncovered"] += u["words"]
            continue
        verdict = ("unlabelled" if line["speaker"] is None
                   else "right" if labels.get(line["speaker"]) == u["speaker"] else "wrong")
        out["words"] += u["words"]
        out[f"words_{verdict}"] += u["words"]
        if i in short:
            out["short_words"] += u["words"]
            out[f"short_{verdict}"] += u["words"]
    return out


def line_view(lines, turns):
    """Lines as a user reads them: a line's true speaker is the reference speaker with the
    most overlap; a labelled line is wrong when its speaker differs under the best
    one-to-one map of line speakers to reference speakers (counted in lines)."""
    index, pairs = Lines(turns), []
    for line in lines:
        shared = Counter()
        source = line.get("source") if index.sourced else None
        for t, o in index.overlapping(line["start_ms"], line["end_ms"], source):
            shared[t["speaker"]] += o
        if shared:
            pairs.append((line["speaker"], max(shared, key=shared.get)))
    labels = mapping((h, r, 1) for h, r in pairs if h is not None)
    out = Counter(lines_reference=len(pairs))
    for h, r in pairs:
        key = "lines_reference_unlabelled" if h is None else "lines_right" if labels.get(h) == r else "lines_wrong"
        out[key] += 1
    return out


def der(prefix, recording, turns, rttm, uem):
    """ADR 0058 strict DER (scripts/qa/diarization_score.py) as speaker-seconds."""
    s = diarization_score.score(recording, turns, rttm, uem)
    return {f"{prefix}_reference_s": s["reference_speaker_seconds"], f"{prefix}_missed_s": s["missed_speaker_seconds"],
            f"{prefix}_false_alarm_s": s["false_alarm_speaker_seconds"],
            f"{prefix}_confusion_s": s["confusion_speaker_seconds"]}


def remote_lines(lines):
    """fn-64's figure: system-track lines of a two-track meeting left without a speaker."""
    remote = [x for x in lines if x.get("source") == "system"]
    return {"remote_lines": len(remote), "remote_unlabelled": sum(x["speaker"] is None for x in remote)}


def pooled(files):
    total = Counter()
    for counts in files:
        total.update(counts)
    return total


def summarise(files, resamples=BOOTSTRAP_RESAMPLES):
    """Each metric over the pooled files, with a 95 % percentile bootstrap over files."""
    files = list(files)
    total = pooled(files)
    rng = random.Random(BOOTSTRAP_SEED)
    draws = [pooled(rng.choice(files) for _ in files) for _ in range(resamples if len(files) > 1 else 0)]
    out = {}
    for m in METRICS:
        value = m.value(total)
        if value is None:
            continue
        spread = sorted(v for v in (m.value(d) for d in draws) if v is not None)
        out[m.name] = {"value": value, **interval(spread)}
    return out


def delta(current, baseline, resamples=BOOTSTRAP_RESAMPLES):
    """Per-metric current minus baseline over the files both hold, with a paired 95 %
    bootstrap over those files. `current`/`baseline` map file id -> counts."""
    common = sorted(set(current) & set(baseline))
    if not common:
        return {}
    rng = random.Random(BOOTSTRAP_SEED)
    draws = []
    for _ in range(resamples if len(common) > 1 else 0):
        pick = [rng.choice(common) for _ in common]
        draws.append((pooled(current[f] for f in pick), pooled(baseline[f] for f in pick)))
    now, then = pooled(current[f] for f in common), pooled(baseline[f] for f in common)
    out = {}
    for m in METRICS:
        a, b = m.value(now), m.value(then)
        if a is None or b is None:
            continue
        spread = sorted(x - y for x, y in ((m.value(c), m.value(d)) for c, d in draws)
                        if x is not None and y is not None)
        out[m.name] = {"value": a - b, "files": len(common), **interval(spread)}
    return out


def interval(spread):
    if not spread:
        return {"lo": None, "hi": None}
    return {"lo": spread[int(0.025 * (len(spread) - 1))], "hi": spread[int(round(0.975 * (len(spread) - 1)))]}
