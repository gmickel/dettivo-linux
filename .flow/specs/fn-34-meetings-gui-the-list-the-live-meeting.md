# Meetings GUI: the list, the live meeting, the detail, speakers and notes

## Conversation Evidence

> user (turn 1): "your job is to read research and create a full masterplan/product vision document/prd that we can then use to break down into flow-next specs to get this developed as quickly as possible"
> user (turn 16): "ok go with the defaults, then do /flow-next:strategy"
> masterplan S-27 row: "`meetings-gui-list-live-detail-speakers-notes` | phase 4 | depends on S-17, S-26 | Meetings routes in FR-U2; baselines `meetings-list.png`, `meeting-live.png`, `meeting-detail.png`, `import-and-disclosure.png` | Drive pack with seeded meetings and a live rig run; visual diff against the four meeting baselines"
> masterplan FR-U2: "Routes: onboarding, home, history, history detail, meetings, meeting live, meeting detail, settings sub-routes ... Each route is openable by an environment variable in QA mode."
> masterplan Meeting, live baseline: "Header: a recording square in accent, the title, both source meters with their device and app names, the elapsed time as the largest type on screen, Pause and Stop. Transcript rows are time, source label (You in speaker gold, Remote in speaker olive) and text with a 1.7 line height. The provisional segment renders in weight 300 and muted colour until finalised. The footer states the labelling rule in one sentence ... Notes are a real Markdown editor with a live caret; analysis is a quiet card that explains when it runs and that it never overwrites notes (FR-G6). The disclosure state and copy action sit in the rail footer (FR-G9)."
> masterplan Meetings list: "Weeks as section labels; rows carry when, title with a one-line summary, length, speaker swatches with names, and a state chip (analysed, partial, notes only). The right rail is the new-meeting form: source checkboxes with live meters, engine, speakers, analysis and disclosure state, one primary button, and the config section that sets the defaults."
> masterplan Meeting detail: "Title and facts, a talk-time bar per speaker with renamed names, tabs for Transcript, Notes and Analysis with a Raw and Polished toggle, the transcript with speaker names in their colours, and the analysis rail with summary, decisions and action items plus notes, export and audio facts. Delete is text in the urgent colour in the footer."
> masterplan Import and disclosure: "The import dialog maps one to one onto `transcripts.import` (target kind, engine, language, diarize, analyse) and states the chunking and the estimated time. The disclosure dialog shows the macOS message verbatim, Copy message, Not now, and Acknowledge and start, and says it is asked once."
> masterplan coverage table: "Re-run dialog, export sheet, speaker rename popover | Small; designed inline during S-20 and S-27 against the pattern"
> masterplan QA-14: "A visual regression job renders every surface and state at 1x and 2x, dark and light, in the default Omarchy theme plus at least two others, and compares against approved baselines with a perceptual diff"

## Goal & Context

<!-- Goal & Context: 30% [user], 50% [paraphrase], 20% [strategy] -->

Meetings get their three screens. The `meetings`, `meetings.live` and `meetings.detail` routes, scaffolds today in `MeetingsRoute.qml`, are built to `meetings-list.png`, `meeting-live.png`, `meeting-detail.png` and `import-and-disclosure.png`: the list by week with speaker swatches and state chips beside the new-meeting rail, the live screen with both meters, the elapsed time as display type, segments arriving provisional then final in source colours, the notes editor and the analysis card, and the detail with the talk-time bar, the three tabs with the Raw and Polished toggle, the analysis rail and the urgent-text delete. The import dialog maps onto `transcripts.import`, the disclosure dialog asks once with the macOS message and a copy action, and the speaker rename popover is designed inline against the pattern. Everything is a client of the `meetings.*` methods and the `meeting.segment`, `meeting.state` and `audio.level` streams; the drive pack runs the seeded meetings on both drivers and a live meeting on the rig, and the visual gate holds every state on five palettes at two scales. [paraphrase]

## Architecture & Data Models

<!-- Architecture & Data Models: 55% [paraphrase], 45% [inferred] -->

