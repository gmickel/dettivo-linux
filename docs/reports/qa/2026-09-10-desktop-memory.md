# Desktop memory reference measurements, 2026-09-10

The approved 320 MiB RSS / 128 MiB anonymous contract passes all ten fresh native Wayland and Xvfb reference launches. Startup, live theme timing and the repository build/test/lint gate pass. This recalibrates the allowance without claiming reduced RAM use. Earlier failures remain below with their original limits and verdicts; separate visual and installed-session approvals are unchanged.

## Contract and setup

[ADR 0059](../../adr/0059-desktop-memory-budgets-follow-measured-reference-profiles.md) adopts inclusive RSS ≤262,144 KiB and anonymous memory ≤98,304 KiB. PSS remains uncapped. Linux procfs labels KiB as `kB`; one MiB is 1,024 KiB. The measurements cover only `dettivo-app`, excluding the separate daemon and engine processes. Changing acceptance frees no RAM.

Thor ran kernel `7.2.3-arch1-3`, Qt `6.11.2`, Hyprland `0.56.2`, NVIDIA driver `610.57.04` and an RTX 4090, with Release Qt (`-O3 -DNDEBUG`) and release-profile Rust companions. Every measured window used OpenGL and the built-in dark theme. The probes ran sequentially after builds had stopped. Each app was a new process under the fixed seeded private profile with Home visible, a connected daemon and five continuously observed idle seconds. No capture, playback, downloads or analysis were requested; the status evidence records those inactive states. The profile disables hotkeys and uses mock capture/insertion. Its first launch starts with a fresh private cache; later launches reuse that profile's cache.

The initial native series was tiled at 1996×1112 logical pixels on DP-3 at 1.25× DPR. The controlled native series uses the established 1280×820 logical size on the same display and DPR. A one-shot Hyprland `exec_cmd` float/size effect applies only to the private QA process; no global scale, persistent rule or renderer policy changed. The launcher preserves the app PID through `exec`, records it separately from `hyprctl`, and guards termination with the process start-time identity. Xvfb used display `:105`, a 1600×1000 server screen, and a 1280×820 logical app window at 1× DPR. The app reports its actual Qt platform as `wayland` or `xcb`, respectively.

The [complete JSON](2026-09-10-desktop-memory.json) retains every sample, exact KiB and MiB, individual metric results, limits, process identities, binary SHA-256 values, build/tree state, display metadata and sampling clocks. Its source commit is the pre-implementation `2169393c` plus the explicitly recorded dirty task diff, so the binary hashes identify the tested executable precisely. Historical fn-17 and [fn-54 evidence](2026-09-08-live-desktop.md) keep their original failed verdicts.

## Controlled native reference

| Launch | App PID | RSS KiB / MiB | PSS KiB / MiB | Anonymous KiB / MiB | First frame ms | Result |
|---|---:|---:|---:|---:|---:|---|
| 1 | 3519527 | 276228 / 269.75 | 189035 / 184.60 | 104320 / 101.88 | 346 | Memory and startup fail |
| 2 | 3519620 | 214228 / 209.21 | 131554 / 128.47 | 50012 / 48.84 | 325 | Startup fail |
| 3 | 3519715 | 214408 / 209.38 | 131713 / 128.63 | 50084 / 48.91 | 322 | Startup fail |
| 4 | 3519825 | 214536 / 209.51 | 131727 / 128.64 | 50144 / 48.97 | 333 | Startup fail |
| 5 | 3519900 | 213932 / 208.92 | 131115 / 128.04 | 49428 / 48.27 | 325 | Startup fail |
| Maximum | | 276228 / 269.75 | 189035 / 184.60 | 104320 / 101.88 | 346 | Fail |

The application-object clock was 2 ms on each launch. The cumulative QML clock was 102, 105, 109, 107 and 105 ms; the remaining time to first frame was 244, 220, 213, 226 and 220 ms. These clocks preserve the journal's cumulative meanings. The remaining interval includes Qt/display work and is not a GPU-only render measurement. Keeping the reference size did not resolve the failures. This comparison establishes no allocation or timing root cause.

Raw evidence is `/tmp/fn55-memory-wayland-controlled-lua/run-1789038907-3519449/app_memory.wayland.atspi/`, with `startup.json`, `receipt.json` and each `launch-N/` containing proc status, `smaps_rollup`, proc stat, the app journal, stdout/stderr, app PID and the launch acknowledgement. The command exited 1.

