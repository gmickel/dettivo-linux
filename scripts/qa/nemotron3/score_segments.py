#!/usr/bin/env python3
"""Score fn-64 R6: speakers on transcript segments, numbers and identities only.

Each system labels segments through segment_assign.py (the product rule for turns, the
segment-based adapter for a `+segment` system), and three views are aggregated:

- real meetings: the remote (system-track) segments of each staged alias's product
  transcript, read-only from the Dettivo database, and how many are left without a
  speaker, per class (EN-, DE-) and pooled;
- AMI test split: the labelled segments become turns and are scored by
  scripts/qa/diarization_score.py under ADR 0058, once with the only_words reference
  turns as the segments (<work>/ami/<name>.rttm) and once with Whisper segments
  (<whisper>/<name>.json, the engine CLI's JSON). A `+segment` system is also scored on
  only the segments its base system's product rule labels, which splits its confusion
  into the segments both label and the ones only the adapter labels. A line view scores
  the segments as transcript lines: a line's true speaker is the reference speaker with
  the most overlap, and a labelled line is wrong when its label differs from the true
  speaker under the best one-to-one mapping of labels to reference speakers;
- a port check: the product rule on the baseline's turns against the labels the product
  stored for the same meetings;
- the judge: <work>/<pack>-results.json with <work>/<pack>-key.json, per class.
"""
import argparse
import json
import sqlite3
import sys
from collections import Counter
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path[:0] = [str(HERE), str(HERE.parent)]
import diarization_score  # noqa: E402
import judge_pack  # noqa: E402
import score_eval  # noqa: E402
import segment_assign  # noqa: E402


def whisper_segments(path):
    return [{"start_ms": s["start_ms"], "end_ms": s["end_ms"]} for s in json.loads(Path(path).read_text())["segments"]]


def reference_turns(rttm):
    out = []
    for line in Path(rttm).read_text().splitlines():
        f = line.split()
        if f and f[0] == "SPEAKER":
            start = round(float(f[3]) * 1000)
            out.append({"start_ms": start, "end_ms": start + round(float(f[4]) * 1000), "speaker": f[7]})
    return out


def line_view(segments, labels, rttm):
    """(lines with reference speech, of them labelled, of those wrong) for one recording."""
    reference = reference_turns(rttm)
    pairs = []
    for s, label in zip(segments, labels):
        shared = {}
        for t in reference:
            o = min(s["end_ms"], t["end_ms"]) - max(s["start_ms"], t["start_ms"])
            if o > 0:
                shared[t["speaker"]] = shared.get(t["speaker"], 0) + o
        if shared:
            pairs.append((label, max(shared, key=shared.get)))
    labelled = [(h, r) for h, r in pairs if h is not None]
    hyps, refs = sorted({h for h, _ in labelled}), sorted({r for _, r in labelled})
    counts = Counter(labelled)
    matched = diarization_score.maximum_assignment([[counts[(h, r)] for r in refs] for h in hyps])
    return len(pairs), len(labelled), len(labelled) - matched


def ami(work, system, segmentation, whisper, probs):
    scores, restricted, unlabelled, total = [], [], 0, 0
    lines = [0, 0, 0]
    for name in score_eval.AMI_TEST:
        rttm, uem = work / f"ami/{name}.rttm", work / f"ami/{name}.uem"
        segments = reference_turns(rttm) if segmentation == "reference_words" \
            else whisper_segments(whisper / f"{name}.json")
        segments = [s for s in segments if s["end_ms"] > s["start_ms"]]
        labels = segment_assign.label_segments(segments, system, name, work / "runs", probs, room_audio=True)
        turns = [dict(s, speaker=label) for s, label in zip(segments, labels) if label is not None]
        scores.append(diarization_score.score(name, turns, rttm, uem))
        if system.endswith(segment_assign.SEGMENT):
            base = segment_assign.label_segments(segments, system[:-len(segment_assign.SEGMENT)], name,
                                                 work / "runs", probs, room_audio=True)
            shared = [dict(s, speaker=la) for s, la, b in zip(segments, labels, base)
                      if la is not None and b is not None]
            restricted.append(diarization_score.score(name, shared, rttm, uem))
        unlabelled += labels.count(None)
        total += len(segments)
        lines = [a + b for a, b in zip(lines, line_view(segments, labels, rttm))]
    out = {"score_percent": score_eval.pooled_der(scores), "segments": total, "unlabelled_segments": unlabelled,
           "per_recording_confusion_percent": {n: round(100 * s["confusion"], 2)
                                               for n, s in zip(score_eval.AMI_TEST, scores)},
           "lines": {"with_reference_speech": lines[0], "labelled": lines[1], "wrong_speaker": lines[2],
                     "unlabelled_percent": round(100 * (lines[0] - lines[1]) / lines[0], 2),
                     "wrong_speaker_percent_of_labelled": round(100 * lines[2] / max(lines[1], 1), 2)}}
    if restricted:
        out["on_segments_the_base_rule_labels_percent"] = score_eval.pooled_der(restricted)
    return out


