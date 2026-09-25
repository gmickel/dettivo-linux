#!/usr/bin/env python3
"""Aggregate the fn-64 runs into the committed evidence JSON (numbers and identities only).

Reads <work>/runs (runner outputs and runs.jsonl), <work>/ami (audio, only_words RTTM and
UEM), <work>/real/<alias> (staged tracks) and, when present, <work>/judge-results.json
(the judge's per-window counts) with <work>/judge-key.json. Aliases are `EN-<n>` or
`DE-<n>`; the output carries only per-class and pooled aggregates for real meetings.
"""
import argparse
import json
import sys
import wave
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path[:0] = [str(HERE), str(HERE.parent)]
import channel_score  # noqa: E402
import diarization_score  # noqa: E402

AMI_TEST = ["ES2004a", "TS3003a", "IS1009b", "EN2002c"]
AMI_TRAIN = ["ES2002a", "EN2001b"]


def seconds(wav):
    with wave.open(str(wav)) as w:
        return w.getnframes() / w.getframerate()


def load(runs_dir):
    records = {}
    for line in (runs_dir / "runs.jsonl").read_text().splitlines():
        r = json.loads(line)
        if r["exit"] == 0:
            r["output"] = json.loads((runs_dir / f"{r['key']}.json").read_text())
            records[r["key"]] = r
    return records


def timing(rs, audio):
    wall = sum(r["wall_seconds"] for r in rs)
    return {"runs": len(rs), "audio_seconds": round(audio, 2), "wall_seconds": round(wall, 2),
            "cpu_seconds": round(sum(r["cpu_seconds"] for r in rs), 2),
            "realtime_factor": round(audio / wall, 1) if wall else None,
            "max_rss_mib": round(max(r["max_rss_kib"] for r in rs) / 1024, 1),
            "meeting_busy_during_any": any(r.get("meeting_busy_at_start", r.get("meeting_recording_at_start"))
                                           or r.get("meeting_busy_at_end", r.get("meeting_recording_at_end")) for r in rs)}


def pooled_der(scores):
    total = sum(s["reference_speaker_seconds"] for s in scores)
    out = {k: round(100 * sum(s[k] * s["reference_speaker_seconds"] for s in scores) / total, 3)
           for k in ("der", "missed", "false_alarm", "confusion")}
    out["exact_counts"] = f"{sum(s['hypothesis_speakers'] == s['reference_speakers'] for s in scores)}/{len(scores)}"
    out["hypothesis_speakers"] = [s["hypothesis_speakers"] for s in scores]
    return out


def ami(records, work, system, names, speakers=None):
    scores, rs, audio = [], [], 0.0
    suffix = f".k{speakers}" if speakers else ""
    for name in names:
        r = records.get(f"{name}.{system}{suffix}")
        if not r:
            return None
        scores.append(diarization_score.score(name, r["output"]["turns"], work / f"ami/{name}.rttm",
                                              work / f"ami/{name}.uem"))
        rs.append(r)
        audio += seconds(work / f"ami/{name}.wav")
    return {"score_percent": pooled_der(scores), "timing": timing(rs, audio),
            "per_recording_der_percent": {n: round(100 * s["der"], 2) for n, s in zip(names, scores)},
            "per_recording_confusion_percent": {n: round(100 * s["confusion"], 2) for n, s in zip(names, scores)}}


def disagreement(ref_turns, hyp_turns):
    ref = diarization_score.hypothesis_inputs(ref_turns)
    hyp = diarization_score.hypothesis_inputs(hyp_turns)
    return diarization_score.sweep(ref, hyp)


def weighted(items, key, weight):
    total = sum(i[weight] for i in items)
    return round(100 * sum(i[key] * i[weight] for i in items) / total, 2) if total else None


