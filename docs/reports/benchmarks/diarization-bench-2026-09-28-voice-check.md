# Diarization bench, 2026-09-28

Tree `e209d5fc0878`, assignment variant `product`. Values are pooled over each split's files; brackets are 95% bootstrap intervals over files.

## ami-dev

18 files, 580 minutes.

| Metric | Current engine (sherpa-onnx, CPU) | Nemotron int8 (fn-64 ONNX runner, CPU) |
|---|---:|---:|
| Attribution error (headline) | 18.60% [13.90, 23.63] | 11.98% [10.55, 13.29] |
| wrong speaker (WDER) | 18.60% [13.90, 23.63] | 11.98% [10.55, 13.29] |
| unlabelled | 0.00% [0.00, 0.00] | 0.00% [0.00, 0.00] |
| Short-turn error (<=3 words) | 56.86% [55.19, 58.73] | 51.41% [50.06, 52.66] |
| DER of labelled lines | 41.83% [35.82, 48.25] | 36.92% [33.25, 41.18] |
| missed | 27.80% [25.41, 29.95] | 27.88% [25.44, 30.04] |
| false alarm | 8.08% [6.11, 10.56] | 8.08% [6.11, 10.59] |
| confusion | 5.96% [2.49, 9.99] | 0.97% [0.77, 1.21] |
| Engine DER (turns) | 23.32% [18.49, 28.72] | 17.42% [15.58, 20.04] |
| missed | 5.62% [4.87, 6.47] | 15.33% [13.64, 17.56] |
| false alarm | 6.46% [5.74, 7.30] | 1.73% [1.35, 2.21] |
| confusion | 11.24% [6.83, 16.20] | 0.36% [0.24, 0.54] |
| Lines, right speaker | 80.47% [75.34, 85.66] | 88.11% [86.33, 89.58] |
| Lines, wrong speaker | 19.53% [14.29, 24.65] | 11.88% [10.41, 13.66] |
| Lines, unlabelled | 0.00% [0.00, 0.00] | 0.01% [0.00, 0.03] |
| Labelled lines, wrong speaker | 19.53% [14.29, 24.65] | 11.88% [10.41, 13.66] |
| Words the voice check fixed | 3.90% [3.02, 4.75] | 1.57% [1.06, 2.44] |
| Words the voice check broke | 1.41% [1.14, 1.65] | 1.05% [0.84, 1.31] |
| Units the voice check moved | 10.47% [8.39, 12.50] | 4.89% [3.91, 6.19] |
| Engine speed (x realtime) | 13.23x [12.28, 14.36] | 52.08x [45.21, 57.27] |
| Voice check embedding speed (x realtime) | 99.09x [90.96, 109.39] | 105.18x [98.00, 112.13] |

## ami-test (held out)

4 files, 126 minutes.

| Metric | Current engine (sherpa-onnx, CPU) | Nemotron int8 (fn-64 ONNX runner, CPU) |
|---|---:|---:|
| Attribution error (headline) | 11.85% [7.82, 13.80] | 11.52% [9.03, 13.05] |
| wrong speaker (WDER) | 11.83% [7.80, 13.78] | 11.52% [9.03, 13.05] |
| unlabelled | 0.01% [0.00, 0.08] | 0.00% [0.00, 0.00] |
| Short-turn error (<=3 words) | 57.46% [56.44, 61.10] | 54.30% [53.58, 57.63] |
| DER of labelled lines | 37.39% [29.82, 44.11] | 37.63% [29.85, 44.57] |
| missed | 30.85% [22.81, 36.51] | 30.96% [22.89, 36.64] |
| false alarm | 5.55% [3.85, 10.21] | 5.55% [3.80, 10.32] |
| confusion | 0.99% [0.76, 1.45] | 1.13% [0.74, 2.26] |
| Engine DER (turns) | 15.67% [13.66, 18.04] | 21.78% [13.38, 29.20] |
| missed | 9.25% [6.28, 10.94] | 20.16% [10.78, 27.45] |
| false alarm | 3.03% [2.16, 4.41] | 0.90% [0.48, 1.53] |
| confusion | 3.39% [2.67, 3.77] | 0.71% [0.24, 1.79] |
| Lines, right speaker | 87.02% [83.26, 93.78] | 83.86% [79.71, 91.03] |
| Lines, wrong speaker | 12.83% [5.57, 16.74] | 16.14% [8.97, 20.29] |
| Lines, unlabelled | 0.15% [0.00, 0.65] | 0.00% [0.00, 0.00] |
| Labelled lines, wrong speaker | 12.85% [5.61, 16.74] | 16.14% [8.97, 20.29] |
| Words the voice check fixed | 3.77% [1.26, 5.24] | 1.25% [0.44, 1.61] |
| Words the voice check broke | 1.50% [0.52, 2.06] | 0.80% [0.35, 0.99] |
| Units the voice check moved | 9.31% [4.91, 11.96] | 3.62% [1.93, 4.38] |
| Engine speed (x realtime) | 13.60x [12.47, 15.10] | 54.80x [54.11, 56.03] |
| Voice check embedding speed (x realtime) | 103.16x [87.01, 127.32] | 99.24x [85.19, 110.58] |

