# Text insertion: virtual keyboard, libei, clipboard chain and target guards

## Conversation Evidence

> user (turn 1): "your job is to read research and create a full masterplan/product vision document/prd that we can then use to break down into flow-next specs to get this developed as quickly as possible"
> user (turn 16): "ok go with the defaults, then do /flow-next:strategy"
> masterplan S-08 row: "`text-insertion-virtual-keyboard-libei-clipboard-app-detection` | phase 1 | depends on S-02 | FR-I1 to I9, FR-M4 app identity, T9, T10; QA-8 | Insertion matrix pack under cua and CDP; unit tests for keymap generation, chain selection and target guards; never-into-self test"
> masterplan FR-I1: "Insertion tries, in order, the backends available in the session: the in-process Wayland virtual keyboard (`zwp_virtual_keyboard_v1` ...), portal RemoteDesktop through libei (GNOME, KDE), `ydotool` when its daemon socket exists, `xdotool` on X11, then clipboard plus a paste keystroke, then clipboard only. The chosen backend is reported in `InsertionResult` and can be pinned in settings."
> masterplan FR-I8: "Insertion never reports success after pasting into Dettivo's own windows, and never falls back to a global paste when the target cannot be verified."
> masterplan T9: "In-process Wayland virtual keyboard for insertion ... the keymap trick is what `wtype` does, and in-process removes an external dependency, gives precise error reporting and lets the fallback chain be tested as a unit."

## Goal & Context

<!-- Goal & Context: 30% [user], 50% [paraphrase], 20% [strategy] -->

Text lands where the user was typing. `dettivo-insert` implements the backend chain from the masterplan as one unit: the in-process Wayland virtual keyboard with a generated xkb keymap, portal RemoteDesktop through libei, `ydotool`, `xdotool`, clipboard plus an app-aware paste keystroke, clipboard only; captures the target app id and pid at hotkey time and re-verifies them before touching anything; never pastes into Dettivo itself; restores the clipboard afterwards; and reports the macOS-shaped `InsertionResult` with the Linux backend name. The daemon answers `insert.perform`, the CLI gains `dettivo insert`, and the QA rig gains the insertion matrix pack that records backend, latency and outcome per target. [paraphrase]

## Architecture & Data Models

<!-- Architecture & Data Models: 55% [paraphrase], 45% [inferred] -->

- `dettivo-insert` (depends on `dettivo-proto`): an `Inserter` trait `insert(text, target, mode) -> InsertionResult` over a `Chain` of `Backend`s, each with `available(&Session) -> Availability` (reason when not), `can_type(text) -> bool` (keymap coverage), and `insert`. Order per FR-I1; `[insert] backend` pins one. [paraphrase]
- Virtual keyboard backend: `wayland-client` binding `zwp_virtual_keyboard_manager_v1`; per insertion it generates an xkb keymap (`xkbcommon` text format) whose keycodes cover exactly the characters of the text (the `wtype` approach), uploads it, types keycodes with press and release and a small inter-key delay, then restores nothing (the keymap is per virtual keyboard). Characters beyond the keymap budget (more than the keycode range) split the text into batches with a fresh keymap each. [paraphrase]
- libei backend: `ashpd` RemoteDesktop session with keyboard device through `reis`; the portal session is created lazily on first use and kept; a denied portal reports `Availability::Denied`. [paraphrase]
- `ydotool` (socket at `$YDOTOOL_SOCKET` or `/run/user/<uid>/.ydotool_socket`) and `xdotool` (X11 only, `DISPLAY` set) run as commands with a timeout; never installed or configured automatically (FR-P5). [paraphrase]
- Clipboard backends: `wl-clipboard-rs` on Wayland, `x11-clipboard` on X11; save the current clipboard, set the text, send the paste keystroke through the first keystroke-capable backend (terminals `Ctrl+Shift+V`, everything else `Ctrl+V`, `[insert] paste_keys` per app id), restore the clipboard after a short delay; clipboard-only skips the keystroke and reports `copied_to_clipboard`. [paraphrase]
- Target capture: a `FocusProbe` per environment: Hyprland IPC (`activewindow` over the Hyprland socket), `wlr-foreign-toplevel-management` on other wlroots compositors, KDE window management on Plasma, none on GNOME; X11 `_NET_ACTIVE_WINDOW` plus `_NET_WM_PID` under XWayland or X11. The probe yields `Target { app_id, pid, title_hash, is_dettivo }`; `is_dettivo` is true for any Dettivo window (app ids `dettivo`, `dettivo-app`, `dettivo-osd`, `dettivo-qa-target` in QA). AT-SPI focus reports a secure text field where available (FR-I6). [paraphrase]
- Guards: `expected_target_bundle_id` and `expected_target_pid` are compared with the probe before any backend runs; a mismatch is `CONFLICT`; an unverifiable target (no probe available) allows only clipboard-only, never a paste keystroke (FR-I8). [paraphrase]
- Undo: the virtual keyboard and libei backends record the inserted length and, within five seconds, `insert.undo` (Linux addition) types Shift+Left repeated then Delete; other backends report `undo_supported = false` so the OSD offers "copy again" (FR-I9). [paraphrase]
- Latency and outcome per backend are returned in the result's `backend` block and logged without the text. [inferred]

## API Contracts

<!-- API Contracts: 70% [paraphrase], 30% [inferred] -->