## Private Xvfb reference

| Launch | App PID | RSS KiB / MiB | PSS KiB / MiB | Anonymous KiB / MiB | First frame ms | Result |
|---|---:|---:|---:|---:|---:|---|
| 1 | 3493694 | 230100 / 224.71 | 169049 / 165.09 | 83696 / 81.73 | 242 | Pass |
| 2 | 3494171 | 214648 / 209.62 | 153896 / 150.29 | 74620 / 72.87 | 148 | Pass |
| 3 | 3494741 | 215908 / 210.85 | 155064 / 151.43 | 75876 / 74.10 | 146 | Pass |
| 4 | 3494899 | 214944 / 209.91 | 154212 / 150.60 | 75080 / 73.32 | 146 | Pass |
| 5 | 3495064 | 215028 / 209.99 | 154202 / 150.59 | 75016 / 73.26 | 146 | Pass |
| Maximum | | 230100 / 224.71 | 169049 / 165.09 | 83696 / 81.73 | 242 | Pass |

Raw evidence is `/tmp/fn55-memory-xvfb/run-1789038264-3493631/app_memory.xvfb.atspi/`. The command exited 0. This result does not substitute for native Wayland evidence.

## Retained initial native series and failed setup

| Launch | App PID | RSS KiB | PSS KiB | Anonymous KiB | First frame ms | Result |
|---|---:|---:|---:|---:|---:|---|
| 1 | 3492751 | 277132 | 189939 | 105556 | 356 | Memory and startup fail |
| 2 | 3492830 | 215676 | 132976 | 51428 | 336 | Startup fail |
| 3 | 3492933 | 215616 | 132752 | 51116 | 329 | Startup fail |
| 4 | 3493032 | 215580 | 132721 | 51152 | 325 | Startup fail |
| 5 | 3493108 | 215696 | 132891 | 51352 | 326 | Startup fail |
| Maximum | | 277132 | 189939 | 105556 | 356 | Fail |

The maxima were 270.64 MiB RSS, 185.49 MiB PSS and 103.08 MiB anonymous. Raw evidence is `/tmp/fn55-memory-wayland/run-1789038198-3492688/app_memory.wayland.atspi/`. The command exited 1. These five failures remain recorded as the original tiled profile, separate from the later controlled reference.