def real(records, work, system, aliases, baseline):
    mix, track, rs, audio = [], [], [], 0.0
    for alias in aliases:
        d = work / "real" / alias
        m, s = records.get(f"{alias}.mix.{system}"), records.get(f"{alias}.system.{system}")
        if not m or not s:
            return None
        c = channel_score.score(m["output"]["turns"], str(d / "microphone.wav"), str(d / "system.wav"))
        c["single"] = c["frames_local"] + c["frames_remote"]
        mix.append(c)
        base = records.get(f"{alias}.system.{baseline}")
        entry = {"clusters": len({t["speaker"] for t in s["output"]["turns"]}),
                 "speech_seconds": sum(t["end_ms"] - t["start_ms"] for t in s["output"]["turns"]) / 1000}
        if base and system != baseline and base["output"]["turns"] and s["output"]["turns"]:
            dis = disagreement(base["output"]["turns"], s["output"]["turns"])
            entry.update(disagreement=dis["der"], confusion_vs_baseline=dis["confusion"],
                         weight=dis["reference_speaker_seconds"])
        track.append(entry)
        rs += [m, s]
        audio += 2 * seconds(d / "system.wav")
    out = {
        "meetings": len(aliases), "audio_minutes_per_track": round(audio / 2 / 60, 1),
        "mix_vs_track_reference_percent": {
            "side_confusion": weighted(mix, "side_confusion", "single"),
            "side_missed": weighted(mix, "side_missed", "single"),
            "local_missed": weighted(mix, "local_missed", "frames_local"),
            "overlap_resolved": weighted(mix, "overlap_resolved", "frames_overlap"),
            "overlap_share_of_scored_speech": round(100 * sum(c["frames_overlap"] for c in mix)
                                                    / sum(c["single"] + c["frames_overlap"] for c in mix), 2)},
        "mix_clusters": [c["clusters"] for c in mix], "mix_clusters_material": [c["clusters_material"] for c in mix],
        "system_track_clusters": [e["clusters"] for e in track],
        "timing_mix_plus_system_track": timing(rs, audio),
    }
    if all("disagreement" in e for e in track):
        out["system_track_vs_baseline_percent"] = {"disagreement": weighted(track, "disagreement", "weight"),
                                                   "confusion": weighted(track, "confusion_vs_baseline", "weight")}
    return out


def judged(work, aliases, pack="judge"):
    results, key = work / f"{pack}-results.json", work / f"{pack}-key.json"
    if not results.exists():
        return None
    results, key = json.loads(results.read_text()), json.loads(key.read_text())
    out = {}
    for window, counts in results.items():
        if window.rsplit("-w", 1)[0] not in aliases:
            continue
        for column, system in key[window].items():
            agg = out.setdefault(system, {"windows": 0, "labelled_remote_lines": 0,
                                                        "wrong": 0, "unsure": 0, "unlabelled": 0})
            agg["windows"] += 1
            for k in ("labelled_remote_lines", "wrong", "unsure", "unlabelled"):
                agg[k] += counts[column][k]
    for agg in out.values():
        agg["wrong_speaker_percent"] = round(100 * agg["wrong"] / max(agg["labelled_remote_lines"], 1), 2)
    return out


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--work", type=Path, required=True)
    parser.add_argument("--systems", required=True)
    parser.add_argument("--baseline", default="baseline-cpu")
    args = parser.parse_args()
    records = load(args.work / "runs")
    aliases = sorted(p.name for p in (args.work / "real").iterdir())
    classes = {"en": [a for a in aliases if a.startswith("EN-")], "de": [a for a in aliases if a.startswith("DE-")],
               "pooled": aliases}
    out = {"ami_test_auto": {}, "ami_test_known4": {}, "ami_train_auto": {}, "real": {}, "judge": {}}
    for system in args.systems.split(","):
        out["ami_test_auto"][system] = ami(records, args.work, system, AMI_TEST)
        out["ami_test_known4"][system] = ami(records, args.work, system, AMI_TEST, 4)
        out["ami_train_auto"][system] = ami(records, args.work, system, AMI_TRAIN)
        out["real"][system] = {c: real(records, args.work, system, a, args.baseline) for c, a in classes.items()}
    out["judge"] = {c: judged(args.work, a) for c, a in classes.items()}
    print(json.dumps(out, indent=1))


if __name__ == "__main__":
    main()
