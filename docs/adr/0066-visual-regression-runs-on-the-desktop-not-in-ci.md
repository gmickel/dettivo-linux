# 0066. Visual regression runs on the desktop, not in CI

Status: Accepted 2026-09-24, amends 0021 (where the matrix blocks) and 0040 (what the rig runs)

## What this gives you

A red visual check means the product looks different from its approved design, never that a CI container drew the fonts differently. The matrix runs where the approved renders were made, on an Omarchy desktop with the fonts and themes the product uses, and it still blocks every release.

## Situation

The first rig run on the public repository, on 2026-09-24, failed 690 of 710 visual comparisons. On the development desktop the same commit failed 260. Those 260 are real drift from the UX work of 2026-09-14 to 2026-09-24, which went unapproved while Actions was unavailable. The other 430 came from the CI container alone, which renders every surface differently from the desktop the baselines were approved on. A check that fails for the machine it runs on tells nobody anything about the design.

## Decision

`rig.yml` no longer has a `visual` job. `just qa-visual` and `just qa-visual-canary` run on the desktop: by the worker before a change that touches a surface, and as steps 8 and 9 of `dettivo-qa pack release`, whose receipt the release workflow requires before it publishes (ADR 0051). Baselines are approved with `just qa-visual-approve` as before.

## Consequences

The nightly rig no longer shows visual drift, so drift surfaces when a worker runs the matrix or at the release gate. The release gate still refuses a version whose renders differ from the approved baselines, so a release cannot ship unapproved visuals. The 260 drifted renders need a review and approval before the next release gate can pass.
