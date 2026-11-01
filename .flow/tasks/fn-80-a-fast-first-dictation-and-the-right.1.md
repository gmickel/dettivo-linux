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
## Evidence
- Commits: 27a966695060a280a7bdfebbad5bb37d3360570a, e79aba1e6334b8b1b9ed7f59b598414af8170ad8, 7f907c99c1e3d94597c17d6697e0c4b0967bdce6, 2f8cfb34e2407eec929667eaf88c1b4998809c4b
- Tests: just lint, GATE_SKIPPED:cargo-test:docs-only - continuation diff 7f907c9..2f8cfb3 classified tier-B (no executable paths touched), GATE_SKIPPED:build-test-lint:docs-only - continuation diff classified tier-B; lint ran separately
- PRs: