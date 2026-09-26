"""One bench run, stage by stage: inputs, engines, assignment, scoring.

Returns the scoreboard: per split and engine, the per-file counts of metrics.py, plus
the stage ledger and the engine identities. Scoring results are cached by the keys of
everything they read and by the scoring code's own hash.
"""
from pathlib import Path

import channels
import engines
import metrics
from assign import Assigner
from cache import key
from data import database, label_window, meeting_row, recordings, reference_units

HERE = Path(__file__).resolve().parent
SCORING_CODE = [HERE / "metrics.py", HERE / "channels.py", HERE / "stages.py", HERE.parent / "diarization_score.py"]


def inputs(cache, cfg, rec, full, db):
    """(segments key, segments, wav, room_audio) for a recording, or None when missing."""
    if rec["split"].startswith("ami"):
        k, segments = engines.asr_result(cache, cfg["asr"], rec["wav"], full)
        return None if segments is None else (k, segments, rec["wav"], True)
    segments, system_audio = meeting_row(db, rec["dir"])
    cache.note("segments", "read")
    window = label_window(rec["reference"])
    if window:
        segments = [s for s in segments if s["end_ms"] > window[0] and s["start_ms"] < window[1]]
    return key(segments), segments, (rec["system"] if system_audio else rec["mic"]), not system_audio


def score(rec, lines, result, mix, frames):
    """The file's counts from metrics.py (and channels.py for two-track meetings)."""
    counts = {}
    ref = rec["reference"]
    if ref:
        units = reference_units(ref)
        counts.update(metrics.attribution(lines, units))
        if ref["kind"] == "ami":
            labelled = [x for x in lines if x["speaker"] is not None]
            counts.update(metrics.line_view(lines, rttm_turns(ref["rttm"])))
            counts.update(metrics.der("der", rec["id"], labelled, ref["rttm"], ref["uem"]))
            counts.update(metrics.der("engine", rec["id"], result["turns"], ref["rttm"], ref["uem"]))
        else:
            counts.update(metrics.line_view(lines, units))
    if not rec["split"].startswith("ami"):
        counts.update(metrics.remote_lines(lines))
        if frames is not None and mix is not None:
            counts.update(channels.mix_proxy(mix["turns"], *frames))
    return counts


def rttm_turns(path):
    out = []
    for line in Path(path).read_text().splitlines():
        f = line.split()
        if f and f[0] == "SPEAKER":
            start = round(float(f[3]) * 1000)
            out.append({"start_ms": start, "end_ms": start + round(float(f[4]) * 1000), "speaker": f[7]})
    return out


def run(root, cfg, cache, variant, params, full, heldout, python):
    assigner = Assigner(cache, variant, params)
    code = key([cache.file_hash(p) for p in SCORING_CODE])
    db = database()
    board = {"variant": variant, "params": params, "heldout": heldout, "splits": {},
             "engines": {n: {"label": e["label"], "identity": engines.identity(cache, e)}
                         for n, e in cfg["engines"].items()}}
    jobs = []
    for rec in recordings(root, heldout):
        prepared = inputs(cache, cfg, rec, full, db)
        if prepared is None:
            continue
        segments_key, segments, wav, room_audio = prepared
        two_track = not rec["split"].startswith("ami") and not room_audio
        frames_key, frames = channels.frames(cache, rec["mic"], rec["system"]) if two_track else (None, None)
        for name, engine in cfg["engines"].items():
            engine_key, result = engines.engine_result(cache, name, engine, wav, full, python)
            if result is None:
                continue
            mix_key, mix = (None, None)
            if two_track:
                mix_key, mix = engines.engine_result(cache, name, engine, engines.mix_wav(cache, rec["mic"],
                                                                                         rec["system"]),
                                                     full, python)
            window = label_window(rec["reference"])
            track_ms = round(engines.seconds(wav) * 1000)
            assign_key = assigner.job_key(segments_key, engine_key, room_audio, track_ms, window)
            probs = result.get("probs") and str(cache.path("engine", Path(result["probs"]).stem, ".npy"))
            lines = assigner.request(assign_key, {"room_audio": room_audio, "track_ms": track_ms,
                                                  "segments": segments, "turns": wire_turns(result["turns"]),
                                                  "probs": probs})
            jobs.append((rec, name, result, lines, assign_key, mix, mix_key, frames, frames_key))
    computed = assigner.run()
    for rec, name, result, lines, assign_key, mix, mix_key, frames, frames_key in jobs:
        lines = lines if lines is not None else computed[assign_key]
        ref = rec["reference"]
        ref_files = [] if not ref else [v for k, v in ref.items() if k != "kind"]
        score_key = key(code, assign_key, [cache.file_hash(p) for p in ref_files], mix_key, frames_key)
        counts = cache.get("score", score_key)
        if counts is None:
            counts = score(rec, lines, result, mix, frames)
            cache.put("score", score_key, counts)
            cache.note("score", "computed")
        else:
            cache.note("score", "cached")
        counts = dict(counts, audio_s=result["run"]["audio_seconds"], engine_wall_s=result["run"]["wall_seconds"])
        cell = board["splits"].setdefault(rec["split"], {}).setdefault(
            name, {"files": {}, "audio_minutes": 0.0})
        cell["files"][rec["id"]] = counts
        cell["audio_minutes"] += result["run"]["audio_seconds"] / 60
    pool(board["splits"], "local", ("local-en", "local-de"))
    board["splits"] = {s: board["splits"][s] for s in sorted(board["splits"], key=split_order)}
    board["stages"] = {stage: dict(c) for stage, c in cache.ledger.items()}
    return board


def pool(splits, name, parts):
    """A split holding every file of `parts` (English and German meetings pooled)."""
    for part in parts:
        for engine, cell in splits.get(part, {}).items():
            into = splits.setdefault(name, {}).setdefault(engine, {"files": {}, "audio_minutes": 0.0})
            into["files"].update(cell["files"])
            into["audio_minutes"] += cell["audio_minutes"]


def wire_turns(turns):
    """Turns as the engine protocol carries them: string speaker labels (the fn-64
    Nemotron runner prints channel numbers)."""
    return [{"start_ms": t["start_ms"], "end_ms": t["end_ms"], "speaker": str(t["speaker"])} for t in turns]


def split_order(split):
    from data import SPLITS
    return SPLITS.index(split) if split in SPLITS else len(SPLITS)
