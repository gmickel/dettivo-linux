# History UI: the list, detail, search, re-run and export

## Conversation Evidence

> user (turn 1): "your job is to read research and create a full masterplan/product vision document/prd that we can then use to break down into flow-next specs to get this developed as quickly as possible"
> user (turn 16): "ok go with the defaults, then do /flow-next:strategy"
> masterplan S-20 row: "`history-ui-list-detail-search-rerun-export` | phase 3 | depends on S-17, S-11 | FR-Y2 to Y5 in GUI; baseline `history.png` | Drive pack with seeded data; visual diff against the History baseline"
> masterplan History baseline: "Three columns: sidebar, list with search and kind filters, detail. Search is a focused input with the shell's 2 px focus border and a hit count; matches are highlighted in the list with a 12 % accent fill. Days are section labels, rows are time plus text plus a meta line; meetings appear in the same list with a chip and speaker count so history is one timeline, not two. Detail leads with Enhanced (the text that was inserted, with the model and preset that produced it), then Raw in weight 300 so the difference reads at a glance, then the audio strip with the played portion in accent. Facts are a two-column key-value grid, including insertion backend and stop-to-insert time, because those are the numbers the product promises. Keyboard hints sit in the footer; the destructive action is text in the urgent colour, never a red button."
> masterplan FR-Y3: "Re-run transcribes the retained audio with a chosen engine, model or mode and stores a new version linked to the original."
> masterplan coverage table: "Re-run dialog, export sheet, speaker rename popover | Small; designed inline during S-20 and S-27 against the pattern"

## Goal & Context

<!-- Goal & Context: 30% [user], 50% [paraphrase], 20% [strategy] -->

History becomes a place, not a list. The app's `history` and `history.detail` routes are built to `history.png`: the three columns, a search input with the hit count and highlighted matches, day section labels, rows with time, text and a meta line, and a detail that leads with the inserted text and its model and preset, then the raw text in a lighter weight, the audio strip with playback, and the facts grid with the insertion backend and the stop-to-insert time. Re-run, export and delete are the transcripts API the store already answers, surfaced through a re-run dialog and an export sheet designed inline against the pattern, and the whole route drives against the seeded data on both drivers with the visual diff against the baseline. [paraphrase]

## Architecture & Data Models

<!-- Architecture & Data Models: 55% [paraphrase], 45% [inferred] -->

- Host: `HistoryModel` grows paging through `transcripts.list` with `since`/`until`/`app_id` filters, day grouping, and `transcripts.search` with the FTS snippet and match ranges for highlighting; `HistoryDetailModel` loads `transcripts.get` (final text, raw text, segments and words, the insertion block with backend and the timings' stop-to-insert, model, mode, preset from the policy notice, app id, duration, audio path) and drives playback of the retained audio through a `QMediaPlayer` (Qt Multimedia) over the decoded WAV, exposing position for the played portion; actions map to `transcripts.rerun` (with the job's progress from `job.progress`), `transcripts.export` plus `transfer.pull` into a file chosen through the portal file chooser (`ashpd`-style D-Bus call from the host), and `transcripts.delete` after the urgent-text confirmation. [inferred]
- QML under `qt/qml/Dettivo/app/history/`: `HistoryRoute.qml` (the list column with `SearchField` showing the 2 px focus border and the hit count, kind filter chips, day labels, rows with tabular time, one line of text, the meta line, meeting rows with the chip and speaker count once meetings exist), `HistoryDetail.qml` (Enhanced first with model and preset, Raw in weight 300, `AudioStrip` with the played portion in accent, the two-column `FactsGrid`, footer hints, the delete text action), `RerunDialog.qml` (engine, model and mode pickers from the catalogue and the modes, progress from the job) and `ExportSheet.qml` (format choice, destination through the portal), both designed inline against the settings pattern and `states-and-hint-sheet.png` for their empty, loading and error states. `/` focuses search, `Escape` returns from detail to the list (the router already does), `Enter` opens a row, `j`/`k` move. [paraphrase]
- Highlighting: the search model returns match ranges per row; rows render them with the 12 percent accent fill through a `StyledText` span built by the module's HTML escaping helper. [paraphrase]
- Drives: `history_seeded` (opens `history` with `DETTIVO_E2E_SEED=1`, asserts the day labels and twelve rows, searches for a seeded word and asserts the hit count and highlighted row, opens a row and asserts the detail's Enhanced, Raw and facts including the backend, triggers a re-run through the dialog with the mock engine and waits for the linked item, exports JSON through the sheet to the profile's directory and reads it back, deletes an item after the confirmation), on both drivers under Xvfb; the visual diff against `history.png` crops (list, detail, facts) on the default and light themes. [paraphrase]