- `insert.perform { mode, text | source_ref, expected_target_bundle_id?, expected_target_pid? }` → `InsertionResult { outcome: inserted | copiedToClipboard | failed, method: paste | fallback_copy | note_write, target_app, backend }` in the contract shape; the Linux `backend` field is already a recorded delta; `source_ref` answers `NOT_FOUND` until S-11 persists transcripts, except for the session's last transcript once S-07 lands. [paraphrase]
- Linux addition `insert.undo` → `{ undone: bool, reason? }` and `insert.target` → the current `Target` for the OSD and the QA rig, recorded in `docs/api/linux-deltas.md` with fixtures. [inferred]
- CLI: `dettivo insert --mode --text | --kind --id --expected-target-bundle-id --expected-target-pid`, plus `dettivo insert undo` and `dettivo insert target` (Linux additions). [paraphrase]
- Config: `[insert] backend = "auto"` (`virtual_keyboard`, `libei`, `ydotool`, `xdotool`, `clipboard_paste`, `clipboard`), `paste_keys = { "foot" = "ctrl+shift+v", ... }`, `terminal_app_ids = [...]`, `inter_key_delay_ms = 2`, `restore_clipboard = true`, `clipboard_restore_delay_ms = 300`, documented in `docs/config.md`. [inferred]
- `system.capabilities.insertion_backend` names the backend the chain would use right now; `dettivo doctor` lists every backend with its availability and reason. [paraphrase]
- QA: `dettivo-qa drive insertion_matrix` runs the targets available on the machine from the masterplan's 9.7 table (`dettivo-qa-target` always; `foot` or `ghostty`, Chromium through `agent-browser`, and the others when installed), reads the text back, and writes `insertion-matrix.json` with backend, latency and outcome per target into the evidence directory (QA-8). [paraphrase]

## Edge Cases & Constraints

- Text with characters outside the current keymap: the virtual keyboard generates a keymap for them; a backend that cannot guarantee coverage is demoted for that text (FR-I4). [paraphrase]
- Target changed between hotkey press and insertion: `CONFLICT`, nothing typed, clipboard untouched. [paraphrase]
- Focused Dettivo window: outcome `failed` with reason `target_is_self`, never a paste. [paraphrase]
- Secure field detected: outcome `copiedToClipboard` with reason `secure_field`. [paraphrase]
- Clipboard restore races the target's paste: the restore waits `clipboard_restore_delay_ms` and never restores if the user changed the clipboard meanwhile. [inferred]
- No Wayland display and no X11: only clipboard backends are available and the doctor says why. [inferred]

## Acceptance Criteria

- **R1:** Keymap generation unit tests cover ASCII, accented Latin, CJK, emoji and mixed text: every character maps to exactly one keycode, batches split above the keycode budget, and the generated keymap compiles with `xkbcommon`. Errors: an unmappable code point is reported by index and the backend demotes itself for that text. [paraphrase]
- **R2:** Chain selection unit tests prove the FR-I1 order, the pin from `[insert] backend`, demotion on keymap coverage, and that an unverifiable target permits only clipboard-only. Errors: a pinned backend that is unavailable fails with its availability reason, never a silent fallback. [paraphrase]
- **R3:** Target guard tests: `expected_target_bundle_id` or `expected_target_pid` mismatch is `CONFLICT` before any backend runs; a Dettivo window as target is `failed` with `target_is_self` (the never-into-self test drives the `dettivo-qa-target` window and asserts nothing was typed). Errors: as stated. [paraphrase]
- **R4:** On the development machine under Hyprland, `dettivo insert --text` through the virtual keyboard lands the exact text in `dettivo-qa-target` and in a terminal (`foot` or `ghostty`) with the app-aware paste keystroke for the clipboard path, the clipboard is restored afterwards, and the result names the backend and target; under Xvfb in CI the same drive passes with `xdotool` or the clipboard path. Errors: an unavailable backend is skipped with its reason in the log. [paraphrase]
- **R5:** `insert.perform`, `insert.undo` and `insert.target` fixtures pass against the live daemon; `dettivo doctor` lists every backend with availability and reason; `system.capabilities.insertion_backend` names the chain's choice. Errors: `source_ref` for an unknown transcript is `NOT_FOUND`. [paraphrase]
- **R6:** `dettivo-qa drive insertion_matrix` writes `insertion-matrix.json` with backend, latency and outcome per available target, runs in the CI drive job for the Xvfb-reachable targets, and the `[insert]` keys are documented and printed by `config print-default`. Errors: a target that is not installed is recorded as `skipped` with the reason, never as a failure. [paraphrase]

## Boundaries

- No hotkey bindings or portal GlobalShortcuts; S-09 captures the target at press time through this crate's probe. [paraphrase]
- No dictation wiring beyond the `Inserter` trait S-07 calls; this spec ships the chain behind it. [paraphrase]
- No OSD "copy again" surface; S-10 reads `undo_supported`. [paraphrase]

## Decision Context

### Motivation
<!-- scope: business -->

- The Windows port learned that insertion needs target verification, never a global paste, and never success after pasting into itself; the in-process virtual keyboard makes the chain testable as a unit. [paraphrase]

## Strategy Alignment

- **Complete speech workflows, proven by drives:** text landing in the right window is the last step of the first dogfood, and the matrix report proves it per target. [strategy:Complete speech workflows, proven by drives]
- **Omarchy-native, beautiful by default:** Hyprland IPC and the wlroots protocols are first-class, GNOME and KDE follow through portals. [strategy:Omarchy-native, beautiful by default]

## Requirement coverage

| R-ID | Task |
|---|---|
| R1 | fn-N.M (TBD) |
| R2 | fn-N.M (TBD) |
| R3 | fn-N.M (TBD) |
| R4 | fn-N.M (TBD) |
| R5 | fn-N.M (TBD) |
| R6 | fn-N.M (TBD) |
