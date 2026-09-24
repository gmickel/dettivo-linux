---
satisfies: [R1, R2, R3, R4, R5, R6, R7]
---
# fn-48-cleanup-security-boundaries-6-review.1 Implement security-boundary cleanup

## Description
TBD

## Acceptance
Every R-ID in the parent spec's Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
# fn-48 security-boundary implementation handover

All six security findings are fixed and required R7 verification is complete. Full AT-SPI coverage supplies the passing drive checks. The additional CUA settings failure is attributed by an old/new service-environment comparison and remains an explicit advisory limitation; it is not counted as a pass. All implementation code is committed.

Base commit: d2d246e1d1650240a984acc69bdb7c93d8f1edfb
Final implementation and full-gate SHA: 2d1e9f9445109c2db40ad56e3cef40054dbb47fd
Workspace: /home/gordon/work/dettivo-linux
Elapsed time at final handover preparation: approximately 81 minutes of the 90-minute cap.

### Finding dispositions

- R1 / daemon/F8: FIXED. Dedicated transfer directories are created or hardened to 0700; new upload/export files are reserved exclusively as 0600. Existing files and symlinks are never truncated; occupied IDs are skipped, and stale files remain behind private directories. Retained file descriptors protect writes. Tests cover umask 000, fresh/existing directories, symlink refusal, collision preservation, upload/export rebinding and expiry. Red exit 101: /tmp/fn48-daemon-transfer-red.log. Green exit 0: /tmp/fn48-daemon-transfer-green.log and /tmp/fn48-daemon-integration-green.log.
- R2 / daemon/F19: FIXED. Cancellation reasons and trusted endpoint values were removed from daemon logs. Successful marker-bearing cancellation and endpoint-trust requests preserve operation diagnostics without the markers. Red exit 101: /tmp/fn48-daemon-logs-red.log. Green exit 0: /tmp/fn48-daemon-integration-green.log.
- R3 / agent-surfaces/F2: FIXED. REST passes its resolved token to the socket client before reading settings or forwarding requests. Five sources (environment, token file, isolated Secret Service adapter, token flag, token-file flag) are exercised across both actual hosting modes with peer_token. The max_body_bytes=4 test proves settings were read through authenticated IPC. Red exit 101: /tmp/fn48-tokens-red.log. Green exit 0: /tmp/fn48-cli-green.log. Secret Service is tested through a deterministic private secret-tool adapter, not a live account.
- R4 / agent-surfaces/F5: FIXED. MCP decodes percent escapes from bytes and rejects incomplete escapes, invalid hex and invalid decoded UTF-8 with ordinary protocol errors. REST safely checks the bearer prefix. Subsequent requests survive; MCP covers both framings. Red exit 101: /tmp/fn48-mcp-red.log and /tmp/fn48-auth-red.log. Green exit 0: /tmp/fn48-mcp-green.log and /tmp/fn48-auth-green.log. The malformed-resource fixture and Linux delta row register the correction.
- R5 / agent-surfaces/F18: FIXED. REST status preserves actual HTTP status and response error body. The fixture-backed 200/401/500/503/refusal matrix proves exits 0/3/1/2/2, human and JSON output, and daemon/process host reports. Red exit 101: /tmp/fn48-cli-red.log. Green exit 0: /tmp/fn48-cli-green.log and /tmp/fn48-status-final.log. Controlled HTTP/IPC servers exercise the failure matrix; actual hosts are covered by the token tests.
- R6 / qa-rig/F1: FIXED. One command builder clears inherited environments and forwards a deliberate display/session/accessibility/audio/locale allowlist. Profiles own HOME, config, state, data, cache and temporary storage. All profile-backed daemon, CLI, GUI, rendering, pacing and CUA control-process launchers use it. Ten actual launcher boundaries run under poisoned parent overrides and leave external temporary sentinels unchanged; explicit scenario overrides remain available. Red exit 101: /tmp/fn48-qa-red.log and /tmp/fn48-qa-cua-red.log. Green exit 0: /tmp/fn48-qa-all.log. Intentional live Omarchy operations, host diagnostics and engine benchmarks with explicit model/input paths retain their distinct host semantics.
- R7: SATISFIED. ADR 0050 is indexed, every finding has evidence, the final full gate is green, contract replay is green, the full AT-SPI drive pack is green with four explicit skips, and the final AT-SPI settings rerun is green. R7 requires passing drive coverage, not every backend; the additional CUA settings failure is attributed below and remains failed.

### Required verification and exact heads

