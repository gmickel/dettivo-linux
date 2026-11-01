# Benchmark reports

The numbers Dettivo for Linux promises, measured on real machines: `dettivo-qa bench` (`just bench`) runs the NFR steps from the fixtures the tests use and files one report per host and tier here, every figure beside its calibrated target ([ADR 0029](../../adr/0029-nfr-calibration-and-the-benchmark-suite.md), the suite in [docs/qa.md](../../qa.md)). This table is rendered from the latest report per host and tier; `just bench` rewrites it.

## thor on the cpu tier (2026-09-14)

AMD Ryzen 9 7950X3D 16-Core Processor with NVIDIA GeForce RTX 4090 (NVIDIA 610.57.04), 61 GiB, commit `19613bb5be85`; DETTIVO_FORCE_CPU=1 forces the CPU tier. Load average 8.1 at the start and 3.7 at the end of the run (1 min).

| Step | NFR | Metric | Measured | Target | Verdict |
|---|---|---|---|---|---|
| `first_insert` | NFR-2 | first_insert_p50 | 356 ms | <= 1500 ms | met |
| `first_insert` · whisper large-v3-turbo | NFR-2 | first_insert_p50 | 6059 ms | <= 1500 ms | not met |
| `first_insert` · parakeet parakeet-v3 | NFR-2 | first_insert_p50 | 356 ms | <= 1500 ms | met |
| `stt_throughput` | NFR-5 | stt_realtime_factor | 23.99 x realtime | >= 5.00 x realtime | met |
| `stt_throughput` · whisper large-v3-turbo | NFR-5 | stt_realtime_factor | 2.39 x realtime | >= 5.00 x realtime | not met |
| `stt_throughput` · parakeet parakeet-v3 | NFR-5 | stt_realtime_factor | 23.99 x realtime | >= 5.00 x realtime | met |
| `llm_rewrite` | - | - | - | - | recorded |
| `idle_footprint` | NFR-6 | daemon_idle_rss | 24.2 MiB | <= 60.0 MiB | met |
| `startup` | NFR-8 | socket_ready_p50 | 282 ms | <= 300 ms | met |
| `meeting_throughput` · whisper base.en (cpu) | NFR-5 | stt_realtime_factor | 35.38 x realtime | >= 5.00 x realtime | met |
| `diarization_throughput` | NFR-4 | diarization_realtime_factor | 16.11 x realtime | >= 4.00 x realtime | met |
| `gpu_workload_proof` | - | - | - | - | skip: the run was on the cpu tier (--cpu): no GPU work expected; diarization used CPU: no CUDA proof expected |
| `meeting_token_coverage` | - | token_coverage | 8 of 8 tokens | every token on its source | met |

- the insert leg of first_insert runs through the mock inserter; the dictation pack's first_insert_timing measures the real chain
- the CPU run is --cpu on the GPU desktop until a CPU-only VM exists (S-16 boundary)
- meetings: recorded 2026-09-14 from commit `19613bb5be85` by `dettivo-qa pack meetings --cpu`, the pack passed (311701 ms of audio in 8809 ms)

## thor on the gpu tier (2026-11-01)

AMD Ryzen 9 7950X3D 16-Core Processor with NVIDIA GeForce RTX 4090 (NVIDIA 615.78.08), 61 GiB, commit `c4ca60940821`; a Vulkan device is installed and dettivo-engine-whisper runs on it. Load average 8.5 at the start and 0.9 at the end of the run (1 min).

| Step | NFR | Metric | Measured | Target | Verdict |
|---|---|---|---|---|---|
| `first_insert` | NFR-1 | first_insert_p50 | 522 ms | <= 1000 ms | met |
| `first_insert` · whisper large-v3-turbo | NFR-1 | first_insert_p50 | 522 ms | <= 1000 ms | met |
| `first_insert` · parakeet parakeet-v3 | NFR-1 | first_insert_p50 | 266 ms | <= 1000 ms | met |
| `warm_up` | - | - | - | - | recorded |
| `gpu_threads` | - | - | - | - | recorded |
| `stt_throughput` | NFR-4 | stt_realtime_factor | 82.40 x realtime | >= 20.00 x realtime | met |
| `stt_throughput` · whisper large-v3-turbo | NFR-4 | stt_realtime_factor | 82.40 x realtime | >= 20.00 x realtime | met |
| `stt_throughput` · parakeet parakeet-v3 | NFR-4 | stt_realtime_factor | 141.54 x realtime | >= 20.00 x realtime | met |
| `llm_rewrite` | - | - | - | - | recorded |
| `idle_footprint` | NFR-6 | daemon_idle_rss | 49.2 MiB | <= 60.0 MiB | met |
| `startup` | NFR-8 | socket_ready_p50 | 202 ms | <= 300 ms | met |
| `language_wer` | - | - | - | - | recorded |

- the insert leg of first_insert runs through the mock inserter; the dictation pack's first_insert_timing measures the real chain

