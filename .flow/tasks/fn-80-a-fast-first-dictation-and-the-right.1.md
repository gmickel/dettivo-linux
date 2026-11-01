---
satisfies: [R1, R2, R3, R4, R5, R6]
---
# fn-80-a-fast-first-dictation-and-the-right.1 Implement A fast first dictation and the right model for each language

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
The daemon warms every preloaded speech engine with 30 s of silence. It is skipped within 60 s of a take, and a waiting take goes first. Engine state shows cold, loaded or warm in `speech.engines`, `engine.state`, `dettivo speech status` and `dettivo doctor`. GPU speech engines default to four CPU threads. The catalogue carries a per-language FLEURS ranking, which `speech.providers.list {language}`, `dettivo speech providers --language` and the first-run preselect read. The work adds three bench steps, ADRs 0084 and 0085, and the 2026-11-01-thor-gpu benchmark report.

R2 holds once the driver's shader cache is built: the first take after the daemon starts measured 330 ms against 296 ms warm for Whisper (1.11) and 149 ms against 185 ms for Parakeet (0.81). On 2026-11-01 Gordon accepted the empty-cache case (the first run after an install or a GPU driver update) as an exception to R2. In that case Whisper's first take measured 783 ms against 306 ms warm (2.56). The cause is that Whisper's text-decoder shaders compile only on a take that produces words, and R1 keeps the warm-up silent. ADR 0084 records the exception (commit 2f8cfb3).

Gates: `just lint` is green (rc 0) on the final docs commit. The spec's test gates did not run in this continuation because its diff is docs-only (tier B). The test evidence for the earlier code commits comes from their own sessions and from the reviewer's focused runs.

stage: impl-review - ran (codex gpt-6.1-sol high, 3 rounds) - SHIP at 2f8cfb3; findings 2-4 fixed, R2 finding withdrawn on the recorded human decision
Tier: implementer (project routing block: claude-opus-5-5) (model: claude-opus-5-5)
baseline: build green pre-edit (cargo build -p dettivod -p dettivo-cli -p dettivo-qa); tests not run pre-edit

stage: plan-sync - skipped(config: planSync.enabled != true)

QA follow-up (QA-fn80-1, P1): with `[dictation] language = "en"` the fifth speech row pushed the Models step's Enhanced rows over its footer at 1280x820. ModelsStep now reports the height its rows need above the footer, and OnboardingRoute grows its scroll content to that height, so English scrolls and German keeps its unscrolled layout. `tst_models_step_overflow.qml` failed before the fix (footer at 733 over rows ending at 734) and passes after it. Renders under `scripts/qa/xvfb-session.sh` with an isolated profile: `.flow/tmp/qa-fn-80-a-fast-first-dictation-and-the-right/fix-en-models.png`, `fix-en-models-scrolled.png`, `fix-de-models.png`, `fix-de-models-scrolled.png` (commit 58bca61).

stage: impl-review - ran (codex gpt-6.1-sol high, 1 reviewer) - SHIP at 58bca61 on base 8265636, no findings

QA follow-up (QA-fn80-2, P2): on the five-row English Models step, Tab to Back scrolled only until the button met the window edge, so the footer note's second line was half cut off. FocusFlickable.reveal now scrolls to the outermost ancestor that declares `revealMargin`, with that margin around it, and FirstRunFooter declares `revealMargin: Theme.space5`, so focusing a footer button brings the note and buttons into view together. `tst_models_step_overflow.qml` gained `test_focusing_a_footer_button_reveals_the_whole_footer`, which failed before the fix (footer ends at 822 in the 820 px window) and passes after it. Renders under `scripts/qa/xvfb-session.sh` with an isolated profile, 8 Tabs: `.flow/tmp/qa-fn-80-a-fast-first-dictation-and-the-right/fix2/en-tab8.png` (Back focused, both note lines visible with margin) and `fix2/de-tab8.png` (four rows, no scroll, unchanged) (commits ee054ed, ed945be).

stage: impl-review - ran (codex gpt-6.1-sol high, 1 reviewer) - SHIP at ed945be on base 643a696, no findings
Tier: implementer (dispatch IMPLEMENTER: claude-opus-5-5) (model: claude-opus-5-5)
baseline: green (ctest -R "dettivo-quick-test|dettivo-first-run-test|onboarding|dettivo-app-render", 5/5 pre-edit at 643a696)
## Evidence
- Commits: 27a966695060a280a7bdfebbad5bb37d3360570a, e79aba1e6334b8b1b9ed7f59b598414af8170ad8, 7f907c99c1e3d94597c17d6697e0c4b0967bdce6, 2f8cfb34e2407eec929667eaf88c1b4998809c4b, 58bca614579d63ede64357c600cd4decf0f76aeb, ee054edbee8a85b21d1650d28c64d1b5e736b457, ed945be044767aeb8e98a79a2981d768df10f888
- Tests: just lint, GATE_SKIPPED:cargo-test:docs-only - continuation diff 7f907c9..2f8cfb3 classified tier-B (no executable paths touched), GATE_SKIPPED:build-test-lint:docs-only - continuation diff classified tier-B; lint ran separately, QT_QPA_PLATFORM=offscreen ctest --test-dir build/qt -R "dettivo-quick-test|dettivo-first-run-test|onboarding|dettivo-app-render" (QA fix 58bca61, 5/5 passed), just lint (QA fix 58bca61, rc 0), QT_QPA_PLATFORM=offscreen ctest --test-dir build/qt -R "dettivo-quick-test|dettivo-first-run-test|onboarding|dettivo-app-render" (QA-fn80-2 fix ed945be, 5/5 passed; new test red at ee054ed), just lint (QA-fn80-2 fix ed945be, rc 0)
- PRs: