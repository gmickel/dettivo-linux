# Fix meeting timer stuck at 00:00

The compact meeting status display must show elapsed recording time so Gordon can tell how long the active meeting has run.

## Report

During meeting dogfooding on 2026-09-16, Gordon reported that the timer stayed at `00:00` while the meeting continued. The supplied screenshot shows a compact strip with a back chevron, cyan audio activity glyph, `00:00`, and another icon. Identify the exact surface before editing; do not assume this is the main live-view elapsed label.

Original screenshot: `/tmp/codex-clipboard-rm8oob.png` (temporary user-provided evidence).

## Scope and acceptance

- Reproduce the stuck timer on the pictured surface during an active meeting.
- Make the displayed duration advance and agree with meeting elapsed time, including when opening the surface after recording has started.
- Check stop and reopen behavior and add focused regression coverage for the discovered cause.
- Keep this separate from the already observed live provisional-fragment replacement bug.

## Execution boundary

Gordon subsequently authorized implementation through flow-next-flow, with no full release. The original recording is complete. Build and verify locally; the conductor applies the hotfix only after checking the daemon is idle. Keep the original meeting intact.


## Observed cause and native QA follow-up

The compact strip is the Omarchy plugin. DettivoState.setMeeting formats live_last_end_ms on meeting.state events, and has no ticking clock. Silence and sparse state events therefore freeze its counter. Derive elapsed time from the recording start/duration anchor, refresh independently of transcript segments, and restore the anchor from snapshots on reconnect. Stop must end the recording indication.

In this same local hotfix, correct the fn-63 speaker-stage caption found during native QA. Its fixed width elides `3 speakers · 154 segments …`, hiding `unassigned`. Keep the unassigned count visibly meaningful, with the full detail available accessibly; shorten the wording or make the smallest layout adjustment. This is a presentation correction to fn-63, not a change to speaker assignment.


The same native render exposed a footer collision at the default 1280×820 size after adding the title shortcut. Keep the meeting-detail shortcut hints readable without overlapping the separate delete-meeting control. Shorten labels or reserve its width; preserve the title shortcut in the visible hints and the full keyboard help. Verify the corrected caption and footer at the default/minimum supported size.


Clock ownership is per meeting. While a new meeting records, state events from an older meeting finishing speaker identification or analysis must not clear or rewind the new meeting clock. An explicit connection/snapshot reset with no active meeting must still clear the display. Cover this with a regression using two synthetic meeting IDs.
