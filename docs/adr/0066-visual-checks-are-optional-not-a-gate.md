# 0066. Visual checks are optional, not a gate

Status: Accepted 2026-09-24, amends 0021 (the matrix no longer blocks) and 0040 (the rig no longer runs it)

## What this gives you

A change or a release is held up only by checks that prove behaviour. The render comparison stays available as a tool: `just qa-visual` shows how every surface compares with its reference picture, whenever someone wants to look.

## Situation

The baselines under `docs/design/` were rendered from the Claude Design artboards so that agents could build every surface autonomously and know when a screen matched the design. That build is finished, and Gordon has used the product day to day. A strict pixel gate now blocks work over differences a person has already looked at and accepted. The first rig run on the public repository failed 690 of 710 comparisons, 430 of them from the CI container's font rendering alone.

## Decision

The render comparison gates nothing. `rig.yml` has no `visual` job, and `dettivo-qa pack release` drops its `visual` and `visual_canary` steps (thirteen steps remain). `just qa-visual`, `just qa-visual-canary`, `just qa-visual-approve` and `just qa-beauty` stay as they are, to run by hand on the desktop. They never go into a GitHub workflow, because a CI container draws fonts and themes differently from the desktop the references come from.

## Consequences

Drift from the reference pictures shows up only when someone runs `just qa-visual`. The design bar still holds, and a person judges it on the running product. The baselines stay in the repository as references and can be refreshed with `just qa-visual-approve` at any time.