- Pre-edit baseline: cargo test -p dettivod -p dettivo-cli -p dettivo-mcp -p dettivo-rest -p dettivo-qa, exit 0; /tmp/fn48-baseline.log. Formatting baseline also exited 0; /tmp/fn48-baseline-fmt.log. No baseline receipt handoff was used.
- Original implementation gate: just build test lint, exit 0 at 70a6c046; /tmp/fn48-gate.log.
- Final required gate: just build test lint, exit 0 at 2d1e9f94; /tmp/fn48-terminal-gate.log. All 74 Qt tests passed. Receipt: .flow/tmp/green-receipts/2d1e9f94-unittest.json.
- Final contract replay at 2d20b5d1: exit 0; 85 passed, 0 failed, 44 documented skips and 6 pending methods admitted by false capability flags. MCP 48/48 and REST 55/55 passed in the same run. /tmp/fn48-contract-final.log. The later change only affects settings tab selection; final full-gate replay tests remain green.
- Full AT-SPI pack at 2d20b5d1: exit 0; 25 passed, 0 failed, 4 skipped. /tmp/fn48-drives-final.log; evidence /tmp/fn48-drive-evidence-final.
- After the tab-selector change at 2d1e9f94: settings_roundtrip on AT-SPI exited 0; /tmp/fn48-settings-atspi-final.log and /tmp/fn48-settings-atspi-final.
- Affected CUA pack at 2d20b5d1: placeholder_window, osd_dictation and meetings_import_gui passed; settings_roundtrip failed on tab discovery. /tmp/fn48-cua.log and /tmp/fn48-cua-evidence. After the selector fix at 2d1e9f94, settings passed tab discovery but failed numeric text commit; /tmp/fn48-settings-cua-final.log and /tmp/fn48-settings-cua-final.
- ShellCheck was unavailable. Packaging lint reported that omission; no ShellCheck pass is claimed.
- The four AT-SPI omissions are hotkeys_hyprland, omarchy_bar and omarchy_setup_idempotent without a Hyprland session, plus meeting_token_coverage without diarize/diarization-en. No model installation or production Omarchy opt-in was performed.

### Verification dependency repairs and preserved failures

The inherited contract matcher rejected four identical normalized replies because it sent raw error/capability scalars through a shape-only comparator. The repair compares only documented variable subtrees by shape and keeps envelopes, errors and fixed capability values exact, including every capability-array entry. Transcript fixtures gained already-registered meeting/search row variants while retaining their original examples. Unchanged baseline blob IDs, production/schema references and red/green proof are in /tmp/fn48-replay-handover.md. Focused replay tests passed 8/8 and typed fixtures passed 6/6.

The initial fixture addition selected the wrong seeded dictation ID. The final gate correctly failed its existing seeded-identity assertion in /tmp/fn48-final-gate.log. Commit 2d20b5d1 corrected the example to the actual API search hit, without changing normalization or expected identity checks. Daemon contract tests then passed 7/7 (/tmp/fn48-contract-fixture-repair.log), and QA replay passed 8/8 (/tmp/fn48-replay-final.log).

The first AT-SPI pack was red (/tmp/fn48-drives.log). An ordered OSD reproduction proved a valid target_is_self refusal after preceding scenarios; the old assertion guessed reason substrings. The drive now compares displayed outcome and reason with the exact stopped take's persisted insertion facts. Ordered red and green evidence: /tmp/fn48-osd-sequence.log and /tmp/fn48-osd-sequence-fixed.log. Import dialog cancellation/reopening exposed an asynchronous dismissal race; the drive now observes closed/open state before acting. Red and green: /tmp/fn48-import-retry.log and /tmp/fn48-import-fixed.log. The final full AT-SPI pack passed both.

CUA appends its non-actionable containers after action tokens, so the settings helper's positional lookup found no tabs despite five real tabs being present. The bounded selector repair uses container geometry when available and retains positional behavior for geometry-less snapshots. A focused regression preserves action tokens and excludes another tab group. Red and green: /tmp/fn48-cua-tabs-red-semantic.log and /tmp/fn48-cua-tabs-green.log.

### Remaining CUA boundary

/tmp/fn48-cua-typing-attribution.md records the private controlled probe. Starting from field 5000, CUA type_text("7\n") leaves literal field value "50007\n" while configuration remains default 5000. Explicit Return commits 50007. Explicit Ctrl+A before another type operation still fails to replace the text. CUA reports route=accessibility, mode=foreground, escalation.reason=delivery_failed and effect=unverifiable. Its schema offers no keyboard/replace switch.

A controlled old/new CUA service-launch comparison now supplies the missing environment evidence. With the same current binaries, app, client policy, field and actions, both the inherited-safe service environment and Profile::command environment append the literal text, leave configuration at its default until Return, and then commit 50007. Both use private HOME/XDG/data/cache locations. The routes and escalation results match. Exact log: /tmp/fn48-cua-service-ab.log; policies, safe key differences and binary hashes: /tmp/fn48-cua-typing-attribution.md. The successful probe exit means diagnostic execution, not a passed settings drive.

The comparison attributes this failure outside the fn48 service-isolation change. No unconditional select-all, input fallback, tool installation or source workaround was added. Complete AT-SPI drive coverage fulfills R7; the supplementary CUA settings limitation remains recorded for the correctness follow-up.

### Advisory observations

