"""Where the bench's inputs live and which recordings each split holds.

Everything sits in the protected eval directory (mode 700, `$DETTIVO_EVAL_DIR`, default
~/.local/share/dettivo-eval/bench): AMI audio and references under ami/, the local
selection of retained meetings in selection.json, fn-72 labels under labels/, the cache,
the scoreboards and the baselines. Meetings are read in place and read-only; the
database is opened read-only.
"""
import json
import os
import sqlite3
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
DB = Path(os.environ.get("XDG_DATA_HOME", Path.home() / ".local/share")) / "dettivo/dettivo.db"
MODELS = DB.parent / "models"

# pyannote AMI-diarization-setup "only_words": dev is the tuning set; the four test
# meetings fn-64 reported on are the held-out set.
AMI_DEV = ["IS1008a", "IS1008b", "IS1008c", "IS1008d", "ES2011a", "ES2011b", "ES2011c", "ES2011d",
           "TS3004a", "TS3004b", "TS3004c", "TS3004d", "IB4001", "IB4002", "IB4003", "IB4004", "IB4010", "IB4011"]
AMI_TEST = ["ES2004a", "TS3003a", "IS1009b", "EN2002c"]
HELDOUT = {"ami-test"}
SPLITS = ["ami-dev", "ami-test", "local-en", "local-de", "local", "labelled-en", "labelled-de"]

DEFAULT_CONFIG = {
    "engines": {
        "current": {"label": "Current engine (sherpa-onnx, CPU)", "kind": "diarize",
                    "binary": "/usr/lib/dettivo/engines/dettivo-engine-diarize",
                    "model": str(MODELS / "diarize/diarization-en"), "threads": 4, "provider": "cpu"},
        "nemotron": {"label": "Nemotron int8 (fn-64 ONNX runner, CPU)", "kind": "nemotron-onnx",
                     "model": "{eval}/models/nemotron/model_quantized.onnx", "threads": 4, "provider": "cpu"},
    },
    "asr": {"binary": "/usr/lib/dettivo/engines/dettivo-engine-whisper",
            "model": str(MODELS / "whisper/large-v3-turbo/ggml-large-v3-turbo.bin"), "language": "en"},
}

BUSY = """select count(*) from meetings where status in ('recording', 'stopping', 'stopped', 'transcribing')
    or json_extract(diarization, '$.status') in ('queued', 'running') or analysis_status in ('queued', 'running')"""


def eval_dir():
    return Path(os.environ.get("DETTIVO_EVAL_DIR", Path.home() / ".local/share/dettivo-eval/bench"))


def config(root):
    """bench.json over the defaults, one level deep per engine."""
    out = json.loads(json.dumps(DEFAULT_CONFIG))
    path = root / "bench.json"
    if path.exists():
        local = json.loads(path.read_text())
        for name, engine in local.get("engines", {}).items():
            out["engines"].setdefault(name, {}).update(engine)
        out["asr"].update(local.get("asr", {}))
    for engine in [*out["engines"].values(), out["asr"]]:
        for k, v in engine.items():
            if isinstance(v, str):
                engine[k] = v.replace("{eval}", str(root))
    return out


def database():
    return sqlite3.connect(f"file:{DB}?mode=ro", uri=True, timeout=5)


def busy():
    """A meeting is recording or finalising (transcript, speaker pass, analysis)."""
    try:
        with database() as db:
            return db.execute(BUSY).fetchone()[0] > 0
    except sqlite3.Error:
        return True  # unknown counts as busy


def selection(root):
    """{alias: {"dir", "language"}}; aliases are EN-<n> and DE-<n>."""
    path = root / "selection.json"
    return json.loads(path.read_text()) if path.exists() else {}


def meeting_row(db, directory):
    row = db.execute("select segments, system_audio from meetings where id = ?", (Path(directory).name,)).fetchone()
    if not row:
        raise LookupError("a selected meeting is no longer in the database")
    return json.loads(row[0] or "[]"), bool(row[1])


def recordings(root, heldout=False):
    """Every recording the run scores, as dicts: id, split, wav (the diarized track),
    room_audio, reference ({"kind": "ami"|"labels", ...} or None), meeting fields."""
    out = []
    for split, names in (("ami-dev", AMI_DEV), ("ami-test", AMI_TEST)):
        if split in HELDOUT and not heldout:
            continue
        for name in names:
            d = root / "ami"
            if not (d / f"{name}.wav").exists():
                print(f"{name}: audio missing, skipped (rerun `just diar-bench-setup`)")
                continue
            out.append({"id": name, "split": split, "wav": d / f"{name}.wav", "room_audio": True,
                        "reference": {"kind": "ami", "rttm": d / f"{name}.rttm", "uem": d / f"{name}.uem",
                                      "words": d / f"{name}.words.json"}})
    for alias, entry in sorted(selection(root).items()):
        directory = Path(entry["dir"])
        language = entry["language"]
        base = {"id": alias, "dir": directory, "language": language,
                "mic": directory / "microphone.wav", "system": directory / "system.wav"}
        out.append(dict(base, split=f"local-{language}", reference=None))
        labels = root / "labels" / f"{alias}.json"
        if labels.exists():
            out.append(dict(base, split=f"labelled-{language}", reference={"kind": "labels", "path": labels}))
    return out


def reference_units(reference):
    """Reference units: {start_ms, end_ms, speaker, words[, source]}. AMI: one per word.
    Labels (fn-72): one per labelled line; lines the labeller left unknown are skipped."""
    if reference["kind"] == "ami":
        return [dict(w, words=1) for w in json.loads(Path(reference["words"]).read_text())]
    labels = json.loads(Path(reference["path"]).read_text())
    return [{"start_ms": x["start_ms"], "end_ms": x["end_ms"], "speaker": x["speaker"],
             "words": x["words"], "source": x.get("source")}
            for x in labels["lines"] if x.get("speaker") is not None]


def label_window(reference):
    """The labelled excerpt's [start_ms, end_ms), or None for a whole meeting."""
    if reference and reference["kind"] == "labels":
        window = json.loads(Path(reference["path"]).read_text()).get("window_ms")
        return tuple(window) if window else None
    return None
