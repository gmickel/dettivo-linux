# 0059. Desktop memory budgets follow measured reference profiles

Status: Accepted 2026-09-10; reference budgets revised with Gordon's approval 2026-09-12; supersedes the app memory requirement in fn-17 R4 and [ADR 0020](0020-app-shell-thin-host-single-instance.md)

## What this gives you

The Thor desktop app has a defined memory allowance and a repeatable release check. RSS must stay at or below 320 MiB and anonymous memory at or below 128 MiB; PSS remains visible without a hard cap. Recalibrating these limits changes acceptance and saves no RAM by itself.

## Situation

The original requirement called its RSS limit 60 MB, while the gate enforced 60 × 1024 KiB, or 60 MiB. RSS counts resident shared libraries and graphics-driver mappings. PSS apportions shared pages across processes. Anonymous memory includes heaps and stacks, including Qt and driver allocations, so it cannot identify Dettivo-owned objects alone.

Two Home snapshots from the 2026-09-08 QA pass establish the development reference:

| Snapshot | RSS | PSS | Anonymous | First frame |
|---|---:|---:|---:|---:|
| Deferred playback initialization | 235,556 KiB / 230.04 MiB | 140,273 KiB / 136.99 MiB | 80,532 KiB / 78.64 MiB | 255 ms |
| Final app-route rerun | 235,948 KiB / 230.42 MiB | 179,341 KiB / 175.14 MiB | 81,056 KiB / 79.16 MiB | 252 ms |

Both snapshots used Release Qt and debug Rust binaries, fixture data, a separate daemon and isolated Xvfb sessions, with Home sampled after five seconds. They are mixed-build historical measurements, not packaged native-Wayland release evidence. PSS changed while RSS stayed almost constant, which makes session context necessary and a narrow PSS cap premature.

The [fn-54 report](../reports/qa/2026-09-08-live-desktop.md) preserves its separate final isolated RSS reading of `236272 KB`, the original failed memory verdict, and the remaining visual and installed-session gaps. Its raw evidence is under `.flow/tmp/qa-live-20260908/`, including `final-reruns.log` and `native-private/`; its receipt is `.flow/review-receipts/qa-fn-54-live-desktop-qa-after-cleanup.json`. The final tested implementation was `96f894ed`, with the build/test/lint gate green. None of these historical results becomes a pass through this decision.

## Decision

The designated Thor reference profile enforces RSS ≤327,680 KiB (320 MiB) and anonymous memory ≤131,072 KiB (128 MiB) for the app process. An exact limit passes; one KiB above either limit fails. PSS is recorded without a cap. One MiB is 1,024 KiB. The daemon and separate engine processes are excluded and must be identified as such in evidence.

Release validation uses five independent fresh app launches per mode, native Wayland on Thor and private Xvfb as the reproducible secondary profile. Every launch uses Release Qt and release-profile Rust companions, fixed fixture data and Home connected to the daemon. Sampling follows five seconds idle with no capture, playback, download or analysis. Evidence records each value and the maxima, raw KiB and MiB, limits and per-metric verdicts, process identity, build configuration, renderer, display protocol, window dimensions, scale, theme and sampling timing. [docs/qa.md](../qa.md#desktop-memory-reference-runs) gives the commands.

The controlled reference uses the established 1280×820 logical app size. Native Wayland keeps the actual monitor scale, with only transient sizing of the QA window. The [2026-09-10 measurement](../reports/qa/2026-09-10-desktop-memory.md) preserves the initial tiled native series separately. Both native series missed the original 256 MiB / 96 MiB limits. Those failed verdicts remain unchanged; the approved revision requires fresh native validation.

Missing, unreadable, malformed or partial metrics fail the measurement. An early process exit fails; an unsupported environment or unavailable release binary is an explicit failure or blocker. No absent metric becomes zero or satisfies a gate. Every launch must meet both limits; failed runs retain their results and do not trigger an automatic budget increase. First frame remains under 300 ms and theme application under 100 ms, with the actual theme swap measured separately by `app_theme_live`.

## Earlier improvements and rejected experiments

Deferred media initialization removed eager FFmpeg startup from Home. First frame fell from approximately 1,556 ms to 252-256 ms and sampled RSS fell by about 90 MiB. The player now initializes on first real playback, with pending-seek and pause behavior covered by regression tests ([ADR 0056](0056-live-desktop-actions-and-evidence.md)). This improvement remains in place.

Lazy page loading and a Vulkan scene-graph experiment did not establish sufficient additional improvement and were reverted. Neither experiment forms part of this change. Replacing Qt solely to reach 60 MiB RSS would require a separate architectural decision. Renderer policy, toolkit replacement, daemon/model optimization and new features remain outside this decision.

ADR 0020 recorded 240-310 MB RSS on the NVIDIA development machine, including substantial shared driver and Qt mappings. Its approximately 31 MB anonymous figure predates the complete app. The 2026-09-08 development reference was approximately 79 MiB anonymous. The historical decision and measurements remain available with their original units and verdicts.

## Consequences

The original 256 MiB / 96 MiB proposal left approximately 11% RSS and 21% anonymous headroom above the 2026-09-08 development snapshot. Native cold launches subsequently exceeded that proposal. The approved 320 MiB / 128 MiB limits leave approximately 18% RSS and 24% anonymous headroom above the 2026-09-12 cold sample of 278,064 KiB RSS and 105,412 KiB anonymous. They are engineering allowances for the reference platform, not statistically established upper bounds. Anonymous memory adds an allocation-growth guard while RSS retains visibility into the resident set. A 60 MiB anonymous optimization goal remains exploratory and is not a release requirement.

Driver versions, other graphics clients, renderer, scale, theme and loaded routes can change measured usage. Comparisons need equivalent recorded profiles; other machines and configurations carry their own labels. Fresh QA evidence must distinguish a pass under the replacement contract from an optimization. Visual approval and installed-session coverage remain separate requirements that this memory decision cannot clear.

## First-frame timestamp correction, 2026-09-12

Startup evidence now records the first render-thread swap against the original launch clock. The GUI thread receives that captured value before writing the journal. A controlled native diagnostic measured direct swaps at 216-250 ms while queued GUI delivery reported 320-354 ms for the same frames. The regression blocks GUI processing and verifies that its delay cannot change the captured timestamp or produce duplicate delivery.

Earlier reports retain their original timings and verdicts. This corrects measurement accuracy without claiming faster rendering or changing the 300 ms requirement. The [diagnostic report](../reports/qa/2026-09-10-desktop-memory.md#2026-09-12-startup-clock-diagnosis) also retains rejected allocator-trim and shader-compiler-release experiments. Native cold memory remained an acceptance blocker under the original 256 MiB / 96 MiB limits.

## Approved reference recalibration, 2026-09-12

Gordon approved the proposed 320 MiB RSS / 128 MiB anonymous limits with “ok, yes raise it”. These limits replace the earlier 256 MiB / 96 MiB proposal for Thor. They retain PSS reporting without a cap, exact inclusive boundaries, five independent fresh launches in each designated mode, first frame under 300 ms and theme application under 100 ms.

This changes the acceptance contract and claims no RAM reduction. Earlier failed measurements retain their original limits and verdicts. Renderer policy, window geometry, scale, cold-cache launch behavior and the app-process measurement scope remain unchanged. Fresh validation is recorded in the [desktop memory report](../reports/qa/2026-09-10-desktop-memory.md).
