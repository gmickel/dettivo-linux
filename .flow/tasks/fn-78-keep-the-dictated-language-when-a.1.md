---
satisfies: [R1, R2, R3, R4]
---
# fn-78-keep-the-dictated-language-when-a.1 Implement Keep the dictated language when a vocabulary is set

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
`dettivo_speech::vocabulary_prompt` builds the Whisper prompt: words joined by `", "`, closed with `.` unless the last word ends in `.`, `!` or `?`, `None` when empty. Live dictation, history re-run, retry, meetings and imports call it instead of joining the list themselves. `docs/engines.md` describes the prompt; `CHANGELOG.md` Unreleased credits @DanielKillenberger (#37).

Defect route:
- prior fixes: open PRs none; recent commits on the five call sites none touching the prompt; memory bug track empty; issue #37 links the reporter's fork branch `fn-78-keep-the-dictated-language-when-a` as reference only (CONTRIBUTING takes no code), not used.
- diagnosis: confirmed by the reporter's A/B through `dettivo-engine-whisper --wav` (bare list 3/103 takes changed language, terminated list 0/103; a forced `en` did not help). Local probe on 40 of Gordon's English recordings with large-v3-turbo did not reproduce: 0 flips under the bare or the terminated list, so the terminator changed no unaffected take.
- introduced by: skipped: no known-good revision (behaviour since the first public commit).
- base: `cargo test -p dettivo-speech --lib vocabulary_prompt` failed at 043ce30 (`Some("Dettivo")` vs `Some("Dettivo.")`) | head: 3/3 pass.
- live: not run: the flipping recordings are the reporter's; none on this machine reproduce.

stage: impl-review - skipped(config: review backend none during the fast build, CLAUDE.md)

stage: plan-sync - skipped(config: planSync.enabled != true)
## Evidence
- Commits: a62303cd9ea935465bf72edaf3795ec7a71c7628, 0890dd2bf8aeed89412c7edadb6b92fa03ea7168
- Tests: cargo test -p dettivo-speech -p dettivo-session, cargo test -p dettivod --no-fail-fast
- PRs: