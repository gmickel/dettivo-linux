#!/usr/bin/env python3
"""Check the ONNX port (nemotron3_diarize.py) against the Hugging Face transformers model.

Runs the first --seconds of a WAV through both: transformers' offline forward of
nvidia/Nemotron-3-Diarization (PyTorch, CPU, float32) and the port with a given ONNX graph.
Prints the largest per-frame probability difference, the share of 10 ms frames whose
over-0.5 decision differs per speaker channel, and the strict DER of the port's turns
scored against the reference model's turns (both with the recorder's post-processing).
Needs torch, transformers with nemotron3_diarization, librosa and onnxruntime.
"""
import argparse
import json
import sys
from pathlib import Path

import numpy as np
import torch
from transformers import AutoFeatureExtractor, AutoModelForAudioFrameClassification

HERE = Path(__file__).resolve().parent
sys.path[:0] = [str(HERE), str(HERE.parent)]
import diarization_score  # noqa: E402
import nemotron3_diarize as port  # noqa: E402


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--wav", required=True)
    parser.add_argument("--hf-model", required=True, help="directory with config.json and model.safetensors")
    parser.add_argument("--onnx", required=True)
    parser.add_argument("--seconds", type=float, default=600)
    args = parser.parse_args()
    samples = port.read_wav(args.wav)[: int(args.seconds * port.RATE)]

    extractor = AutoFeatureExtractor.from_pretrained(args.hf_model)
    model = AutoModelForAudioFrameClassification.from_pretrained(args.hf_model, dtype=torch.float32).eval()
    inputs = extractor(samples, sampling_rate=port.RATE, return_tensors="pt")
    with torch.no_grad():
        reference = model(input_features=inputs["input_features"]).logits.sigmoid()[0].numpy()

    session, _, _ = port.session_for(args.onnx, "cpu", 4)
    ported = port.probabilities(session, samples)
    n = min(len(reference), len(ported))
    reference, ported = reference[:n], ported[:n]
    ref_turns, port_turns = port.turns(reference), port.turns(ported)
    agreement = diarization_score.sweep(diarization_score.hypothesis_inputs(ref_turns),
                                        diarization_score.hypothesis_inputs(port_turns))
    print(json.dumps({
        "frames": n, "frames_reference": int(len(reference)),
        "max_abs_probability_difference": float(np.abs(reference - ported).max()),
        "decision_flip_share": float(((reference > 0.5) != (ported > 0.5)).mean()),
        "port_vs_reference_der": agreement["der"], "port_vs_reference_confusion": agreement["confusion"],
        "reference_speech_share": float((reference > 0.5).any(axis=1).mean()),
        "port_speech_share": float((ported > 0.5).any(axis=1).mean()),
    }, indent=1))


if __name__ == "__main__":
    main()
