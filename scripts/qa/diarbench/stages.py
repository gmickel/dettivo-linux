"""One bench run, stage by stage: inputs, engines, assignment, scoring.

Returns the scoreboard: per split and engine, the per-file counts of metrics.py, plus
the stage ledger and the engine identities. Scoring results are cached by the keys of
everything they read and by the scoring code's own hash.
"""
from pathlib import Path

import channels
import engines
import metrics
from assign import Assigner, wants_voice
from cache import key
from data import database, label_window, meeting_row, recordings, reference_units

HERE = Path(__file__).resolve().parent
SCORING_CODE = [HERE / "metrics.py", HERE / "channels.py", HERE / "stages.py", HERE.parent / "diarization_score.py"]


def inputs(cache, cfg, rec, full, db):
    """(segments key, segments, wav, room_audio) for a recording, or None when missing. A
    meeting's segments are the stored ones, or with `asr.meetings` the configured Whisper's."""
    if rec["split"].startswith("ami"):
        k, segments = engines.asr_result(cache, cfg["asr"], rec["wav"], full)
        return None if segments is None else (k, segments, rec["wav"], True)
    try:
        segments, system_audio = meeting_row(db, rec["dir"])
    except LookupError:
        cache.note("segments", "missing")  # deleted since setup; the next setup drops it
        return None
    cache.note("segments", "read")
    if cfg["asr"].get("meetings"):
        segments = engines.meeting_asr_result(cache, cfg["asr"], rec, full)[1]
        if segments is None:
            return None
    window = label_window(rec["reference"])
    if window:
        segments = [s for s in segments if s["end_ms"] > window[0] and s["start_ms"] < window[1]]
    return key(segments), segments, (rec["system"] if system_audio else rec["mic"]), not system_audio


def score(rec, lines, result, mix, frames, report, mic_lines, before=None):
    """The file's counts from metrics.py (and channels.py for two-track meetings);
    `before` is the lines without the voice check, when it ran."""
    counts = {"voice_units": report.get("voice_checked", 0), "voice_moved": report.get("voice_moved", 0)}
    ref = rec["reference"]
    if ref:
        units = reference_units(ref)
        counts.update(metrics.attribution(lines, units))
        if before is not None:
            counts.update(metrics.fixed_broken(before, lines, units))
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
        if frames is not None:
            counts.update(channels.you_lines(lines, frames[0], frames[1]))
            counts.update(two_track=1, mic_lines=mic_lines, bleed_dropped=report.get("dropped_bleed", 0),
                          you_relabelled=report.get("you_relabelled", 0),
                          single_remote=int(report.get("single_remote", False)),
                          shared_mic=int(report.get("shared_mic", False)))
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
    embed = engines.embedder(cfg)
    if embed is None:
        print("no voice embedding model: the voice check and speaker prints see no voice")
        cache.note("embed", "no model")
    for name, engine in list(cfg["engines"].items()):
        if not engines.available(engine):
            print(f"{name}: {engine['binary']} is not installed, skipped")
            del cfg["engines"][name]
    assigner = Assigner(cache, variant, params)
    code = key([cache.file_hash(p) for p in SCORING_CODE])
    db = database()
    board = {"variant": variant, "params": params, "heldout": heldout, "splits": {},
             "engines": {n: {"label": e["label"], "identity": engines.identity(cache, e)}
                         for n, e in cfg["engines"].items()}}
    voice = wants_voice(params) and embed is not None
    queued = []
    for rec in recordings(root, heldout):
        prepared = inputs(cache, cfg, rec, full, db)
        if prepared is None:
            continue
        segments_key, segments, wav, room_audio = prepared
        two_track = not rec["split"].startswith("ami") and not room_audio
        frames_key, frames = channels.frames(cache, rec["mic"], rec["system"]) if two_track else (None, None)
        embed_key, vectors = (engines.embeddings(cache, embed["model"], rec, segments, full)
                              if two_track and embed else (None, []))
        extra = {"dir": str(rec["dir"]), "embeddings": vectors} if two_track else {}
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
            pre_key = assigner.job_key(segments_key, engine_key, room_audio, track_ms, window,
                                       (frames_key, embed_key) if two_track else None)
            probs = result.get("probs") and str(cache.path("engine", Path(result["probs"]).stem, ".npy"))
            job = {"room_audio": room_audio, "track_ms": track_ms, "segments": segments,
                   "turns": wire_turns(result["turns"]), "probs": probs, **extra}
            mic_lines = sum(s["source_type"] == "microphone" for s in segments)
            queued.append((rec, name, result, job, pre_key, wav, mix, mix_key, frames, frames_key, mic_lines))
    # The voice check (ADR 0076) embeds the sentence units the rule forms, so the units
    # come first, then their voices on the diarized track, then the labelling.
    spans = assigner.spans({q[4]: q[3] for q in queued}) if voice else {}
    jobs = []
    for rec, name, result, job, pre_key, wav, mix, mix_key, frames, frames_key, mic_lines in queued:
        assign_key, embed_run = pre_key, None
        if voice:
            embed_key, units, embed_run = engines.unit_embeddings(
                cache, embed["model"], wav, spans[pre_key], full, embed["threads"], embed["provider"])
            job = dict(job, unit_embeddings=units)
            assign_key = key(pre_key, embed_key)
        done = assigner.request(assign_key, job)
        jobs.append((rec, name, result, done, assign_key, mix, mix_key, frames, frames_key, mic_lines, embed_run))
    computed = assigner.run()
    for rec, name, result, done, assign_key, mix, mix_key, frames, frames_key, mic_lines, embed_run in jobs:
        done = done if done is not None else computed[assign_key]
        lines, report = done["lines"], done["report"]
        ref = rec["reference"]
        ref_files = [] if not ref else [v for k, v in ref.items() if k != "kind"]
        score_key = key(code, assign_key, [cache.file_hash(p) for p in ref_files], mix_key, frames_key)
        counts = cache.get("score", score_key)
        if counts is None:
            counts = score(rec, lines, result, mix, frames, report, mic_lines, done.get("before"))
            cache.put("score", score_key, counts)
            cache.note("score", "computed")
        else:
            cache.note("score", "cached")
        counts = dict(counts, audio_s=result["run"]["audio_seconds"], engine_wall_s=result["run"]["wall_seconds"])
        if embed_run:
            counts.update(voice_audio_s=result["run"]["audio_seconds"], voice_embed_s=embed_run["wall_seconds"])
        cell = board["splits"].setdefault(rec["split"], {}).setdefault(
            name, {"files": {}, "audio_minutes": 0.0})
        cell["files"][rec["id"]] = counts
        window = label_window(ref)
        seconds = result["run"]["audio_seconds"]
        cell["audio_minutes"] += (min(seconds, (window[1] - window[0]) / 1000) if window else seconds) / 60
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