def real(work, system, selection, probs, classes):
    db = sqlite3.connect(f"file:{judge_pack.DB}?mode=ro", uri=True)
    counts = {}
    for alias, directory in selection.items():
        segments = [s for s in judge_pack.meeting_segments(db, directory) if s["source_type"] == "system"]
        labels = segment_assign.label_segments(segments, system, f"{alias}.system", work / "runs", probs)
        counts[alias] = (len(segments), labels.count(None))
    out = {}
    for c, aliases in classes.items():
        remote = sum(counts[a][0] for a in aliases)
        missing = sum(counts[a][1] for a in aliases)
        out[c] = {"remote_segments": remote, "unlabelled": missing,
                  "unlabelled_percent": round(100 * missing / remote, 2) if remote else None}
    return out


def stored_check(work, system, selection, probs):
    """The product rule on `system`'s turns against the labels the product stored: the
    labelled-or-not decision, and the speaker (up to renaming) where both label."""
    db = sqlite3.connect(f"file:{judge_pack.DB}?mode=ro", uri=True)
    segments = same = both = agree = 0
    for alias, directory in selection.items():
        remote = [s for s in judge_pack.meeting_segments(db, directory) if s["source_type"] == "system"]
        ours = segment_assign.label_segments(remote, system, f"{alias}.system", work / "runs", probs)
        pairs = Counter((o, s.get("speaker_id")) for o, s in zip(ours, remote))
        segments += len(remote)
        same += sum(n for (o, st), n in pairs.items() if (o is None) == (st is None))
        mapping = {}
        for (o, st), n in pairs.most_common():
            if o is not None and st is not None and o not in mapping and st not in mapping.values():
                mapping[o] = st
        both += sum(n for (o, st), n in pairs.items() if o is not None and st is not None)
        agree += sum(n for (o, st), n in pairs.items() if o is not None and mapping.get(o) == st)
    return {"system": system, "remote_segments": segments, "same_labelled_or_not": same,
            "labelled_by_both": both, "same_speaker": agree}


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--work", type=Path, required=True)
    parser.add_argument("--selection", type=Path, required=True)
    parser.add_argument("--probs", type=Path, required=True, help="directory with <key>.npy")
    parser.add_argument("--whisper", type=Path, required=True, help="directory with <AMI name>.json")
    parser.add_argument("--systems", required=True)
    parser.add_argument("--pack", default="judge-r6")
    parser.add_argument("--baseline", default="baseline-cpu")
    args = parser.parse_args()
    selection = json.loads(args.selection.read_text())
    aliases = sorted(selection)
    classes = {"en": [a for a in aliases if a.startswith("EN-")], "de": [a for a in aliases if a.startswith("DE-")],
               "pooled": aliases}
    systems = args.systems.split(",")
    out = {
        "real_unlabelled": {s: real(args.work, s, selection, args.probs, classes) for s in systems},
        "ami_test_segments": {seg: {s: ami(args.work, s, seg, args.whisper, args.probs) for s in systems}
                              for seg in ("reference_words", "whisper")},
        "judge": {c: score_eval.judged(args.work, a, args.pack) for c, a in classes.items()},
        "product_rule_port_check": stored_check(args.work, args.baseline, selection, args.probs),
    }
    print(json.dumps(out, indent=1))


if __name__ == "__main__":
    main()
