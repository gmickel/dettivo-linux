# Diarization bench, 2026-09-27

Whisper large-v3-turbo with DTW word timings (ADR 0074) on the RTX 4090, AMI through the engine CLI and the retained meetings through the product finalisation (`asr.meetings`), `pause_ms` 500. Tree `edbddb6db651` plus the fn-74 changes, assignment variant `product`. Values are pooled over each split's files; brackets are 95% bootstrap intervals over files.

## ami-dev

18 files, 580 minutes.

| Metric | Current engine (sherpa-onnx, CPU) | Nemotron int8 (fn-64 ONNX runner, CPU) |
|---|---:|---:|
| Attribution error (headline) | 21.07% [16.15, 26.00] | 12.49% [10.82, 14.13] |
| wrong speaker | 21.07% [16.15, 26.00] | 12.49% [10.82, 14.13] |
| unlabelled | 0.00% [0.00, 0.00] | 0.00% [0.00, 0.00] |
| Short-turn error (<=3 words) | 61.70% [60.33, 62.94] | 52.21% [51.04, 53.31] |
| DER of labelled lines | 43.25% [37.31, 49.62] | 37.19% [33.41, 41.56] |
| missed | 27.97% [25.55, 30.17] | 27.98% [25.56, 30.16] |
| false alarm | 8.07% [6.11, 10.54] | 8.07% [6.11, 10.59] |
| confusion | 7.21% [3.80, 11.24] | 1.14% [0.80, 1.70] |
| Engine DER (turns) | 23.32% [18.49, 28.72] | 17.42% [15.58, 20.04] |
| missed | 5.62% [4.87, 6.47] | 15.33% [13.64, 17.56] |
| false alarm | 6.46% [5.74, 7.30] | 1.73% [1.35, 2.21] |
| confusion | 11.24% [6.83, 16.20] | 0.36% [0.24, 0.54] |
| Lines, right speaker | 78.98% [73.88, 83.83] | 88.75% [86.69, 90.36] |
| Lines, wrong speaker | 21.02% [16.16, 25.87] | 11.24% [9.62, 13.28] |
| Lines, unlabelled | 0.00% [0.00, 0.00] | 0.01% [0.00, 0.02] |
| Labelled lines, wrong speaker | 21.02% [16.16, 25.87] | 11.24% [9.62, 13.28] |
| Engine speed (x realtime) | 13.23x [12.28, 14.36] | 52.08x [45.21, 57.27] |

## ami-test (held out)

4 files, 126 minutes.

| Metric | Current engine (sherpa-onnx, CPU) | Nemotron int8 (fn-64 ONNX runner, CPU) |
|---|---:|---:|
| Attribution error (headline) | 14.11% [8.58, 16.91] | 11.96% [9.07, 13.62] |
| wrong speaker | 14.09% [8.55, 16.89] | 11.96% [9.07, 13.62] |
| unlabelled | 0.01% [0.00, 0.08] | 0.00% [0.00, 0.00] |
| Short-turn error (<=3 words) | 60.85% [59.68, 63.63] | 54.87% [53.90, 57.41] |
| DER of labelled lines | 38.11% [30.71, 44.48] | 37.69% [30.04, 44.56] |
| missed | 30.94% [22.87, 36.63] | 31.03% [22.95, 36.73] |
| false alarm | 5.55% [3.85, 10.21] | 5.55% [3.80, 10.32] |
| confusion | 1.62% [1.09, 1.78] | 1.10% [0.73, 2.20] |
| Engine DER (turns) | 15.67% [13.66, 18.04] | 21.78% [13.38, 29.20] |
| missed | 9.25% [6.28, 10.94] | 20.16% [10.78, 27.45] |
| false alarm | 3.03% [2.16, 4.41] | 0.90% [0.48, 1.53] |
| confusion | 3.39% [2.67, 3.77] | 0.71% [0.24, 1.79] |
| Lines, right speaker | 87.52% [84.96, 93.69] | 84.34% [80.79, 90.46] |
| Lines, wrong speaker | 12.34% [5.67, 15.01] | 15.66% [9.54, 19.21] |
| Lines, unlabelled | 0.15% [0.00, 0.64] | 0.00% [0.00, 0.00] |
| Labelled lines, wrong speaker | 12.35% [5.71, 15.02] | 15.66% [9.54, 19.21] |
| Engine speed (x realtime) | 13.60x [12.47, 15.10] | 54.80x [54.11, 56.03] |

## local-en

4 files, 276 minutes.

| Metric | Current engine (sherpa-onnx, CPU) | Nemotron int8 (fn-64 ONNX runner, CPU) |
|---|---:|---:|
| Remote lines unlabelled | 0.42% [0.27, 0.81] | 2.22% [1.56, 3.65] |
| Local/remote proxy (mix) | 0.63% [0.43, 1.22] | 0.57% [0.10, 1.80] |
| Engine speed (x realtime) | 19.44x [17.73, 23.20] | 49.72x [46.58, 57.96] |

## local-de

6 files, 278 minutes.

| Metric | Current engine (sherpa-onnx, CPU) | Nemotron int8 (fn-64 ONNX runner, CPU) |
|---|---:|---:|
| Remote lines unlabelled | 0.31% [0.05, 0.88] | 0.98% [0.36, 2.66] |
| Local/remote proxy (mix) | 0.90% [0.52, 1.81] | 0.14% [0.08, 0.27] |
| Engine speed (x realtime) | 17.51x [16.19, 20.40] | 48.98x [41.93, 56.25] |

## local

10 files, 555 minutes.

| Metric | Current engine (sherpa-onnx, CPU) | Nemotron int8 (fn-64 ONNX runner, CPU) |
|---|---:|---:|
| Remote lines unlabelled | 0.36% [0.17, 0.68] | 1.50% [0.78, 2.67] |
| Local/remote proxy (mix) | 0.77% [0.52, 1.23] | 0.35% [0.11, 0.91] |
| Engine speed (x realtime) | 18.42x [17.06, 20.69] | 49.35x [45.19, 54.75] |

Metric definitions: [docs/diarization-bench.md](../../diarization-bench.md). Engine identities are in the JSON beside this page.
