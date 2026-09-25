#!/usr/bin/env python3
"""Nemotron 3 Diarization offline through ONNX Runtime, as a side-by-side post-pass.

A Python port of omarchy-meeting-recorder v1.1.0 (src/nemotron.rs and src/diarize.rs),
which follows `Nemotron3DiarizationSpeakerCache` in Hugging Face transformers: log-mel
features, 340-frame chunks with 40 look-ahead frames, a 40-frame FIFO and the 264-frame
arrival-order speaker cache. Post-processing is the recorder's: probability over 0.5,
same-speaker pauses under 500 ms bridged, runs under 300 ms dropped, then either the
known count kept (largest speakers) or clusters under max(4 s, 4 % of speech) absorbed.
`--postprocess none` keeps the raw over-0.5 runs, to isolate what post-processing costs.

Prints the diarization engine's native JSON ({"turns": [...]}) with a "timing" block.
Experiment only (fn-64); nothing in the product calls this.
"""
import argparse
import json
import math
import time
import wave

import numpy as np

HOP, N_FFT, WIN, MELS, RATE, PRE = 160, 512, 400, 128, 16000, 0.97
H, FACTOR, N = 512, 8, 8
CHUNK, RIGHT, FIFO, UPDATE = 340, 40, 40, 300
CACHE, SILENCE_SLOTS, THRESH = 264, 1, 0.25
LATEST_BOOST, MIN_POS_RATE, STRONG_RATE, WEAK_RATE = 0.05, 0.5, 0.75, 1.5


def read_wav(path):
    with wave.open(path, "rb") as w:
        if w.getframerate() != RATE or w.getnchannels() != 1 or w.getsampwidth() != 2:
            raise SystemExit(f"{path}: need 16 kHz mono PCM16")
        return np.frombuffer(w.readframes(w.getnframes()), dtype="<i2").astype(np.float32) / 32768.0


def mel_filters():
    f_sp, min_log_hz, logstep = 200.0 / 3.0, 1000.0, math.log(6.4) / 27.0
    min_log_mel = min_log_hz / f_sp

    def hz_to_mel(hz):
        return min_log_mel + math.log(hz / min_log_hz) / logstep if hz >= min_log_hz else hz / f_sp

    def mel_to_hz(mel):
        return min_log_hz * math.exp(logstep * (mel - min_log_mel)) if mel >= min_log_mel else f_sp * mel

    top = hz_to_mel(RATE / 2)
    points = np.array([mel_to_hz(top * i / (MELS + 1)) for i in range(MELS + 2)])
    bins = N_FFT // 2 + 1
    fft_hz = np.arange(bins) * RATE / 2 / (bins - 1)
    lower, center, upper = points[:-2, None], points[1:-1, None], points[2:, None]
    rising = (fft_hz - lower) / (center - lower)
    falling = (upper - fft_hz) / (upper - center)
    weights = np.maximum(0.0, np.minimum(rising, falling)) * (2.0 / (upper - lower))
    return weights.astype(np.float32)  # MELS x bins


