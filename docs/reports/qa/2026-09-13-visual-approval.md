# Reviewed visual package accepted

Gordon approved the Heimdall package on 13 September 2026: "i have reviewed the package and am happy with all for now". The visual checkpoint is satisfied for all eighteen surfaces, including heading-sized state sentences and the three explanatory actionless states. Native AT-SPI and CUA keyboard verification now both pass the complete scenario. The visual and runtime acceptance evidence is complete.

## Package and baseline identity

The package is `~/Downloads/Dettivo-review-2026-09-12` on Heimdall. Its manifest SHA-256 is `46bc4a7bb6b53eec5a3cab01063a5c42b73d9bd5c52b481e620706e4ebd307aa`.

All 4,820 manifest files matched. Every one of the 710 retained, freshly rendered and approved images matched the reviewed package byte for byte. The existing approval CLI recorded Gordon Mickel and 2026-09-13 for all 710 entries. The [checklist receipt](../../design/checklist.md), [baseline ledger](../../design/baselines.md), design README and [ADR 0042](../../adr/0042-design-checklist-gate-and-the-human-checkpoint.md) carry the accepted decision. Historical artboards remain provenance; comparison thresholds were preserved.

## Keyboard fixes

An independent Qt key trace showed the question-mark shortcut activating before a queued Shift press reached the new hint sheet. The sheet now ignores modifier-only presses, so the opening chord cannot immediately dismiss it. Ordinary keys, including J and Escape, still close it. The focused regression failed before this change and passed afterward.

CUA treated the uppercase F in `Super+F` as a shifted glyph. A controlled probe showed uppercase F failing to toggle fullscreen and lowercase f succeeding. The adapter now treats single ASCII key names case-insensitively, matching the AT-SPI adapter. Its regression also failed before the correction and passed afterward.

These are keyboard behavior changes; the post-fix visual run still matches every accepted baseline.

## Verification

| Check | Result | Evidence |
|---|---|---|
| Canonical `just build test lint` | Exit 0; 1,010 Rust test passes and 79 Qt entries | `/tmp/fn38-approved-final-gate.log` |
| Post-fix strict visual matrix | 710 pass, zero differences, zero first approvals | `/tmp/fn38-approved-final-visual/visual-report.json` |
| Negative visual canary | All ten deliberate regressions rejected | `/tmp/fn38-approved-canary/canary-all/visual-report.json` |
| Hint-sheet regression | Five Qt test cases pass | `/tmp/fn38-modifier-green.log` |
| CUA chord regression | Pass | `/tmp/fn38-cua-case-green.log` |
| Complete CUA Xvfb keyboard drive | All fifteen routes, reverse Tab and conventions pass | `/tmp/fn38-approved-keyboard-cua.json` |
| Complete AT-SPI Xvfb keyboard drive | All fifteen routes, reverse Tab and conventions pass | `/tmp/fn38-approved-keyboard-atspi.json` |

## Earlier native attempts

The native XWayland app was observed as a mapped compositor client, but the compositor did not acknowledge it as the active window after focus and cursor requests. The shared accessibility-bus attempt did not complete; retrying with a private accessibility bus did not establish a pass either. Owned test processes were stopped and both attempts are retained as incomplete, not as application failures or successful checks.

The evidence roots are `/tmp/fn38-approved-native-atspi` and `/tmp/fn38-approved-native-private-atspi`, including `activation.json` and `timeout.json`. The earlier failures remain in the [12 September report](2026-09-12-beauty.md). These earlier attempts supplied no native acceptance; the unlocked results below supersede their session diagnosis.

## Native session diagnosis

The 13 September retry identified the missing precondition. Quickshell reported `locked=true`, `sessionLocked=true` and `secure=true`, with the lock active since 05:48:06 UTC. Hyprland independently reported `LOCK` in `solitaryBlockedBy` and the monitor powered off. `loginctl LockedHint=no` did not describe the compositor lock. The original observations remain under `/tmp/fn38-native-session-diagnosis`.

