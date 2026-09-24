# Accessible names

Every control and component in the Dettivo module exposes an accessible role and a stable name, and the QA drives find elements by that name alone on both drivers. This page is the table a drive author reads: what each fixed name is and where it lives. `qt/qml/Dettivo/accessible-names.txt` is the machine list `scripts/lint-accessible-names.sh` checks the sources and this page against, so a name that changes is caught before a drive breaks.

## The app's routes

Each route's page carries its title as the accessible name of a pane and of its heading; `DETTIVO_E2E_OPEN` and `dettivo app open` name the route, the drive asserts the title ([docs/app.md](../app.md)).

| Route | Title |
|---|---|
| `home` | `Home` |
| `history` | `History` |
| `history.detail` | `Dictation` |
| `meetings` | `Meetings` |
| `meetings.live` | `Meeting live` |
| `meetings.detail` | `Meeting` |
| `settings`, `settings.<section>` | `Settings / General`, `Settings / Hotkeys`, `Settings / Models`, `Settings / Polish`, `Settings / Insertion`, `Settings / Meetings`, `Settings / Agents`, `Settings / Diagnostics` (the plain `Settings` opens General) |
| `onboarding` | `First run` |

## The window

| Name | Element | Where |
|---|---|---|
| `Dettivo` | The mark and the word in the sidebar header | `app/Sidebar.qml` |
| `Sidebar` | The sidebar pane | `app/Sidebar.qml` |
| `Home`, `History`, `Meetings`, `Settings`, `Agents` | The route buttons; `Accessible.selected` marks the open one | `app/SidebarItem.qml` |
| `Speech engine: <name>, <state>`, `Language model: <name>, <state>`, `Socket mode: socket, <mode>` | The three footer rows | `app/SidebarFooterRow.qml` |
| `Daemon state` | The banner pane over every page; hidden while the daemon answers | `app/DaemonBanner.qml` |
| `Daemon unavailable`, `Configuration needs attention` | The banner's sentence while the daemon is away or its file does not parse | `app/DaemonBanner.qml` |
| `Keyboard hints` | The hint line at the bottom of every page | `app/AppWindow.qml` |
| `Keyboard hint sheet`, `Global keys` | The sheet `?` opens over any page (its rows are `<key>: <what>`) and the line that defers the global keys to Hyprland | `app/HintSheet.qml` |
| `States` | The page that renders one designed state or the hint sheet under `DETTIVO_E2E_STATE` | `app/StatesPage.qml` |

## Home

| Name | Element | Where |
|---|---|---|
| `Ready.`, `Listening.`, `Transcribing.`, `Inserting.`, `Daemon unavailable.`, `Configuration needs attention.` | The status sentence (a heading) | `app/HomeRoute.qml` through `app/PageHeader.qml` |
| `Hold <hold> to dictate into <app>. <toggle> toggles.` | The line under the sentence, with the real chords and target | `app/HomeRoute.qml` |
| `Instrument strip` | The bordered strip | `app/InstrumentStrip.qml` |
| `Mode` | The label over the mode segment | `app/InstrumentStrip.qml` |
| `Dictation mode` | The mode segment (`Raw`, `Polish`, `Enhanced` are its tabs) | `app/InstrumentStrip.qml` |
| `Audio level` | The waveform | `components/Waveform.qml` |
| `Start dictation`, `Stop dictation`, `Start meeting`, `Return to meeting` | The primary actions | `app/InstrumentStrip.qml` |
| `Today` | The Today pane; its heading carries the newest day's label (`Today`, `Yesterday`, `Fri 13 Feb`) | `app/TodayList.qml` |
| `Today's dictations` | The list; rows are named by their title | `app/TodayList.qml` |
| `Engines and agents` | The rail pane | `app/RightRail.qml` |
| `Engines`, `Agents` | The rail headings | `app/SectionHeading.qml` |
| `<engine>: <state>`, `socket: <mode>`, `mcp: <hosts>`, `rest: <state>`, `last call: <call>` | The rail rows | `app/RailRow.qml` |

## History

The list column and the dictation detail ([docs/app.md](../app.md)); a drive reads a row by its text, a fact by `label: value`, and the dialogs by the names below.

