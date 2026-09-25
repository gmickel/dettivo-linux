#!/usr/bin/env python3
"""Build blinded speaker-attribution packets for a model judge (fn-64), locally.

For each staged meeting alias, the product's own timed transcript segments (read-only
from the Dettivo database) get a speaker from each system's system-track turns, the way
the speaker pass assigns one: the speaker overlapping the segment most, none when no turn
overlaps. Microphone segments are `You`. Windows of consecutive segments are sampled
with a fixed seed; in each, the two systems are shown as columns A and B in a random
order, with labels renumbered R1, R2, ... by first appearance so neither system is
recognisable. The A/B key goes to a separate file the judge never reads.

Everything written here holds transcript text and stays in the protected directory.
"""
import argparse
import json
import random
import sqlite3
from pathlib import Path

DB = Path.home() / ".local/share/dettivo/dettivo.db"


def assign(segment, turns):
    best, speaker = 0, None
    for t in turns:
        shared = min(segment["end_ms"], t["end_ms"]) - max(segment["start_ms"], t["start_ms"])
        if shared > best:
            best, speaker = shared, str(t["speaker"])
    return speaker


def relabel(labels):
    order, out = [], []
    for label in labels:
        if label in ("You", None):
            out.append(label or "?")
            continue
        if label not in order:
            order.append(label)
        out.append(f"R{order.index(label) + 1}")
    return out


def stamp(ms):
    return f"{ms // 60000:02d}:{ms // 1000 % 60:02d}"


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--selection", type=Path, required=True)
    parser.add_argument("--runs", type=Path, required=True, help="directory with <alias>.system.<system>.json")
    parser.add_argument("--systems", required=True, help="two systems, comma-separated")
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--windows", type=int, default=3)
    parser.add_argument("--size", type=int, default=60)
    parser.add_argument("--seed", type=int, default=64)
    args = parser.parse_args()
    systems = args.systems.split(",")
    rng = random.Random(args.seed)
    args.out.mkdir(parents=True, exist_ok=True)
    key = {}
    db = sqlite3.connect(f"file:{DB}?mode=ro", uri=True)
    for alias, directory in json.loads(args.selection.read_text()).items():
        (raw,) = db.execute("select segments from meetings where id = ?", (Path(directory).name,)).fetchone()
        segments = sorted(json.loads(raw), key=lambda s: s["start_ms"])
        turns = {s: json.loads((args.runs / f"{alias}.system.{s}.json").read_text())["turns"] for s in systems}
        labels = {s: [("You" if seg["source_type"] == "microphone" else assign(seg, turns[s])) for seg in segments]
                  for s in systems}
        starts = [i for i in range(0, max(len(segments) - args.size, 1), args.size)
                  if sum(seg["source_type"] == "system" for seg in segments[i:i + args.size]) >= 20]
        for w, start in enumerate(sorted(rng.sample(starts, min(args.windows, len(starts))))):
            name = f"{alias}-w{w + 1}"
            order = systems[:]
            rng.shuffle(order)
            key[name] = {"A": order[0], "B": order[1]}
            span = slice(start, start + args.size)
            cols = [relabel(labels[s][span]) for s in order]
            lines = [f"{i + 1:>3} [{stamp(seg['start_ms'])}] A:{a:<3} B:{b:<3} | {seg['text'].strip()}"
                     for i, (seg, a, b) in enumerate(zip(segments[span], *cols))]
            (args.out / f"{name}.txt").write_text("\n".join(lines) + "\n")
    (args.out.parent / "judge-key.json").write_text(json.dumps(key, indent=1))
    print(f"{len(key)} windows")


if __name__ == "__main__":
    main()
