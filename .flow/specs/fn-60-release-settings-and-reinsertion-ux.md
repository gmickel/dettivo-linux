# Release settings and reinsertion UX

## Conversation Evidence

> user (turn 1): "starting to think about release and am dogfooding (will dogfood meetings tomorrow)"
> user (turn 1): "[Image #1] what should the insert again button do, i mean it will surely never insert,. cos i'm in dettivo and not in an app where it can insert?"
> user (turn 1): "UI/UX: [Image #2] should we show the snippet if it's not editable?"
> user (turn 1): "[Image #3] same here, is there value in showing this?"
> user (turn 1): "surely either people edit the config toml and we should probably deliver a heavily commented and explained config file with the app or they use the UI?"
> user (turn 1): "[Image #4] not helpful as an input box"
> user (turn 1): "[Image #5] not helpful as an input box, probably needs to be a separate menu point in settings?"
> user (turn 1): "[Image #6] these kind of settings probably need a reset to default option, there may be other such options, especially the more technical ones , intervals etc. alternatively we could have a simple/advanced switcher for the settings"
> user (turn 1): "[Image #7] not a helpful input for someone that doesn't know the options (u probably see the pattern)"
> user (turn 1): "[Image #8] should we expose this in the prod build? do we give users to use whatever model they want already?"
> user (turn 1): "[Image #9] helpful?"
> user (turn 1): "[Image #10] definitely a page that needs simple/advanced mode/"
> user (turn 1): "the agents pages probably also need a simple/advanced"
> user (turn 1): "etc, $flow-next-flow capture and then work on it, low token limits today, so implement in the most efficient way possibe after thinking heavily about how to improve ui/ux"

## Goal & Context

<!-- Source: [paraphrase] -->
Release dogfooding exposed settings that require users to know configuration syntax and a history action whose destination is unclear. Make everyday choices understandable from the UI and preserve the fully configurable file workflow. Deliver this as one cohesive release UX pass using shared controls.

## Architecture & Data Models

<!-- Source: [inferred] -->
Keep the existing settings model and TOML persistence contract. Use shared presentation and reset behavior, preserving validation, environment override handling, external-file refresh and unknown custom values. Inspect the existing insertion focus contract before choosing the smallest safe history interaction.

## Edge Cases & Constraints

- Hidden advanced settings must retain their values; switching views must never write configuration. [inferred]
- A default reset must use the canonical default, preserve unrelated config/comments and display errors or environment overrides honestly. [inferred]
- Model controls must handle inherited choices, unavailable/custom selections and empty catalogues without silently changing values. [inferred]
- Preserve theme tokens, keyboard navigation, accessibility, file-size limits and minimum-window usability. [strategy:Omarchy-native, beautiful by default]

## Acceptance Criteria

- **R1:** History offers a clearly explained, usable way to put a selected transcript into another app. It must not immediately attempt to insert into Dettivo itself. Inspect and document existing targeting; use an explicit copy/paste fallback if reliable focus restoration is unavailable. [inferred]
- **R2:** Settings default to a simple view containing everyday controls; advanced mode exposes technical paths, timing, pipeline, agent connection details and troubleshooting. Raw TOML previews and generated shortcut snippets are collapsed or advanced-only, while relevant setup actions remain discoverable. [paraphrase]
- **R3:** Language and meeting/analysis model choices use labeled selection controls with automatic/inherited choices. Transform options use individually labeled controls with explanations. Supported values are discoverable and existing custom values are preserved. [inferred]
- **R4:** Vocabulary has a dedicated, discoverable editor for adding and removing terms without requiring comma-separated syntax knowledge; preserve the config vocabulary list. [paraphrase]
- **R5:** Editable settings expose a clear reset-to-default action, including paths and technical numeric settings, with effective defaults and validation handled by the existing config contract. [paraphrase]
- **R6:** Experimental model controls are kept out of everyday production settings and clearly identified as advanced/experimental if exposed. Document the actual supported custom-model workflow without implying arbitrary model compatibility. [inferred]
- **R7:** Ship an explained, commented config reference and update user documentation for the UI/file alternatives, simple/advanced settings, vocabulary and history workflow. [paraphrase]

## Boundaries

- No meeting-engine changes or new custom-model loader; tomorrow's meeting dogfooding remains separate. [inferred]
- Do not mutate Gordon's live configuration, reinstall the app or restart his running services as part of implementation. Use isolated verification fixtures. [inferred]
- Preserve fn-59 and its existing PR; this change gets its own branch/PR based on the current dogfooding fixes. [inferred]

## Decision Context

- Use one spec and one owner because the requested outcome is a consistent release UI and the controls share settings infrastructure. Skip task decomposition to conserve tokens. [inferred]
- Simple/advanced is presentation, not a second source of configuration. Put raw technical details behind deliberate disclosure. [inferred]
- Prefer explicit copy/paste over speculative focus restoration when the compositor cannot provide a reliable destination. [inferred]

## Verification

Focused Qt/settings/history regression tests, isolated visual inspection of affected pages, and the repository gate `just build test lint`. Record actual evidence and any unavailable native checks. Update the relevant ADR for the settings presentation decision.