class Features:
    """Pre-emphasis, centred zero-padded STFT (400-sample Hann in 512), log-mel on demand."""

    def __init__(self, samples):
        emph = np.empty_like(samples)
        emph[:1] = samples[:1]
        emph[1:] = samples[1:] - PRE * samples[:-1]
        self.padded = np.concatenate([np.zeros(N_FFT // 2, np.float32), emph, np.zeros(N_FFT // 2, np.float32)])
        self.frames = 1 + len(samples) // HOP
        self.valid = len(samples) // HOP
        offset = (N_FFT - WIN) // 2
        window = np.zeros(N_FFT, np.float32)
        k = np.arange(WIN, dtype=np.float32)
        window[offset:offset + WIN] = 0.5 - 0.5 * np.cos(2 * np.pi * k / (WIN - 1))
        self.window = window
        self.mel = mel_filters()

    def log_mel(self, first, rows):
        out = np.zeros((rows, MELS), np.float32)
        end = min(first + rows, self.frames, self.valid)
        if end > first:
            idx = np.arange(first, end)[:, None] * HOP + np.arange(N_FFT)[None, :]
            frames = self.padded[np.minimum(idx, len(self.padded) - 1)] * self.window
            power = np.abs(np.fft.rfft(frames, axis=1)).astype(np.float32) ** 2
            out[: end - first] = np.log(power @ self.mel.T + 2.0 ** -24)
        return out


def sigmoid(x):
    return 1.0 / (1.0 + np.exp(-x))


def frame_scores(probs):
    budget = CACHE // N - SILENCE_SLOTS
    min_positive = math.floor(budget * MIN_POS_RATE)
    log_p = np.log(np.maximum(probs, THRESH))
    log_c = np.log(np.maximum(1.0 - probs, THRESH))
    scores = log_p - log_c + log_c.sum(axis=1, keepdims=True) - math.log(0.5)
    speech = probs > 0.5
    scores[~speech] = -np.inf
    positive = scores > 0
    enough = positive.sum(axis=0, keepdims=True) >= min_positive
    scores[~positive & speech & enough] = -np.inf
    return scores


def boost(scores, count, amount):
    count = min(count, scores.shape[0])
    for s in range(N):
        top = np.argsort(-scores[:, s], kind="stable")[:count]
        scores[top, s] += amount


def compress(embeds, probs, silence):
    frames = embeds.shape[0]
    scores = frame_scores(probs)
    scores[CACHE:] += LATEST_BOOST
    budget = CACHE // N - SILENCE_SLOTS
    boost(scores, math.floor(budget * STRONG_RATE), -2.0 * math.log(0.5))
    boost(scores, math.floor(budget * WEAK_RATE), -math.log(0.5))
    scored = frames + SILENCE_SLOTS
    flat = np.concatenate([scores, np.full((SILENCE_SLOTS, N), np.inf, np.float32)]).T.reshape(-1)
    order = np.argsort(-flat, kind="stable")[:CACHE]
    sentinel = scored * N
    picked = np.sort(np.where(flat[order] == -np.inf, sentinel, order))
    frame = np.where(picked == sentinel, frames, np.minimum(picked % scored, frames))
    embeds = np.concatenate([embeds, silence[None, :]])
    probs = np.concatenate([probs, np.zeros((1, N), np.float32)])
    return embeds[frame], probs[frame]


class Cache:
    def __init__(self):
        self.embeds = np.zeros((0, H), np.float32)
        self.probs = np.zeros((0, N), np.float32)
        self.fifo = np.zeros((0, H), np.float32)
        self.compressed = False

    def cached(self):
        return np.concatenate([self.embeds, self.fifo])

    def update(self, inputs, logits, chunk_frames, silence):
        cache_len, fifo_len = len(self.embeds), len(self.fifo)
        probs = sigmoid(logits).reshape(-1, FACTOR, N).mean(axis=1)
        start = cache_len + fifo_len
        fifo = np.concatenate([self.fifo, inputs[start:start + chunk_frames]])
        popped = 0 if len(fifo) <= FIFO else min(max(UPDATE, len(fifo) - FIFO), len(fifo))
        if popped:
            fifo_probs = probs[cache_len:cache_len + len(fifo)]
            stored = self.probs[:cache_len] if self.compressed else probs[:cache_len]
            embeds = np.concatenate([self.embeds, fifo[:popped]])
            cprobs = np.concatenate([stored, fifo_probs[:popped]])
            fifo = fifo[popped:]
            if len(embeds) > CACHE:
                embeds, cprobs = compress(embeds, cprobs, silence)
                self.compressed = True
            self.embeds, self.probs = embeds, cprobs
        self.fifo = fifo


def probabilities(session, samples):
    feats = Features(samples)
    steps = -(-feats.frames // FACTOR)
    cache, out, silence, start = Cache(), [], None, 0
    while start < steps:
        end = min(start + CHUNK, steps)
        look = min(end + RIGHT, steps)
        rows = (look - start) * FACTOR
        cached = cache.cached()
        total = len(cached) + look - start
        logits, chunk_embeds, sil = session.run(None, {
            "input_features": feats.log_mel(start * FACTOR, rows)[None],
            "cached_embeds": cached[None],
            "attention_mask": np.ones((1, total), np.int64),
        })
        logits, chunk_embeds = logits[0], chunk_embeds[0]
        silence = sil if silence is None else silence
        out.append(logits[len(cached) * FACTOR:(len(cached) + end - start) * FACTOR])
        cache.update(np.concatenate([cached, chunk_embeds]), logits, end - start, silence)
        start = end
    return sigmoid(np.concatenate(out)[: feats.frames])


def segments(probs, bridge_ms=500, min_ms=300):
    raw = []
    for s in range(N):
        on = np.concatenate([[False], probs[:, s] > 0.5, [False]])
        edges = np.flatnonzero(on[1:] != on[:-1])
        runs = []
        for a, b in zip(edges[::2], edges[1::2]):
            a, b = int(a) * 10, int(b) * 10
            if runs and a - runs[-1][1] < bridge_ms:
                runs[-1][1] = b
            else:
                runs.append([a, b])
        raw += [(a, b, s) for a, b in runs if b - a >= min_ms]
    return raw


def reassign(raw, keep):
    anchors = [t for t in raw if keep(t[2])]
    if not anchors:
        return raw
    out = []
    for a, b, s in raw:
        if not keep(s):
            mid = (a + b) // 2
            s = min(anchors, key=lambda t: t[0] - mid if mid < t[0] else max(mid - t[1], 0))[2]
        out.append((a, b, s))
    return out


def spoken(raw):
    totals = {}
    for a, b, s in raw:
        totals[s] = totals.get(s, 0) + b - a
    return totals


def turns(probs, speakers=None, postprocess="recorder"):
    if postprocess == "none":
        raw = segments(probs, 0, 0)
    elif speakers:
        raw = segments(probs)
        totals = spoken(raw)
        kept = [s for s, _ in sorted(totals.items(), key=lambda kv: (-kv[1], kv[0]))[: max(speakers, 1)]]
        raw = reassign(raw, lambda s: s in kept)
    else:
        raw = segments(probs)
        totals = spoken(raw)
        floor = max(sum(totals.values()) * 4 // 100, 4000)
        raw = reassign(raw, lambda s: totals.get(s, 0) >= floor)
    raw.sort()
    order = []
    result = []
    for a, b, s in raw:
        if s not in order:
            order.append(s)
        result.append({"start_ms": a, "end_ms": b, "speaker": order.index(s)})
    return result


def session_for(model, provider, threads):
    import onnxruntime as ort
    options = ort.SessionOptions()
    options.intra_op_num_threads = threads
    options.inter_op_num_threads = 1
    if provider == "cuda":
        ort.preload_dlls()
        providers = [("CUDAExecutionProvider", {"device_id": 0}), "CPUExecutionProvider"]
    else:
        providers = ["CPUExecutionProvider"]
    session = ort.InferenceSession(model, options, providers=providers)
    active = session.get_providers()[0]
    if provider == "cuda" and active != "CUDAExecutionProvider":
        raise SystemExit(f"CUDA requested, session runs on {active}")
    return session, active, ort.__version__


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--wav", required=True)
    parser.add_argument("--model", required=True, help="ONNX graph (weights beside it)")
    parser.add_argument("--provider", choices=("cpu", "cuda"), default="cpu")
    parser.add_argument("--threads", type=int, default=4)
    parser.add_argument("--speakers", type=int)
    parser.add_argument("--postprocess", choices=("recorder", "none"), default="recorder",
                        help="none: raw 0.5-threshold runs, no bridging, dropping or absorbing (diagnostic)")
    args = parser.parse_args()
    t0 = time.perf_counter()
    samples = read_wav(args.wav)
    t1 = time.perf_counter()
    session, active, version = session_for(args.model, args.provider, args.threads)
    t2 = time.perf_counter()
    probs = probabilities(session, samples)
    t3 = time.perf_counter()
    result = {"turns": turns(probs, args.speakers, args.postprocess)}
    t4 = time.perf_counter()
    result["timing"] = {
        "audio_seconds": len(samples) / RATE, "read_seconds": t1 - t0, "load_seconds": t2 - t1,
        "inference_seconds": t3 - t2, "postprocess_seconds": t4 - t3,
        "provider": active, "onnxruntime": version, "threads": args.threads,
        "raw_channels_active": int((probs > 0.5).any(axis=0).sum()),
    }
    print(json.dumps(result))


if __name__ == "__main__":
    main()
