---
satisfies: [R1, R2, R3, R4, R5, R6, R7]
---
# fn-38-beauty-pass-every-surface-against-the.1 Beauty pass: every surface against the checklist, copy, states, light theme and the review

## Description
Implement the beauty checklist, state and copy checks, approved visual baselines, and complete keyboard verification on both drivers under Xvfb and native Hyprland.

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
The beauty pass satisfies all seven requirements. Gordon approved the exact reviewed Heimdall package, all 710 current visual comparisons pass, and both keyboard drivers pass the complete scenario under Xvfb and on the native desktop.

R1/R3: eighteen contact sheets render, thirteen machine checks pass with no failures, and the copy lint plus its negative cases pass in the canonical gate. R2/R5: all states, scales and palettes match the approved baselines; the fresh 710-entry matrix passes and the canary rejects all ten deliberate regressions. R6/R7: Gordon's 13 September approval is recorded for all eighteen surfaces, including the accepted heading sizes and three actionless states, with the baseline ledger, checklist, user docs and ADR updated.

R4: native AT-SPI passed fifteen routes in 179,347 ms and native CUA passed fifteen routes in 235,238 ms. Both have zero unreached controls, fifteen exact reverse-Tab transitions, all four conventions, sixteen acknowledged native activations and seventeen decoded nonblank screenshots. Both isolated Xvfb drivers also pass. Normal unlock and paused competing desktop input supplied the missing runtime preconditions; no desktop or game configuration was changed. Earlier locked-session and lost-focus failures remain retained as negative evidence.

The final combined just build test lint gate passed at 5ff758be with 1,011 Rust tests and 79 Qt tests. The guard branch's full gate also passed with the same counts; this completion adds documentation and receipts only. The complete record is docs/reports/qa/2026-09-13-visual-approval.md. The combined final-review gallery has 710 selectable captures across 18 surfaces and was checked in a browser.

stage: impl-review - skipped(policy: user requested no implementation review during work; final human walkthrough follows completion)
stage: QA - ran; all seven requirements covered with captured runtime and visual evidence
## Evidence
- Commits: c2a57537376f0e70e6d6fed6f5f7388315c02b68, 3908165a18a4faa66026db1d2fad91ea937c6b71, 14add4fc845d5c1839e8da1bd75d5e1b9f858ae2, bd282092
- Tests: mise exec just@1.58.0 -- just build test lint:PASS: 1,011 Rust tests and 79 Qt tests; source guard /tmp/fn38-native-final-gate.log; combined candidate 5ff758be /tmp/dettivo-final-complete-gate.log, Fresh strict visual: 710/710 PASS; qa-evidence/final-review-20260913T125300Z/visual/visual-report.json, Fresh visual canary: all 10 deliberate regressions rejected; qa-evidence/final-review-20260913T125300Z/canary/canary-all/visual-report.json, Fresh beauty: 18 surfaces, 13 machine passes, zero failures; qa-evidence/final-review-20260913T125300Z/beauty/beauty-report.md, Complete Xvfb keyboard_only: CUA 168502 ms and AT-SPI 131369 ms PASS; /tmp/dettivo-final-guard-keyboard-{cua,atspi}.json, Native AT-SPI: PASS, 15 routes, 15 exact reverse-Tab transitions, 4 conventions; /tmp/fn38-native-atspi-verified.json, CUA_DRIVER_DELIVERY_MODE=foreground timeout 600s bash /tmp/fn38-native-observed-bus.sh cua /tmp/fn38-unlocked-native-cua-coordinated-2:PASS, 235238 ms, python3 /tmp/fn38-verify-native.py /tmp/fn38-unlocked-native-cua-coordinated-2 cua /tmp/fn38-native-cua-verified.json:PASS, 15 routes, 15 exact reverse-Tab transitions, 4 conventions, 16 native activations, 17 PNGs, scripts/check-docs.sh:PASS after completion report
- PRs: https://github.com/gmickel/dettivo-linux/pull/42