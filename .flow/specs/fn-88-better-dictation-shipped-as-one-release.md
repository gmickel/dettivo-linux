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

- 2026-11-01: fn-79 built on branch fn-79-clean-dictation-takes-a-real-speech (pushed, head 672b334), task left in_progress: the gpt-6.1-sol review fixed 3 of 4 findings, then flowctl escalated "review loop stalled (same-not-fixed-lineage)" after 4 of 8 rounds on R1's "excludes the post-release tail".
- 2026-11-01: QUESTION (blocks goal R1 through fn-79 R1): may up to one PipeWire quantum delivered after the stop request count as inside the take for R1? Options: (a) accept that tolerance and record it in an ADR, plus the two-line MockCapture stop recheck; cost one short fix commit and a human reset of the review counter; (b) add capture timestamps to Event::Pcm and drop samples stamped after the stop; cost a protocol-level change in dettivo-audio and another review round. Recommendation: (a), because the stop cue plays after the stop and cannot fall inside one quantum. Answer by editing this spec; reset the fn-79.1 review counter (human-only) so the loop can resume.
- 2026-11-01: fn-79 measured spec premise: Parakeet v3 returns nothing on some short takes with the 400 ms tail (jfk 8.50-10.48 s empty at 300/400/500/1000 ms, full at 0/100/200/700/1500 ms; v2 unaffected); the child retries unpadded on an empty padded result and a test covers the cut.
- 2026-11-01: fn-82 built on branch fn-82-hold-to-talk-that-survives-lost (pushed, head 8f885ae): task done, gpt-6.1-sol review SHIP at 164ea9d, completion review not_required. QA NO: P1 the hotkeys_hyprland drive's double tap lands about 380 ms apart (40 ms settle per key edge), outside the 300 ms window, so the drive's lock and lost-release steps never run live (fix belongs in fn-82's drive); P2 pre-existing, cause unconfirmed, a Hyprland hold release sometimes never reached the daemon (2 of 5 drive runs). R2 (GNOME/KDE rebind) has no live scenario on this Omarchy machine.
- 2026-11-01: QUESTION (blocks goal R1 through fn-82 R2): how should fn-82's R2 be proven live when this machine has no GNOME or KDE session? Options: (a) a GNOME or KDE VM or second machine for one QA pass; cost a setup session; (b) accept the unit and daemon tests in crates/dettivod/tests/hotkeys.rs as R2's evidence, recorded with its date here; cost nothing, risk an untested desktop path. Recommendation: (b) for this release, with (a) noted for the cleanup phase.
- 2026-11-01: run stopped NEEDS_HUMAN on fn-79's review escalation; fn-82's PR waits for its drive fix and the R2 answer.
- 2026-11-01: ANSWER from Gordon (2026-11-01), fn-79 R1: "a" - up to one PipeWire quantum delivered after the stop request counts as inside the take; recorded in an ADR with the MockCapture stop recheck. Gordon approved the fn-79.1 impl review counter reset ("reset approved"), done.
- 2026-11-01: ANSWER from Gordon (2026-11-01), fn-82 R2: "b" - the unit and daemon tests in crates/dettivod/tests/hotkeys.rs are R2's evidence for this release; a GNOME/KDE live pass is a cleanup-phase follow-up.
- 2026-11-01: fn-79 merged into the integration line via PR #40 (squash a457be6, head 8d22d85): impl review SHIP on gpt-6.1-sol high at 088517b, QA SHIP at 3395e8b (F1, F2 fixed in the child), three Copilot threads fixed and resolved, CI gate/smoke green. QA P2 F3 (pre-existing: `dettivo dictation stop` on a gated take answers CONFLICT instead of naming silence) is logged for fn-85, whose R4 covers stop exit codes.
- 2026-11-01: a fn-85 worker crashed the live Hyprland session (hyprctl repl calling hl.is_key_down); the desktop is in safe mode, Gordon's call. Every brief now forbids compositor IPC. Live-desktop QA (fn-82's recheck) waits for Gordon.
- 2026-11-01: QUESTION (blocks goal R1 through fn-85 R2): no source on this desktop reports held modifiers (hyprctl devices has none, hl.is_key_down crashes Hyprland, no input-group evdev access, a compositor toggle sends no release). Options: (a) a fixed settle delay after the toggle (about 250 ms) with the 2 s cap, no key-state source; (b) evdev modifier state, needs Gordon in the input group; (c) narrow R2 (Gordon edits fn-85's criteria). Recommendation: (a).