The first controlled-launch attempt used the older Hyprland dispatcher syntax. Hyprland 0.56.2 rejected all five requests with `']' expected near ';'` in its Lua parser before any app launch; the exact messages are retained under `/tmp/fn55-memory-wayland-controlled/run-1789038823-3517301/app_memory.wayland.atspi/`. The JSON records these as failed setup, with no invented metrics. The corrected launcher uses the current [Hyprland dispatch API](https://wiki.hypr.land/Configuring/Basics/Dispatchers/#executing-with-rules).

The first implementation build also failed because renderer metadata was placed in the non-Quick host library. The correction keeps that metadata in the Quick application layer. `/tmp/fn55-release-build.log` retains the compiler error; `/tmp/fn55-release-build-final.log` records the successful release build. No renderer fallback was added.

## Pre-change app control comparison

The app built from pre-fn-55 commit `2169393c` independently reproduces the native misses. Five old-app launches and five current-app launches used the same release Rust companions, identical 100 ms status polling, fixed seeded profiles and native 1280×820 windows at 1.25× DPR. Each variant began with its own fresh private cache. The old app lacks the new status metadata, so the comparison verified its native window and scale through only that PID's compositor entry and retained its proc mappings. Both variants used the same observer.

| Launch | Old RSS / anonymous KiB | Old first frame ms | Current RSS / anonymous KiB | Current first frame ms |
|---|---:|---:|---:|---:|
| 1 | 275648 / 103956 | 367 | 276388 / 104492 | 361 |
| 2 | 214060 / 49876 | 325 | 214156 / 49796 | 323 |
| 3 | 214252 / 49876 | 324 | 214020 / 49696 | 320 |
| 4 | 213924 / 49708 | 320 | 213820 / 49704 | 329 |
| 5 | 214232 / 49944 | 322 | 213992 / 49824 | 321 |

Each first launch exceeded both memory limits. All ten first frames exceeded 300 ms. The old/current maxima were RSS 275648/276388 KiB, PSS 189139/189814 KiB and anonymous memory 103956/104492 KiB. The cumulative application-object clock was 2 ms throughout; QML completed in 104-114 ms on the old app and 105-108 ms on the current app. The remaining first-frame interval was 214-253 ms and 215-255 ms, respectively. This bounded comparison establishes that the failure class predates fn-55. It does not establish the root cause or prove zero overhead from the new metadata.

The [control JSON](2026-09-10-desktop-memory-control.json) carries all ten individual samples, PID/window evidence, exact MiB, PSS and source/binary identities. The raw directory is `/tmp/fn55-native-ab/`, including the temporary probe source and launcher source. The collector completed with exit 0, which means collection succeeded; its recorded budget failures remain failures. The pre-change app was built in the detached worktree `/tmp/fn55-prechange-BoWU5G/repo/`, with `/tmp/fn55-prechange-build.log` retaining the Release build. The temporary probe was removed from the repository after its source and evidence were preserved.

## Verification and remaining work

The focused baseline passed 164 QA unit/integration tests before edits (`/tmp/fn55-baseline.log`). The new regression first failed on the old 61,440 KiB limit and missing anonymous memory being converted to zero (`/tmp/fn55-regression-red.log`). The corrected tests cover both inclusive memory boundaries, one-KiB excess, MiB conversion, partial/malformed/unreadable metrics, early process exit, release/platform/process identity, idle readiness and literal shell arguments. Existing binary-selection tests cover missing release binaries and rejection of newer debug builds as substitutes.

The release `app_routes` drive passed all routes with first frame 247 ms, RSS 229976 KiB and anonymous memory 83592 KiB. Its evidence is `/tmp/fn55-app-routes/run-1789038324-3496804/app_routes.atspi/`. The actual theme swap passed in 6 ms, observed in 10 ms, under `/tmp/fn55-app-theme/run-1789038397-3502579/app_theme_live.atspi/`. The synchronized-generation timing collector and deferred media initialization remain intact.

The canonical `just build test lint` gate passed before the interruption (`/tmp/fn55-full-gate.log`) and passed again during recovery with exit 0, 963 Cargo test passes and 75/75 CTest entries (`/tmp/fn55-recovery-gate.log`, `/tmp/fn55-recovery-gate.exit`). The recovery run verified the implementation on `2169393c` plus the preserved task diff before its partial commit. It did not repeat or relabel the earlier memory captures as measurements of that later commit. R1, R3, R4 and R6 are implemented; R2 and R5 remain unmet because the controlled native reference fails. Startup cleanup in fn-52 is separately authorized and may affect later measurements, but this report promises no outcome from it. Repeat the fixed reference on that final tree without changing limits.

The existing visual matrix remains 393 passes, 107 differences and 90 first approvals across 590 entries. No golden, visual tolerance, fn-38 human walkthrough, installed Omarchy plugin or personal-session integration was approved or changed by this task. Those prior gaps stay separate from the new native memory/startup failures.

## Reproduce on the final tree

Build first, then keep the machine quiet while each command completes. Choose an unused Xvfb display; `105` was free for this report.

```sh
just build-release build-qt
DETTIVO_BUILD_PROFILE=release target/release/dettivo-qa drive app_memory.wayland --evidence /tmp/fn55-final-wayland
XVFB_DISPLAY_NUMBER=105 scripts/qa/xvfb-session.sh env DETTIVO_BUILD_PROFILE=release target/release/dettivo-qa drive app_memory.xvfb --evidence /tmp/fn55-final-xvfb
XVFB_DISPLAY_NUMBER=105 scripts/qa/xvfb-session.sh env DETTIVO_BUILD_PROFILE=release DETTIVO_TIMING_GATE=1 target/release/dettivo-qa drive app_routes --evidence /tmp/fn55-final-routes
XVFB_DISPLAY_NUMBER=105 scripts/qa/xvfb-session.sh env DETTIVO_BUILD_PROFILE=release DETTIVO_TIMING_GATE=1 target/release/dettivo-qa drive app_theme_live --evidence /tmp/fn55-final-theme
just build test lint
```

Each memory command performs exactly five sequential fresh app launches. Keep each command's exit code and evidence even when it fails; every designated reference launch must pass before fn-55 can be marked done.

## Post-cleanup reference rerun

The fn-52 startup cleanup is complete. Native Wayland still fails R2 and R5, so fn-55 remains blocked. All five Xvfb launches pass. The [post-cleanup JSON](2026-09-10-desktop-memory-post-cleanup.json) retains all ten samples, clocks, actual display and idle state, binary hashes and the failed native setup attempt. These measurements do not replace or relabel the earlier failures above.

The tested code is `f324e285a57e02142d6f3d04d486438b15d48880`. The clean checkout was `258cb001a97ab4532863132f5c8195319f52d6ba`, whose only later change is a Flow task receipt. Release companions embed f324e285. Both series ran sequentially after builds and other runtime checks stopped. The reference remains 1280 by 820 logical pixels, OpenGL, builtin-dark, app-process-only memory and five continuously observed idle seconds. Native DP-3 remains at 1.25 DPR; private Xvfb display 132 is at DPR 1. Active downloads were observed as zero for every designated sample.

| Mode / launch | RSS KiB | PSS KiB | Anonymous KiB | First frame ms | Result |
|---|---:|---:|---:|---:|---|
| wayland / 1 | 275888 | 190696 | 104024 | 347 | Memory and startup fail |
| wayland / 2 | 214056 | 132166 | 49612 | 334 | Startup fail |
| wayland / 3 | 214348 | 132415 | 49816 | 322 | Startup fail |
| wayland / 4 | 214424 | 132557 | 49944 | 329 | Startup fail |
| wayland / 5 | 214448 | 132458 | 49896 | 322 | Startup fail |
| xvfb / 1 | 230028 | 169601 | 83508 | 238 | Pass |
| xvfb / 2 | 214220 | 154112 | 74160 | 145 | Pass |
| xvfb / 3 | 214304 | 154186 | 74256 | 146 | Pass |
| xvfb / 4 | 215008 | 154813 | 74760 | 146 | Pass |
| xvfb / 5 | 214884 | 154712 | 74608 | 147 | Pass |

Native launch 1 exceeds RSS by 13,744 KiB and anonymous memory by 5,720 KiB. Its RSS is 269.42 MiB and anonymous memory is 101.59 MiB. Native first frames are 347, 334, 322, 329 and 322 ms; all fail the strict under-300 ms requirement. Later launches pass both memory limits. Xvfb maxima are RSS 230,028 KiB, PSS 169,601 KiB, anonymous memory 83,508 KiB and first frame 238 ms. PSS remains uncapped. This rerun establishes no allocation or timing root cause.

The first native command could not start because the shell lacked `WAYLAND_DISPLAY`. It is retained as a failed setup with no samples. `hyprctl instances -j` identified the existing `wayland-1` socket; setting that environment variable for the command enabled the designated native series. No compositor configuration, display scale or renderer policy changed.

The final release timing checks also passed on Xvfb. `app_routes` reported first frame 236 ms with `DETTIVO_TIMING_GATE=1`; `app_theme_live` applied the theme in 7 ms, observed in 10 ms, against the 100 ms budget. These supplemental checks do not replace the designated reference series.

The [combined QA report](2026-09-10-combined-cleanup.md) records the final build/test/lint gate, contract replay, GUI and notes checks, visual failures and remaining approval gaps. No further unchanged native series was sampled to select a passing result.

## 2026-09-12 startup clock diagnosis

The app now captures first-frame time on the render thread. fn-55 remains blocked on native cold memory. Commit `9c31b0c3` corrects the timestamp without moving the launch clock or changing the 300 ms limit. The final combined release tree still requires both five-launch reference modes, live theme timing and the repository gate.

The [diagnostic JSON](2026-09-12-desktop-memory-diagnostics.json) retains all twenty samples, original reports, binary hashes, environment metadata and direct-clock observations. The pre-edit reference used base `2c3cabef`. Each diagnostic used a separately rebuilt Release Qt app and unchanged release-profile Rust companions. Every series used Home, the connected daemon, five idle seconds, builtin-dark, OpenGL, 1280 by 820 logical pixels on DP-3 at DPR 1.25. These diagnostic builds are distinct from final committed release validation. All four commands exited 1.

| Series / launch | RSS KiB | PSS KiB | Anonymous KiB | Journal first frame ms | Recorded result |
|---|---:|---:|---:|---:|---|
| pre-edit reference / 1 | 269256 | 184396 | 97020 | 400 | Fail |
| pre-edit reference / 2 | 213872 | 132246 | 49472 | 346 | Fail |
| pre-edit reference / 3 | 214240 | 132585 | 49728 | 346 | Fail |
| pre-edit reference / 4 | 209360 | 127745 | 45020 | 331 | Fail |
| pre-edit reference / 5 | 208828 | 127295 | 44652 | 332 | Fail |
| direct versus queued clock diagnostic / 1 | 276636 | 191778 | 103816 | 348 | Fail |
| direct versus queued clock diagnostic / 2 | 214576 | 132984 | 50076 | 446 | Fail |
| direct versus queued clock diagnostic / 3 | 214576 | 133031 | 50216 | 453 | Fail |
| direct versus queued clock diagnostic / 4 | 213916 | 132435 | 49716 | 456 | Fail |
| direct versus queued clock diagnostic / 5 | 214064 | 132550 | 50080 | 453 | Fail |
| allocator trim diagnostic (rejected) / 1 | 274168 | 189600 | 101376 | 354 | Fail |
| allocator trim diagnostic (rejected) / 2 | 212864 | 131277 | 48428 | 345 | Fail |
| allocator trim diagnostic (rejected) / 3 | 212840 | 131356 | 48596 | 325 | Fail |
| allocator trim diagnostic (rejected) / 4 | 212832 | 131338 | 48520 | 320 | Fail |
| allocator trim diagnostic (rejected) / 5 | 213012 | 131454 | 48548 | 324 | Fail |
| shader compiler release diagnostic (rejected) / 1 | 276984 | 192704 | 104212 | 234 | Fail |
| shader compiler release diagnostic (rejected) / 2 | 216208 | 134733 | 52016 | 231 | Pass |
| shader compiler release diagnostic (rejected) / 3 | 216536 | 134853 | 52016 | 236 | Pass |
| shader compiler release diagnostic (rejected) / 4 | 215904 | 134350 | 51492 | 222 | Pass |
| shader compiler release diagnostic (rejected) / 5 | 214988 | 133379 | 50660 | 222 | Pass |

### Clock correction

Direct render-thread observations in the quiet allocator diagnostic were 250, 241, 222, 216 and 221 ms. The same launches reached the existing GUI callback at 354, 345, 325, 320 and 324 ms. The queued signal delivery added 103-104 ms. The first diagnostic also demonstrated delayed GUI delivery, but its warm QML clocks rose to 184-192 ms with no established cause; those absolute timings remain recorded.

`FirstFrameTimer` captures the first swap using the original elapsed clock and sends the captured value to the GUI thread. The GUI continues to own journal writes, status updates and theme access. The regression emits two swaps from a separate thread while GUI processing is blocked for 100 ms. It failed against automatic connection delivery and passed after direct timestamp capture, including the one-delivery assertion. The pacing suite passed eight checks. Logs remain at `/tmp/fn55-first-frame-red.log` and `/tmp/fn55-first-frame-green.log`.

The original first-frame records remain unchanged. The correction establishes more accurate measurement; it does not claim that rendering itself became faster.

### Memory attribution and rejected experiments

The observed native context was NVIDIA GeForce RTX 4090, OpenGL ES 3.2, NVIDIA 610.57.04. A watcher read each process's full `smaps` once, four seconds after its PID announcement, before the ordinary five-second gate sample. It never read mappings during startup. These files are named `proc-smaps-full.txt` under each diagnostic's `launch-N/` directory.

In the first diagnostic, anonymous mappings used 77,684 KiB cold and 23,948 KiB on launch 2. The ordinary heap remained near 11,624 KiB. Cold resident file mappings added 5,648 KiB for the NVIDIA compiler and 2,608 KiB for EGL core compared with launch 2. LLVM mappings occupied 19,180 KiB and Gallium mappings 3,300 KiB in both launches. This supports an association between cold shader compilation and the extra memory, while anonymous mappings alone cannot prove allocation ownership.

A temporary allocator diagnostic called `malloc_info` and `malloc_trim(0)` after startup. The allocator reported 5,006,426 free bytes across 67,747,840 bytes of arenas. The trimmed launch retained 75,492 KiB of anonymous mappings. RSS still measured 274,168 KiB and anonymous memory 101,376 KiB. This experiment did not satisfy either memory limit and was removed.

A final diagnostic called `glReleaseShaderCompiler` once on the render thread after rendering. The [Khronos specification](https://registry.khronos.org/OpenGL/specs/es/3.2/es_spec_3.2.pdf) defines this as a resource-release hint that permits later compilation. The driver returned no GL error, but cold RSS remained 276,984 KiB and anonymous memory 104,212 KiB. The experiment was removed. Another worktree's Rust integration tests ran during this last series, so its absolute timing is excluded from release timing proof.

No allocator tuning, compiler hint, cache prewarming, renderer override, DPR change or budget increase remains in production. The remaining native cold RSS failure requires a demonstrated fix before fn-55 can close. Visual approvals and installed-session acceptance remain separate.

## Final production reference, 2026-09-12

Every native first frame now passes the unchanged 300 ms limit. Native launch 1 still fails both memory limits, so R2 remains unmet and fn-55 stays blocked. All five Xvfb launches, route checks and live theme timing pass.

This series used the committed clock correction `9c31b0c3`, with documentation changes staged and no diagnostic code. Release Qt app SHA-256 is `a046cab4c551dacdb042aada3506d8a2ac5572eefe35c569ca1bc4724604ab47`. Rust companions use the release profile and their unchanged binaries from the pre-edit baseline; the companion JSON records each hash. Both build trees and the other GUI QA work paused for these four sequential drives. The native reference remained DP-3 at 1.25 DPR, OpenGL and 1280 by 820 logical pixels; Xvfb display 141 used DPR 1. Every designated memory sample observed zero active downloads and the required idle state.

| Mode / launch | RSS KiB | PSS KiB | Anonymous KiB | First frame ms | Result |
|---|---:|---:|---:|---:|---|
| wayland / 1 | 278064 | 193493 | 105412 | 237 | Memory fail |
| wayland / 2 | 216140 | 134480 | 51508 | 210 | Pass |
| wayland / 3 | 215976 | 134081 | 51180 | 226 | Pass |
| wayland / 4 | 215784 | 133995 | 51144 | 210 | Pass |
| wayland / 5 | 216684 | 135010 | 51948 | 218 | Pass |
| xvfb / 1 | 229736 | 169651 | 83132 | 232 | Pass |
| xvfb / 2 | 214652 | 154926 | 74500 | 143 | Pass |
| xvfb / 3 | 214880 | 155119 | 74620 | 144 | Pass |
| xvfb / 4 | 215412 | 155716 | 75148 | 144 | Pass |
| xvfb / 5 | 215104 | 155359 | 74964 | 147 | Pass |

The native maxima are RSS 278,064 KiB, PSS 193,493 KiB and anonymous memory 105,412 KiB. Cold RSS exceeds its cap by 15,920 KiB; anonymous memory exceeds its cap by 7,108 KiB. Native first frames are 237, 210, 226, 210 and 218 ms. The Xvfb maxima are RSS 229,736 KiB, PSS 169,651 KiB, anonymous memory 83,132 KiB and first frame 232 ms. PSS remains uncapped.

The supplemental release `app_routes` drive passed with first frame 249 ms, RSS 228,968 KiB and anonymous memory 82,548 KiB. `app_theme_live` applied the new theme in 7 ms, observed in 10 ms. These supplemental checks preserve the timing requirements and do not replace the failed designated native memory launch.

Raw evidence and exact command arrays are under `/tmp/fn55-clock-final/`; the [companion JSON](2026-09-12-desktop-memory-diagnostics.json) retains the final reports and command exit codes. Native exited 1; Xvfb, routes and theme exited 0. All preceding failed measurements remain intact.

The first canonical gate attempt passed 1,000 Cargo tests, all 78 CTest entries and the preceding lint steps. Its docs self-test failed because the new, unstaged evidence JSON was absent from that test's tracked-file copy. Staging the evidence file fixed the cause; `mise exec just@1.58.0 -- just docs` then passed. The original failure remains in `/tmp/fn55-clock-canonical-gate.log` and the focused repair in `/tmp/fn55-clock-docs-retry.log`.

The final exact `mise exec just@1.58.0 -- just build test lint` rerun passed with exit 0 after all evidence files were staged. It passed 1,000 Cargo tests, all 78 CTest entries and every lint/documentation step. The complete log is `/tmp/fn55-clock-canonical-final.log`. This green repository gate leaves the separately measured native cold memory failure unchanged.

## Approved contract validation, 2026-09-12

All ten fresh reference launches pass the approved Thor limits of RSS at most 320 MiB (327,680 KiB) and anonymous memory at most 128 MiB (131,072 KiB). PSS remains uncapped. Gordon approved these limits with “ok, yes raise it” after reviewing the earlier native cold failures. This is a contract recalibration and claims no reduction in RAM use. The original 60 MiB contract, intermediate 256 MiB / 96 MiB proposal and all earlier failed measurements remain historical evidence above.

The validated checkout was `79f5b87f8f4051e2e273e02097d3f1ce08f89e0a` with a clean working tree, Release Qt and release-profile Rust companions. The [approved-contract JSON](2026-09-12-approved-desktop-memory.json) records every binary hash, original report, command and exit code. The Qt app SHA-256 is `a046cab4c551dacdb042aada3506d8a2ac5572eefe35c569ca1bc4724604ab47`.

Each mode used a new private fixture profile; its first launch was cold and its next four reused that profile's cache. All samples observed Home connected to the daemon after five idle seconds, with capture, playback, downloads and analysis inactive. The native reference remained 1280 by 820 logical pixels, DP-3, DPR 1.25 and OpenGL; Xvfb used DPR 1. Both used builtin-dark. Builds, browser checks and large gallery transfers paused during the sequential reference drives. The app-process scope excludes the daemon and engine processes.

| Mode / launch | RSS KiB | PSS KiB | Anonymous KiB | First frame ms | Result |
|---|---:|---:|---:|---:|---|
| wayland / 1 | 276868 | 193555 | 104084 | 241 | Pass |
| wayland / 2 | 214880 | 133662 | 50044 | 220 | Pass |
| wayland / 3 | 214244 | 133329 | 49688 | 218 | Pass |
| wayland / 4 | 214816 | 133754 | 50064 | 219 | Pass |
| wayland / 5 | 214424 | 133409 | 49752 | 218 | Pass |
| xvfb / 1 | 229028 | 169289 | 82416 | 235 | Pass |
| xvfb / 2 | 215380 | 155807 | 74964 | 147 | Pass |
| xvfb / 3 | 217752 | 158392 | 77440 | 145 | Pass |
| xvfb / 4 | 214200 | 154666 | 73736 | 144 | Pass |
| xvfb / 5 | 214504 | 155212 | 74452 | 148 | Pass |

Native maxima are RSS 276,868 KiB, PSS 193,555 KiB, anonymous memory 104,084 KiB and first frame 241 ms. Xvfb maxima are RSS 229,028 KiB, PSS 169,289 KiB, anonymous memory 82,416 KiB and first frame 235 ms. Every first frame meets the unchanged under-300 ms requirement. The supplemental route drive passed at 240 ms; live theme application passed at 7 ms, observed in 10 ms, against the unchanged 100 ms limit.

### Verification and retained failures

The numeric regression verifies that both exact limits pass and one KiB above either fails, that PSS remains uncapped, and that measured values and budget labels convert correctly between KiB and MiB. Unavailable and malformed metrics remain failures. The changed budget expectations failed against the old constant before implementation; added label assertions then exposed the remaining hard-coded MiB labels before they were derived from the KiB constants. The focused library checks pass. An initial binary-target test selection collected zero tests and is recorded as inconclusive; it supplies no passing evidence.

The first approved-budget live series returned exit 0 but failed evidence inspection because its sample labels still said 256 MiB / 96 MiB while enforcement used the new KiB limits. The companion JSON retains those original reports as invalid metadata. The fresh accepted series above followed the label fix and passed explicit checks of every raw value, conversion, budget, idle state and reference environment.

A canonical-gate attempt caught the renamed test's stale evidence-map reference. The active map and its generated JSON/Markdown reports now resolve the renamed regression; the focused map check passed. A later attempt exposed an existing binary-revision probe test returning no revision for its temporary shell executable. That test passed unchanged in a focused retry. Both failed logs remain retained. The final exact `mise exec just@1.58.0 -- just build test lint` run passed with exit 0, 1,000 Cargo tests, all 78 CTest entries and every lint/documentation step. Its log is `/tmp/fn55-raised-canonical-verified.log`. The packaging recipe reported optional ShellCheck unavailable; its remaining checks ran, and this task changes no packaging source.

The approved contract and fresh evidence satisfy fn-55 R1-R6. This result grants no separate visual-baseline or installed-session approval. The earlier renderer and allocator experiments remain rejected, and the first-frame clock correction remains in place.
