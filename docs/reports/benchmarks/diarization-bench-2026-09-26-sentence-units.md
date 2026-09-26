# Diarization bench, 2026-09-26

Tree `6b65338c2798`, assignment variant `product`. Values are pooled over each split's files; brackets are 95% bootstrap intervals over files.

## ami-dev

18 files, 580 minutes.

| Metric | Current engine (sherpa-onnx, CPU) | Nemotron int8 (fn-64 ONNX runner, CPU) |
|---|---:|---:|
| Attribution error (headline) | 22.43% [17.61, 27.56] | 14.65% [12.71, 16.61] |
| wrong speaker | 22.43% [17.61, 27.56] | 14.64% [12.70, 16.60] |
| unlabelled | 0.00% [0.00, 0.00] | 0.00% [0.00, 0.01] |
| Short-turn error (<=3 words) | 61.95% [60.96, 62.96] | 54.16% [53.20, 55.38] |
| DER of labelled lines | 44.39% [36.57, 53.84] | 37.83% [32.08, 45.39] |
| missed | 15.11% [13.32, 16.71] | 15.12% [13.33, 16.73] |
| false alarm | 18.80% [13.80, 25.20] | 18.73% [13.69, 24.96] |
| confusion | 10.48% [6.28, 15.36] | 3.98% [3.16, 5.04] |
| Engine DER (turns) | 23.32% [18.49, 28.72] | 17.42% [15.58, 20.04] |
| missed | 5.62% [4.87, 6.47] | 15.33% [13.64, 17.56] |
| false alarm | 6.46% [5.74, 7.30] | 1.73% [1.35, 2.21] |
| confusion | 11.24% [6.83, 16.20] | 0.36% [0.24, 0.54] |
| Lines, right speaker | 79.43% [74.07, 84.84] | 88.95% [86.67, 90.67] |
| Lines, wrong speaker | 20.57% [15.15, 25.86] | 11.03% [9.32, 13.29] |
| Lines, unlabelled | 0.00% [0.00, 0.00] | 0.02% [0.00, 0.06] |
| Labelled lines, wrong speaker | 20.57% [15.15, 25.86] | 11.03% [9.32, 13.29] |
| Engine speed (x realtime) | 13.23x [12.28, 14.36] | 52.08x [45.21, 57.27] |

## ami-test (held out)

4 files, 126 minutes.

| Metric | Current engine (sherpa-onnx, CPU) | Nemotron int8 (fn-64 ONNX runner, CPU) |
|---|---:|---:|
| Attribution error (headline) | 16.05% [10.12, 19.40] | 14.70% [10.82, 17.24] |
| wrong speaker | 16.04% [10.09, 19.38] | 14.69% [10.82, 17.22] |
| unlabelled | 0.01% [0.00, 0.09] | 0.01% [0.00, 0.06] |
| Short-turn error (<=3 words) | 62.04% [61.01, 65.82] | 57.41% [56.24, 61.51] |
| DER of labelled lines | 36.09% [27.40, 48.18] | 34.81% [26.69, 49.10] |
| missed | 16.69% [9.59, 21.70] | 16.67% [9.59, 21.66] |
| false alarm | 15.78% [10.00, 31.57] | 14.71% [8.84, 30.75] |
| confusion | 3.62% [2.97, 4.07] | 3.44% [2.47, 5.55] |
| Engine DER (turns) | 15.67% [13.66, 18.04] | 21.78% [13.38, 29.20] |
| missed | 9.25% [6.28, 10.94] | 20.16% [10.78, 27.45] |
| false alarm | 3.03% [2.16, 4.41] | 0.90% [0.48, 1.53] |
| confusion | 3.39% [2.67, 3.77] | 0.71% [0.24, 1.79] |
| Lines, right speaker | 87.90% [85.68, 92.61] | 86.25% [82.97, 91.39] |
| Lines, wrong speaker | 12.00% [6.90, 14.32] | 13.72% [8.52, 17.03] |
| Lines, unlabelled | 0.11% [0.00, 0.54] | 0.04% [0.00, 0.18] |
| Labelled lines, wrong speaker | 12.01% [6.94, 14.32] | 13.72% [8.53, 17.03] |
| Engine speed (x realtime) | 13.60x [12.47, 15.10] | 54.80x [54.11, 56.03] |

## local-en

4 files, 276 minutes.

| Metric | Current engine (sherpa-onnx, CPU) | Nemotron int8 (fn-64 ONNX runner, CPU) |
|---|---:|---:|
| Remote lines unlabelled | 2.47% [1.04, 5.96] | 4.56% [2.43, 10.77] |
| Local/remote proxy (mix) | 0.63% [0.43, 1.22] | 0.57% [0.10, 1.80] |
| Engine speed (x realtime) | 19.44x [17.73, 23.20] | 49.72x [46.58, 57.96] |

## local-de

6 files, 278 minutes.

| Metric | Current engine (sherpa-onnx, CPU) | Nemotron int8 (fn-64 ONNX runner, CPU) |
|---|---:|---:|
| Remote lines unlabelled | 0.03% [0.00, 0.07] | 0.60% [0.16, 1.75] |
| Local/remote proxy (mix) | 0.90% [0.52, 1.81] | 0.14% [0.08, 0.27] |
| Engine speed (x realtime) | 17.51x [16.19, 20.40] | 48.98x [41.93, 56.25] |

## local

10 files, 555 minutes.

| Metric | Current engine (sherpa-onnx, CPU) | Nemotron int8 (fn-64 ONNX runner, CPU) |
|---|---:|---:|
| Remote lines unlabelled | 1.14% [0.22, 2.55] | 2.40% [0.76, 4.96] |
| Local/remote proxy (mix) | 0.77% [0.52, 1.23] | 0.35% [0.11, 0.91] |
| Engine speed (x realtime) | 18.42x [17.06, 20.69] | 49.35x [45.19, 54.75] |

Metric definitions: [docs/diarization-bench.md](../../diarization-bench.md). Engine identities are in the JSON beside this page.