- Visual matrix: 590 entries, 393 passed, 107 failed and 90 awaiting first approval. The correct prior report is .flow/tmp/beauty-20260908/visual-final/visual-report.json, not the earlier morning report. All 590 outcomes match; all 107 failing PNGs are byte-identical. Full attribution and same-binary environment controls: /tmp/fn48-visual-attribution.md. Current matrix: /tmp/fn48-visual/visual-report.json, log /tmp/fn48-visual.log. No goldens, tolerances or approvals changed.
- Standalone cold-cache Xvfb pacing remains FAILED: the 2-second and serial 10-second probes each recorded one startup gap. /tmp/fn48-pacing.log and /tmp/fn48-pacing-serial.log. Controlled same-binary old-safe/new environments both show one cold-cache gap and zero drops with warmed private caches; /tmp/fn48-pacing-attribution.md. This is an inherited cold-start observation exposed by fresh-cache isolation, not a physical-display performance pass. Thresholds and startup accounting were unchanged.

stage: impl-review - skipped(policy: user requested no review)
stage: plan-sync - skipped(config: planSync.enabled != true)

No push, PR, merge, release, installation, review dispatch or other task lifecycle mutation occurred. The other three queued tasks were left untouched.
## Evidence
- Commits: 70a6c0463d4682a817b86a054f3801f6af4c8a93, 9faf497477ba66c85dfa68dc0028c24c5524d932, 10ad218a0620f2fb2e8d3af13518897c8aef77bc, 2d20b5d17585e5243a37b32626c5eca5b1014516, 2d1e9f9445109c2db40ad56e3cef40054dbb47fd
- Tests: baseline: green; cargo test -p dettivod -p dettivo-cli -p dettivo-mcp -p dettivo-rest -p dettivo-qa (exit 0; /tmp/fn48-baseline.log), baseline: just fmt (exit 0; /tmp/fn48-baseline-fmt.log), umask 000; cargo test -p dettivod --bin dettivod transfers:: (exit 0 after red exit 101; /tmp/fn48-daemon-transfer-green.log), cargo test -p dettivod --test logs --test history_export --test polish (exit 0; /tmp/fn48-daemon-integration-green.log), cargo test -p dettivo-cli --test rest_status --test rest_tokens (exit 0 after red; /tmp/fn48-cli-green.log), MCP malformed-resource and REST malformed-bearer regressions (exit 0 after red; /tmp/fn48-mcp-green.log, /tmp/fn48-auth-green.log), cargo test -p dettivo-qa (exit 0; /tmp/fn48-qa-all.log; launcher poison regressions red before fix), cargo test -p dettivo-qa --lib replay (8 passed, exit 0; /tmp/fn48-replay-final.log), cargo test -p dettivod --test contract (7 passed, exit 0 after correcting task-caused fixture mismatch; /tmp/fn48-contract-fixture-repair.log), cargo test -p dettivo-qa --lib segmented_tabs_keep (exit 0 after semantic red exit 101; /tmp/fn48-cua-tabs-green.log), just build test lint (exit 0 at 2d1e9f94; /tmp/fn48-terminal-gate.log; 74 Qt tests passed; ShellCheck unavailable and not run), XVFB_DISPLAY_NUMBER=120 scripts/qa/xvfb-session.sh target/debug/dettivo-qa contract (exit 0 at 2d20b5d1; 85 pass, 0 fail, 44 skip, 6 admitted pending; MCP 48 pass; REST 55 pass; /tmp/fn48-contract-final.log), XVFB_DISPLAY_NUMBER=119 scripts/qa/xvfb-session.sh target/debug/dettivo-qa drive all --driver atspi --evidence /tmp/fn48-drive-evidence-final (exit 0 at 2d20b5d1; 25 pass, 4 skip; /tmp/fn48-drives-final.log), XVFB_DISPLAY_NUMBER=119 scripts/qa/xvfb-session.sh target/debug/dettivo-qa drive settings_roundtrip --driver atspi --evidence /tmp/fn48-settings-atspi-final (exit 0 at 2d1e9f94; /tmp/fn48-settings-atspi-final.log), CUA affected drives: placeholder_window, osd_dictation, meetings_import_gui passed; settings_roundtrip failed (/tmp/fn48-cua.log), XVFB_DISPLAY_NUMBER=120 scripts/qa/xvfb-session.sh target/debug/dettivo-qa drive settings_roundtrip --driver cua --evidence /tmp/fn48-settings-cua-final (exit 1 at 2d1e9f94; remaining text-delivery limitation; /tmp/fn48-cua-typing-attribution.md), visual --out /tmp/fn48-visual (exit 1 advisory; 393 pass, 107 inherited differences, 90 first approvals; all 590 outcomes match prior final report; /tmp/fn48-visual-attribution.md), osd-pacing --seconds 10 --evidence /tmp/fn48-pacing-serial (exit 1 advisory cold-cache Xvfb startup gap; old-safe/new controls agree; /tmp/fn48-pacing-attribution.md), initial Xvfb :99 setup exit 1 before suite execution, inconclusive; retried on verified-unused displays without modifying :99, Controlled old-safe/new CUA service environment A/B (diagnostic exit 0, identical failed input semantics in both; no isolation regression; /tmp/fn48-cua-service-ab.log, /tmp/fn48-cua-typing-attribution.md). R7 drive coverage is provided by passing AT-SPI, not by relabeling this CUA outcome.
- PRs:
