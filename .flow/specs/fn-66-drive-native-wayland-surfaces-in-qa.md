# Drive native Wayland surfaces in QA with the Omarchy Cua Driver

## Goal & Context

Our GUI QA runs the Qt surfaces under X11 because of a limit found on 2026-09-03. On Hyprland, cua-driver 0.20 could list and drive only XWayland windows, and Wayland input needed a RemoteDesktop portal that Hyprland does not provide (ADR 0011). So QA forces `QT_QPA_PLATFORM=xcb`, keeps an in-repo AT-SPI and XTEST fallback driver, and proves the Wayland-only behaviour indirectly: the layer-shell OSD, virtual-keyboard insertion and the bar go through daemon tests, `hyprctl` checks and screenshots (ADR 0011 Consequences).

On 2026-09-25 Cua announced a stable Cua Driver for Omarchy, built with DHH, Spencer Bull and Vaxry. It adds a native synthetic cursor to Hyprland, where the compositor owns both cursors and routes the agent's input to the right window, separate from the user's. It ships on the Omarchy Edge channel, with cua-driver-rs v0.29.1 released the same day. If it works as described, QA can drive the real Wayland-native surfaces as users run them, in the background while someone keeps working on the same desktop.

This spec answers whether it does, on Dettivo's surfaces, and records the result. Adopting it as the QA driver is a follow-up decision driven by these results.

Source: https://x.com/trycua (thread, 2026-09-25) · https://github.com/trycua/cua · https://github.com/omacom/omarchy

## Approach

- Re-probe on Thor with the current cua-driver-rs release and the Omarchy Edge channel. Record `check_permissions`, `list_windows` and input paths as the 2026-09-03 probe did.
- Drive each Dettivo surface natively, with no `xcb` override: the app window, settings, history, meetings, the layer-shell OSD pill, and dictation insertion into a Wayland-native target app.
- Run it in the background while the user's own cursor and keyboard stay in use, and check that neither interferes with the other.
- Compare with the current X11 route on the same GUI pack scenarios: pass/fail parity, flakiness over repeated runs, and time.

## Quick commands

- `just build test lint`
- `just qa-visual` and the GUI packs by hand (ADR 0066: optional tools, never gates)

## Acceptance

- **R1:** A probe report records the cua-driver version, the Omarchy channel and version, the Hyprland version, and what the driver can list, read (accessibility tree) and send input to among Wayland-native windows. It sits next to the 2026-09-03 result it updates.
- **R2:** Each Dettivo surface (app routes, settings, history, meetings, OSD pill) is enumerated and driven natively without `QT_QPA_PLATFORM=xcb`, or the report names exactly what fails and why.
- **R3:** Dictation insertion into a Wayland-native target app is verified end to end through the native driver, or the gap is named.
- **R4:** A background run while the user keeps using their own cursor and keyboard shows no stolen focus, moved user cursor or misrouted input, or the report names each interference seen.
- **R5:** The existing GUI pack scenarios run on the native route and on the current X11 route, and the report compares pass/fail parity, flakiness over at least three repeated runs, and wall time.
- **R6:** The report ends with a recommendation: adopt the native route, keep X11, or run both. A follow-up spec and an ADR amending 0011 are drafted only if the recommendation is to change the route.

## Boundaries

- Switching Thor's Omarchy channel to Edge changes the whole desktop's packages, so Gordon approves it before it happens. The run records the channel before and after, and how to switch back.
- No change to the QA route, CI or the release gate in this spec. Visual checks stay optional and are never gates (ADR 0066).
- CI keeps Xvfb and the current drivers. Whether the native route can run headless in CI is recorded as a finding, not built.
- Never run while a meeting is recording or during Gordon's live work without his go-ahead.

## Decision Context

Captured on 2026-09-25 from Cua's Omarchy announcement. ADR 0011 made "re-check Wayland support at each cua-driver upgrade" a standing consequence, and this is the upgrade that claims to change the answer. The expected payoff is QA that exercises the real Wayland paths (layer-shell OSD, virtual-keyboard insertion) instead of an X11 stand-in, and can run in the background on a working desktop.
