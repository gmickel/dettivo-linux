# Working F9 shortcuts after onboarding

## Conversation Evidence

> user (turn 1): "i am dogfooding the current release of dettivo-linux, f9 doesn't seem to do anything?"
> user (turn 2): "ok $flow-next-flow that but do it for me manually this time, the spec should also delete that and QA it that it actually works"

> user (turn 3): "its there now, needs beautifying. the polish/enhanced selector is super ugly."
> user (turn 3): "also fix the code if you didn't already when you do that so that these 2 issues are fixes for good, the plugin, the keybinding thing"

> user (turn 4): "no QA while i'm still working on dogfooding it myself. instead leave the PR open so we can add more dogfooded fixes in there"

Native acceptance QA remains pending while dogfooding continues. Gordon subsequently requested "merge, install etc". PR #57 merged on 2026-09-15; that delivery does not claim the remaining native checks passed.

## Goal & Context

<!-- Goal & Context: [paraphrase] user turns 1-2; diagnosis [inferred] from live inspection. -->
An Omarchy user who completes Dettivo's shortcut setup can hold F9, speak, and release it to insert text. During release dogfooding, setup wrote a binding snippet without loading it into the compositor configuration. The daemon and speech engine were running, but Hyprland had no F9 binding. A manual include restored the bindings for the current session.

## Edge Cases & Constraints

- [inferred] Setup preserves unrelated compositor configuration and existing user customizations. Repeating setup leaves one effective binding per action and one press/release pair for F9.
- [inferred] If installing or activating the bindings fails, onboarding explains the failure and offers a retry. It must not present a written but inactive snippet as working shortcuts.

## Acceptance Criteria

- **R1:** Completing the onboarding shortcut setup activates F9 on Omarchy/Hyprland without a manual configuration edit. [paraphrase]
- **R2:** Before acceptance QA, remove the manual workaround installed during this dogfooding session and the generated Dettivo shortcut snippet, preserving a recoverable backup and unrelated settings. Confirm the shortcut is absent before running the repaired onboarding flow. [paraphrase]
- **R3:** On the native desktop, run onboarding from that clean shortcut state, hold F9, speak a known sentence, and release it. Verify listening starts on press, recording stops on release, and the expected text reaches the intended field. Retain evidence of the setup and observed result. [paraphrase]
- **R4:** Repeat setup and reload the compositor. F9 still works, unrelated configuration survives, and duplicate includes or duplicate effective bindings are absent. [inferred]
- **R5:** A failed installation or activation produces an actionable onboarding error. Focused regression coverage protects the missing-include failure, repeated setup, and failure reporting; the repository build, test, and lint gate passes. [inferred]

- **R6:** The shipped Omarchy plugin works from a fresh installation without local patches. Its idle waveform is visible, clicking it opens a functioning panel, and its recording pill follows real dictation. The package and installer supply all required QML modules and shared-state registration. [paraphrase]
- **R7:** The Raw, Polish, and Enhanced selector and adjacent action controls use a cohesive, theme-aware flat treatment consistent with Dettivo and the Omarchy bar. Replace the native-looking bevels and verify idle, selected, hover, keyboard-focus, and disabled states visually. [paraphrase]
- **R8:** Native acceptance QA removes the session's plugin patches and QML-path workaround before installing the repository-built result. Verify the visible idle waveform, panel controls, mode switching, recording pill, and F9 dictation; repeated installation remains safe. [paraphrase]

## Boundaries

- [paraphrase] The immediate manual repair enables current dogfooding. Acceptance of the product fix requires removing that repair before QA and leaving shortcuts working through the repaired setup afterward.
- [inferred] This fix addresses the Omarchy/Hyprland onboarding path. Other desktop shortcut backends retain their existing behavior.

## Decision Context

- [inferred] Live inspection found a generated but unsourced snippet, no F9 bindings, and a running daemon with a loaded speech engine. After the manual include and reload, the compositor listed both F9 bindings and no configuration errors. These checks establish activation only; they do not establish successful speech insertion.
- [paraphrase] Removing the workaround before QA prevents a preconfigured desktop from masking the onboarding defect.

## Strategy Alignment

- [strategy:Omarchy-native, beautiful by default] Working compositor hotkeys make speech available through the user's native desktop.
- [strategy:Complete speech workflows, proven by drives] Acceptance includes a native onboarding-to-insertion drive with recorded evidence.

## Strategy Conflicts

- [inferred] None identified.
