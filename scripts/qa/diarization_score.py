#!/usr/bin/env python3
"""Strict DER: zero collar, overlap included, UEM union, optimal speaker mapping.

Input is native JSON {"turns": [{"start_ms": 0, "end_ms": 1000, "speaker": 0}]}.
Times are rounded to integer microseconds (half to even). No transcript is emitted.
Only one recording/channel is scored; multi-channel references are rejected.
"""
import argparse
from collections import Counter, defaultdict
from decimal import Decimal, InvalidOperation, ROUND_HALF_EVEN
import hashlib
import json
import math
from pathlib import Path


def timestamp(value, scale, context):
    try:
        number = Decimal(str(value))
    except InvalidOperation as error:
        raise ValueError(f"{context}: invalid timestamp") from error
    if not number.is_finite():
        raise ValueError(f"{context}: timestamp must be finite")
    if number < 0:
        raise ValueError(f"{context}: negative timestamp")
    return int((number * scale).to_integral_value(rounding=ROUND_HALF_EVEN))


def reference_inputs(recording, rttm_data, uem_data):
    reference, regions, channels = [], [], set()
    for kind, data in (("RTTM", rttm_data), ("UEM", uem_data)):
        matched = False
        for line_number, line in enumerate(data.decode("utf-8").splitlines(), 1):
            if not line.strip() or line.lstrip().startswith("#"):
                continue
            fields = line.split()
            context = f"{kind} line {line_number}"
            if kind == "RTTM":
                if len(fields) != 10 or fields[0] != "SPEAKER":
                    raise ValueError(f"{context}: expected a 10-field SPEAKER row")
                _, uri, channel, start, duration, _, _, speaker, _, _ = fields
                a = timestamp(start, 1_000_000, context)
                length = timestamp(duration, 1_000_000, f"{context} duration")
                if length <= 0:
                    raise ValueError(f"{context}: duration must be positive")
                # Round the endpoint itself, rather than accumulating rounded durations.
                b = timestamp(Decimal(start) + Decimal(duration), 1_000_000, context)
                if b <= a:
                    raise ValueError(f"{context}: empty interval at microsecond resolution")
                if speaker == "<NA>":
                    raise ValueError(f"{context}: missing speaker")
                if uri == recording:
                    reference.append((a, b, speaker))
            else:
                if len(fields) != 4:
                    raise ValueError(f"{context}: expected four fields")
                uri, channel, start, end = fields
                a = timestamp(start, 1_000_000, context)
                b = timestamp(end, 1_000_000, context)
                if b <= a:
                    raise ValueError(f"{context}: interval must have end > start")
                if uri == recording:
                    regions.append((a, b))
            if uri == recording:
                channels.add(channel)
                matched = True
        if not matched:
            raise ValueError(f"{kind}: recording {recording!r} not found")
    if len(channels) != 1:
        raise ValueError("RTTM and UEM must identify one matching channel")
    merged = []
    for start, end in sorted(regions):
        if merged and start <= merged[-1][1]:
            merged[-1] = (merged[-1][0], max(merged[-1][1], end))
        else:
            merged.append((start, end))
    return reference, merged


def hypothesis_inputs(turns):
    if not isinstance(turns, list):
        raise ValueError("hypothesis turns must be a list")
    result = []
    for index, turn in enumerate(turns):
        context = f"hypothesis turn {index}"
        if not isinstance(turn, dict) or not {"start_ms", "end_ms", "speaker"} <= turn.keys():
            raise ValueError(f"{context}: missing start_ms, end_ms or speaker")
        for key in ("start_ms", "end_ms"):
            if type(turn[key]) not in (int, float, Decimal):
                raise ValueError(f"{context}: {key} must be numeric")
        a = timestamp(turn["start_ms"], 1000, context)
        b = timestamp(turn["end_ms"], 1000, context)
        speaker = turn["speaker"]
        if type(speaker) not in (str, int) or not str(speaker).strip():
            raise ValueError(f"{context}: speaker must be a nonempty string or integer")
        if b <= a:
            raise ValueError(f"{context}: interval must have end > start")
        result.append((a, b, str(speaker)))
    return result


def maximum_assignment(matrix):
    """Maximum-weight one-to-one assignment using the Hungarian algorithm."""
    return sum(matrix[row][column] for row, column in assignment_pairs(matrix))


def assignment_pairs(matrix):
    """The (row, column) pairs of a maximum-weight one-to-one assignment."""
    if not matrix or not matrix[0]:
        return []
    transposed = len(matrix) > len(matrix[0])
    if transposed:
        matrix = list(zip(*matrix))
    rows, columns = len(matrix), len(matrix[0])
    u, v = [0] * (rows + 1), [0] * (columns + 1)
    matched, predecessor = [0] * (columns + 1), [0] * (columns + 1)
    for row in range(1, rows + 1):
        matched[0], column = row, 0
        minimum, used = [float("inf")] * (columns + 1), [False] * (columns + 1)
        while True:
            used[column] = True
            current_row = matched[column]
            delta, next_column = float("inf"), 0
            for candidate in range(1, columns + 1):
                if used[candidate]:
                    continue
                cost = -matrix[current_row - 1][candidate - 1] - u[current_row] - v[candidate]
                if cost < minimum[candidate]:
                    minimum[candidate], predecessor[candidate] = cost, column
                if minimum[candidate] < delta:
                    delta, next_column = minimum[candidate], candidate
            for candidate in range(columns + 1):
                if used[candidate]:
                    u[matched[candidate]] += delta
                    v[candidate] -= delta
                else:
                    minimum[candidate] -= delta
            column = next_column
            if matched[column] == 0:
                break
        while column:
            previous = predecessor[column]
            matched[column] = matched[previous]
            column = previous
    pairs = [(row - 1, column - 1) for column, row in enumerate(matched[1:], 1) if row]
    return [(column, row) for row, column in pairs] if transposed else pairs