The keyboard scenario now reads Hyprland through bounded IPC before launching an app. It records `keyboard-desktop.json` and fails explicitly when the session is locked or its state cannot be read. The focused regression failed before the guard and passed after it. Native retries with private accessibility buses returned the expected locked-session failure in 102 ms on AT-SPI and 284 ms on CUA, with no app windows launched. Their receipts are under `/tmp/fn38-native-lock-private-atspi` and `/tmp/fn38-native-lock-private-cua`. These are negative precondition checks. Gordon subsequently unlocked the desktop, allowing the native runs below.

The first pre-edit canonical gate hit the existing intermittent CLI revision-probe test failure. Its unchanged focused retry passed. `/tmp/fn38-native-baseline.log` and `/tmp/fn38-native-baseline-retry.log` retain both observations. The redundant second full baseline was stopped when implementation began; it is inconclusive and supplies no green receipt.

## Combined candidate verification

The native-session guard passes the canonical gate with 1,011 Rust tests and 79 Qt tests at `14add4fc`; `/tmp/fn38-native-final-gate.log` records exit 0. The integration checkout combines the beauty changes with the completed desktop memory-budget work. Its full gate before the guard passed 1,010 Rust tests and 79 Qt tests (`/tmp/dettivo-final-candidate-gate.log`). After integrating the guard, all 184 QA library tests, the QA build, formatting, file-length and documentation checks pass (`/tmp/dettivo-final-guard-tests.log` and `/tmp/dettivo-final-guard-checks.log`).

Both complete Xvfb keyboard drives also pass after integration: CUA in 168,502 ms and AT-SPI in 131,369 ms, including all fifteen routes, reverse Tab and conventions. Their reports and captured screenshots are under `/tmp/dettivo-final-guard-keyboard-cua` and `/tmp/dettivo-final-guard-keyboard-atspi`, with the top-level JSON reports at the same paths plus `.json`. The native runs below exercise the same keyboard implementation.

The final full combined gate at `5ff758be` passed 1,011 Rust tests and 79 Qt tests, build, lint and docs (`/tmp/dettivo-final-complete-gate.log`). Fresh renders from that candidate pass all 710 strict comparisons and reject all ten deliberate canary regressions. All eighteen contact sheets rendered, with thirteen machine checks passing and none failing. The captures are under `qa-evidence/final-review-20260913T125300Z` in the main checkout; the gallery navigation and approved-reference comparison were checked in a browser.

## Unlocked native verification

Gordon unlocked Thor normally on 13 September. Quickshell recorded `locked=false`, `sessionLocked=false` and `secure=false` at 12:48:39 UTC, and Hyprland's monitor evidence no longer contained `LOCK`. Native AT-SPI then passed all fifteen routes in 179,347 ms. Every route reached its required controls and returned to the exact expected target on Shift+Tab. The hint sheet, search, Escape and fullscreen toggle all passed. All sixteen launched XWayland windows received acknowledged compositor focus.

The receipt is `/tmp/fn38-unlocked-native-atspi/evidence/run-1789303845-3766167/keyboard_only.atspi/receipt.json`. An independent audit matched the fifteen routes to the source, checked every reverse transition and convention, and decoded all seventeen nonblank screenshots. `/tmp/fn38-native-atspi-verified.json` records those checks and screenshot hashes. Home, Hotkeys and the open hint sheet were also inspected visually.

CUA's first unlocked attempt used its ordinary background delivery mode. Its initial owned-window activation was not acknowledged, while RTSPrepLab was active, and Home had no focused samples. It failed in 28,867 ms. `/tmp/fn38-unlocked-native-cua` retains the activation mismatch, screenshots, tree and failed receipt.

A sequential retry scoped `CUA_DRIVER_DELIVERY_MODE=foreground` to the QA process. It passed eleven routes, with all twelve initial window activations acknowledged. Settings / Meetings then recorded 47 focused samples followed by 151 empty samples; `Copy message` remained unreached and reverse Tab had no observed target. The run failed in 308,361 ms. `/tmp/fn38-unlocked-native-cua-foreground` retains that failure. RTSPrepLab was active when the failure was inspected. CUA's foreground delivery activates the target for each action and restores the prior window afterward; it cannot guarantee uninterrupted focus against concurrent desktop input. The failed observations are retained rather than accepted as keyboard coverage.

