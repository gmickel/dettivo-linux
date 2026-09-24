# Desktop memory budgets grounded in measured usage

## Conversation Evidence

> user (turn 1): "ok, what is RSS"
> user (turn 2): "ok, and is this realistic to get it to 60MB"
> user (turn 3): "ok, we need to recalibrate that, ie enter a different number that is more precise and add capture a spec for it with full context of what you already know"

> user (2026-09-12, approving the proposed 320 MiB RSS / 128 MiB anonymous reference caps): "ok, yes raise it"

## Approved contract, 2026-09-12

Gordon approved RSS at most **320 MiB (327,680 KiB)** and anonymous memory at most **128 MiB (131,072 KiB)** for the designated Thor reference. PSS remains uncapped. This supersedes the original inferred 256 MiB / 96 MiB proposal after native cold launches exceeded it. The original proposal, failed samples, rejected cleanup experiments and their verdicts remain historical evidence. This is an explicitly approved recalibration and claims no reduction in RAM use.

## Goal & Context

<!-- Source: user intent paraphrased; measurement context verified from local QA evidence. Numerical replacement budgets are inferred proposals. -->

Give Gordon a realistic, explicitly defined desktop memory requirement and a regression check that measures it consistently. Replace the original 60 MB RSS requirement with a measured allowance for the existing native app, while preserving visibility into avoidable allocation growth. [paraphrase]

The original gate used 60 × 1024 KiB, or 60 MiB, despite documentation calling it 60 MB. RSS includes resident shared libraries and graphics-driver mappings. PSS apportions shared pages across processes. Anonymous memory covers allocations such as heaps and stacks, including allocations made by Qt and drivers; it is not an exact count of Dettivo-owned objects.

Two post-fix, five-second Home snapshots from the 2026-09-08 QA pass were:

| Snapshot | RSS | PSS | Anonymous | First frame |
|---|---:|---:|---:|---:|
| Deferred playback initialization | 235,556 KiB / 230.04 MiB | 140,273 KiB / 136.99 MiB | 80,532 KiB / 78.64 MiB | 255 ms |
| Final app-route rerun | 235,948 KiB / 230.42 MiB | 179,341 KiB / 175.14 MiB | 81,056 KiB / 79.16 MiB | 252 ms |

These used Release Qt and debug Rust binaries in isolated Xvfb sessions, with fixture data and a separate daemon. They establish a development reference, not a packaged native-Wayland release distribution. The PSS variation at almost unchanged RSS also argues for retaining the breakdown and session context.

The original inferred replacement was **RSS at most 256 MiB**, plus **anonymous memory at most 96 MiB**, for the designated Thor desktop reference profile. PSS remains reported without a new hard cap. That proposal left about 11% RSS and 21% anonymous-memory headroom above the 2026-09-08 sample. These are explicit engineering allowances, not statistically established upper bounds or claimed memory savings. [inferred]

## Architecture & Data Models

<!-- Source: verified existing measurement behavior; proposed enforcement is inferred. -->

Extend the existing app timing and footprint evidence rather than introduce a second benchmark framework. Preserve raw KiB values and report MiB, budget, result, process identity, build configuration, renderer, display protocol and measurement timing. Scope the desktop-app budget to its own process; identify the daemon and engine processes as excluded rather than implying this is total workstation consumption.

## Edge Cases & Constraints

- Shared mappings, other running graphics clients, driver versions, rendering backend, display scale and loaded routes can affect the result. Compare equivalent reference profiles and label other profiles explicitly. [inferred]
- Missing, unreadable or malformed memory metrics cannot count as zero or produce a pass. A process that exits before sampling is a failed measurement. [inferred]
- A requirement change preserves historical measurements and verdicts. A fresh result under the replacement contract must distinguish recalibration from actual optimization. [inferred]

## Acceptance Criteria

