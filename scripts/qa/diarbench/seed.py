#!/usr/bin/env python3
"""Seed the bench cache from fn-64's protected working directory, so nothing it already
computed runs again: the current engine's and Nemotron's turns (automatic count) on the
AMI test meetings and on each staged meeting's system track and mix, Nemotron's frame
probabilities, the run records (wall time) and Whisper's AMI test segments.

A result is imported only under the identity it was produced with: the engine binaries
and models must still hash to what the fn-64 receipt recorded, or the step says so and
imports nothing for that system. Idempotent.
"""
import json
import shutil
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import engines  # noqa: E402
from cache import Cache, key  # noqa: E402
from data import config, eval_dir, selection  # noqa: E402
from setup import FN64, RECEIPT  # noqa: E402

SYSTEMS = {"baseline-cpu": "current", "nemotron-cpu-int8": "nemotron"}


def receipt_matches(cache, cfg):
    receipt = json.loads(RECEIPT.read_text())
    current, asr = cfg["engines"]["current"], cfg["asr"]
    model = Path(current["model"])
    ok = {
        "current": cache.file_hash(current["binary"]) == receipt["systems"]["baseline-cpu"]["engine_sha256"]
        and all(cache.file_hash(model / f) == h for f, h in receipt["models"]["diarization-en"].items()),
        "nemotron": cache.file_hash(cfg["engines"]["nemotron"]["model"])
        == receipt["models"]["nemotron3_onnx_sha256"]["model_quantized.onnx"],
        "whisper": cache.file_hash(asr["binary"]) == receipt["systems"]["whisper-ami-segments"]["engine_sha256"]
        and cache.file_hash(asr["model"]) == receipt["systems"]["whisper-ami-segments"]["model_sha256"],
    }
    for name, match in ok.items():
        if not match:
            print(f"seed: {name} no longer matches the fn-64 receipt; its results are not imported")
    return ok


def wav_for(root, cache, record, meetings):
    """The audio a fn-64 job ran on, as the bench names it."""
    job = record["id"]
    if "." not in job:
        return root / "ami" / f"{job}.wav"
    alias, track = job.split(".", 1)
    if alias not in meetings:
        return None
    directory = Path(meetings[alias]["dir"])
    if track == "system":
        return directory / "system.wav"
    source = FN64 / "real" / alias / "mix.wav"
    path = engines.mix_path(cache, directory / "microphone.wav", directory / "system.wav")
    if not path.exists() and source.exists():
        shutil.copyfile(source, path)
    return path


def main():
    root = eval_dir()
    cfg = config(root)
    cache = Cache(root)
    ok = receipt_matches(cache, cfg)
    meetings = selection(root)
    imported = 0
    for line in (FN64 / "runs/runs.jsonl").read_text().splitlines():
        record = json.loads(line)
        name = SYSTEMS.get(record["system"])
        if not name or not ok[name] or record["exit"] or record.get("speakers"):
            continue
        wav = wav_for(root, cache, record, meetings)
        if wav is None or not wav.exists():
            continue
        engine = cfg["engines"][name]
        k = f"{name}-" + key(cache.file_hash(wav), engines.identity(cache, engine))
        if cache.get("engine", k) is not None:
            continue
        output = json.loads((FN64 / "runs" / f"{record['key']}.json").read_text())
        run = {"wall_seconds": record["wall_seconds"], "cpu_seconds": record["cpu_seconds"],
               "max_rss_kib": record["max_rss_kib"], "audio_seconds": engines.seconds(wav), "seeded_from": "fn-64"}
        probs = FN64 / "r6/probs" / f"{record['id']}.npy"
        if name == "nemotron" and probs.exists():
            shutil.copyfile(probs, cache.path("engine", k, ".npy"))
        cache.put("engine", k, {"turns": output["turns"], "run": run,
                                "probs": f"{k}.npy" if name == "nemotron" and probs.exists() else None},
                  index=f"{cache.file_hash(wav)}:{name}")
        imported += 1
    if ok["whisper"]:
        identity = engines.asr_identity(cache, cfg["asr"])
        for path in sorted((FN64 / "r6/whisper").glob("*.json")):
            wav = root / "ami" / f"{path.stem}.wav"
            if not wav.exists():
                continue
            k = key(cache.file_hash(wav), identity)
            if cache.get("asr", k) is None:
                segments = engines.product_segments(json.loads(path.read_text())["segments"])
                cache.put("asr", k, {"segments": segments, "run": {"seeded_from": "fn-64"}},
                          index=f"{cache.file_hash(wav)}:whisper")
                imported += 1
    cache.save()
    print(f"seed: {imported} results imported from fn-64")


if __name__ == "__main__":
    main()