The lock was resolved, and the remaining native CUA run was scheduled with uninterrupted desktop focus and a read-only active-PID transition trace in `/tmp/fn38-native-observed-bus.sh`. No game process, desktop configuration, application code or approved image was changed during these retries.

## Native CUA completed

After Gordon confirmed the desktop was unlocked and competing input paused, the coordinated native CUA run passed in 235,238 ms. All fifteen routes reached their required controls, all fifteen reverse-Tab transitions returned to the exact expected target, and the hint sheet, search, Escape and fullscreen convention checks passed. All sixteen test windows received acknowledged native focus. The independent audit decoded seventeen nonblank screenshots and recorded their hashes; the open hint sheet was also inspected visually.

The complete evidence is `/tmp/fn38-unlocked-native-cua-coordinated-2`, including `activation.json`, `focus-transitions.json`, `exit-code.txt` and the scenario's `receipt.json` and `keyboard.json`. `/tmp/fn38-native-cua-verified.json` records the audit. The earlier coordinated attempt in `/tmp/fn38-unlocked-native-cua-coordinated` stopped after 234 ms because the desktop had locked again; it remains a failed precondition check, with no app window launched.

R4 is complete: both native drivers and both isolated Xvfb drivers pass the complete keyboard scenario. The final combined gate, fresh 710-entry visual matrix, negative canary and Gordon's recorded visual approval supply the remaining acceptance evidence. No implementation requirement remains open for the beauty pass.


## Release rehearsal and sustained capture, 14 September

The release candidate recorded three consecutive meetings of 60, 900 and 900 seconds through two isolated PipeWire null sinks. Both audio tracks were retained for their full durations, live transcription advanced, and each meeting reached `completed`. The daemon's sampled RSS started at 19.5 MiB, ended at 29.8 MiB and peaked at 55.4 MiB over 31 minutes. The [bounded soak receipt](2026-09-14-meeting-soak.json) records durations, status and the tested daemon's SHA-256. The copied package predates the packaging layout repair; its meeting implementation is unchanged by that repair.

The final snapshots showed two automatic summaries `ready` and the third still `running`. Automatic diarization reported the missing `diarize/diarization-en` model. After downloading and verifying that configured model, an explicit retry over all three retained recordings reached diarization `ready`, with two source-associated speaker labels, and generated nonempty summaries. The temporary probe initially expected `completed` for diarization; the API returns `ready`. Correcting that probe allowed it to recognize the already finished operation. The original reports remain in the receipt's raw evidence directory.

This test used alternating clips from the public JFK fixture. It establishes capture and processing behavior for 31 minutes; it does not establish eight-hour reliability, real-headset behavior, multi-person speaker accuracy or summary accuracy. The initial full release rehearsal passed the contract, dictation, GUI, 710-image visual matrix, negative canary and all 19 install checks. It also exposed a meetings QA race after closing export. The scenario now waits for the observed sheet dismissal before clicking speaker rename, and fresh AT-SPI and CUA seeded drives pass.

The publish gate still requires fresh benchmark ancestry and a release receipt from the merged code. The earlier report's failed benchmark rows and failed meetings step remain evidence of that rehearsal. Copilot returned its quota error; this is not a reviewed-head receipt.

The follow-up meetings pack passed all 14 required steps in 172,858 ms, including capture recovery, live transcription, export goldens, token coverage and all six GUI drives. DER was 0.0204 with two speakers. Whisper processed the fixture at 25.26x realtime on Vulkan against the 20x target; diarization ran at 15.97x on CPU against 4x. GPU workload attribution remained an allowed external skip. Allocation in 23 of 23 samples proves process presence only, so no inference-execution proof is claimed.

That run used the exact shared-library companions from the tested installation package, staged beside the engine in the generated development output after its first attempt failed to load `libsherpa-onnx-c-api.so`. The installed package already contained them and had passed its installed-runtime checks. No source or user configuration changed for the retry. The failed run remains at `/tmp/dettivo-meetings-release-followup`; the passing report is `/tmp/dettivo-meetings-release-followup-runtime/run-1789341753-3059732/pack-meetings/meetings-pack.json`. These branch results do not replace the merged-code publish receipt.