- **R1:** Enforce the approved app-idle RSS contract of RSS ≤320 MiB (327,680 KiB) and anonymous memory ≤128 MiB (131,072 KiB) for the designated Thor reference profile, superseding the original 60 MB/MiB contract and the intermediate 256 MiB / 96 MiB proposal. Report PSS without enforcing an unsupported universal PSS limit. Errors and boundaries: each exact limit passes; one KiB above either limit fails. [direct approval, 2026-09-12]
- **R2:** Verify the replacement through five independent fresh app launches per reference mode, sampling Home after five seconds idle with the daemon connected, no active capture, playback, download or analysis, fixed fixture data and recorded window/scale/theme. Exercise native Wayland on Thor and retain Xvfb as the reproducible secondary profile. Use a Release Qt app with release-profile Rust companions for release validation; label the existing mixed-build snapshots as historical context. Every run must satisfy the limits; report individual values and maxima. Errors: missing metrics, early exit, unsupported environment or unavailable release binaries are explicit failures or blockers, never successful zero values. [inferred]
- **R3:** Keep raw measurements, exact units, enforced limits, environment metadata and per-metric results in the existing evidence/reporting path. Preserve existing evidence-field meanings; do not silently reinterpret RSS as PSS or anonymous memory. Errors: invalid or partially readable samples are surfaced and cannot satisfy an enforced gate. [inferred]
- **R4:** Update the active requirement, gate, user/QA documentation and decision record together. Retain the original fn-17 requirement and fn-54 failure evidence as historical context with a superseding decision, and issue fresh QA evidence for the new contract. Errors: inconsistent units, stale active 60 MB claims or a rewritten historical pass are verification failures. [inferred]
- **R5:** Add regression coverage for the numeric boundaries, unavailable metrics and unit conversion; rerun the affected live timing/footprint checks and required repository build/test/lint gate. Preserve first frame under 300 ms and theme application under 100 ms. Errors: unrelated outstanding visual approvals and installed-session gaps remain separately reported and cannot be cleared by this memory change. [inferred]
- **R6:** Preserve the full measurement context, earlier improvements, rejected experiments and rationale below so later work does not repeat disproven approaches or describe recalibration as reduced RAM use. No error surface beyond accuracy and traceability of the recorded evidence. [paraphrase]

## Boundaries

- Preserve the native Qt application, local processing and separate engine processes. [strategy:Local engines and performance]
- Toolkit replacement, renderer-policy changes, daemon/model memory optimization, new product features and visual-baseline approval are outside this memory-contract change. [inferred]
- A remaining 60 MiB anonymous-memory optimization goal is exploratory, not an adopted release requirement. Failed reference runs must be reported; do not automatically increase limits to make them pass. [inferred]

## Decision Context

- Deferred media initialization removed eager FFmpeg startup from Home. First frame improved from approximately 1,556 ms to 252-256 ms, and sampled RSS fell by about 90 MiB. The media player now initializes on first real playback; pending-seek and pause behavior have regression coverage. Preserve this improvement. [inferred]
- Lazy page loading and a Vulkan scene-graph experiment did not establish sufficient additional improvement and were reverted. A toolkit rewrite solely to reach 60 MiB RSS would be a separate architectural decision. [inferred]
- ADR 0020 already documented 240-310 MB RSS on the NVIDIA development machine and substantial shared driver/Qt mappings. Its historical approximately 31 MB anonymous figure predates the complete app and is not the current allocation baseline. The latest evidence is approximately 79 MiB anonymous. [inferred]
- The original inferred proposal chose 256 MiB RSS as a reference-platform envelope and 96 MiB anonymous as an additional allocation-growth guard; it is retained here as historical rationale. Keep PSS visible because it estimates proportional sharing, while its observed variation makes a narrow hard cap premature. This changes the acceptance contract; it does not itself free memory. [inferred]

- Gordon approved 320 MiB RSS and 128 MiB anonymous on 2026-09-12 after a native cold sample reached 278,064 KiB RSS and 105,412 KiB anonymous. The approved limits leave about 18% and 24% headroom above that sample. Fresh runs must prove the new contract without relabeling old failed verdicts. [direct approval; headroom calculated]

## Strategy Alignment

This follows the native Qt approach and the Local engines and performance track's hardware-specific benchmarking. It preserves the Complete speech workflows, proven by drives track's demand for executable evidence. No strategy conflict was identified.

## Related evidence

The implementation handoff is fn-54, with the 2026-09-08 live desktop report, QA-MEMORY finding, deferred-playback startup snapshot, final app-route startup snapshot, native trace and reverted-experiment records. ADRs 0020 and 0056 explain the existing host and playback lifetime. The final tested implementation was 96f894ed; its build/test/lint gate passed. Exact artifact locations remain available in the linked fn-54 report and should be carried into task-level evidence references during breakdown.

## Requirement coverage

| Requirement | Task |
|---|---|
| R1 | fn-55-desktop-memory-budgets-grounded-in.1 |
| R2 | fn-55-desktop-memory-budgets-grounded-in.1 |
| R3 | fn-55-desktop-memory-budgets-grounded-in.1 |
| R4 | fn-55-desktop-memory-budgets-grounded-in.1 |
| R5 | fn-55-desktop-memory-budgets-grounded-in.1 |
| R6 | fn-55-desktop-memory-budgets-grounded-in.1 |
