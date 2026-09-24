# Dettivo for Linux — agent operating instructions

Dettivo for Linux is the native Omarchy/Hyprland port of Gordon Mickel's speech workstation: a Rust daemon, Qt 6 Quick surfaces and ggml engine processes. This file is how an agent works in this repository. `CLAUDE.md` carries the same instructions for hosts that read that name.

## Orchestration

The build runs as an autonomous loop and the human's judgment lives in the specs, not in the loop.

- **Models: Claude Opus 5.5 implements, gpt-6-astra reviews.** The conducting session and every role that writes or changes code run on Claude Opus 5.5 (`claude-opus-5-5`): the flow-next `worker`, `pr-comment-resolver` and `quality-auditor`, pinned at dispatch wherever a plugin default differs. Every review runs on `gpt-6-astra` through whichever subscription has usage left: `codex exec` (review backend `codex`) first, `cursor-agent` when Codex is exhausted and its catalogue lists the model. Reviews are off during the fast build. A review judges YAGNI and overengineering first: code, options, abstractions, layers or configuration the spec does not require. Style and polish come after. When no subscription can reach gpt-6-astra, say so and defer the review rather than substituting another model. Scouts and `plan-sync` keep the plugin's cheaper defaults, because they inventory and summarise rather than decide.
- **Drive with pilot and land, never ask.** `$flow-next-pilot` advances one ready spec one stage per tick; `$flow-next-land` babysits the resulting PR to merge. Run them without setup or confirmation questions. Ambiguity becomes a `NEEDS_HUMAN` verdict, not a prompt.
- **Overlap while a PR waits.** A spec whose only unmet dependency is an open PR is worked now in its own git worktree under `~/work/dettivo-linux-wt/<spec>` on a branch cut from that PR's head, via `$flow-next-work <spec> --branch=current --no-plan mode:autonomous` followed by `$flow-next-make-pr`. Pick specs with disjoint file areas; land's server-side catch-up brings the PR onto `main` after its base merges.
- **Fast-build mode until v1.** Specs are captured with `--no-plan` and worked directly. No plan review, no implementation review, `pipeline.qa` off, review backend `none`. A cleanup phase after v1 turns reviews, QA and blocking visual regression back on.
- **PR review bots: GitHub Copilot only.** Request Copilot when a PR becomes ready for review. Keep automatic draft and every-push reviews off; request another review explicitly after substantial changes. Never trigger, request or count the Codex, Cursor or any other review bot on a pull request; the gpt-6-astra reviews above run locally through the CLIs. `land.automatedReviewers` names `copilot-pull-request-reviewer`; land merges only a head Copilot has reviewed with zero open threads after the patience window.
- **PR bodies come from make-pr.** The make-pr footer marker must stay the last line of the body; put any attribution or session link above the footer.

## Repository rules

- **This is the public repository, licensed GPL-3.0-or-later.** `gmickel/dettivo-linux` starts at the first public commit `e995766` (2026-09-24), and every change lands here. The pre-public history lives in the private archive `gmickel/dettivo-linux-dev` (checkout `~/work/dettivo-linux-dev`, old worktrees under `~/work/dettivo-linux-dev-wt`). Read the archive for context when you need it, but never commit to it, push to it, or bring its commits or branches here: every commit there carries the MIT licence, and one merge, cherry-pick or push would make that history public. Commit SHAs and PR numbers cited in `.flow/` evidence and `docs/reports/` from before 2026-09-24 refer to the archive.
- **One command builds everything.** `just build test lint` is the gate; CI runs the same recipes in a digest-pinned Arch container and every job is reproducible locally with the recipes on its `run:` line. A change ships only with that gate green.
- **ADRs are the documentation.** Decisions live under `docs/adr/`; add or update a record whenever a decision lands or changes. Every document, ADR or user-facing, is drafted with `$flow-next-prose` and leads with what it does for the reader. Never a list of caveats.
- **Design is a gated requirement.** Every surface resolves from Omarchy theme tokens and matches the approved baselines under `docs/design/`. Beautiful is the bar, on every theme.
- **Files stay small.** Rust and C++ under 500 lines, QML under 300; split rather than allowlist hand-written code. Crate dependency edges are declared and linted.
- **Configuration is files.** Everything is configurable through `config.toml`; the GUI is a convenience, never the only path.
- **Planning documents never enter this repository.** The masterplan and research live in the vault; `STRATEGY.md`, the ADRs, design baselines and user docs are the in-repo record.
- **Commits** follow Conventional Commits. Stage with `git add -A` inside a flow-next task commit; stage only named files anywhere else.

<!-- BEGIN FLOW-NEXT -->
<!-- flow-next:snippet:v2 -->
## Flow-Next

This project uses Flow-Next for ALL task tracking. `flowctl` comes from the flow-next plugin install — every flow-next skill resolves it itself, and on Claude Code it is also on PATH. Do NOT create markdown TODOs or use TodoWrite. Cold session: `flowctl brief` first — one bounded call (specs, ready tasks, memory); go deeper with `show`/`cat`/`anchor <task-id>`.

- Lifecycle: `flowctl list` / `show fn-N.M` / `start fn-N.M` / `done fn-N.M --summary-file s.md --evidence-json e.json` (e.json: `{"commits": ["<sha>"], "tests": ["<cmd>"], "prs": []}`)
- BEFORE any other flowctl operation, or when unsure of a flag: run `flowctl usage` (CLI cheatsheet + orchestration recipes) or `flowctl --help`.
- BEFORE bridging work to another model/CLI (`codex exec`, `cursor-agent`, `claude -p`, `grok`) or picking an implementation/review model: run `flowctl usage` and follow "Orchestration & model steering" exactly.
- Creating a spec: write it directly — `$flow-next-plan` is task breakdown only. `flowctl spec create --title "Short title" --plan-file plan.md --json`, then `$flow-next-plan <spec-id>`. Scaffold cascade (first match wins): `SPEC.md` -> `spec.md` -> bundled template.
- Substantial replies (reports, reviews, multi-section answers): invoke `$flow-next-prose` BEFORE drafting — the artifact prose contract applies to chat replies too. Short conversational turns skip it.
- If `flowctl` is not found: your shell lacks the plugin's `scripts/` dir on PATH (only Claude Code injects it). Resolve it the way the skills do - the plugin install's `scripts/flowctl` (Claude/Droid: plugin-root env var; Codex: `${CODEX_HOME:-$HOME/.codex}/scripts/flowctl`; Cursor/Grok: two levels above any flow-next SKILL.md) - or update/reinstall the flow-next plugin. A repo with no `.flow/` yet: run `$flow-next-setup`.
<!-- END FLOW-NEXT -->

<!-- flow-next:model-routing:start -->
## Model routing

Implementation stays on Claude Opus 5.5 (`claude-opus-5-5`): the conductor, the worker, the resolver and the quality auditor. Reviews go to gpt-6-astra, from the other model family, through Codex or Cursor, whichever has usage. Scouts keep the plugin's defaults, so the two scout tiers are deliberately left unset here. An explicit instruction in the moment still wins over this block.

reviewer: gpt-6-astra
implementer: claude-opus-5-5
<!-- fast scout: unset - the plugin's fast-tier default (inventory scanning) -->
<!-- thinking scout: unset - the plugin's judgment-tier default (bounded analysis) -->
<!-- flow-next:model-routing:end -->
