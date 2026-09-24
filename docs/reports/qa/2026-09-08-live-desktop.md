# Live desktop QA after cleanup

Dettivo's Home actions, state summaries, keyboard navigation and small-window controls now work through the running app. This pass fixed confirmed defects on `fix/live-qa-20260908`, starting from `ba02db93`. The final implementation head is `96f894ed`. The 84-route GUI pack and direct keyboard walkthrough ran at `4351e0eb`; the later sample-ordering fix was rechecked through first run and the full visual matrix. The final accessibility-name addition was rechecked through all app routes and first run.

The release QA verdict remains **NEEDS_WORK**. The idle-memory requirement, design approval and installed-session coverage remain open. This report records agent inspection; it does not supply Gordon's fn-38 R6 walkthrough or approve visual references in his name.

## What changed

| Finding | Observed failure and correction | Regression evidence |
|---|---|---|
| Home meeting action, P1 | Start meeting was disabled. It now queues the existing disclosure flow on the surviving window context. | `meeting-home-before.log`, `meetings_live_gui`, Home QML tests |
| Model state, P1 | Loading `tiny.en` also marked `tiny` warm. The row now requires exact model identity and readiness. | `warm-before.log`, `warm-after.log`, Models screenshots |
| State summaries, P1 | Home hid refused dictation starts and displayed stale model/configuration facts. REST always read off and MCP hosts never updated. Error reasons and refreshed model, health, capability and host sources now reach the UI. Last-call telemetry explicitly reads untracked. | `manual3/missing-verified-click.png`, `manual4/home-after-host-write.png`, `manual5/home-final.png`, `daemon-head.log`, host tests |
| Accessibility, P2 | Hidden optional buttons appeared unnamed in both drivers. Disabled configuration fields advertised editability. Visibility and read-only semantics now match the controls. | `hidden-before.log`, `controls-after.log`, GUI pack accessibility results |
| Dialog ownership, P2 | Overridden popup containers bypassed the shared style's inspectable background. Dialogs now supply children inside the shared container. | `dialogs-after.log`, unchanged-reference dialog comparison |
| Narrow windows, P1 | Settings actions clipped beyond the viewport and first-run content overlapped its footer. Scroll containers now expose both axes where needed and reveal the focused editor. | `manual3/models-small-revealed.png`, `manual5/models-small-focused.png`, `scroll-tests.log` |
| Keyboard navigation, P1 | Home advertised `j`/`k` and Enter without implementing them. Home now focuses today's rows and opens the selected take. Shared list rows expose keyboard and accessibility activation. | `home-keys-test.log`, `manual5/home-j-focused-final.png`, `manual5/keyboard-route-final.json` |
| Startup, P1 | Constructing the history player initialized FFmpeg before the first window. Playback objects are now allocated on first real play; pending seeks survive media loading. | `native-trace/home.log`, `media-tests-4.log`, `manual5/keyboard-route-final.json` |
| QA isolation, P1 | A disposable profile inherited the operator's `CODEX_HOME`. Both supported host override roots now point inside the disposable profile. | Original Agents tree, profile isolation tests, final Agents roundtrip |
| QA driving | Valid configuration paths triggered negative-text checks, timing samples violated chunk constraints, transient accessibility nodes aborted snapshots, and CUA text selection lacked an explicit target/input mode. Exact field-value exceptions, valid fixtures, whole-tree retries and isolated-session foreground delivery correct these failures. | QA unit tests, GUI pack, `cua-meetings-final.log`, `cua-import-final.log` |

The implementation decisions are in [ADR 0056](../../adr/0056-live-desktop-actions-and-evidence.md). No approved visual baseline or comparison threshold changed.

## Live coverage

All paths below are relative to `.flow/tmp/qa-live-20260908/`. Raw screenshots, accessibility trees, daemon responses and logs remain local and ignored by Git. The committed QA receipt is `.flow/review-receipts/qa-fn-54-live-desktop-qa-after-cleanup.json`.

