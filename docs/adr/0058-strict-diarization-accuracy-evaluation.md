# 0058. Diarization changes require a strict accuracy evaluation

Status: Accepted 2026-09-09; current implementation accepted for delivery with failed accuracy gates by Gordon on 2026-09-10. Amends [0035](0035-sherpa-onnx-diarization-engine-and-the-speaker-pass.md).

## What this gives you

Diarization changes have a reproducible score against published speaker references. Automatic speaker counts, speech detection and speaker confusion are evaluated together. The [accuracy record](../reports/benchmarks/diarization-accuracy-2026-09-09.md) preserves development results and the validation criteria.

## Context

The [real-meeting experiment](../reports/benchmarks/diarization-real-meetings-2026-09-09.md) found 112-200 automatic labels on four-person meetings. Its manual utterance-segment diagnostics remain useful historical evidence, but their reference intervals differ from the published AMI word-level protocol. CPU/CUDA reproducibility and throughput do not establish diarization accuracy.

## Decision

Use published AMI `only_words` RTTM and UEM files, pinned by commit and file hashes. Use published VoxConverse 0.3 RTTM and a full-recording UEM for the second recording condition. Score with zero collar and overlap included. Sum speaker-error seconds before dividing by reference speaker-seconds for pooled DER; do not average recording percentages.

The repository scorer, `scripts/qa/diarization_score.py`, uses an integer-microsecond event sweep and a global optimal speaker assignment. It rejects malformed inputs and evaluation regions with no reference speech. Its optional `--cross-check` verifies the result against `pyannote.metrics`; ordinary scoring needs only Python's standard library. `just test-diarization-score` run the regression tests as part of the main test gate.

Declare development and held-out recordings before inference. Freeze the model hashes and all inference, clustering and postprocessing parameters before evaluating the held-out set. A held-out recording used for subsequent tuning becomes development data, and a fresh held-out set is required. Local holdout does not establish that a pretrained model never saw a recording.

The working engineering gate requires pooled DER at most 15%, every recording at most 25%, pooled speaker confusion at most 5%, exact automatic speaker count on at least 80% of recordings, and no count above twice the reference count. At least six recordings must span multiple speaker counts and recording conditions. CPU and CUDA use the same protocol, and their pooled DER may differ by at most one percentage point. These criteria were declared before held-out evaluation and remain fixed after results arrive.

The selected development candidate uses the separate `diarization-en` catalogue identity with English VoxCeleb ERes2Net. At threshold 0.6 it trains average-linkage cosine clusters on at least two seconds of non-overlapping speech and removes small training clusters using a minimum population of 1% rounded to even and bounded from one to fifteen. It then consolidates weak centroid fragments at support-weighted angular cost 1.5 and assigns every embedding to the resulting centroids. The [engine guide](../engines.md) defines the cost. Consolidation is disabled when the population floor is one; fewer than two reliable embeddings use all embeddings. Speech and second-speaker vote thresholds are 0.4 and 1.2, with 0.1-second filtering and gap filling. A positive expected count retains the prior complete-linkage, vote and duration behavior. The C API builds this same code for CPU and CUDA; CUDA also retains the convolution-search patch from ADR 0057. Existing explicit configuration values and the original model identity remain available, with migration described in [configuration](../config.md).

## Consequences

Gordon explicitly requested delivery of fn-42 and fn-56 in their current state on 2026-09-10. The existing native calibration is accepted for main and practical beta evaluation despite both failed held-out rounds. The second round remains a 17.72% pooled CPU DER result with two exact counts out of seven; its full CUDA comparison remains incomplete. The original numerical gates above and all negative evidence stay unchanged. Task completion records this delivery decision, not a high-accuracy pass. No experimental replacement model is promoted, and this decision does not certify production readiness or authorize a public release.

Calibration can replay cached development embeddings and segmentation labels after verifying identical output for an unchanged configuration. Cache records must identify the audio, models, inference settings and probe. Negative candidates remain evidence; they do not change the gate. Existing personal model files and configuration remain outside the experiment.

Passing this small local validation set would supply accuracy evidence for the measured change. It would not by itself justify a production-readiness claim. The separate historical CUDA speed criterion remains unmet and is accepted as a delivery limitation in ADR 0057.
