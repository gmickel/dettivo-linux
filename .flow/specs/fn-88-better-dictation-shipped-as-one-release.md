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

This goal runs at full rigor, the way Dettivo was built before fast-build mode, and overrides the repository's fast-build settings (review backend `none`, `pipeline.qa` off) for every child:

- **Per child:** work (`--no-plan`, already recorded on every child) → implementation review → QA → make-pr into the integration branch → land.
- **Implementation review on every child**, on `gpt-6.1-sol` at reasoning effort `high` per `CLAUDE.md`'s model routing: review backend `codex` first, `cursor-agent` when Codex has no usage left. The review judges YAGNI and overengineering first. A child moves on only with a `SHIP` verdict. When neither subscription can reach `gpt-6.1-sol`, the goal stops with a question rather than substituting another model.
- **QA on every child**, with `/flow-next:qa` against the running app on this Omarchy desktop, scenarios derived from the child's R-IDs. A child moves on only with a `YES` ship verdict.
- **Bugs are fixed as they appear.** Every review finding, every QA P0 and P1, every CI failure and every review-bot thread on the child's pull request is fixed (or answered with a reason) on that child's branch before it lands. A QA P2 is fixed in the child unless it belongs to another child's scope, where it is logged for that child. Nothing is deferred to a later child or to the final pull request.
- **CI green on every child's pull request** into the integration branch.
- **Final gate before `main`:**
  - a whole-diff implementation review on `gpt-6.1-sol` at reasoning effort `high` (as above) over the integration branch, with its fixes pushed there;
  - a QA pass over the combined build covering every child's R-IDs together, because children touch the same take pipeline;
  - the final pull request into `main` then needs green CI and a Cursor Bugbot or GitHub Copilot review of its head with no open thread, per the repository rules.

## Acceptance

- **R1:** Every child above is closed in Flow with its own acceptance criteria met, a `SHIP` implementation-review receipt on `gpt-6.1-sol` at reasoning effort `high`, a `YES` QA receipt, no open finding from either, and its pull request merged into the integration branch with CI green. A child whose spec ends in a measurement gate or an ADR-only outcome (fn-80's idle timeout, fn-84 stage 2, fn-86's gate) counts as met when that ADR is merged.
- **R2:** `just build test lint` passes in CI on the final pull request into `main`.
- **R3:** `CHANGELOG.md` has a single `[Unreleased]` section that describes all eight changes for users, ready to become the 0.5.0 notes. The docs each child names are updated.
- **R4:** Before the final pull request opens, the whole-diff review on `gpt-6.1-sol` at reasoning effort `high` returns `SHIP` and the combined QA pass returns `YES`, both recorded on the integration branch's head.
- **R5:** The final pull request into `main` is merged by land under the invoking session's consent, after a review bot has reviewed its head and no thread is open.

## Boundaries

- No release, tag or version bump during the goal. Gordon cuts 0.5.0 once, after the final pull request merges, following `docs/RELEASING.md`.
- Nothing from fn-87 (packaging) and nothing from fn-65 (dictating during a meeting) is built here. fn-81 only fixes the rules fn-65 will build on.
- `[audio] keep_microphone_ready` ships defaulting to `false`.
- Never edit a child's acceptance criteria. A child that cannot meet one stops the goal with a question in the log.
- The repository rules in `CLAUDE.md` hold: files stay under their size limits, ADRs record every decision, config stays in `config.toml`, visual checks never become gates.

## Decision Context

- The default for the always-ready microphone is off (decided 2026-11-01): no indicator surprise on a fresh install, opt-in for people who lose first words.
- One release for all eight (decided 2026-11-01).
- Full rigor for this goal, overriding fast-build mode: implementation review and QA on every child, bugs fixed as they appear, a whole-diff review and a combined QA pass before `main` (decided 2026-11-01).

## Goal log

- 2026-11-01: goal captured from the eight dictation specs; children fn-79 to fn-86 adopted, marked ready, route `no_plan` recorded on each.
- 2026-11-01: rigor set to full (per-child gpt-6.1-sol review and QA, fix-as-found, final whole-diff review and combined QA), from Gordon's words: "QA should be on for each spec/pr, impl-review too imo, bugs fixed as they appear".

- 2026-11-01: integration line goal/fn-88-better-dictation-shipped-as-one-release opened from main 4052810; each child lands there through its own pull request, and one final pull request takes the goal to main. Review backend codex resolves to gpt-6.1-sol at high.
