#!/usr/bin/env python3
"""`just diar-bench-setup`: prepare the protected eval directory for the diarization bench.

Idempotent; each step skips what is already in place:
1. the directory itself, mode 700;
2. the Python environment (.venv: numpy and ONNX Runtime, pinned);
3. AMI dev and test audio (Mix-Headset), the pyannote only_words references and the
   word timings from the AMI manual annotations, each checked against a pinned hash;
4. the Nemotron ONNX model, checked against the fn-64 receipt's hashes, and a check
   that the product's diarization and Whisper models are installed;
5. selection.json: the finished retained meetings in English and German, never one
   that is recording or finalising (its speaker pass or analysis queued or running);
   existing aliases are kept;
6. the fn-64 working directory, when present, seeds the cache (seed.py).
"""
import argparse
import hashlib
import io
import json
import os
import shutil
import subprocess
import sys
import tarfile
import time
import urllib.error
import urllib.request
import wave
import xml.etree.ElementTree as ET
import zipfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from data import AMI_DEV, AMI_TEST, config, database, eval_dir  # noqa: E402

PYTHON_PACKAGES = ["numpy==2.5.3", "onnxruntime-gpu==1.30.0"]
AMI_AUDIO = "https://groups.inf.ed.ac.uk/ami/AMICorpusMirror/amicorpus/{0}/audio/{0}.Mix-Headset.wav"
AMI_SETUP = ("https://github.com/pyannote/AMI-diarization-setup/archive/"
             "67c2d539286e89f68952d5dcf83912bd9f01dfae.tar.gz")
# sha256 of the [(path, sha256)] list of the dev and test RTTM and UEM files used here.
AMI_SETUP_MANIFEST = "df828ba9641efd189fe255d0649edea23d1136046671f020ac3cb3eaa608a900"
AMI_WORDS = "https://groups.inf.ed.ac.uk/ami/AMICorpusAnnotations/ami_public_manual_1.6.2.zip"
AMI_WORDS_SHA256 = "b56e5babb2496b8795deeeda7e71178d7fbc9963f94276cf2a3f4b56ebbc9f9d"
NEMOTRON = ("https://huggingface.co/onnx-community/Nemotron-3-Diarization-ONNX/resolve/"
            "353b6f8ad2cac3580e982d7fbdf0a010786b0406/onnx/{}")
RECEIPT = HERE.parents[2] / "docs/reports/benchmarks/diarization-nemotron3-2026-09-25.json"
FN64 = Path.home() / ".local/share/dettivo-eval/fn64"
MIN_MEETING_MS = 120_000


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 22), b""):
            h.update(block)
    return h.hexdigest()


def fetch(url, dest, expect=None):
    part = dest.with_name(dest.name + ".part")
    print(f"  fetching {url.rsplit('/', 1)[-1]}", flush=True)
    for attempt in range(3):
        try:
            urllib.request.urlretrieve(url, part)
            break
        except (urllib.error.URLError, OSError) as error:
            if attempt == 2:
                raise SystemExit(f"{dest.name}: {error}")
            print(f"  retrying after: {error}", flush=True)
            time.sleep(30 * (attempt + 1))  # the AMI mirror throttles bursts with 403s
    if expect and sha256(part) != expect:
        part.unlink()
        raise SystemExit(f"{dest.name}: checksum mismatch")
    os.replace(part, dest)


def environment(root):
    python = root / ".venv/bin/python"
    if python.exists():
        return
    print("python environment")
    uv = shutil.which("uv")
    if uv:
        subprocess.run([uv, "venv", "-q", str(root / ".venv")], check=True)
        subprocess.run([uv, "pip", "install", "-q", "--python", str(python), *PYTHON_PACKAGES], check=True)
    else:
        subprocess.run([sys.executable, "-m", "venv", str(root / ".venv")], check=True)
        subprocess.run([str(python), "-m", "pip", "install", "-q", *PYTHON_PACKAGES], check=True)


def check_wav(path):
    with wave.open(str(path)) as w:
        if (w.getframerate(), w.getnchannels(), w.getsampwidth()) != (16000, 1, 2):
            raise SystemExit(f"{path.name}: expected 16 kHz mono PCM16")


def ami(root):
    d = root / "ami"
    d.mkdir(exist_ok=True)
    print("AMI audio")
    for name in AMI_DEV + AMI_TEST:
        wav = d / f"{name}.wav"
        if not wav.exists():
            seeded = FN64 / "ami" / f"{name}.wav"
            if seeded.exists():
                shutil.copyfile(seeded.resolve(), wav)  # the exact audio fn-64's results are keyed by
            else:
                try:
                    fetch(AMI_AUDIO.format(name), wav)
                except SystemExit as error:
                    print(f"  skipped {name} for now ({error}); rerun setup to fetch it")
                    continue
        check_wav(wav)
    names = [(split, n) for split, names in (("dev", AMI_DEV), ("test", AMI_TEST)) for n in names]
    if not all((d / f"{n}.rttm").exists() and (d / f"{n}.uem").exists() for _, n in names):
        print("AMI references")
        with urllib.request.urlopen(AMI_SETUP) as r:
            archive = tarfile.open(fileobj=io.BytesIO(r.read()))
        top = archive.getnames()[0].split("/")[0]
        files, parts = {}, []
        for split, n in names:
            for rel, out in ((f"only_words/rttms/{split}/{n}.rttm", f"{n}.rttm"), (f"uems/{split}/{n}.uem", f"{n}.uem")):
                data = archive.extractfile(f"{top}/{rel}").read()
                files[out] = data
                parts.append((rel, hashlib.sha256(data).hexdigest()))
        if hashlib.sha256(json.dumps(parts).encode()).hexdigest() != AMI_SETUP_MANIFEST:
            raise SystemExit("AMI references: checksum mismatch")
        for out, data in files.items():
            (d / out).write_bytes(data)
    if not all((d / f"{n}.words.json").exists() for _, n in names):
        print("AMI word timings")
        zip_path = d / "ami_public_manual_1.6.2.zip"
        if not zip_path.exists():
            fetch(AMI_WORDS, zip_path, AMI_WORDS_SHA256)
        with zipfile.ZipFile(zip_path) as z:
            agents = speakers(z.read("corpusResources/meetings.xml"))
            for _, n in names:
                (d / f"{n}.words.json").write_text(json.dumps(words(z, n, agents[n])))


