# Better dictation, shipped as one release

## Goal & Context

Eight specs make dictation catch every word, run fast from the first take, honour the vocabulary on every engine, survive flaky hotkeys and leave the desktop as it found it. They ship together as one release (0.5.0), so users get one coherent upgrade and one set of notes.

The children already exist and are adopted as they are:

| Order | Spec | What it delivers |
|---|---|---|
| 1 | fn-79-clean-dictation-takes-a-real-speech | A real speech gate, VAD and engine output guards |
| 2 | fn-80-a-fast-first-dictation-and-the-right | A warm first dictation and per-language model recommendations |
| 3 | fn-82-hold-to-talk-that-survives-lost | Portal app id, robust hold-to-talk, double-tap lock, `press`/`release` verbs |
| 4 | fn-81-catch-the-first-and-last-words-of-every | Pre-roll (opt-in ring, default off), release tail, minimum hold, meeting rules |
| 5 | fn-83-vocabulary-entries-with-spoken-aliases | Vocabulary aliases and the term-recall benchmark |
| 6 | fn-85-delivery-that-leaves-the-desktop-as-it | Full clipboard restore, modifier settle, continuation joins, `stop --print`, media ducking |
| 7 | fn-84-vocabulary-on-parakeet | Vocabulary biasing on Parakeet (needs fn-83) |
| 8 | fn-86-streaming-parakeet-dictation-with-live | Streaming Parakeet, only if the measurement gate says so (needs fn-81) |

The order puts the shared foundations first: fn-79 changes the take pipeline that fn-81, fn-85 and fn-86 build on, and fn-82's `press`/`release` verbs carry the `--pressed-at` timestamp fn-81 needs. Specs with disjoint files may run in parallel.

Packaging for more distributions (fn-87) is not part of this goal or this release.

## Approach

How strict each child's path is, in the repository's fast-build mode:

- Each child goes through work (`--no-plan`, already recorded on every child), make-pr into the integration branch, and land.
- No plan review, no per-child implementation review, no QA. Review backend `none`.
- Children land on the integration branch without waiting for review bots. CI must be green on each child's pull request.
- The final pull request from the integration branch into `main` is a normal pull request under the repository rules: CI green, and Cursor Bugbot or GitHub Copilot has reviewed its head with no open thread.
- No final implementation review on the whole diff (fast-build mode: `skipped(goal: fast-build, review backend none)`).

## Acceptance

- **R1:** Every child above is closed in Flow with its own acceptance criteria met and its pull request merged into the integration branch. A child whose spec ends in a measurement gate or an ADR-only outcome (fn-80's idle timeout, fn-84 stage 2, fn-86's gate) counts as met when that ADR is merged.
- **R2:** `just build test lint` passes in CI on the final pull request into `main`.
- **R3:** `CHANGELOG.md` has a single `[Unreleased]` section that describes all eight changes for users, ready to become the 0.5.0 notes. The docs each child names are updated.
- **R4:** The final pull request into `main` is merged by land under the invoking session's consent, after a review bot has reviewed its head and no thread is open.

## Boundaries

- No release, tag or version bump during the goal. Gordon cuts 0.5.0 once, after the final pull request merges, following `docs/RELEASING.md`.
- Nothing from fn-87 (packaging) and nothing from fn-65 (dictating during a meeting) is built here. fn-81 only fixes the rules fn-65 will build on.
- `[audio] keep_microphone_ready` ships defaulting to `false`.
- Never edit a child's acceptance criteria. A child that cannot meet one stops the goal with a question in the log.
- The repository rules in `CLAUDE.md` hold: files stay under their size limits, ADRs record every decision, config stays in `config.toml`, visual checks never become gates.

## Decision Context

- The default for the always-ready microphone is off (decided 2026-11-01): no indicator surprise on a fresh install, opt-in for people who lose first words.
- One release for all eight (decided 2026-11-01).

## Goal log

- 2026-11-01: goal captured from the eight dictation specs; children fn-79 to fn-86 adopted, marked ready, route `no_plan` recorded on each.
