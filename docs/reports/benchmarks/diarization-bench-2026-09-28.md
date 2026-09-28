# Diarization bench, 2026-09-28

Tree `750e42336b8e`, assignment variant `product`. Values are pooled over each split's files; brackets are 95% bootstrap intervals over files.

## ami-dev

18 files, 580 minutes.

| Metric | Current engine (sherpa-onnx, CPU, main build) | Nemotron q8_0 (dettivo-engine-nemotron, CUDA, main build) |
|---|---:|---:|
| Attribution error (headline) | 18.60% [13.90, 23.63] | 12.00% [10.54, 13.28] |
| wrong speaker (WDER) | 18.60% [13.90, 23.63] | 12.00% [10.54, 13.28] |
| unlabelled | 0.00% [0.00, 0.00] | 0.00% [0.00, 0.00] |
| Short-turn error (<=3 words) | 56.86% [55.19, 58.73] | 51.43% [50.15, 52.64] |
| DER of labelled lines | 41.83% [35.82, 48.25] | 36.94% [33.26, 41.20] |
| missed | 27.80% [25.41, 29.95] | 27.89% [25.45, 30.06] |
| false alarm | 8.08% [6.11, 10.56] | 8.08% [6.11, 10.60] |
| confusion | 5.96% [2.49, 9.99] | 0.97% [0.77, 1.24] |
| Engine DER (turns) | 23.32% [18.49, 28.72] | 17.40% [15.57, 19.99] |
| missed | 5.62% [4.87, 6.47] | 15.27% [13.59, 17.46] |
| false alarm | 6.46% [5.74, 7.30] | 1.76% [1.36, 2.24] |
| confusion | 11.24% [6.83, 16.20] | 0.37% [0.25, 0.56] |
| Lines, right speaker | 80.47% [75.34, 85.66] | 87.99% [86.12, 89.54] |
| Lines, wrong speaker | 19.53% [14.29, 24.65] | 12.00% [10.45, 13.85] |
| Lines, unlabelled | 0.00% [0.00, 0.00] | 0.01% [0.00, 0.02] |
| Labelled lines, wrong speaker | 19.53% [14.29, 24.65] | 12.00% [10.45, 13.85] |
| Words the voice check fixed | 3.90% [3.02, 4.75] | 1.55% [1.03, 2.41] |
| Words the voice check broke | 1.41% [1.14, 1.65] | 1.05% [0.82, 1.31] |
| Units the voice check moved | 10.47% [8.39, 12.50] | 4.89% [3.87, 6.16] |
| Engine speed (x realtime) | 13.53x [12.72, 14.55] | 330.67x [325.82, 334.76] |
| Voice check embedding speed (x realtime) | 114.16x [109.12, 119.64] | 114.48x [109.84, 119.69] |

## ami-test (held out)

4 files, 126 minutes.

| Metric | Current engine (sherpa-onnx, CPU, main build) | Nemotron q8_0 (dettivo-engine-nemotron, CUDA, main build) |
|---|---:|---:|
| Attribution error (headline) | 11.85% [7.82, 13.80] | 11.54% [9.03, 13.09] |
| wrong speaker (WDER) | 11.83% [7.80, 13.78] | 11.54% [9.03, 13.09] |
| unlabelled | 0.01% [0.00, 0.08] | 0.00% [0.00, 0.00] |
| Short-turn error (<=3 words) | 57.46% [56.44, 61.10] | 54.36% [53.70, 57.55] |
| DER of labelled lines | 37.39% [29.82, 44.11] | 37.64% [29.85, 44.56] |
| missed | 30.85% [22.81, 36.51] | 30.97% [22.89, 36.67] |
| false alarm | 5.55% [3.85, 10.21] | 5.55% [3.80, 10.32] |
| confusion | 0.99% [0.76, 1.45] | 1.11% [0.74, 2.23] |
| Engine DER (turns) | 15.67% [13.66, 18.04] | 21.69% [13.30, 29.07] |
| missed | 9.25% [6.28, 10.94] | 20.06% [10.68, 27.34] |
| false alarm | 3.03% [2.16, 4.41] | 0.92% [0.48, 1.55] |
| confusion | 3.39% [2.67, 3.77] | 0.71% [0.25, 1.77] |
| Lines, right speaker | 87.02% [83.26, 93.78] | 83.87% [79.68, 91.13] |
| Lines, wrong speaker | 12.83% [5.57, 16.74] | 16.13% [8.87, 20.32] |
| Lines, unlabelled | 0.15% [0.00, 0.65] | 0.00% [0.00, 0.00] |
| Labelled lines, wrong speaker | 12.85% [5.61, 16.74] | 16.13% [8.87, 20.32] |
| Words the voice check fixed | 3.77% [1.26, 5.24] | 1.19% [0.46, 1.51] |
| Words the voice check broke | 1.50% [0.52, 2.06] | 0.77% [0.35, 0.95] |
| Units the voice check moved | 9.31% [4.91, 11.96] | 3.52% [2.09, 4.16] |
| Engine speed (x realtime) | 13.00x [11.65, 15.24] | 308.62x [298.99, 316.81] |
| Voice check embedding speed (x realtime) | 118.51x [109.82, 134.47] | 116.58x [109.43, 133.76] |