def speakers(meetings_xml):
    """{meeting: {agent letter: global speaker name}} (the names the RTTMs use)."""
    out = {}
    for meeting in ET.fromstring(meetings_xml).iter("meeting"):
        out[meeting.get("observation")] = {s.get("nxt_agent"): s.get("global_name") for s in meeting.iter("speaker")}
    return out


def words(z, meeting, agents):
    out = []
    for agent, speaker in agents.items():
        for w in ET.fromstring(z.read(f"words/{meeting}.{agent}.words.xml")).iter("w"):
            if w.get("punc") == "true" or w.get("starttime") is None or w.get("endtime") is None:
                continue
            start, end = round(float(w.get("starttime")) * 1000), round(float(w.get("endtime")) * 1000)
            out.append({"start_ms": start, "end_ms": max(start, end), "speaker": speaker})
    return sorted(out, key=lambda x: (x["start_ms"], x["end_ms"]))


def models(root, cfg):
    receipt = json.loads(RECEIPT.read_text())["models"]["nemotron3_onnx_sha256"]
    d = root / "models/nemotron"
    d.mkdir(parents=True, exist_ok=True)
    for name in ("model_quantized.onnx", "model_quantized.onnx_data"):
        out = d / name
        if out.exists():
            continue
        print(f"Nemotron model: {name}")
        if (FN64 / "model" / name).exists():
            shutil.copyfile(FN64 / "model" / name, out)
            if sha256(out) != receipt[name]:
                out.unlink()
                raise SystemExit(f"{name}: checksum mismatch")
        else:
            fetch(NEMOTRON.format(name), out, receipt[name])
    missing = [p for p in (cfg["engines"]["current"]["model"], cfg["engines"]["current"]["binary"],
                           cfg["asr"]["model"], cfg["asr"]["binary"]) if not Path(p).exists()]
    for p in missing:
        print(f"missing: {p} (install dettivo, fetch the diarization model with "
              "scripts/models/fetch-diarization-model.sh, or download large-v3-turbo in Settings)")


def eligible(directory):
    """fn-64's rule: a finished meeting with one take per track of equal length."""
    try:
        takes = [json.loads((directory / f).read_text()) for f in ("takes.json", "system-takes.json")]
    except (OSError, ValueError):
        return False
    if (directory / "live-checkpoint.json").exists() or any(len(t.get("takes", t)) != 1 for t in takes):
        return False
    try:
        lengths = {wave.open(str(directory / f)).getnframes() for f in ("microphone.wav", "system.wav")}
    except (OSError, wave.Error):
        return False
    return len(lengths) == 1


def selection(root):
    path = root / "selection.json"
    current = json.loads(path.read_text()) if path.exists() else {}
    seed = FN64 / "selection.json"
    if seed.exists():
        for alias, directory in json.loads(seed.read_text()).items():
            current.setdefault(alias, {"dir": directory, "language": alias[:2].lower()})
    known = {entry["dir"] for entry in current.values()}
    with database() as db:
        rows = db.execute("""select audio_dir, language, duration_ms from meetings
            where status = 'completed' and system_audio = 1
            and coalesce(json_extract(diarization, '$.status'), '') not in ('queued', 'running')
            and coalesce(analysis_status, '') not in ('queued', 'running') order by started_at""").fetchall()
    for audio_dir, language, duration_ms in rows:
        directory = str(Path(audio_dir)) if audio_dir else None
        if language not in ("en", "de") or (duration_ms or 0) < MIN_MEETING_MS or not directory:
            continue
        if directory in known or not eligible(Path(directory)):
            continue
        prefix = language.upper()
        n = 1 + max([int(a.split("-")[1]) for a in current if a.startswith(prefix + "-")] or [0])
        current[f"{prefix}-{n}"] = {"dir": directory, "language": language}
        known.add(directory)
    path.write_text(json.dumps(dict(sorted(current.items())), indent=1))
    path.chmod(0o600)
    by_language = {}
    for entry in current.values():
        by_language[entry["language"]] = by_language.get(entry["language"], 0) + 1
    print("local meetings: " + ", ".join(f"{n} {lang}" for lang, n in sorted(by_language.items())))


def main():
    argparse.ArgumentParser(prog="just diar-bench-setup", description=__doc__.splitlines()[0]).parse_args()
    root = eval_dir()
    root.mkdir(parents=True, exist_ok=True)
    root.chmod(0o700)
    environment(root)
    ami(root)
    cfg = config(root)
    models(root, cfg)
    selection(root)
    if FN64.exists():
        subprocess.run([str(root / ".venv/bin/python"), str(HERE / "seed.py")], check=True)
    print(f"ready: {root}. Next: `just diar-bench --full` once, then `just diar-bench`.")


if __name__ == "__main__":
    main()