def crop(turns, regions):
    return [(max(a, start), min(b, end), label)
            for a, b, label in turns for start, end in regions
            if min(b, end) > max(a, start)]


def sweep(reference, hypothesis):
    events = defaultdict(list)
    for side, turns in enumerate((reference, hypothesis)):
        for start, end, label in turns:
            events[start].append((side, label, 1))
            events[end].append((side, label, -1))
    active, labels, overlap = [Counter(), Counter()], [set(), set()], Counter()
    total = missed = false_alarm = common = previous = 0
    for now in sorted(events):
        delta = now - previous
        r, h = [{label for label, count in side.items() if count > 0} for side in active]
        labels[0].update(r)
        labels[1].update(h)
        total += len(r) * delta
        missed += max(len(r) - len(h), 0) * delta
        false_alarm += max(len(h) - len(r), 0) * delta
        common += min(len(r), len(h)) * delta
        for first in r:
            for second in h:
                overlap[first, second] += delta
        for side, label, change in events[now]:
            active[side][label] += change
        previous = now
    if not total:
        raise ValueError("No reference speech in UEM; cannot produce a DER pass")
    correct = maximum_assignment([[overlap[r, h] for h in sorted(labels[1])]
                                  for r in sorted(labels[0])])
    confusion = common - correct
    assert confusion >= 0
    return {"der": (missed + false_alarm + confusion) / total,
            "missed": missed / total, "false_alarm": false_alarm / total,
            "confusion": confusion / total, "reference_speaker_seconds": total / 1_000_000,
            "missed_speaker_seconds": missed / 1_000_000,
            "false_alarm_speaker_seconds": false_alarm / 1_000_000,
            "confusion_speaker_seconds": confusion / 1_000_000,
            "reference_speakers": len(labels[0]), "hypothesis_speakers": len(labels[1])}


def cross_check(reference, hypothesis, regions, result):
    from pyannote.core import Annotation, Segment, Timeline
    from pyannote.metrics.diarization import DiarizationErrorRate

    annotations = []
    for turns in (reference, hypothesis):
        annotation = Annotation()
        for index, (a, b, label) in enumerate(turns):
            annotation[Segment(a / 1_000_000, b / 1_000_000), index] = label
        annotations.append(annotation)
    region = Timeline([Segment(a / 1_000_000, b / 1_000_000) for a, b in regions])
    detail = DiarizationErrorRate(collar=0, skip_overlap=False)(
        *annotations, uem=region, detailed=True)
    if not math.isfinite(detail["total"]) or detail["total"] <= 0:
        raise ValueError("pyannote cross-check has no finite reference speech")
    if abs(detail["total"] - result["reference_speaker_seconds"]) > 1e-7:
        raise ValueError("pyannote cross-check mismatch for reference speaker seconds")
    for key, external in (("der", "diarization error rate"), ("missed", "missed detection"),
                          ("false_alarm", "false alarm"), ("confusion", "confusion")):
        value = detail[external] if key == "der" else detail[external] / detail["total"]
        if not math.isfinite(value) or abs(result[key] - value) > 1e-7:
            raise ValueError(f"pyannote cross-check mismatch for {key}")
    result["independent_crosscheck"] = "pyannote.metrics"


def score(recording, turns, rttm, uem, check=False):
    rttm_data, uem_data = Path(rttm).read_bytes(), Path(uem).read_bytes()
    reference, regions = reference_inputs(recording, rttm_data, uem_data)
    hypothesis = hypothesis_inputs(turns)
    reference, hypothesis = crop(reference, regions), crop(hypothesis, regions)
    result = sweep(reference, hypothesis)
    result.update(recording=recording, collar_seconds=0, overlap_included=True,
                  protocol="strict DER; UEM union; global optimal speaker assignment; microsecond half-even rounding",
                  scorer="stdlib event sweep + Hungarian assignment",
                  rttm_sha256=hashlib.sha256(rttm_data).hexdigest(),
                  uem_sha256=hashlib.sha256(uem_data).hexdigest())
    if check:
        cross_check(reference, hypothesis, regions, result)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path, help="native JSON containing turns")
    parser.add_argument("--rttm", type=Path, required=True)
    parser.add_argument("--uem", type=Path, required=True)
    parser.add_argument("--recording", required=True)
    parser.add_argument("--cross-check", action="store_true", help="require optional pyannote.metrics check")
    args = parser.parse_args()
    try:
        data = json.loads(args.input.read_text(), parse_float=Decimal)
        if not isinstance(data, dict) or "turns" not in data:
            raise ValueError("input must be a JSON object containing turns")
        result = score(args.recording, data["turns"], args.rttm, args.uem, args.cross_check)
    except (ValueError, OSError, ImportError, ArithmeticError) as error:
        parser.error(str(error))
    print(json.dumps(result, indent=2, allow_nan=False))


if __name__ == "__main__":
    main()
