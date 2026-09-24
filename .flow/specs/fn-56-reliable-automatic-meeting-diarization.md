# Reliable automatic meeting diarization


## Delivery decision (2026-09-10)

Gordon explicitly requested delivery of fn-42 and fn-56 in their current state to synchronize main. The criteria below govern this delivery only. The original criteria and every failed measurement remain historical evidence below; completion records accepted limitations, never a benchmark pass. This delivery covers the existing code and documentation, with no new model experiments, host installation, release tag, or public release.

## Acceptance Criteria

- **R1:** Deliver the strict AMI/VoxConverse evaluation, scorer cross-checks, pinned inputs and declared recording sets. Preserve the distinction between manual-segment diagnostics and word-level strict DER.
- **R2:** Retain upstream reproduction, calibrated model/clustering experiments, cache identities and negative results. Unsuccessful alternatives are not promoted into the product.
- **R3:** Deliver the current native automatic-clustering calibration and catalogue/configuration integration, preserving explicit speaker counts, CPU availability, provider fallback boundaries, cancellation and personal data isolation.
- **R4:** Record Gordon's explicit acceptance of the current implementation for delivery despite both failed held-out rounds. Preserve the original numerical gates and outcomes unchanged, including the second round's 17.72% pooled CPU DER and two exact counts out of seven. No held-out accuracy pass or production-readiness claim is made.
- **R5:** Retain existing native CPU/CUDA paired-development evidence and the incomplete second-round CUDA evaluation with its infrastructure failure. Run the repository build/test/lint gate on the delivery code and update the ADR and accuracy report to distinguish accepted delivery from validated high accuracy.

## Goal & Context

Gordon requested: "this is terrible yes and not ready for productions, regardless of cpu/gpu, lets work on it until we have high accuracy". Automatic diarization on three real AMI meetings produced 112-200 labels for four annotated people. Supplying the known count largely removed speaker confusion. Quality is the priority for this work; CPU and CUDA must both be evaluated. The existing fn-42 GPU speed target remains a separate, unresolved criterion.

This branch starts from the fn-42 implementation and its recorded negative results at 05e7f80a. Those results are historical evidence, not acceptance of the current model or automatic-clustering defaults.

## Historical acceptance criteria (retained unchanged)

- **R1:** Establish a reproducible accuracy evaluation using published AMI word-level references and evaluation regions, zero forgiveness collar and overlap included. Cross-check the scorer against an established implementation. Preserve the earlier manual-segment diagnostics as a separate protocol. Predeclare development and held-out recording sets; the three previously inspected meetings are development data, never held-out proof.
- **R2:** Reproduce the failure through upstream Sherpa with identical assets/settings, then test model-appropriate clustering calibration and supported embedding/segmentation alternatives. Cache intermediate inference results when that safely accelerates calibration. Preserve negative trials, exact configuration/model hashes, and source attribution. Ground any product changes in measured improvements rather than forcing known participant counts in automatic mode.
- **R3:** Implement the selected accuracy improvements through the native engine, configuration/model catalogue and daemon/UI paths that depend on them. Preserve CPU availability, explicit speaker-count support, GPU fallback, cancellation and data isolation. Existing model files and personal configuration must not be overwritten by development experiments.
- **R4:** Before held-out evaluation, freeze the chosen parameters and model. Working engineering quality gate: strict pooled DER at most 15%, no recording above 25%, pooled speaker confusion at most 5%, exact automatic count on at least 80% of held-out recordings, and no count greater than twice the annotated count. Use at least six held-out recordings with multiple speaker counts and more than one corpus or recording condition. Run without oracle speaker counts; report per-recording errors, counts and resource use. These inferred targets were declared before evaluation and must not be weakened after observing held-out outcomes.
- **R5:** Verify both CPU and CUDA with the same scoring protocol. Report quality differences rather than requiring arbitrary speaker-ID string equality; the pooled DER difference must not exceed one percentage point. Retain the existing short-fixture checks alongside the real-audio gate, and run the repository build/test/lint gate after implementation. Update affected docs and ADRs. Do not call the result production-ready solely because a small validation set passes.

## Boundaries

- No live-system installation, changes to personal model/configuration files, cloud audio uploads, external messages or new model-access acceptance without authorization.
- Public, appropriately licensed audio and test-only model downloads may be staged outside the repository. No raw meeting audio or transcript text is committed.
- Model, clustering and segmentation changes are permitted when quality evidence justifies them. Timing optimizations must not trade away the accuracy gate.
- Do not start the held cleanup backlog, alter fn-38 human approvals, or silently clear fn-42's historical speed failure.

## Evidence Pointers

- docs/reports/benchmarks/diarization-real-meetings-2026-09-09.md and JSON companion.
- docs/reports/benchmarks/diarization-cuda-tuning-2026-09-09.md.
- Raw AMI recordings/annotations and test harnesses under /home/gordon/work/dettivo-linux-wt/_factory/fn42-real-meetings-nQC21b/.
- Upstream Sherpa model guidance documents threshold calibration and alternate embedding models; a complete local pyannote pipeline is a useful reference if existing access permits it.
