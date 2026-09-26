#!/usr/bin/env python3
"""`just diar-label`: label who said each transcript line of a retained meeting.

Opens a page on 127.0.0.1 over one meeting of the bench's selection, or the excerpt of
it with the most speaker changes. Drafts blend every engine's cached output in the
bench cache, never one system's, and lines where the engines disagree come first.
Progress stays in <eval>/labelling/<alias>.json; when every line is decided the labels
go to <eval>/labels/<alias>.json in the bench's format, where `just diar-bench` picks
them up. See docs/diarization-labelling.md.
"""
import argparse
from collections import Counter
import json
import os
import string
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

from cache import Cache  # noqa: E402
from data import config, database, eval_dir, meeting_row, selection  # noqa: E402
import engines  # noqa: E402
from metrics import Lines, mapping  # noqa: E402

MINUTES = (10, 20)


def meeting_lines(segments):
    """The product's segments as labelling lines, in time order."""
    lines = [{"start_ms": s["start_ms"], "end_ms": s["end_ms"], "source": s.get("source_type") or "microphone",
              "text": s.get("text") or "", "words": len((s.get("text") or "").split())}
             for s in segments if s["end_ms"] > s["start_ms"]]
    return sorted(lines, key=lambda x: (x["start_ms"], x["end_ms"]))


def winner(index, line):
    """The engine speaker with the most overlap with the line, or None."""
    shared = Counter()
    for turn, ms in index.overlapping(line["start_ms"], line["end_ms"]):
        shared[turn["speaker"]] += ms
    return max(shared, key=shared.get) if shared else None


def letter(i):
    return string.ascii_uppercase[i] if i < 26 else f"S{i + 1}"


def blend(lines, outputs):
    """Per line: the letters the engines propose (unordered, so the page cannot tell which
    engine said what), `agree` when every engine proposes the same speaker, and that
    speaker as the draft. The first engine's speakers are the letter space; each other
    engine maps onto it under the best one-to-one map by shared line time."""
    if len(outputs) < 2:
        raise ValueError("a draft blends at least two engines' outputs")
    names = list(outputs)
    indexes = {n: Lines(outputs[n]) for n in names}
    picks = {n: [winner(indexes[n], x) for x in lines] for n in names}
    ref = names[0]
    clusters = {ref: [None if p is None else f"{ref}:{p}" for p in picks[ref]]}
    for n in names[1:]:
        onto = mapping((p, r, x["end_ms"] - x["start_ms"]) for p, r, x in zip(picks[n], picks[ref], lines)
                       if p is not None and r is not None)
        clusters[n] = [None if p is None else f"{ref}:{onto[p]}" if p in onto else f"{n}:{p}" for p in picks[n]]
    letters = {}
    for i in range(len(lines)):
        for n in names:
            c = clusters[n][i]
            if c is not None and c not in letters:
                letters[c] = letter(len(letters))
    out = []
    for i in range(len(lines)):
        proposed = [None if clusters[n][i] is None else letters[clusters[n][i]] for n in names]
        agree = None not in proposed and len(set(proposed)) == 1
        out.append({"candidates": sorted({p for p in proposed if p}), "status": "agree" if agree else "disagree",
                    "speaker": proposed[0] if agree else None})
    return out


def pick_window(lines, guesses, minutes):
    """The [start_ms, end_ms) window of `minutes` holding the most changes of guessed
    speaker between consecutive lines (the earliest on a tie); None when the meeting
    is no longer than the window."""
    span = minutes * 60_000
    end = max(x["end_ms"] for x in lines)
    if end - lines[0]["start_ms"] <= span:
        return None
    changes = [0]
    for a, b in zip(guesses, guesses[1:]):
        changes.append(changes[-1] + (a != b))
    best, j = None, 0
    for i, line in enumerate(lines):
        start = line["start_ms"]
        if start + span > end:
            break
        while j + 1 < len(lines) and lines[j + 1]["start_ms"] < start + span:
            j += 1
        count = changes[j] - changes[i]
        if best is None or count > best[0]:
            best = (count, start)
    return [best[1], best[1] + span]


def in_window(line, window):
    """The bench's excerpt rule (stages.inputs): a line that overlaps the window."""
    return window is None or (line["end_ms"] > window[0] and line["start_ms"] < window[1])


def session(alias, language, segments, outputs, draft, minutes, whole=False):
    """A new labelling session: the excerpt's lines with their drafts."""
    lines = meeting_lines(segments)
    if not lines:
        raise ValueError(f"{alias} has no transcript lines")
    drafts = blend(lines, outputs) if len(outputs) >= 2 else None
    if draft == "blend" and drafts is None:
        raise ValueError("a blend needs two engines' cached outputs: run `just diar-bench --full`, "
                         "or label from a blank draft with --draft blank")
    guesses = ([d["speaker"] or (d["candidates"] or [None])[0] for d in drafts] if drafts
               else [x["source"] for x in lines])
    window = None if whole else pick_window(lines, guesses, minutes)
    lines = [x for x in lines if in_window(x, window)]
    if draft == "blend":
        drafts = blend(lines, outputs)
        # Letters in order of how many lines propose them, so key 1 is the most frequent speaker.
        seen = Counter(c for d in drafts for c in d["candidates"])
        rename = {s: letter(i) for i, s in enumerate(sorted(seen, key=lambda s: (-seen[s], s)))}
        for line, d in zip(lines, drafts):
            line.update(candidates=sorted(rename[c] for c in d["candidates"]), status=d["status"],
                        speaker=rename.get(d["speaker"]), confirmed=False)
        speakers = list(rename.values())
        tag = "blend:" + "+".join(outputs)
    else:
        for line in lines:
            line.update(candidates=[], status="blank", speaker=None, confirmed=False)
        speakers, tag = [], "blank"
    return {"schema": 1, "alias": alias, "language": language, "draft": tag, "window_ms": window,
            "speakers": speakers, "names": {}, "lines": lines}