- Host (`qt/host/app/`): `MeetingsModel` pages `meetings.list` with week grouping from `started_at`, exposing the Linux list fields (`summary`, `speaker_count`, `has_notes`, `analysis_status`, `is_partial`) and a swatch list from `meetings.get` for the visible rows; `MeetingLiveModel` starts a meeting (`meetings.disclosure.get` first, the dialog when unacknowledged, then `meetings.start` with the rail's capture, title, engine, `expected_speakers`, `analyze`), subscribes to `meeting.state`, `meeting.segment` and `audio.level` per source, keeps the segment list with the provisional tail replaced in place, runs the elapsed clock from `started_at`, saves the notes editor through `meetings.notes.set` with `source = live` on a 1 s debounce, and stops through `meetings.stop`, following `stopping`, `transcribing` and `completed` into the detail route; `MeetingDetailModel` loads `meetings.get` (segments with `speaker`, `speaker_id`, `polished_text`, the speakers with `talk_ms`, notes, analysis, diarization) and drives `meetings.notes.set`, `meetings.analyze`, `meetings.diarize`, `meetings.speakers.rename` and `suggest`, `transcripts.export` plus `transfer.pull` through the history spec's export path and directory variable, and `meetings.delete` with `[meetings] delete_artifact_policy`; `MeetingsActions` wraps `transcripts.import { target_kind: "meeting" }` with the job's progress. The C++ stays view glue; every host file under 500 lines. [inferred]
- QML under `qt/qml/Dettivo/app/meetings/` (each file under 300 lines): `MeetingsRoute.qml` (the week labels, `MeetingRow.qml` with when, title, summary, length, `SpeakerSwatches.qml`, `StateChip.qml`, the empty state from `states-and-hint-sheet.png`), `NewMeetingRail.qml` (source checkboxes with the device names from `audio.devices` and meters that show `audio.level` while a meeting runs, engine from `speech.providers.list` filtered to meeting-capable, expected speakers, analysis toggle, the disclosure state with its copy action, one primary button, the config section naming the keys it writes), `MeetingLive.qml` (`LiveHeader.qml` with the recording square, title, `SourceMeter.qml` twice, the elapsed display, Pause and Stop; `LiveTranscript.qml` rows with time, source label in speaker gold or olive, text at 1.7 line height, provisional in weight 300 and muted; the labelling-rule footer; `NotesEditor.qml` with a live caret; `AnalysisCard.qml`), `MeetingDetail.qml` (title and facts, `TalkTimeBar.qml`, `TabBar` with Transcript, Notes and Analysis, the Raw and Polished toggle, `SpeakerTranscript.qml` with names in their colours, `AnalysisRail.qml` with summary, decisions, action items, notes, export and audio facts, the urgent delete text), `SpeakerRenamePopover.qml` (name field with the suggestions list, Enter applies, Escape cancels, designed inline against the pattern), `ImportDialog.qml` (target kind, engine, language, diarize, analyse, the chunking sentence and the estimate from the file's duration) and `DisclosureDialog.qml` (the message verbatim, Copy message, Not now, Acknowledge and start, the asked-once sentence). Speaker colours are the design system's four speaker roles by `color_index`. [paraphrase]
- Keys: `n` new meeting, `Enter` opens, `j` and `k` move, `Escape` returns, `s` stops a live meeting after the urgent confirmation, `1`, `2`, `3` pick the detail tabs, `p` toggles Raw and Polished, `r` renames the focused speaker, `e` exports, `d` deletes with the urgent text confirmation, `i` imports; the footer hints name them. [inferred]
- History: the meeting rows in the history timeline get their chip and speaker count, the placeholder the history spec left. [paraphrase]
- Renders and drives: `dettivo-app --render <png> --sample --open meetings|meetings.live|meetings.detail` draws each screen with sample facts and no daemon, `DETTIVO_E2E_MEETING_STATE=list|list-empty|live|detail-transcript|detail-notes|detail-analysis|rename|import|disclosure` picks the state; manifest entries `meetings`, `meeting-live`, `meeting-detail` and `meeting-dialogs` cite the four artboards with crop rules for the crop script. Drives: `meetings_seeded` (opens `meetings` with the seed, asserts the week labels, the three rows with chips and swatches, opens the analysed meeting, asserts the talk-time bar, the tabs, the polished toggle, the analysis rail, renames a speaker through the popover and sees the transcript relabel, exports `md` to the QA export directory and matches the golden, deletes the notes-only meeting after the confirmation); `meetings_live_gui` (a fresh profile under `DETTIVO_E2E_DISCLOSURE=pending`, `n` opens the rail, Acknowledge and start goes through the dialog, `meetings.live` shows both meters and segments arriving provisional then final in source colours with the elapsed time, Stop lands on `meetings.detail` with the transcript; the mock fixtures in CI, the null sinks on this machine); `meetings_import_gui` (the jfk clip through the dialog to a meeting row with progress). Both drivers under Xvfb, the negative text scan over every tree, QA-13 Quick Tests in `tst_app_meetings.qml`. [paraphrase]

## API Contracts

<!-- API Contracts: 70% [paraphrase], 30% [inferred] -->

- No new daemon methods: the routes are clients of `meetings.*` (including the S-25 and S-26 additions), `transcripts.import|export`, `transfer.pull`, `job.progress`, `speech.providers.list`, `audio.devices`, `config.get|set` and the three event topics. A field a screen needs that the daemon does not return is registered as a Linux addition with a fixture in the spec's ADR, never read from the store directly (FR-U6). [inferred]
- QA variables (documented in `docs/app.md` and `docs/qa.md`): `DETTIVO_E2E_MEETING_STATE`, `DETTIVO_E2E_OPEN=meetings|meetings.live|meetings.detail` with `DETTIVO_E2E_ROUTE=<meeting-id>` for a detail; the history spec's export directory variable is reused. `dettivo app open meetings.detail --id <uuid>` and `dettivo app open meetings.live` route the running window. [inferred]
- Config the rail's section writes through `config.set`: `[meetings] system_audio = true`, `microphone = true`, `expected_speakers = 0`, `[meetings.diarization] enabled`, `[meetings.analysis] auto`, `[speech] meeting_model`, `[meetings] delete_artifact_policy`; the state file remembers `[app] last_meeting_tab`. [inferred]
- Accessible names for every element in `docs/qa/a11y-names.md` and `accessible-names.txt`. [paraphrase]

## Edge Cases & Constraints

- Pause: the contract has no pause verb; the button renders disabled with the reason in its accessible description, and the ADR records the gap for a later addition. [inferred]
- Engine slower than realtime: skipped windows show as a gap row; the footer sentence stays. [paraphrase]
- Stop while segments are still finalising: the live screen shows `Stopping` then `Transcribing` with the chunk progress before the detail opens. [paraphrase]
- A partial meeting in the list: the chip says partial and the row's actions are Recover and Discard through `meetings.recover|discard`. [inferred]
- Diarization not run or unavailable: the talk-time bar shows You and Remote by source and the analysis rail names the download. [inferred]
- Disclosure already acknowledged: the dialog never shows; the rail footer states the date and offers Copy message. [paraphrase]
- Daemon restart mid-live: the banner shows, the screen keeps the segments it has, and reconnect resumes the stream or lands on the partial meeting. [inferred]

## Acceptance Criteria

- **R1:** `meetings_seeded` passes on both drivers under Xvfb: week labels, the three seeded rows with chips and swatches, the detail's facts, talk-time bar, tabs, Raw and Polished toggle, analysis rail, the rename through the popover relabelling the transcript, the `md` export matching the golden, and the delete after the urgent confirmation; the negative text scan is clean. Errors: as stated. [paraphrase]
- **R2:** `meetings_live_gui` passes with the mock fixtures in CI and on the rig here: the disclosure dialog on a fresh profile, Acknowledge and start, both meters, provisional then final segments in source colours, the elapsed time, Stop through `Stopping` and `Transcribing` into the detail with the transcript. Errors: Not now leaves the list without starting; a refused start names the gate from the daemon's `CONFLICT`. [paraphrase]
- **R3:** `meetings_import_gui` imports the jfk clip through the dialog with the target kind, engine, language, diarize and analyse fields mapped onto `transcripts.import`, shows the chunking sentence and the estimate, tracks the job and opens the row. Errors: a rejected file names the daemon's reason. [paraphrase]
- **R4:** Visual gate: the manifest entries for the list (with and without rows), the live screen, the three detail tabs, the rename popover and the two dialogs render through `--sample` and pass the diff against the four artboards' crops and the approved renders on all five palettes (`black-gold`, `catppuccin-latte`, `tokyo-night`, `builtin-dark`, `builtin-light`) at 1x and 2x, with the speaker colours resolved from the theme tokens. Errors: a missing crop or fixture fails by name; a first approval is reported with the command, never passed. [paraphrase]
- **R5:** Keys and accessibility: `n`, `Enter`, `j`/`k`, `Escape`, `s`, `1`/`2`/`3`, `p`, `r`, `e`, `d`, `i` work as listed and the footer hints name them; every element has an accessible name (lint), the routes open by `DETTIVO_E2E_OPEN` and `dettivo app open`, and `tst_app_meetings.qml` covers each component's states; Pause is disabled with its reason. Errors: as stated. [paraphrase]
- **R6:** `docs/app.md` gains the meetings section (routes, keys, the state variable, the rail's config keys), `docs/qa/a11y-names.md` the names, the history chip is documented; an ADR records the live model's event handling, the pause gap and the inline-designed popover. Errors: as stated. [paraphrase]

## Boundaries

- No daemon behaviour: diarization, notes, analysis, export and delete semantics come from S-25 and S-26. [paraphrase]
- No bar panel Stop meeting or panel disclosure copy (S-21). [paraphrase]
- No settings routes (S-19). [paraphrase]
- No meeting templates. [paraphrase]

## Decision Context

### Motivation
<!-- scope: business -->

- Meetings are the second half of the product promise; the three screens on the approved baselines, proven by a live rig run and the seeded drives, are what turns the daemon's meeting lane into something a person uses. [paraphrase]

## Strategy Alignment

- **Omarchy-native, beautiful by default:** four approved baselines held on five palettes at two scales, speaker colours from the tokens, keyboard first. [strategy:Omarchy-native, beautiful by default]
- **Complete speech workflows, proven by drives:** the seeded, live and import drives walk start, live transcript, stop, rename, export and delete on both drivers. [strategy:Complete speech workflows, proven by drives]

## Requirement coverage

| R-ID | Task |
|---|---|
| R1 | fn-N.M (TBD) |
| R2 | fn-N.M (TBD) |
| R3 | fn-N.M (TBD) |
| R4 | fn-N.M (TBD) |
| R5 | fn-N.M (TBD) |
| R6 | fn-N.M (TBD) |