## local-en

4 files, 276 minutes.

| Metric | Current engine (sherpa-onnx, CPU) | Nemotron int8 (fn-64 ONNX runner, CPU) |
|---|---:|---:|
| Remote lines unlabelled | 0.51% [0.39, 0.81] | 2.35% [1.74, 3.65] |
| Local/remote proxy (mix) | 0.63% [0.43, 1.22] | 0.57% [0.10, 1.80] |
| You lines on remote-only speech | 5.16% [3.04, 6.65] | 5.16% [3.04, 6.65] |
| Local speech outside You lines | 10.11% [7.99, 11.68] | 10.11% [7.99, 11.68] |
| Microphone lines dropped as bleed | 4.75% [2.46, 6.52] | 4.75% [2.46, 6.52] |
| Remote lines relabelled You | 0.00% [0.00, 0.00] | 0.00% [0.00, 0.00] |
| Meetings under the single-remote rule | 0.00% [0.00, 0.00] | 0.00% [0.00, 0.00] |
| Meetings switched to the shared mic | 0.00% [0.00, 0.00] | 0.00% [0.00, 0.00] |
| Units the voice check moved | 1.27% [0.70, 1.57] | 0.73% [0.51, 1.10] |
| Engine speed (x realtime) | 19.44x [17.73, 23.20] | 49.72x [46.58, 57.96] |
| Voice check embedding speed (x realtime) | 113.79x [105.90, 117.94] | 111.90x [102.96, 130.25] |

## local-de

6 files, 278 minutes.

| Metric | Current engine (sherpa-onnx, CPU) | Nemotron int8 (fn-64 ONNX runner, CPU) |
|---|---:|---:|
| Remote lines unlabelled | 0.25% [0.00, 0.83] | 0.50% [0.15, 1.11] |
| Local/remote proxy (mix) | 0.90% [0.52, 1.81] | 0.14% [0.08, 0.27] |
| You lines on remote-only speech | 9.97% [8.41, 11.47] | 9.97% [8.41, 11.47] |
| Local speech outside You lines | 9.33% [8.68, 10.31] | 9.33% [8.68, 10.31] |
| Microphone lines dropped as bleed | 5.92% [4.36, 7.36] | 5.92% [4.36, 7.36] |
| Remote lines relabelled You | 0.00% [0.00, 0.00] | 0.00% [0.00, 0.00] |
| Meetings under the single-remote rule | 33.33% [0.00, 66.67] | 33.33% [0.00, 66.67] |
| Meetings switched to the shared mic | 0.00% [0.00, 0.00] | 0.00% [0.00, 0.00] |
| Units the voice check moved | 0.84% [0.39, 1.27] | 0.37% [0.18, 0.68] |
| Engine speed (x realtime) | 17.51x [16.19, 20.40] | 48.98x [41.93, 56.25] |
| Voice check embedding speed (x realtime) | 111.14x [96.93, 129.36] | 111.87x [105.45, 126.27] |

## local

10 files, 555 minutes.

| Metric | Current engine (sherpa-onnx, CPU) | Nemotron int8 (fn-64 ONNX runner, CPU) |
|---|---:|---:|
| Remote lines unlabelled | 0.36% [0.16, 0.67] | 1.27% [0.60, 2.18] |
| Local/remote proxy (mix) | 0.77% [0.52, 1.23] | 0.35% [0.11, 0.91] |
| You lines on remote-only speech | 7.69% [5.79, 9.51] | 7.69% [5.79, 9.51] |
| Local speech outside You lines | 9.71% [8.71, 10.89] | 9.71% [8.71, 10.89] |
| Microphone lines dropped as bleed | 5.39% [4.08, 6.63] | 5.39% [4.08, 6.63] |
| Remote lines relabelled You | 0.00% [0.00, 0.00] | 0.00% [0.00, 0.00] |
| Meetings under the single-remote rule | 20.00% [0.00, 50.00] | 20.00% [0.00, 50.00] |
| Meetings switched to the shared mic | 0.00% [0.00, 0.00] | 0.00% [0.00, 0.00] |
| Units the voice check moved | 1.02% [0.65, 1.36] | 0.51% [0.33, 0.78] |
| Engine speed (x realtime) | 18.42x [17.06, 20.69] | 49.35x [45.19, 54.75] |
| Voice check embedding speed (x realtime) | 112.44x [103.66, 120.96] | 111.89x [105.72, 122.17] |

Metric definitions: [docs/diarization-bench.md](../../diarization-bench.md). Engine identities are in the JSON beside this page.