def apply(state, update):
    """Takes the page's labels into the session; raises ValueError on a malformed update."""
    if not isinstance(update, dict):
        raise ValueError("an update is an object")
    speakers, names, lines = update.get("speakers"), update.get("names"), update.get("lines")
    if not (isinstance(speakers, list) and all(isinstance(s, str) and s for s in speakers)):
        raise ValueError("speakers: a list of non-empty strings")
    if not (isinstance(names, dict) and all(k in speakers and isinstance(v, str) for k, v in names.items())):
        raise ValueError("names: a map of listed speakers to strings")
    if not (isinstance(lines, list) and len(lines) == len(state["lines"])):
        raise ValueError("lines: one entry per session line")
    for x in lines:
        if not (isinstance(x, dict) and (x.get("speaker") is None or x.get("speaker") in speakers)
                and isinstance(x.get("confirmed"), bool)):
            raise ValueError("each line: a listed speaker or null, and confirmed true or false")
    state["speakers"], state["names"] = speakers, {k: v.strip() for k, v in names.items() if v.strip()}
    for line, x in zip(state["lines"], lines):
        line["speaker"], line["confirmed"] = x["speaker"], x["confirmed"]


def complete(state):
    return all(x["confirmed"] for x in state["lines"])


def labels(state):
    """The bench's labels file (docs/diarization-bench.md#labels) for a finished session."""
    names = state["names"]
    return {"schema": 1, "alias": state["alias"], "language": state["language"], "draft": state["draft"],
            "window_ms": state["window_ms"],
            "lines": [{"start_ms": x["start_ms"], "end_ms": x["end_ms"], "source": x["source"],
                       "speaker": None if x["speaker"] is None else names.get(x["speaker"], x["speaker"]),
                       "words": x["words"]} for x in state["lines"]]}


def write_private(path, value):
    """Writes JSON readable by the owner only, atomically."""
    path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    tmp = path.with_suffix(".tmp")
    fd = os.open(tmp, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(fd, "w") as f:
        json.dump(value, f)
    os.replace(tmp, path)


def engine_outputs(root, entry, system_audio):
    """{engine: turns} from the bench cache for the track the drafts read: the summed
    tracks of a two-track meeting (one speaker space for both sides), else the microphone."""
    cache = Cache(root)
    d = Path(entry["dir"])
    wav = engines.mix_path(cache, d / "microphone.wav", d / "system.wav") if system_audio else d / "microphone.wav"
    found = {}
    if wav.exists():
        audio = cache.file_hash(wav)
        for name in config(root)["engines"]:
            _, result = cache.newest("engine", f"{audio}:{name}")
            if result:
                found[name] = [dict(t, speaker=str(t["speaker"])) for t in result["turns"]]
    cache.save()
    return found


def listing(root, chosen):
    with database() as db:
        for alias, entry in sorted(chosen.items()):
            segments, _ = meeting_row(db, entry["dir"])
            minutes = max((s["end_ms"] for s in segments), default=0) / 60_000
            progress = root / "labelling" / f"{alias}.json"
            state = ("labelled" if (root / "labels" / f"{alias}.json").exists() else
                     "in progress" if progress.exists() else "")
            print(f"{alias:6} {entry['language']}  {minutes:5.0f} min  {state}")


def parse():
    parser = argparse.ArgumentParser(prog="just diar-label", description=__doc__.splitlines()[0])
    parser.add_argument("alias", nargs="?", help="a meeting of the bench's selection; omit to list them")
    parser.add_argument("--minutes", type=int, default=15, help="excerpt length, 10 to 20 (default 15)")
    parser.add_argument("--whole", action="store_true", help="label the whole meeting, not an excerpt")
    parser.add_argument("--draft", choices=("blend", "blank"), default="blend", help="how lines are prefilled")
    parser.add_argument("--port", type=int, default=0, help="port on 127.0.0.1 (default: any free one)")
    args = parser.parse_args()
    if not MINUTES[0] <= args.minutes <= MINUTES[1]:
        parser.error(f"--minutes must be {MINUTES[0]} to {MINUTES[1]}")
    return args


def main():
    args = parse()
    root = eval_dir()
    chosen = selection(root)
    if not chosen:
        raise SystemExit(f"{root} has no selection: run `just diar-bench-setup`")
    if not args.alias:
        listing(root, chosen)
        return
    if args.alias not in chosen:
        raise SystemExit(f"{args.alias} is not in the selection ({', '.join(sorted(chosen))})")
    entry = chosen[args.alias]
    path = root / "labelling" / f"{args.alias}.json"
    if path.exists():
        state = json.loads(path.read_text())
        print(f"resuming {path} (delete it to start over)")
    else:
        with database() as db:
            segments, system_audio = meeting_row(db, entry["dir"])
        outputs = engine_outputs(root, entry, system_audio)
        try:
            state = session(args.alias, entry["language"], segments, outputs, args.draft, args.minutes, args.whole)
        except ValueError as error:
            raise SystemExit(str(error))
        write_private(path, state)
    import label_server
    label_server.run(state, path, root / "labels" / f"{args.alias}.json", Path(entry["dir"]), args.port)


if __name__ == "__main__":
    main()