## API Contracts

<!-- API Contracts: 70% [paraphrase], 30% [inferred] -->

- No new daemon methods; the route is a client of `transcripts.list|get|search|rerun|export|delete`, `transfer.pull`, `job.progress`, `speech.providers.list`. If the search result needs match ranges the store does not return yet, `transcripts.search` gains `matches: [{ start, end }]` per item as a Linux addition with a fixture. [inferred]
- The portal file chooser through `org.freedesktop.portal.FileChooser` from the host; under QA the chooser is bypassed by `DETTIVO_E2E_EXPORT_DIR=<dir>` (a new QA variable, documented). [inferred]
- `dettivo app open history.detail --id <uuid>` opens a detail directly. [inferred]
- Accessible names for every element in `docs/qa/a11y-names.md`. [paraphrase]

## Edge Cases & Constraints

- No retained audio: the strip shows why (retention off or expired) and re-run is disabled with the reason. [inferred]
- A re-run in progress: the row shows a progress hairline and the detail the job's stage; a second re-run on the same item is refused with the daemon's `CONFLICT`. [inferred]
- Search with no hits: the designed empty state names the query; the list keeps the filters. [inferred]
- Thousands of items: the list pages through `transcripts.list` with a cursor and virtualises rows. [inferred]
- The daemon restarts mid-view: the list reloads from the store on reconnect. [inferred]

## Acceptance Criteria

- **R1:** `history_seeded` passes on both drivers under Xvfb: day labels, twelve rows with time and meta line, search with hit count and the highlighted match, the detail's Enhanced, Raw, audio strip and facts (backend and stop-to-insert time among them), the negative text scan clean. Errors: as stated. [paraphrase]
- **R2:** Re-run from the dialog with a chosen engine, model or mode produces a linked item shown in the list with progress on the way (mock engine in the drive, tiny.en in a daemon-backed test), and export through the sheet writes the chosen format to the QA export directory and matches the store's export goldens. Errors: re-run without audio is disabled with the reason; a refused export names the reason. [paraphrase]
- **R3:** Delete asks with the urgent text action and removes the item per the artifact policy; the list updates without a reload. Errors: as stated. [paraphrase]
- **R4:** The list and detail match `history.png` region by region on the default and light themes in CI (manifest entries, crops cut by the crop script), with the play-position accent and the weight-300 raw text as designed. Errors: a missing crop fails by name. [paraphrase]
- **R5:** Keyboard: `/` focuses search, `j`/`k` move, `Enter` opens, `Escape` returns to the list, the footer hints name them, and every element has an accessible name (lint). Errors: as stated. [paraphrase]
- **R6:** `docs/app.md` gains the history section (routes, keys, export directory variable, playback), the search match delta is registered if added, and an ADR records the playback and export decisions. Errors: as stated. [paraphrase]

## Boundaries

- No meeting rows beyond the chip placeholder until the meetings GUI (S-27); the timeline is one list ready for them. [paraphrase]
- No settings editors (S-19). [paraphrase]
- No speaker rename popover (S-27). [paraphrase]

## Decision Context

### Motivation
<!-- scope: business -->

- The history detail shows the numbers the product promises, the insertion backend and the stop-to-insert time, and re-run makes retained audio worth keeping. [paraphrase]

## Strategy Alignment

- **Omarchy-native, beautiful by default:** the History baseline on the shell's tokens, keyboard-first. [strategy:Omarchy-native, beautiful by default]
- **Complete speech workflows, proven by drives:** the seeded drive walks list, search, detail, re-run, export and delete on both drivers. [strategy:Complete speech workflows, proven by drives]

## Requirement coverage

| R-ID | Task |
|---|---|
| R1 | fn-N.M (TBD) |
| R2 | fn-N.M (TBD) |
| R3 | fn-N.M (TBD) |
| R4 | fn-N.M (TBD) |
| R5 | fn-N.M (TBD) |
| R6 | fn-N.M (TBD) |
