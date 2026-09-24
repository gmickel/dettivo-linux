# Classify unavailable GPU workload attribution at release

## Source

Gordon requested merging and releasing Dettivo for real meeting use. The merged release gate passed its meeting pack but classified the GPU proof step's explicit unavailable-attribution result as unexplained. Existing release policy names unavailable GPU counters as external prerequisites.

## Scope

Recognize the two concrete unavailable-attribution reasons emitted by the GPU sampler, only for the release meeting pack's GPU proof blocker. Preserve the skipped proof and raw counter evidence. Do not claim inference execution from allocations or aggregate utilization, and keep failed rows blocking release.

## Acceptance Criteria

- **R1:** The NVIDIA allocation-only and aggregate utilization-only skips name an external requirement for a counter that attributes workload to the tested engine. Other steps and unknown attribution reasons remain unexplained.
- **R2:** A failed GPU proof row still fails the release step regardless of its reason classification. The repaired release gate passes only with zero failed steps and zero unexplained blockers.

## Verification

Focused regression tests, the full build/test/lint gate and a fresh complete release gate. Retain the preceding failed release report.