| Name | Element | Where |
|---|---|---|
| `History list` | The list column pane; its heading is `History` with the item count beside it | `app/history/HistoryList.qml` |
| `Search`, `Search history` | The search pane and its field (`/` focuses it) | `components/SearchField.qml` |
| `3 hits`, `1 hit` | The hit count beside the query while a search is set, named by its own text | `components/SearchField.qml` |
| `Clear search` | The glyph that empties the field | `components/SearchField.qml` |
| `Kind filters` | The chips `All`, `Dictation`, `Meeting`, `Import`; `Accessible.checked` marks the chosen one | `app/history/FilterChips.qml` |
| `Dictations` | The rows; each is named by its text, its description is the meta line (`ghostty · Enhanced · 4 s`), and `Accessible.selected` marks the open one | `app/history/HistoryList.qml`, `app/history/HistoryRow.qml` |
| `Dictation detail` | The detail pane; its heading is `Dictation` with `Insert again`, `Re-run` and `Export` beside it | `app/history/HistoryDetail.qml` |
| `When` | `Fri 13 Feb · 16:00 · inserted into ghostty` under the heading | `app/history/HistoryDetail.qml` |
| `Action outcome` | The line under it that names a refused action's reason, or what an action did | `app/history/HistoryDetail.qml` |
| `Enhanced`, `Raw` | The two text blocks; each carries its text as a child | `app/history/DetailBlock.qml` |
| `Audio`, `Audio facts`, `Play`, `Pause`, `Take`, `Playback position` | The audio strip, its length and rate, the play control, the bars, and the clock (or why there is no take) | `app/history/AudioStrip.qml` |
| `Facts` | The facts grid; each cell is `label: value` (`app: ghostty`, `stop to insert: 0.9 s`, `insertion: mock`) | `app/history/FactsGrid.qml` |
| `Re-run progress` | The stage and percentage while a re-run works on the item | `app/history/HistoryDetail.qml` |
| `Delete` | The footer's text action that opens the confirmation | `app/history/HistoryDetail.qml` |
| `Re-run dialog`, `Re-run this dictation`, `Engine`, `Model`, `Mode`, `Re-run refused`, `Cancel`, `Start re-run` | The re-run dialog and its parts | `app/history/RerunDialog.qml` |
| `Export sheet`, `Export heading`, `Destination`, `Export outcome`, `Close`, `Write file` | The export sheet; the formats and scopes are radio buttons named by their text (`JSON`, `Markdown`, `Text`, `Archive with audio`; `This dictation`, `Everything`) | `app/history/ExportSheet.qml` |
| `Delete confirmation`, `Delete this dictation?`, `Delete refused`, `Keep it`, `Delete for good` | The confirmation and its parts | `app/history/DeleteConfirm.qml` |

## Scaffolds and states

| Name | Element | Where |
|---|---|---|
| The state sentence (`Nothing dictated yet.`, `No meetings recorded.`, ...) | A designed state; the reason and the key or button carry their own text | `app/StateView.qml` |
| `Configuration keys` | The line naming the `config.toml` keys a scaffold route writes | `app/RouteScaffold.qml` |
| `Dictations` | The History route's list when the store has rows | `app/HistoryRoute.qml` |
| `Tabs` | A styled tab bar without a name of its own | `DettivoStyle/TabBar.qml` |

## First run

