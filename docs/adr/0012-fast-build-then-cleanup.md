# 0012. Specs are captured without plans or reviews during the build, then a cleanup phase turns the gates on

Status: Accepted 2026-09-03

## What this gives you

The v1 surface arrives quickly, and the quality gates arrive once there is a whole product to run them against instead of slowing each slice.

## Situation

The decision log, design baselines and requirement IDs already exist in this repository, so each spec has its brief before it is captured. Plan reviews, implementation reviews and the flow-next QA pipeline stage each add a round trip per spec.

## Decision

Specs are captured with `/flow-next:capture --no-plan` and worked directly. Plan reviews, implementation reviews and `pipeline.qa` stay off during the build. CI unit and contract jobs, the accessibility lint and advisory visual diffs stay on because they cost nothing to run. When the v1 surface is complete, a cleanup phase enables plan and implementation review, `pipeline.qa`, blocking visual regression, the beauty pass and the docs lint, and works through what they find.

## Consequences

- Architecture decision records under `docs/adr/` are the main documentation and are written as decisions land or change.
- The product masterplan and the discovery research live in Gordon's vault, outside this repository and its history; `STRATEGY.md`, the ADRs, the design baselines and user documentation are the in-repo record.
- Every document is drafted with the prose skill and leads with what it does for the reader.
- Defects that a review would have caught early are accepted as cleanup work; the bound is v1 scope, not the whole roadmap.
