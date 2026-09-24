# 0056. Live desktop actions, readable state and evidence from running windows

Status: Accepted 2026-09-08

## What this gives you

Home starts a meeting through the existing disclosure flow and shows a failed dictation's reason. Model, configuration and agent summaries follow the facts they describe. Smaller windows keep their controls reachable, and opening Home no longer initializes the audio decoder.

The follow-up artboard pass aligns Today and first-run columns, keeps growing transcripts and long analysis readable, and gives compact meeting controls a scrollable, keyboard-accessible layout.

## Decisions

| Area | Decision |
|---|---|
| Actions and state | Home routes a meeting start through the window's surviving context. Dictation failures reach the page. Configuration events refresh health and model facts. REST state comes from capabilities and configured MCP hosts come from the host model. Unavailable call-history telemetry is labelled as untracked. |
| Models | Warm state uses the model's exact file or directory identity and readiness. A short identifier such as `tiny` cannot match `tiny.en`. |
| Accessibility | Hidden optional buttons and switches leave the accessibility tree. Environment-locked text fields are read-only to accessibility as well as disabled for pointer and keyboard input. |
| Keyboard | Home's `j` and `k` move through today's items; Enter opens the focused item. Shared list rows expose keyboard and accessibility activation, and footer hints describe the current route. |
| Layout | Settings retain the table widths of the reference design and scroll horizontally in narrower windows. First run scrolls vertically in short windows. Keyboard focus scrolls its editor into view. |
| Dialogs | The shared style owns popup backgrounds and content containers. Each dialog supplies children inside that container. The delegate-by-delegate style check remains unchanged. |
| Audio lifetime | Waveform previews use the WAV parser alone. The media player and audio output are created on first playback. A pending seek is applied after media becomes seekable, and a pause cancels a pending play. |
| QA isolation | Profiles isolate host configuration overrides as well as HOME and XDG paths. The private Xvfb session permits foreground input within that desktop; ordinary desktop driving keeps its background default. CUA tokens remain authoritative for actions, with missing geometry filled only from unambiguous accessibility matches. |
| QA assertions | Editable configuration paths retain exact values. Their labels and all unrelated text still pass the negative-text scan. Happy-path timing samples satisfy the same overlap constraints as the configuration defaults. A changing accessibility tree is retried whole, never returned partially. |
| Artboard alignment | Today can remove the shared row's horizontal padding without changing other consumers. Engine progress tracks are separate from dividers. First-run header and model columns use the authored alignment while typography follows the active theme. |
| Reading | Live transcript layout settles before positioning; a transcript that fits starts at its beginning. Capture follows its growing tail until the reader scrolls away, and gestures suspend following. Analysis scrolls above its facts and reveals a focused action. |
| Compact controls | Model selectors use available space and idle unload names its seconds unit. The new-meeting rail scrolls above its footer. Narrow Meetings headers stack, while a horizontal table viewport preserves column widths and has keyboard navigation and a fixed focus ring. |
| Content panels | History and meeting content panels use the artboards' transparent interiors and hairline borders. Editable controls retain focus/hover styling. Shared source and fact controls expose their existing actions to the keyboard. |
| Menus | Menu backgrounds composite their raised tint over the page colour, so underlying text cannot show through. Menus size to item labels within window bounds and scroll when their content is taller than the window. |

These decisions extend the app host in ADR 0020, playback in ADR 0025, settings in ADR 0033 and the QA checks in ADRs 0051 and 0052.

## Evidence and approval

The live pass is recorded under `docs/reports/qa/2026-09-08-live-desktop.md`, with a QA receipt for fn-54. Captures include running-window screenshots, accessibility trees, daemon responses and the unchanged-reference visual comparison.

An agent inspection does not supply Gordon's fn-38 R6 walkthrough. Missing or stale visual references remain visible in the report until their approval is recorded. Installed socket and shell-plugin checks require the installation those checks exercise.

The follow-up is recorded in `docs/reports/qa/2026-09-08-beauty-iteration.md`. Its FocusRing reuses the small component already authored on the pending fn-38 branch; it imports no approval or other fn-38 work. Integrating that branch must retain a single component registration.