The `onboarding` route is the whole window while it lasts ([docs/app.md](../app.md#first-run)); every step is a pane named by its title, and the drives read the facts below.

| Name | Element | Where |
|---|---|---|
| `First run` | The route pane | `app/OnboardingRoute.qml` |
| `Step <n> of 3` | The header with the mark and the step marker | `app/OnboardingRoute.qml` |
| `Keys`, `Models`, `Try it` | The step panes and their headings | `app/KeysStep.qml`, `app/ModelsStep.qml`, `app/TryItStep.qml` |
| `Bindings` | The bindings table; `Action`, `Binding`, `Runs` are its column labels | `app/KeysStep.qml` |
| `<action>: <chord>` | One binding row; its key caps carry the key names, `plus` joins them, the hint and the command carry their own text | `app/BindingRow.qml` |
| `Bindings file` | The box with the snippet path and the snippet; the snippet text is the accessible name of the text inside (`Snippet` while empty) | `app/KeysStep.qml` |
| `Portal bindings` | The same box on a desktop without a snippet, carrying the portal's outcome | `app/KeysStep.qml` |
| `Key check` | The panel that says `Press F9 now` and then `F9 held · listening · <input>` | `app/KeysStep.qml` |
| `Speech`, `Speech models` | The label and the radio list of models; a row is a radio button named by the model (`Parakeet TDT 0.6B v3`), its size and state carry their text; `Recommended` is the chip | `app/ModelsStep.qml`, `app/ModelRow.qml` |
| `Download progress` | The accent hairline under the list | `app/ModelsStep.qml` |
| `Enhanced · optional`, `Enhanced models` | The label and the check list: `Qwen3 4B Instruct, local` (`Local language model` until the daemon names it) and `Use Ollama instead` | `app/ModelsStep.qml` |
| `Hold <hold>, say a sentence, let go.` | The line under the Try it title; the key is drawn as a cap and the line reads as one sentence | `app/KeySentence.qml` |
| `Try it panel` | The bordered panel with the level bars, the state label (`Listening · Parakeet v3`) and the field | `app/TryItStep.qml` |
| `Try it field` | The text area the dictation lands in; its value is the text | `app/TryItStep.qml` |
| `inserted via: <backend>`, `stop to insert: <seconds>`, `mode: <mode>` | The three facts under the field | `app/TryItStep.qml` |
| `First run actions` | The footer of every step; `Skip`, `Continue`, `Continue when ready`, `Back`, `Done` and `Open config.toml` are its buttons | `app/FirstRunFooter.qml` |

## Meetings

The list by week beside the new-meeting rail, the live meeting and the meeting detail ([docs/app.md](../app.md#meetings)); a drive reads a row by its title, a transcript row by `<speaker>: <text>`, a speaker by `Rename <name>`, and the dialogs by the names below.

| Name | Element | Where |
|---|---|---|
| `Meetings`, `Meeting live`, `Meeting` | The route pane, named by the screen it shows | `app/meetings/MeetingsRoute.qml` |
| `Meetings keys` | The pane holding the route's shortcuts | `app/meetings/MeetingsKeys.qml` |
| `Meetings page`, `Meetings column`, `Meetings footer` | The list page, its column and the footer with the keys (`Keyboard hints`) and a fact on the right | `app/meetings/MeetingsListPage.qml`, `MeetingsList.qml`, `MeetingsFooter.qml` |
| `Meetings search`, `Search meetings`, `Clear meetings search`, `3 hits` | The search pane, its field, the clear glyph and the hit count while a query is set | `components/SearchField.qml` |
| `Import audio` | The button over the list that opens the import dialog, and the dialog's own heading | `app/meetings/MeetingsList.qml`, `ImportDialog.qml` |
| `Columns` | The column labels `When`, `Title`, `Length`, `Speakers`, `State` | `app/meetings/MeetingsList.qml` |
| `This week`, `Last week`, `Week of 25 Aug` | The week labels between the rows | `app/meetings/MeetingsList.qml` |
| `Meetings list` | Rows are named by title, with summary and length in their description. `Accessible.selected` marks the open row. `Recover` appears for validated retained input; `Discard` remains partial-only; `Cancel transcription` cancels a transcribing meeting or import retry. | `app/meetings/MeetingsList.qml`, `MeetingRow.qml` |
| `Meetings table` | The compact table viewport. Tab focuses it; Left/Right scroll by one spacing step and Home/End reach the first/last columns. The focus ring stays on the viewport while its columns scroll. | `app/meetings/MeetingsList.qml` |
| `Analysed`, `Partial`, `Notes only`, `Transcript`, `Recording`, `Transcribing`, `Failed`, `Empty` | A row's state chip, named by its text | `app/meetings/StateChip.qml` |
| `You, Mara, Tobias`, `2 speakers` | A row's speaker swatches, named by the names they carry (each square is named by its speaker); the count alone until the swatches are read | `app/meetings/SpeakerSwatches.qml` |
| `Meetings notice` | The refusal of the last start under the column labels | `app/meetings/MeetingsList.qml` |
| `New meeting rail`, `New meeting` | The rail and its heading | `app/meetings/NewMeetingRail.qml` |
| `%1 source` (`Microphone source`, `System audio source`), `Microphone`, `System audio`, `%1 level` (`Microphone level`, `System audio level`) | A source row, its square (a check box; the microphone's is fixed on), and its level bars | `app/meetings/SourceRow.qml`, `LevelBars.qml` |
| `Level` | The level bars of a source, when no source names them | `app/meetings/LevelBars.qml` |
| `Meeting engine`, `Expected speakers`, `Analysis after stop`, `Disclosure state` | The rail's fact rows (buttons; the description is the value): the engine menu (one item per provider and model), the speaker count (`Expected speakers field` while it is edited), the analysis toggle, the disclosure with its copy action | `app/meetings/NewMeetingRail.qml`, `RailFactRow.qml` |
| `Start meeting`, `Return to meeting` | The rail's primary action, while no meeting runs and while one does | `app/meetings/NewMeetingRail.qml` |
| `Configuration keys` | The footer naming `[meetings]` in `config.toml` with the link to the settings route; its description lists the keys | `app/meetings/NewMeetingRail.qml` |
| `Live meeting`, `Live header`, `Recording`, `Stopping` | The live pane, its header and the recording square's state | `app/meetings/MeetingLive.qml`, `LiveHeader.qml` |
| `Recording stopped`, `Finalisation stage`, `Finalisation progress`, `Live notice` | Once Stop is accepted: the bold acknowledgement, the stage line (its description reads `Transcribing · 18 of 22 chunks`), the progress bar (a sweep until the chunks are counted), and the daemon's refusal of a stop in the urgent colour | `app/meetings/LiveHeader.qml` |
| `%1 meter` (`Mic meter`, `System meter`), `%1 level` (`Mic level`, `System level`) | The two source meters in the header and their bars; the device or app name carries its own text | `app/meetings/SourceMeter.qml` |
| `Elapsed` | The elapsed display; its description is the time (`00:23:41`) | `app/meetings/LiveHeader.qml` |
| `Pause`, `Stop` | The header's buttons; Pause is disabled with the reason as its description | `app/meetings/LiveHeader.qml` |
| `Transcript`, `Live transcript` | The transcript pane and its list; a row is `<source>: <text>` with the time as its description, a provisional row carries the `provisional` mark (a tilde) beside its time, and `gap 1.5 s` marks a capture gap | `app/meetings/LiveTranscript.qml`, `TranscriptRow.qml` |
| `Notes`, `Notes editor` | The notes pane (its label carries `Markdown · Saved`) and the editor | `app/meetings/NotesEditor.qml` |
| `Analysis card` | The card that says when the analysis runs | `app/meetings/AnalysisCard.qml` |
| `Disclosure state`, `Copy message` | The live rail's footer and its action | `app/meetings/MeetingLive.qml` |
| `Stop confirmation`, `Stop this meeting?`, `Stop refused`, `Keep recording`, `Stop meeting` | The confirmation and its parts | `app/meetings/StopConfirm.qml` |
| `Meeting detail`, `Meeting header`, `Facts`, `Action outcome` | The detail pane, its header, the facts line (its description is the text) and the outcome of the last action | `app/meetings/MeetingDetail.qml`, `DetailHeader.qml` |
| `Re-run`, `Export` | The header's buttons; Re-run is disabled with the reason as its description | `app/meetings/DetailHeader.qml` |
| `Rename meeting`, `Meeting title`, `Rename refused` | The title as the rename control (a button whose description is the title; a click, Return or `t` opens it), the field it turns into (Return saves, Escape restores the saved name) and the daemon's refusal under it | `app/meetings/DetailHeader.qml` |
| `Processing strip`, `Processing`, `Transcript`, `Speakers`, `Analysis`, `<stage> state`, `Processing progress` | The strip under the header while a pass is pending, queued or running or one failed: the sentence (its description is the text), one dot and label per stage (the dot reads `<stage>: running`), each stage's state line (its description reads `queued · after the speakers`) and the bar under the running stage | `app/meetings/ProcessingStrip.qml` |
| `Detail followers` | The pane that follows the actions and the detail for the page; no control | `app/meetings/DetailFollowers.qml` |
| `Speakers`, `Talk time`, `Rename %1` (`Rename <name>`) | The talk-time bar, its slices (`<name>: 58 %`) and the legend entries that open the rename popover; the hint on the right carries its own text | `app/meetings/TalkTimeBar.qml` |
| `Detail tabs`, `Meeting tabs`, `Transcript`, `Notes`, `Analysis`, `Raw`, `Polished` | The tab row, the tab list and its tabs, and the two radio buttons of the toggle | `app/meetings/DetailTabs.qml` |
| `Tab pages`, `Transcript tab`, `Meeting transcript`, `Analysis tab` | The tab pages, the transcript tab and its list (rows as `<speaker>: <text>`), the analysis tab with `Analyse now`, `Analyse again`, `Run speakers again` and `Analysis progress` | `app/meetings/StackLayoutLite.qml`, `SpeakerTranscript.qml`, `AnalysisView.qml` |
| `Analysis rail`, `Analysis`, `Summary`, `Decisions`, `Action items` | The rail, its heading and its blocks; a decision or an action item is a list item named by its text | `app/meetings/AnalysisRail.qml`, `AnalysisBlock.qml` |
| `notes: <facts>`, `exports: <formats>`, `audio: <facts>` | The rail's foot rows | `app/meetings/AnalysisRail.qml` through `app/RailRow.qml` |
| `Delete meeting` | The urgent text in the detail's footer | `app/meetings/MeetingDetail.qml` |
| `Rename speaker`, `Rename speaker heading`, `Speaker name`, `Remembered`, `Use %1` (`Use <name>`), `Rename refused`, `Cancel`, `Apply name` | The rename popover and its parts | `app/meetings/SpeakerRenamePopover.qml` |
| `Meeting export sheet`, `Export meeting`, `Export destination`, `Export outcome`, `Close`, `Write file` | The export sheet; the formats are radio buttons named by their text and the raw switch is a check box named by its text | `app/meetings/MeetingExportSheet.qml` |
| `Delete meeting confirmation`, `Delete this meeting?`, `Delete refused`, `Keep it`, `Delete meeting for good` | The confirmation and its parts | `app/meetings/MeetingDeleteConfirm.qml` |
| `Import dialog`, `Import audio`, `Import file`, `File facts`, `File check`, `File facts row`, `Import engine`, `Import language`, `Import estimate`, `Import refused: %1` (the daemon's reason), `Import actions` with `Cancel` and `Import` | The import dialog and its parts: the path field, the file line once a file is known (a button that reopens the field), the check under the field, the two combo boxes, the check boxes `diarize after transcription` and `summary, decisions, action items`, the chunking sentence with the estimate, the daemon's refusal | `app/meetings/ImportDialog.qml`, `ImportField.qml` |
| `Recording disclosure`, `Recording disclosure heading`, `Disclosure message`, `Copy message`, `Not now`, `Acknowledge and start` | The disclosure dialog and its parts; the message's description is the text | `app/meetings/DisclosureDialog.qml` |

## Settings

Every section is an editor over `config.toml` ([docs/app.md](../app.md#settings)); a drive finds a control by the key it writes, reads the TOML block under the page, and opens the file through the column's action. The names that carry a key or a model id are the templates `%1 row`, `%1 field`, `%1 switch`, `%1 choice`, `%1 refused`, `%1 state: %2`, `Download %1`, `Cancel %1`, `Delete %1` and `Write %1 config`, filled in with the key, the model id or the host.

| Name | Element | Where |
|---|---|---|
| `Settings navigation`, `Settings sections`, `Sections` | The section column, its heading and the tab list; each tab is named by its section (`General`, `Hotkeys`, ...) and `Accessible.selected` marks the open one. On Agents the column reads `Agents` with `Overview`, `MCP hosts`, `REST shim` and `CLI` | `app/settings/SettingsNav.qml` |
| `Configuration file`, `Open config.toml` | The file's path at the foot of the column and the action that opens it in the desktop's editor | `app/settings/SettingsNav.qml` |
| `Settings / <Section>` | The page pane and its heading; the line under it is the section's sentence, or the notice after the file changed on disk | `app/settings/SettingsPage.qml` |
| `Keys this route writes`, `Configuration keys` | The block at the foot of every section and the TOML text inside it, the keys with the values in force | `app/settings/SettingsWrites.qml` |
| `<key> row` | One setting: the label and `<hint> · <key>` carry their text, `env · <VARIABLE>` is the chip when the environment set the value, `<key> refused` is the daemon's message under the control after a refused write | `app/settings/SettingRow.qml` |
| `<key> field`, `<key>` | A text, number, list or table key and its field, named by the key; `Changed on disk` is the line while the field holds an edit the file moved under | `app/settings/SettingField.qml` |
| `<key> switch`, `<key>` | A boolean key and its switch | `app/settings/SettingSwitch.qml` |
| `<key> choice`, `<key>` | A key with a fixed set of values: the segments (tabs named by their labels) or the combo box, named by the key | `app/settings/SettingChoice.qml` |
| `<action>: <caps>` | A Hotkeys binding row (`Hold to talk: F9`); the caps carry the key names, `plus` joins them, `<key>` is the field on the right, `<key> refused` the message under it | `app/settings/HotkeyRow.qml` |
| `Bindings file`, `Snippet`, `Portal bindings`, `Snippet state` | The snippet box and its text, the same box on a desktop without a snippet, and the line beside `Rewrite snippet` and `Copy include line` | `app/settings/HotkeysSection.qml` |
| `Models table` | The list of models; a row is named by the model, `<id> state: <state>` is its state box, `Download <id>`, `Cancel <id>` and `Delete <id>` its actions; `Model action refused` is the daemon's reason under the table; `Engine keys` groups the rows below it | `app/settings/ModelsSection.qml`, `app/settings/ModelTableRow.qml`, `app/settings/ModelsKeys.qml` |
| `Socket`, `REST shim` | The two fact panels of Agents; a fact is `<label>: <value>` | `app/settings/FactPanel.qml` |
| `MCP hosts` | The host rows; a row is named by the host, its file carries its path, `<host> state: <state>` is the state box and `Write <host> config` the action; `Host write refused` is the command's error, `Host entry` the JSON a host receives, `MCP tools` the tool count line, `Agent keys` the rows below | `app/settings/AgentsSection.qml`, `app/settings/AgentHostRow.qml`, `app/settings/AgentsKeys.qml` |
| `Doctor report` | The report box of Diagnostics; every line is its own text, `Run again` and `Copy report` are the actions | `app/settings/DiagnosticsSection.qml` |
| `Disclosure message` | The recording disclosure on Meetings, with `Copy message` beside it; `Speaker keys` groups the speaker pass rows above it and `Analysis keys` the analysis and delete-policy rows below them | `app/settings/MeetingsSection.qml`, `app/settings/MeetingsSpeakerKeys.qml`, `app/settings/MeetingsAnalysisKeys.qml` |

## The pill and the shared components

| Name | Element |
|---|---|
| `Audio level` | `Waveform` |
| `Input level` | `LevelMeter` |
| `status`, `Status dot` | `StatusDot` without a label |
| `Remove` | The close glyph of a closable `Chip` |
| `Menu`, `Popup`, `Progress`, `Tabs`, `Horizontal scroll bar`, `Vertical scroll bar` | The styled controls without a text of their own |
| `Scrollable content` | The focus-revealing viewport in settings and first run |
| `Enhancing`, `Inserted`, `Copied` | The pill's states ([docs/osd.md](../osd.md)); `Listening`, `Transcribing` and the error title are its texts |

## The Omarchy bar and panel

The plugin's surfaces ([docs/omarchy.md](../omarchy.md)); `dettivo-bar` renders them outside the shell for the visual job, and Quickshell exposes no accessibility tree, so the `omarchy_bar` drive asserts state through the CLI and pixels through the baseline crop.

| Name | Element | Where |
|---|---|---|
| `Dettivo idle`, `Dettivo listening`, `Dettivo transcribing`, `Meeting <elapsed>`, `Meeting recording`, `Dettivo error`, `Dettivo not ready` | The bar glyph's sentence; `Accessible.description` carries the state | `components/BarGlyph.qml` |
| `Dettivo panel` | The panel pane; `Accessible.description` carries the state | `components/BarPanel.qml` |
| `Ready.`, `Listening · release to insert`, `Transcribing`, `Inserted into <app>`, `Meeting recording · <elapsed>`, `Daemon unavailable.`, `Install Dettivo`, `Upgrade Dettivo` | The header's sentence; `Dettivo` is the heading while the daemon answers, the sentence is the heading in the hint and unavailable states | `components/BarPanelHeader.qml` |
| `REC` | The chip beside the header while a take or a meeting runs | `components/BarPanelHeader.qml` |
| `Dictation mode` | The mode segment (`Raw`, `Polish`, `Enhanced` are its tabs) | `components/BarPanel.qml` |
| `Dictate`, `Stop`, `Cancel`, `Start meeting`, `Stop meeting` | The two action buttons, by state | `components/BarPanel.qml` |
| `Facts` | The three fact rows; each row is `Engine: <value>`, `Enhanced: <value>`, `Insert into: <value>` | `components/BarPanelFacts.qml` |
| `Recent` | The heading over the last items | `components/BarPanelRecent.qml` |
| `Recent dictations` | The list; rows are named by their title, `Nothing dictated yet.` when empty | `components/BarPanelRecent.qml` |
| `Open Dettivo`, `Open install guide` | The footer button, by state | `components/BarPanel.qml` |
| `Shortcut` | The chord label beside Open Dettivo; `Accessible.description` carries the chord | `components/BarPanel.qml` |

## Release settings controls

| Name | Meaning |
|---|---|
| `Model selections` | The meeting and language model selection group. |
| `Reset %1 to default` | Reset a named setting using the daemon default. |
| `Show advanced settings` | Switch all settings pages between Simple and Advanced. |
| `Vocabulary term` | The term or phrase to add to vocabulary. |
| `Remove vocabulary term %1` | Remove the named vocabulary entry. |