## local-en

4 files, 276 minutes.

| Metric | Current engine (sherpa-onnx, CPU, main build) | Nemotron q8_0 (dettivo-engine-nemotron, CUDA, main build) |
|---|---:|---:|
| Remote lines unlabelled | 0.42% [0.27, 0.81] | 2.18% [1.51, 3.59] |
| Local/remote proxy (mix) | 0.63% [0.43, 1.22] | 0.58% [0.11, 1.80] |
| You lines on remote-only speech | 5.11% [3.04, 6.54] | 5.11% [3.04, 6.54] |
| Local speech outside You lines | 10.09% [7.99, 11.63] | 10.09% [7.99, 11.63] |
| Microphone lines dropped as bleed | 4.63% [2.46, 6.34] | 4.63% [2.46, 6.34] |
| Remote lines relabelled You | 0.00% [0.00, 0.00] | 0.00% [0.00, 0.00] |
| Meetings under the single-remote rule | 0.00% [0.00, 0.00] | 0.00% [0.00, 0.00] |
| Meetings switched to the shared mic | 0.00% [0.00, 0.00] | 0.00% [0.00, 0.00] |
| Units the voice check moved | 1.42% [0.70, 1.79] | 0.88% [0.67, 1.10] |
| Engine speed (x realtime) | 19.02x [17.86, 21.33] | 339.19x [325.46, 348.72] |
| Voice check embedding speed (x realtime) | 124.92x [117.78, 128.58] | 123.22x [111.76, 138.75] |

## local-de

6 files, 278 minutes.

| Metric | Current engine (sherpa-onnx, CPU, main build) | Nemotron q8_0 (dettivo-engine-nemotron, CUDA, main build) |
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
| Engine speed (x realtime) | 16.43x [14.72, 19.93] | 320.71x [308.84, 327.35] |
| Voice check embedding speed (x realtime) | 125.49x [119.28, 137.63] | 122.04x [116.28, 136.49] |

## local

10 files, 555 minutes.

| Metric | Current engine (sherpa-onnx, CPU, main build) | Nemotron q8_0 (dettivo-engine-nemotron, CUDA, main build) |
|---|---:|---:|
| Remote lines unlabelled | 0.32% [0.14, 0.63] | 1.21% [0.58, 2.11] |
| Local/remote proxy (mix) | 0.77% [0.52, 1.23] | 0.35% [0.11, 0.91] |
| You lines on remote-only speech | 7.66% [5.76, 9.50] | 7.66% [5.76, 9.50] |
| Local speech outside You lines | 9.70% [8.71, 10.86] | 9.70% [8.71, 10.86] |
| Microphone lines dropped as bleed | 5.34% [4.06, 6.60] | 5.34% [4.06, 6.60] |
| Remote lines relabelled You | 0.00% [0.00, 0.00] | 0.00% [0.00, 0.00] |
| Meetings under the single-remote rule | 20.00% [0.00, 50.00] | 20.00% [0.00, 50.00] |
| Meetings switched to the shared mic | 0.00% [0.00, 0.00] | 0.00% [0.00, 0.00] |
| Units the voice check moved | 1.08% [0.66, 1.48] | 0.57% [0.35, 0.86] |
| Engine speed (x realtime) | 17.63x [16.09, 19.73] | 329.67x [319.74, 339.04] |
| Voice check embedding speed (x realtime) | 125.21x [120.59, 130.30] | 122.63x [116.18, 132.60] |

Metric definitions: [docs/diarization-bench.md](../../diarization-bench.md). Engine identities are in the JSON beside this page.
