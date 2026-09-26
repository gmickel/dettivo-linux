"""The engine and ASR stages: the product's engine binaries (and, until fn-69, the
fn-64 Nemotron ONNX runner) on cached audio, one fresh niced process per job.

A result is keyed by the audio's hash and the engine's identity (binary or runner hash,
model hash, parameters). A plain run never starts an engine: a missing key falls back
to the newest result for the same audio and engine name, marked stale, or is missing.
`--full` computes every missing key, waiting while a meeting records or finalises.
"""
import json
import os
import subprocess
import sys
import tempfile
import time
import wave
from pathlib import Path

from cache import key
from data import REPO, busy

NEMOTRON_RUNNER = REPO / "scripts/qa/nemotron3/nemotron3_diarize.py"


def identity(cache, engine):
    """What an engine result depends on, without paths."""
    if engine["kind"] == "diarize":
        program = cache.file_hash(engine["binary"])
    elif engine["kind"] == "nemotron-onnx":
        program = cache.file_hash(NEMOTRON_RUNNER)
    else:
        raise SystemExit(f"unknown engine kind {engine['kind']}")
    return {"kind": engine["kind"], "program": program, "model": cache.tree_hash(engine["model"]),
            "threads": engine["threads"], "provider": engine["provider"]}


def asr_identity(cache, asr):
    return {"program": cache.file_hash(asr["binary"]), "model": cache.file_hash(asr["model"]),
            "language": asr["language"]}


def seconds(wav):
    with wave.open(str(wav)) as w:
        return w.getnframes() / w.getframerate()


def idle():
    """No meeting recording or finalising, by the daemon's health and the database."""
    try:
        out = subprocess.run(["dettivo", "--json", "status", "health"], capture_output=True, text=True,
                             timeout=30).stdout
        health = json.loads(out)
    except (OSError, ValueError, subprocess.TimeoutExpired):
        return not busy()  # no daemon running: the database alone decides
    return health.get("recording_state") == "idle" and not health.get("active_jobs") and not busy()


def wait_idle():
    announced = False
    while not idle():
        if not announced:
            print("waiting: a meeting is recording or finalising", file=sys.stderr, flush=True)
            announced = True
        time.sleep(60)


def timed(argv):
    """Runs argv niced once the machine is idle; returns (stdout bytes, run record)."""
    wait_idle()
    with tempfile.TemporaryFile() as out, tempfile.TemporaryFile() as err:
        started = time.perf_counter()
        proc = subprocess.Popen(["nice", "-n", "19", "ionice", "-c3", *argv], stdout=out, stderr=err)
        _, status, usage = os.wait4(proc.pid, 0)
        wall = time.perf_counter() - started
        code = os.waitstatus_to_exitcode(status)
        if code:
            err.seek(0)
            tail = err.read().decode(errors="replace").strip().splitlines()[-3:]
            raise RuntimeError(f"{Path(argv[0]).name} exited {code}: {' | '.join(tail)}")
        out.seek(0)
        return out.read(), {"wall_seconds": round(wall, 3), "cpu_seconds": round(usage.ru_utime + usage.ru_stime, 3),
                            "max_rss_kib": usage.ru_maxrss}


def diarize(engine, wav, probs_path, python):
    if engine["kind"] == "diarize":
        argv = [engine["binary"], "--wav", str(wav), "--model", engine["model"], "--json",
                "--threads", str(engine["threads"]), "--provider", engine["provider"]]
    else:
        argv = [python, str(NEMOTRON_RUNNER), "--wav", str(wav), "--model", engine["model"],
                "--provider", engine["provider"], "--threads", str(engine["threads"]), "--probs", str(probs_path)]
    out, run = timed(argv)
    return json.loads(out)["turns"], run


def engine_result(cache, engine_name, engine, wav, full, python):
    """(key, result) for `engine` on `wav`: {"turns", "run", "probs"}; result None when
    missing. Probabilities stay beside the result as engine/<key>.npy."""
    audio = cache.file_hash(wav)
    k = f"{engine_name}-" + key(audio, identity(cache, engine))
    slot = f"{audio}:{engine_name}"
    hit = cache.get("engine", k)
    if hit is not None:
        cache.note("engine", "cached")
        return k, hit
    if not full:
        stale_key, stale = cache.newest("engine", slot)
        cache.note("engine", "stale" if stale else "missing")
        return stale_key, stale
    probs = cache.path("engine", k, ".npy") if engine["kind"] == "nemotron-onnx" else None
    print(f"running {engine_name} on {Path(wav).name}", file=sys.stderr, flush=True)
    turns, run = diarize(engine, wav, probs, python)
    run["audio_seconds"] = seconds(wav)
    result = {"turns": turns, "run": run, "probs": probs.name if probs else None}
    cache.put("engine", k, result, index=slot)
    cache.note("engine", "computed")
    return k, result


def asr_result(cache, asr, wav, full):
    """(key, segments) for AMI audio through the product's Whisper engine CLI; the
    segments are product Segment objects, zero-length ones dropped."""
    audio = cache.file_hash(wav)
    k = key(audio, asr_identity(cache, asr))
    slot = f"{audio}:whisper"
    hit = cache.get("asr", k)
    if hit is not None:
        cache.note("asr", "cached")
        return k, hit["segments"]
    if not full:
        stale_key, stale = cache.newest("asr", slot)
        cache.note("asr", "stale" if stale else "missing")
        return stale_key, stale and stale["segments"]
    print(f"running whisper on {Path(wav).name}", file=sys.stderr, flush=True)
    out, run = timed([asr["binary"], "--wav", str(wav), "--model", asr["model"], "--language", asr["language"],
                      "--json"])
    segments = product_segments(json.loads(out)["segments"])
    cache.put("asr", k, {"segments": segments, "run": run}, index=slot)
    cache.note("asr", "computed")
    return k, segments


def product_segments(raw):
    """Engine CLI segments as the product's Segment objects (room audio, no speaker)."""
    out = []
    for s in raw:
        if s["end_ms"] <= s["start_ms"]:
            continue
        seg = {"index": len(out), "start_ms": s["start_ms"], "end_ms": s["end_ms"], "text": s.get("text", ""),
               "speaker": None, "source_type": "microphone"}
        if s.get("words"):
            seg["words"] = s["words"]
        out.append(seg)
    return out


def mix_path(cache, mic, system):
    return cache.path("audio", key(cache.file_hash(mic), cache.file_hash(system), "mix"), ".wav")


def mix_wav(cache, mic, system):
    """The two tracks summed and clipped (fn-64's mix), cached by both tracks' hashes."""
    import numpy as np
    path = mix_path(cache, mic, system)
    if path.exists():
        return path
    tracks = []
    for p in (mic, system):
        with wave.open(str(p), "rb") as w:
            assert (w.getframerate(), w.getnchannels(), w.getsampwidth()) == (16000, 1, 2), "16 kHz mono PCM16"
            tracks.append(np.frombuffer(w.readframes(w.getnframes()), dtype="<i2"))
    n = min(len(t) for t in tracks)
    mix = np.clip(tracks[0][:n].astype(np.int32) + tracks[1][:n].astype(np.int32), -32768, 32767).astype("<i2")
    tmp = path.with_suffix(".tmp")
    with wave.open(str(tmp), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(16000)
        w.writeframes(mix.tobytes())
    os.replace(tmp, path)
    return path
