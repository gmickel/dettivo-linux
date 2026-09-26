# Diarization bench, 2026-09-26

Tree `f4abbf7f894d`, assignment variant `product`. Values are pooled over each split's files; brackets are 95% bootstrap intervals over files.

## ami-dev

18 files, 580 minutes.

| Metric | Current engine (sherpa-onnx, CPU) | Nemotron int8 (fn-64 ONNX runner, CPU) |
|---|---:|---:|
| Attribution error (headline) | 28.99% [23.68, 34.31] | 22.43% [19.06, 25.94] |
| wrong speaker | 13.88% [9.57, 18.63] | 7.57% [6.78, 8.20] |
| unlabelled | 15.11% [12.63, 17.73] | 14.87% [12.03, 18.16] |
| Short-turn error (<=3 words) | 79.45% [77.98, 80.76] | 74.44% [73.10, 76.22] |
| DER of labelled lines | 42.20% [35.84, 48.78] | 35.38% [31.39, 40.10] |
| missed | 24.32% [21.43, 27.17] | 24.79% [21.48, 28.42] |
| false alarm | 9.91% [7.70, 12.54] | 8.49% [6.93, 10.30] |
| confusion | 7.97% [4.01, 12.39] | 2.10% [1.82, 2.42] |
| Engine DER (turns) | 23.32% [18.49, 28.72] | 17.42% [15.58, 20.04] |
| missed | 5.62% [4.87, 6.47] | 15.33% [13.64, 17.56] |
| false alarm | 6.46% [5.74, 7.30] | 1.73% [1.35, 2.21] |
| confusion | 11.24% [6.83, 16.20] | 0.36% [0.24, 0.54] |
| Lines, right speaker | 66.43% [61.71, 71.89] | 73.90% [69.77, 77.21] |
| Lines, wrong speaker | 12.12% [7.02, 17.45] | 4.27% [3.72, 4.85] |
| Lines, unlabelled | 21.45% [18.54, 23.95] | 21.83% [18.71, 25.74] |
| Engine speed (x realtime) | 13.23x [12.28, 14.36] | 52.08x [45.21, 57.27] |

## ami-test (held out)

4 files, 126 minutes.

| Metric | Current engine (sherpa-onnx, CPU) | Nemotron int8 (fn-64 ONNX runner, CPU) |
|---|---:|---:|
| Attribution error (headline) | 22.98% [14.45, 28.57] | 21.92% [15.91, 25.87] |
| wrong speaker | 8.73% [6.39, 9.99] | 8.05% [6.04, 9.24] |
| unlabelled | 14.24% [7.63, 18.58] | 13.87% [8.43, 16.62] |
| Short-turn error (<=3 words) | 79.10% [77.03, 80.00] | 75.68% [71.74, 77.27] |
| DER of labelled lines | 34.32% [24.91, 40.96] | 33.99% [25.64, 39.40] |
| missed | 25.60% [14.56, 33.38] | 25.95% [16.60, 32.55] |
| false alarm | 6.75% [4.66, 12.46] | 6.26% [4.25, 11.76] |
| confusion | 1.97% [1.50, 2.27] | 1.78% [1.17, 2.95] |
| Engine DER (turns) | 15.67% [13.66, 18.04] | 21.78% [13.38, 29.20] |
| missed | 9.25% [6.28, 10.94] | 20.16% [10.78, 27.45] |
| false alarm | 3.03% [2.16, 4.41] | 0.90% [0.48, 1.53] |
| confusion | 3.39% [2.67, 3.77] | 0.71% [0.24, 1.79] |
| Lines, right speaker | 75.60% [71.40, 81.88] | 73.58% [70.18, 79.34] |
| Lines, wrong speaker | 5.50% [2.38, 7.11] | 6.77% [2.90, 9.26] |
| Lines, unlabelled | 18.90% [14.03, 21.65] | 19.65% [17.27, 20.99] |
| Engine speed (x realtime) | 13.60x [12.47, 15.10] | 54.80x [54.11, 56.03] |

## local-en

4 files, 276 minutes.

| Metric | Current engine (sherpa-onnx, CPU) | Nemotron int8 (fn-64 ONNX runner, CPU) |
|---|---:|---:|
| Remote lines unlabelled | 26.39% [17.35, 40.46] | 27.04% [18.26, 42.33] |
| Local/remote proxy (mix) | 0.63% [0.43, 1.22] | 0.57% [0.10, 1.80] |
| Engine speed (x realtime) | 19.44x [17.73, 23.20] | 49.72x [46.58, 57.96] |

## local-de

6 files, 278 minutes.

| Metric | Current engine (sherpa-onnx, CPU) | Nemotron int8 (fn-64 ONNX runner, CPU) |
|---|---:|---:|
| Remote lines unlabelled | 11.06% [7.65, 18.61] | 13.29% [8.83, 22.90] |
| Local/remote proxy (mix) | 0.90% [0.52, 1.81] | 0.14% [0.08, 0.27] |
| Engine speed (x realtime) | 17.51x [16.19, 20.40] | 48.98x [41.93, 56.25] |

## local

10 files, 555 minutes.

| Metric | Current engine (sherpa-onnx, CPU) | Nemotron int8 (fn-64 ONNX runner, CPU) |
|---|---:|---:|
| Remote lines unlabelled | 18.01% [11.64, 27.05] | 19.52% [12.96, 28.67] |
| Local/remote proxy (mix) | 0.77% [0.52, 1.23] | 0.35% [0.11, 0.91] |
| Engine speed (x realtime) | 18.42x [17.06, 20.69] | 49.35x [45.19, 54.75] |

Metric definitions: [docs/diarization-bench.md](../../diarization-bench.md). Engine identities are in the JSON beside this page.