| Requirement | Running-app evidence | Limits |
|---|---|---|
| R1. First run, Home, settings, History and Meetings | GUI pack across AT-SPI and CUA; eight settings sections with persisted writes and invalid edits; seeded History; meeting creation, live capture, rename, import and export; direct native-driver walkthrough in `manual` through `manual5` | Data and audio inputs are disposable fixtures. No personal host configuration was written. |
| R2. Session, recovery and theme behavior | `suite/exits.tsv` records all 13 additional AT-SPI scenarios passing. These cover daemon reconnect, live theme changes, dictation OSD, History roundtrip, meeting recovery/live capture, self-insertion prevention and insertion targets. `daemon-head.log` repeats reconnect and missing-model refusal on the final build. | Installed socket activation and shell-plugin loading could not run because this user session has neither installed integration. |
| R3. Visual matrix and resized windows | `visual-verified/visual-report.json` renders all five fixture palettes at 1x and 2x. Full-window captures, narrow settings and focused controls were inspected. `native-private/` renders on Wayland with the actual Last Call theme. | Missing and stale approved references remain failures or first-approval entries. Human beauty approval remains open. |
| R4. Regression and final gate | Focused Qt, QML and QA tests; final GUI pack; final visual matrix; repository build/test/lint gate | Final command results are recorded below. |

The private desktop is Xvfb with a private session/accessibility bus and window manager, normally a 1280 by 820 app window. The narrow-window captures use the app's minimum accepted width and 620-pixel height. Desktop screenshots use synthetic seeded records. Native Wayland rendering and OSD pacing supplement the isolated interactive pass; they do not establish installed shell integration.

## Verification results

| Check | Result |
|---|---|
| GUI pack, `gui-head/run-1788861064-1248831/pack-gui/gui-pack.json` | Pass. 84 routes, zero text findings, accessible naming 1.000 across inspected controls. Two Omarchy-session steps skipped. |
| Additional live scenarios | All 13 AT-SPI cases passed in `suite/exits.tsv`. Final reconnect/missing-model run passed in `daemon-head.log`. CUA meeting rename/import reruns passed. |
| Final first-run rerun | `first_run_steps` passed through AT-SPI in `steps-verified.log` after correcting shared-sample initialization order. |
| Final accessibility rerun | `app_routes` and `first_run_steps` passed in `final-reruns.log` after naming the new scrollable viewport. |
| Visual matrix, `visual-verified/visual-report.json` | Fail. 590 entries, 478 passed, 22 differed, 90 awaiting first approval. Zero style findings. |
| QML lint/format | Pass in `qml-lint-final.log`. Four new tests were formatted after the first full gate reported them; the later accessibility lint identified the viewport name, which was added and documented. |
| QA library | All 146 tests passed in `qa-lib-retry.log`. The earlier full-gate retry had one transient revision-fixture failure; its exact focused rerun also passed. |
| Flow validation and whitespace | `flowctl validate --spec fn-54-live-desktop-qa-after-cleanup --json` and `git diff --check` passed. |
| Full build/test/lint | Pass. `GATE_qa-receipt_EXIT=0 96f894ed`; log `/tmp/dtv-gate-qa-receipt.log`. The Rust suite and all 74 Qt tests passed. |

The full lint command passed independently in `lint-verified.log`, including documentation checks and their planted-failure tests. Packaging reports shellcheck as unavailable on this machine, so shellcheck was skipped; that warning remains in the gate log.

The 22 visual differences comprise two Tokyo Night design-sheet entries (pressed fill), 12 first-run Try it entries (keycap/header height and resulting vertical shift), and eight Agents entries (implemented REST configuration text replacing a future-feature placeholder). The 90 missing approvals comprise 32 Home entries, 18 first-run entries and 40 settings-pattern entries. Comparison thresholds and reference images remain unchanged.

The isolated real-audio check passed with correlation `0.99586` and detected source unloading. A quiet native OSD pacing run captured 2,392 frames in ten seconds with zero dropped frames and render p99 `0.032 ms`. Earlier pacing attempts under concurrent rendering load failed and remain in the evidence directory.

The measured first frame fell from about 1,556 ms to 255-256 ms after deferred playback initialization. The final isolated app reported RSS `236272 KB`, still above the app's 60 MB target in fn-17 R4. This checkout uses a Release Qt build and debug Rust binaries; these local measurements do not certify a packaged release. A lazy-page experiment and a Vulkan-renderer experiment did not establish further improvement and were reverted.

## Remaining work and boundaries

- Gordon's fn-38 R6 design walkthrough and the missing/stale visual references still require explicit approval. This pass leaves them visible.
- Installed `dettivod.socket` and the Dettivo Omarchy plugin were absent. This pass did not install them or restart user accessibility services. An installed-session check still needs the corresponding installation.
- The app remains above its documented RSS target. Further footprint work needs separate measurement and scope.
- These fixes are local commits on the QA branch. No push, PR, merge, release or personal configuration change is part of this receipt.
- Wave-3 cleanup and GPU diarization remain held and unstarted by this pass.
