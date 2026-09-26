#!/usr/bin/env python3
"""Check the product engine (dettivo-engine-nemotron) against the fn-64 ONNX port.

Runs each WAV through both on the same audio: the engine's CLI (NeMo-Speech.cpp, the
catalogue's q8_0 GGUF) with --probs, and nemotron3_diarize.py's port with the given ONNX
graph (the fp32 export for parity). Per recording it prints the largest and mean
per-frame probability difference, the share of 10 ms frames whose over-0.5 decision
differs, and the strict DER and confusion of the engine's turns scored against the
port's (both from the recorder post-processing). With --ami it also scores both against
the AMI reference beside each WAV (<name>.rttm, <name>.uem). The verdict applies the
tolerance fixed in ADR 0073 before the run. The port matched the transformers reference
within 1.1e-5 (crosscheck_reference.py, fn-64), so parity with it carries over.
Needs numpy and onnxruntime; prints aggregate numbers only.
"""
import argparse
import json
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path[:0] = [str(HERE), str(HERE.parent)]
import diarization_score  # noqa: E402
import nemotron3_diarize as port  # noqa: E402

# ADR 0073: per recording, against the fp32 port.
TOLERANCE = {"turn_der": 0.02, "turn_confusion": 0.005, "decision_flip_share": 0.01}


def engine_run(binary, model, wav, provider, probs):
    out = subprocess.run([binary, "--wav", str(wav), "--model", model, "--json", "--provider", provider,
                          "--probs", str(probs)], capture_output=True, text=True, check=True)
    return json.loads(out.stdout)


def compare(engine_probs, port_probs, engine_turns, port_turns):
    n = min(len(engine_probs), len(port_probs))
    a, b = engine_probs[:n], port_probs[:n]
    agreement = diarization_score.sweep(diarization_score.hypothesis_inputs(port_turns),
                                        diarization_score.hypothesis_inputs(engine_turns))
    return {"frames": n, "frames_engine": int(len(engine_probs)), "frames_port": int(len(port_probs)),
            "max_abs_probability_difference": float(np.abs(a - b).max()),
            "mean_abs_probability_difference": float(np.abs(a - b).mean()),
            "decision_flip_share": float(((a > 0.5) != (b > 0.5)).mean()),
            "turn_der": agreement["der"], "turn_confusion": agreement["confusion"],
            "engine_speakers": agreement["hypothesis_speakers"], "port_speakers": agreement["reference_speakers"]}


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--engine", required=True, help="the dettivo-engine-nemotron binary")
    parser.add_argument("--model", required=True, help="the catalogue model directory")
    parser.add_argument("--onnx", required=True, help="the port's ONNX graph (fp32 for parity)")
    parser.add_argument("--engine-provider", default="cpu")
    parser.add_argument("--port-provider", choices=("cpu", "cuda"), default="cpu")
    parser.add_argument("--ami", action="store_true", help="also score both against <name>.rttm/.uem")
    parser.add_argument("wavs", nargs="+")
    args = parser.parse_args()
    session, active, version = port.session_for(args.onnx, args.port_provider, 4)
    rows = []
    with tempfile.TemporaryDirectory() as tmp:
        for wav in map(Path, args.wavs):
            engine = engine_run(args.engine, args.model, wav, args.engine_provider, Path(tmp) / "probs.npy")
            engine_probs = np.load(Path(tmp) / "probs.npy")
            port_probs = port.probabilities(session, port.read_wav(str(wav)))
            port_turns = port.turns(port_probs)
            row = {"recording": wav.stem, "engine_backend": engine["backend"]}
            row.update(compare(engine_probs, port_probs, engine["turns"], port_turns))
            if args.ami:
                rttm, uem = wav.with_suffix(".rttm"), wav.with_suffix(".uem")
                for side, turns in (("engine", engine["turns"]), ("port", port_turns)):
                    s = diarization_score.score(wav.stem, turns, rttm, uem)
                    row[f"{side}_vs_ami"] = {k: s[k] for k in ("der", "missed", "false_alarm", "confusion",
                                                               "hypothesis_speakers")}
            row["within_tolerance"] = all(row[k] <= v for k, v in TOLERANCE.items())
            rows.append(row)
            print(json.dumps(row), file=sys.stderr, flush=True)
    print(json.dumps({"tolerance": TOLERANCE, "port_provider": active, "onnxruntime": version,
                      "recordings": rows, "all_within_tolerance": all(r["within_tolerance"] for r in rows)},
                     indent=1))


if __name__ == "__main__":
    main()
