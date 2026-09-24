---
title: AT-SPI History input loses a leading character before focus settles
date: "2026-09-10"
track: bug
category: integration
module: crates/dettivo-qa/src/driver/atspi.rs
tags: [qa, fn-52, atspi, history]
problem_type: integration
symptoms: History search requests zzqx but observes zqx on current and old apps
root_cause: XTEST flush did not acknowledge field focus; History treated failed value reads as successful delivery
resolution_type: fix
last_updated: "2026-09-10"
---

## Problem

P1 in the QA driver path, confidence 100, pre-existing. The AT-SPI History drive requests `zzqx`, but the live field and no-match sentence contain `zqx`. The current app and the pre-fn-55 app reproduce the loss with the same current release QA tool and daemon. This evidence does not establish a new shared-search-control defect.

## Steps to reproduce

1. Build the release daemon, CLI and QA tool, plus the Release Qt app, from the combined cleanup branch.
2. Run `XVFB_DISPLAY_NUMBER=130 scripts/qa/xvfb-session.sh env DETTIVO_BUILD_PROFILE=release target/release/dettivo-qa drive history_states --driver atspi --evidence /tmp/history-input-repro` on an unused display.
3. The scenario opens empty History, then a fresh seeded app in the same private profile. It clicks Search history and requests `zzqx` through XTEST.
4. Read the field and no-match sentence. The retained failing run shows `zqx` in both.

## Expected

The fn-52 search finding requires: "Run both search flows through typing, Enter, clear and focus restoration."

The entered query must survive delivery, and the observed query and results must match it. A read failure cannot prove that input arrived.

## Actual

The initial release pack had 22 passes, three failures and four skips. Both History search cases failed. The unchanged focused history_states retry again showed `zqx` instead of `zzqx`.

A private A/B comparison reproduced history_states failures in all three current-app runs and two of three old-app runs. The old app is commit `2169393c`; proc mappings verified its own Qt/QML libraries. Both variants used the current release QA tool, daemon, identical fixture setup and unchanged assertions.

A separately labelled 150 ms focus-settle diagnostic passed on both apps. Pacing individual characters without the initial settle still failed on the current app. These diagnostics are not acceptance passes and do not change a threshold or the production driver. Focus-delivery readiness remains under investigation.

## Evidence

- Initial log: `/tmp/dettivo-combined-qa-20260910-Nyxanz/drives.log`
- Retry log: `/tmp/dettivo-combined-qa-20260910-Nyxanz/history-retry.log`
- Failure screenshot: `/tmp/dettivo-combined-qa-20260910-Nyxanz/retries/run-1789060094-1343872/history_states.atspi/screenshot-no-hits.png`
- Failure tree: `/tmp/dettivo-combined-qa-20260910-Nyxanz/retries/run-1789060094-1343872/history_states.atspi/tree-no-hits.json`
- A/B cases: `/tmp/fn52-history-attribution-baseline.tsv`, `/tmp/fn52-history-attribution-old-1.log`, `/tmp/fn52-history-attribution-old-2.log`, `/tmp/fn52-history-attribution-old-3.log` and matching new-app logs.
- Diagnostic cases: `/tmp/fn52-history-attribution-diagnostic.tsv` and its separately named logs.

## Traceability

fn-52 R27/R30 verification dependency. Native Qt app on private Xvfb at 1280x820. Driver is the in-repo AT-SPI/XTEST implementation. No personal configuration, installed integration, renderer or visual approval changed.

## Resolution

The driver now resolves editable text through the owned application's bus connection. After clicking an editable, enabled, focusable field, it waits for the field's actual Focused state and the launched window's `_NET_ACTIVE_WINDOW`, bounded to two seconds. Other controls retain ordinary click behavior. The target's text is read directly, avoiding unrelated list delegates. History retries fresh observations within its existing two-second value bound and refuses to submit when reads fail or return only the field name.

The exact low-level ordering remains inferred. Before repair, the field/window acknowledgment diagnostic passed six runs without a fixed delay, while inter-character pacing alone still failed. The diagnostic first observed focus after the click; it did not capture the precise moment of lost input. The repair encodes the observed readiness condition and keeps the original typing pace.

## Regression evidence

- The focused `only_an_observed_query_allows_submission` test failed before repair because an UnknownObject read submitted `api`. It now rejects persistent read errors and label-only observations, and accepts a fresh complete query after a transient error. `/tmp/fn52-input-regression-red.log` and `/tmp/fn52-input-regression-green.log`.
- The field-state test covers editable, enabled and focusable eligibility; focused Clippy passes. `/tmp/fn52-input-focus-test.log` and `/tmp/fn52-input-clippy.log`.
- The rebuilt release QA runner passed three unchanged `history_states` and three unchanged `history_seeded` runs on private Xvfb 131. `/tmp/fn52-input-live-series-results.txt`, `/tmp/fn52-input-live-1/`, `/tmp/fn52-input-live-2/` and `/tmp/fn52-input-live-3/`.
- The full attribution report and immutable manifest remain at `/tmp/fn52-history-attribution-report.md` and `/tmp/fn52-history-attribution-manifest.json`.

## Complete-tree reacquisition during import

The clean release pack then reproduced a separate verification failure after the first large import completed. During the second import's Cancel/Recover flow, Qt replaced an accessibility delegate while the driver walked the tree. The driver translated UnknownObject into NotFound, and a direct snapshot wait aborted. The unchanged focused import reproduced the same failure. Those observations remain in `/tmp/fn52-input-final-live.log`, `/tmp/fn52-input-final-live/`, `/tmp/fn52-input-import-retry.log` and `/tmp/fn52-input-import-retry/`.

The snapshot helper now permits one immediate fresh complete walk only for the already-recognized UnknownObject error. It returns no partial nodes, adds no sleep, changes no caller timeout, and reports the second transient failure or any permanent error. `reacquisition_is_fresh_bounded_and_only_for_vanished_objects` failed before repair and now covers fresh success, exhausted transient retries, access denial and an unrelated NotFound. `/tmp/fn52-input-tree-red.log`, `/tmp/fn52-input-tree-green.log` and `/tmp/fn52-input-tree-clippy.log` retain the focused checks.

Three consecutive unchanged release `meetings_import_gui` runs passed after the repair, including the same-ID Cancel/Recover path with retained audio and notes. `/tmp/fn52-input-import-fixed-results.txt` and `/tmp/fn52-input-import-fixed-{1,2,3}/` retain those results. This is a bounded snapshot repair for fn52 R20/R26/R30 verification; it does not weaken the import assertions or approve its visual composition.
