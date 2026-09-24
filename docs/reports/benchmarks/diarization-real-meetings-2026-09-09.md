# Real meetings validate CUDA savings and expose speaker-count failure

The tuned CUDA engine reduced wall time by 11–37% and process CPU time by 78–84% on three full AMI recordings. Automatic diarization failed the participant-count sanity check on every recording. Four annotated participants became 112–200 output speaker labels. The outputs repeated exactly within each provider, but CPU and CUDA assignments differed. These results support a compute-offload benefit and require an automatic-clustering fix before recommending this configuration broadly.

A separate control supplied the known four-speaker count for the 21-minute recording. Speaker-confusion error fell from approximately 59–60% to 2–3% against the manual segment annotations. That control used extra information unavailable to automatic mode. It does not turn the automatic validation into a pass, establish full meeting quality, or satisfy fn-42's existing fourfold-throughput criterion.

## Recordings and method

Audio and manual annotations are from the [AMI Meeting Corpus](https://groups.inf.ed.ac.uk/ami/corpus/), released by the AMI project under [CC BY 4.0](https://groups.inf.ed.ac.uk/ami/corpus/license.shtml). The original audio files were used unchanged. Their actual WAV durations, rather than rounded corpus metadata, define the measurement duration.

| Recording | Type | Duration | Annotated participants | Original audio |
|---|---|---:|---:|---|
| ES2002a | Scenario meeting | 21.21 minutes | 4 | [Headset mix](https://groups.inf.ed.ac.uk/ami/AMICorpusMirror/amicorpus/ES2002a/audio/ES2002a.Mix-Headset.wav) |
| IS1009b | Scenario meeting, different participants/site | 34.21 minutes | 4 | [Headset mix](https://groups.inf.ed.ac.uk/ami/AMICorpusMirror/amicorpus/IS1009b/audio/IS1009b.Mix-Headset.wav) |
| EN2001b | Ordinary research-group meeting | 57.53 minutes | 4 | [Headset mix](https://groups.inf.ed.ac.uk/ami/AMICorpusMirror/amicorpus/EN2001b/audio/EN2001b.Mix-Headset.wav) |

All files are mono 16 kHz PCM16. There was no repetition, truncation, resampling, model change or default change. These are English headset mixes, including two scenario-based sessions and one non-scenario meeting. They do not represent all languages, distant microphones or arbitrary production meetings.

Each provider ran twice per recording in a fresh process. CPU/CUDA order was reversed on the second pass and alternated between recordings. The same tuned CUDA-capable engine was invoked with explicit `--provider cpu` or `--provider cuda`, `--threads 4` and automatic speaker counting. This isolates the provider comparison; it is not a comparison between different CPU and GPU packages. Cold-process wall time includes loading and inference. Filesystem caches were not cleared, CPU affinity was not pinned and the workstation was not otherwise load-isolated.

The environment was AMD Ryzen 9 7950X3D, 32 logical CPUs, RTX 4090, NVIDIA driver 610.57.04 and kernel 7.1.8-arch1-3. CUDA 13.3.1 and the previously extracted process-local cuDNN 9.25.0.15 were used. No libraries were installed on the host. `LD_PRELOAD`, the diagnostic convolution hook and forced-CPU environment overrides were absent.

The [machine-readable receipt](diarization-real-meetings-2026-09-09.json) records the repository snapshot, engine, four runtime libraries, models, audio and annotation hashes. Post-flight checks confirmed that the measured engine, libraries, models and audio had not changed. The source-built CUDA C API in the staged package is the tuned library from the [preceding experiment](diarization-cuda-tuning-2026-09-09.md).

## Runtime and resource results

Each central time below is the median of two runs, equivalent to their midpoint. Ranges retain the observed variation. CUDA throughput ratios use the CPU median divided by the CUDA median. These are observations from two runs, not confidence intervals or a universal performance guarantee.

| Recording | CPU seconds, median [range] | CUDA seconds, median [range] | CUDA / CPU throughput | Wall time saved | CPU time saved |
|---|---:|---:|---:|---:|---:|
| ES2002a | 90.87 [90.66–91.07] | 80.64 [80.26–81.02] | 1.13x | 11.3% | 77.5% |
| IS1009b | 193.02 [178.27–207.77] | 121.78 [121.19–122.37] | 1.58x | 36.9% | 83.9% |
| EN2001b | 301.09 [296.44–305.75] | 227.33 [224.94–229.73] | 1.32x | 24.5% | 80.9% |

CUDA was faster in all six paired comparisons. IS1009b's CPU variation was substantial, so its initial 1.71x ratio should not be treated as the final estimate. No cause for that runtime variation was isolated.

CPU time is the engine process's user plus system time, including its threads, obtained with `wait4`. It excludes the separate sampler processes. CPU mode used roughly four cores' worth of processing time; CUDA mode used roughly one. This is evidence of CPU offload, not an energy measurement.

| Recording | CPU peak process RSS | CUDA peak process RSS | CUDA peak sampled GPU memory |
|---|---:|---:|---:|
| ES2002a | 1.14 GiB | 1.80 GiB | 1,082 MiB |
| IS1009b | 2.04 GiB | 2.72 GiB | 844 MiB |
| EN2001b | 4.51 GiB | 5.13 GiB | 844 MiB |

Process RSS peaks come from the kernel's per-child resource usage. GPU memory and exact engine-PID presence were sampled every 500 ms with `nvidia-smi`; those GPU figures are sampled maxima, not allocator peaks. Every CUDA run had exact-PID evidence and no sampler errors. The engine's RSS is separate from the desktop app's memory budget and includes shared resident mappings.

## Automatic speaker output failed validation

All 12 inference processes completed successfully and reported the requested backend. Automatic-mode quality failed despite those successful exits.

| Recording | Annotated participants | CPU labels, both runs | CUDA labels, both runs | CPU-reference provider disagreement |
|---|---:|---:|---:|---:|
| ES2002a | 4 | 113 | 112 | 12.52% |
| IS1009b | 4 | 133 | 134 | 10.63% |
| EN2001b | 4 | 200 | 197 | 20.16% |

The complete turn arrays were identical between each provider's two runs. CPU versus CUDA arrays differed on all three recordings. The disagreement column scores CUDA against CPU after optimal global speaker-label matching, with zero collar and overlap included. It includes timing, missed/extra speech and speaker confusion. It is not an accuracy score against humans, and the residual disagreement means these differences are not merely label renaming.

The [AMI manual annotations v1.6.2](https://groups.inf.ed.ac.uk/ami/AMICorpusAnnotations/ami_public_manual_1.6.2.zip) provide a second diagnostic. The scorer uses each participant's `transcriber_start`/`transcriber_end` utterance-segment intervals, exact millisecond boundaries, optimal one-to-one label assignment, no collar, overlap included and the full recording. These intervals include utterance-internal pauses and are not an official AMI evaluation recipe or a standardized word-level reference. No new pass threshold was introduced.

| Recording | CPU manual-segment DER diagnostic | CUDA manual-segment DER diagnostic |
|---|---:|---:|
| ES2002a | 84.64% | 85.74% |
| IS1009b | 53.40% | 51.19% |
| EN2001b | 80.23% | 80.12% |

The score components and reference file names are retained in the JSON receipt. A speech-activity correlation check over plus/minus 50 seconds found its best alignment at zero shift for all three recordings; no timing correction was applied. This rules out a gross timing offset in that diagnostic, not all annotation limitations. The scorer passed nine synthetic checks covering label permutations, missed and extra speech, merged speakers, overlaps, duplicate same-speaker intervals and empty inputs.

These results do not show a consistent accuracy loss from CUDA alone. Both providers over-fragment speakers, and the manual-segment scores move in both directions. They do show that the short-fixture parity result did not generalize to automatic counting on longer recordings.

## Known four-speaker control

After all automatic runs, ES2002a was processed once more per provider with `--speakers 4`. All other engine/model settings were unchanged. This separate control supplies the corpus's known participant count and does not modify the application's defaults.

| Provider | Output labels | Total manual-segment DER | Speaker-confusion component | Wall seconds |
|---|---:|---:|---:|---:|
| CPU | 4 | 28.93% | 3.15% | 94.54 |
| CUDA | 4 | 27.93% | 2.23% | 83.18 |

Supplying the count removed most of the speaker-confusion component. Missed and extra/unreferenced speech still contributed approximately 26 percentage points to these diagnostic scores. CPU/CUDA turn arrays still differed, with 4.49% CPU-reference disagreement after label matching. This one control supports investigating automatic clustering/count selection first; it is not validation of known-count quality across the entire corpus. Exact-PID GPU sampling was performed in the 12 primary runs, not repeated in this two-run control.

## Artifacts and disposition

Raw files, harnesses and outputs remain outside the repository under `/home/gordon/work/dettivo-linux-wt/_factory/fn42-real-meetings-nQC21b/`:

- `run-20260909-103643/report.json` contains all 12 automatic runs, process resource usage, complete outputs and GPU samples. Its `scores.json` contains full label mappings and repeatability comparisons.
- `known-four-20260909-111250/report.json` and `scores.json` retain the separate known-count control.
- `validate.py`, `score.py`, `known_four.py`, `check_alignment.py` and `summarize.py` retain the execution and scoring procedure. Their hashes are in the compact repository receipt.
- The three original WAVs and the manual-annotation archive remain local; this report does not redistribute the recordings or transcript text.

This was validation of the frozen engine at repository snapshot `11f79850`, not an engine-code change, model calibration, full installed-session QA or release. Documentation/JSON checks accompany this report; the unchanged implementation's prior full build/test/lint gate is recorded in the preceding tuning report.

fn-42 remains blocked. Its fourfold-throughput requirement is still unmet, and this validation adds an automatic speaker-clustering problem that needs separate corrective work. Thresholds, defaults and models were unchanged. No installation, PR or release was performed.
