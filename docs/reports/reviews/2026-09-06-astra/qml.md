# The QML surfaces, the style module and the theme

## Verdict

The shared theme, component module and thin surface hosts are a sound foundation, but the UI is not ready to call complete. The most valuable change is to replace the hand-built interactive rectangles and text with controls drawn by DettivoStyle, preserving Qt’s keyboard, focus and accessibility behavior.

**Verification:** Read-only review of `28b4b7d8`; no files changed. Token, accessible-name and icon lints passed, and all files in scope meet their line limits. Isolated, in-memory Qt probes confirmed the binding, accessibility-action and commit-suppression defects below. The full `just build test lint` gate and current application renders were not run; the existing build lacks the current style module and QML test executable.

## Findings

### F1. Selecting a segment disconnects it from the daemon’s value

- **Kind:** bug
- **Where:** `qt/qml/Dettivo/components/SegmentedControl.qml:71` — `root.currentIndex = segment.index;`. `qt/qml/Dettivo/app/settings/SettingChoice.qml:44` — `currentIndex: root.currentIndex`.
- **Why:** The click assigns over the caller’s binding. Subsequent configuration changes stop updating that selector; a rejected write can also leave it displaying an uncommitted value. The isolated probe selected index 1, changed the external value to 2, and still displayed 1. This follows [Qt’s binding-removal semantics](https://doc.qt.io/qt-6/qtqml-syntax-propertybinding.html).
- **Change:** Delete the internal assignment. Emit the selection request and let the owning model supply the displayed value.
- **Risk:** Standalone callers must own their selection explicitly. Test accepted writes, rejected writes and external changes after the first click on Home, Settings and the bar panel.

### F2. Accessibility actions bypass the behavior that makes controls work

- **Kind:** bug
- **Where:** `qt/qml/DettivoStyle/Switch.qml:26` — `Accessible.onToggleAction: control.toggle()`. `qt/qml/Dettivo/app/settings/SettingSwitch.qml:23` — `onClicked: root.toggled(control.checked)`. `qt/qml/DettivoStyle/TabButton.qml:23` — `Accessible.onPressAction: control.clicked()`.
- **Why:** Toggling the switch changes its check state without firing the handler that writes the setting. Emitting a tab’s `clicked` signal does not select it. Both behaviors reproduced in an isolated Qt probe. The existing style test calls `toggle()` and checks appearance, so it misses the absent application action.
- **Change:** Remove redundant accessibility overrides where Qt already supplies the behavior; otherwise invoke the actual activation method. Qt 6.8, the repository’s floor, provides [AbstractButton.click()](https://doc.qt.io/qt-6/qml-qtquick-controls-abstractbutton.html#click-method).
- **Risk:** Avoid double activation. Test accessibility actions through to one configuration write or one tab change, rather than checking only `checked`.

### F3. Hand-built controls leave essential actions outside keyboard navigation

- **Kind:** simplify
- **Where:** `qt/qml/Dettivo/app/SidebarItem.qml:66` — `TapHandler { … root.activated() }`. `qt/qml/Dettivo/components/SegmentedControl.qml:76` — `Accessible.role: Accessible.PageTab`. `qt/qml/Dettivo/app/meetings/SourceRow.qml:35` — `TapHandler`.
- **Why:** These are rectangles or items with pointer handlers and accessible labels, but no tab-focus or keyboard activation implementation. Segments additionally lack an accessible activation handler. Similar controls independently redraw selection, borders and hover throughout the surfaces. Declaring a role does not supply control behavior.
- **Change:** Use styled buttons, radio buttons, check boxes and delegates underneath these compositions. Keep the surface-specific labels and layout; delete their duplicate interaction and state drawing.
- **Risk:** Preserve the approved appearance and accessible names. Traverse onboarding, navigation, mode selection and meeting sources using only Tab, arrows, Space and Enter.

### F4. Application renders bypass the negative style check entirely

- **Kind:** test
- **Where:** `qt/apps/dettivo-app/app_main.cpp:389` — `const QImage image = window->grabWindow();`, followed by saving and quitting. `qt/host/render.cpp:38` — `styleFindings(window, fontFamily)`.
- **Why:** The app implements a separate screenshot path that never calls the shared checker or plants its negative-test controls. Consequently, the app surfaces in the visual manifest can pass or receive approval without the style enforcement promised by ADR 0021.
- **Change:** Delete the duplicate rendering path and use the shared render-and-check implementation. Register the planted-control and planted-font checks for the app.
- **Risk:** Previously hidden violations will begin failing, including surface-owned control backgrounds. Fix those violations rather than exempting the app.

### F5. Starting a meeting overrides the configuration defaults the rail claims to follow

- **Kind:** contract
- **Where:** `qt/qml/Dettivo/app/meetings/NewMeetingRail.qml:22` — `property bool analyze: true`; line 59 passes `root.analyze, true` to `start`. Line 226 says `config.toml sets these defaults`. `qt/host/app/meeting_live_model.cpp:89` sends explicit `analyze` and `diarize` values.
- **Why:** The rail never initializes those choices from configuration. A user who disables automatic analysis or automatic diarization still gets explicit requests for them from this screen. The daemon treats these as meeting-specific overrides.
- **Change:** Read the effective configuration into the rail and preserve explicit user changes separately, or omit untouched options so the daemon applies its defaults.
- **Risk:** Test both automatic settings disabled, individual overrides enabled, and configuration changes while the rail is open. Verify the resulting request and post-meeting jobs.

### F6. Transcript text can be interpreted as markup instead of displayed faithfully

- **Kind:** bug
- **Where:** `qt/qml/Dettivo/app/history/DetailBlock.qml:86` — `text: root.body`. `qt/qml/Dettivo/app/meetings/TranscriptRow.qml:143` — `text: root.text`. Neither sets `textFormat`.
- **Why:** Qt Text defaults to automatic format detection. A transcript containing recognized markup, such as `<b>example</b>`, can lose its literal tags and change presentation. This matters particularly for a speech workstation used to write code. See [Qt’s text-format behavior](https://doc.qt.io/qt-6/qml-qtquick-text.html#textFormat-prop).
- **Change:** Set `Text.PlainText` for verbatim transcript, title, speaker and analysis strings. Reserve rich text for explicitly assembled, escaped content.
- **Risk:** Audit intentional rich-text labels separately. Check literal tags, ampersands, multiline code and search highlighting.

### F7. A settings field can silently suppress a later valid edit

- **Kind:** bug
- **Where:** `qt/qml/Dettivo/app/settings/SettingField.qml:24` — `property string committedText: ""`; line 49 requires `field.text !== root.committedText` before committing on blur.
- **Why:** The suppression value survives indefinitely. After committing B, receiving B, and later receiving A from elsewhere, editing back to B and leaving the field emits no write. The isolated probe produced only one commit across that sequence.
- **Change:** Limit duplicate suppression to the Enter/blur pair for one editing session. Clear it on a new session and when the authoritative value changes.
- **Risk:** Preserve the useful protection against duplicate writes. Test Enter followed by blur, failed-write retries, and A→B→external A→B.

### F8. Popup opacity was fixed locally instead of in the style

- **Kind:** bug
- **Where:** `qt/qml/DettivoStyle/Popup.qml:38` and `qt/qml/DettivoStyle/Menu.qml:57` — `color: Theme.roleRaisedSurface`. `qt/qml/Dettivo/Theme.qml:93` defines that role as an alpha wash. `qt/qml/Dettivo/app/meetings/DialogFrame.qml:13` supplies the local fix with `Qt.tint(Theme.roleSurface, Theme.roleRaisedSurface)`.
- **Why:** The default popup and menu backgrounds remain translucent, allowing underlying content to compete with their text. ADR 0038 records this exact problem, but its opaque frame is applied only to meetings dialogs. History dialogs and combo-box popups retain the shared defect.
- **Change:** Compose an opaque floating surface inside DettivoStyle. Delete the meetings-only frame and its repeated overrides once the shared style supplies it.
- **Risk:** Inline raised surfaces should retain their washes. Check menus, dropdowns and every dialog over dense content in dark and light palettes.

### F9. The permitted minimum window size makes routes geometrically impossible

- **Kind:** bug
- **Where:** `qt/qml/Dettivo/app/AppWindow.qml:58` — `minimumWidth: Theme.sidebarWidth * 3`. `qt/qml/Dettivo/app/history/HistoryRoute.qml:96` fixes the list width, while lines 120–122 anchor the detail into the remaining space.
- **Why:** With default tokens, the window permits 600 px. The sidebar consumes 200 and History’s list consumes 400, leaving negative space after the separator for the detail. Settings and meetings also retain fixed columns without a narrow-window arrangement.
- **Change:** Enforce a workable minimum derived from the visible columns now. Any supported smaller layout needs explicit collapsing or stacking, rather than shrinking the existing arrangement.
- **Risk:** A minimum larger than the available desktop is also unusable. Check the smallest supported display and window size, larger theme fonts, and both scale factors.

### F10. Keyboard rename targets a different speaker from the selected transcript row

- **Kind:** bug
- **Where:** `qt/qml/Dettivo/app/meetings/SpeakerTranscript.qml:25` changes only `focusRow`. `qt/qml/Dettivo/app/meetings/MeetingDetail.qml:43` renames `speakerAt(root.speakerIndex)`.
- **Why:** Moving with `j` and `k` updates the highlighted transcript row but never updates `speakerIndex`. Pressing `r` therefore renames the initial speaker or the last speaker clicked, rather than the speaker on the selected row. The supplied `currentSpeakerId` property is not used by the transcript component.
- **Change:** Derive the rename target from the selected segment’s speaker ID. Delete the disconnected selection state or synchronize it through one explicit selection signal.
- **Risk:** Test alternating speakers, unlabeled segments and a transcript updated after diarization. Confirm the rename request contains the selected row’s speaker ID.

### F11. The visual comparator cannot enforce the theme contract

- **Kind:** test
- **Where:** `qt/tools/visual-diff/main.cpp:37` — `constexpr int kRows = 18`; line 87 scales with `Qt::IgnoreAspectRatio`. `qa/visual/manifest.toml:22` sets `threshold = 0.55`.
- **Why:** The comparator reduces pictures to coarse contrast and saturation grids, normalizes away information and tolerates shifts. ADR 0021 explicitly records that a colour-only regression passes and that a font-size increase passed the approved-render comparisons. That is inadequate for enforcing per-theme colour, typography and geometry.
- **Change:** Retain the structural score for comparisons with HTML artboards. Compare approved QML renders against matching theme-and-scale renders using a stricter, colour-sensitive check that preserves dimensions.
- **Risk:** Control font and rendering variation before tightening the gate. Use separate canaries for colour, typography, geometry and missing controls.

### F12. The manifest does not cover every surface on five palettes at two scales

- **Kind:** test
- **Where:** `qa/visual/manifest.toml:153` lists only four first-run themes; line 206 still says `Home joins here with the settings spec`; lines 279–288 use General to represent five settings sections.
- **Why:** The manifest expands to 492 entries but contains no Home surface. Its separate Home script checks only two themes at 1x. Several dialogs, non-General settings sections, and loading/error states have no corresponding manifest entries. A representative settings page cannot catch section-specific layout failures.
- **Change:** Move Home into the manifest and delete its separate matrix runner. Restore Tokyo Night for first run and add the missing concrete routes and states, including export, re-run and confirmation dialogs.
- **Risk:** New entries may expose missing approvals. Preserve first-approval failures and obtain actual approval rather than accepting generated references automatically.

### F13. The analysis rail duplicates the full tab and overflows with ordinary long results

- **Kind:** simplify
- **Where:** `qt/qml/Dettivo/app/meetings/AnalysisRail.qml:19` uses an unbounded `Column`; lines 54–72 render the complete summary, decisions and action items. `qt/qml/Dettivo/app/meetings/AnalysisView.qml:18` already provides a scrolling view of that content.
- **Why:** The rail has neither scrolling nor a content bound above its bottom-anchored facts. Longer results overlap those facts and continue outside the window. Maintaining two complete presentations also duplicates state handling.
- **Change:** Prefer a bounded summary and an action opening the full Analysis tab; delete the repeated full lists from the rail. This changes the approved composition and needs design approval. Preserving the composition requires a bounded scrolling region.
- **Risk:** Keep analysis status and actions visible. Test long summaries, many action items and larger fonts.

### F14. First run calls unsuccessful insertion “Inserted”

- **Kind:** bug
- **Where:** `qt/qml/Dettivo/app/TryItStep.qml:31` — `return root.known ? qsTr("Inserted") : qsTr("Idle");`. `qt/host/app/first_run_model.cpp:313` sets `m_resultKnown = true` for all insertion outcomes. `TryItStep.qml:225` also asserts that the words are on the clipboard whenever a reason exists.
- **Why:** “A result exists” is not “insertion succeeded.” Clipboard fallback and unsuccessful insertion receive the success label, and a reason alone does not prove clipboard success.
- **Change:** Expose the actual outcome and derive the label, explanation and clipboard claim from it.
- **Risk:** Exercise inserted, copied-to-clipboard and failed outcomes. Each should show consistent status text and recovery instructions.

### F15. The shared style checker examines only one of a control’s delegates

- **Kind:** test
- **Where:** `qt/host/style_check.cpp:38` iterates `{"background", "contentItem"}`, but line 41 immediately executes `return url`.
- **Why:** A valid style background ends the search. A caller can replace the content item and still pass, despite ADR 0021 promising to reject either delegate created outside the style. The stock-control canary does not exercise this mixed case.
- **Change:** Check both delegates independently. Add negative cases that replace only the background and only the content item.
- **Risk:** Distinguish legitimate application content from a control’s drawing delegates. Report the offending delegate explicitly so fixes do not become broad exemptions.

### F16. Home’s “Start meeting” is a permanently dead action

- **Kind:** bug
- **Where:** `qt/qml/Dettivo/app/InstrumentStrip.qml:124` — the button has `enabled: false`, `text: qsTr("Start meeting")`, and no action handler.
- **Why:** Meetings are implemented, but Home still contains the disabled placeholder from an earlier slice. Nothing about daemon readiness or meeting capability can enable it.
- **Change:** Route the action into the existing meeting setup/start flow. Reuse its disclosure and configuration handling.
- **Risk:** Check connected, disconnected and already-recording states, plus first-use disclosure. Avoid implementing a second start workflow in Home.

### F17. Delete the import target choice that cannot be submitted

- **Kind:** delete
- **Where:** `qt/qml/Dettivo/app/meetings/ImportDialog.qml:187` offers `["Dictation", "Meeting"]`. `qt/qml/Dettivo/app/meetings/ImportFooter.qml:47` permits submission only with `root.ready && root.meeting`.
- **Why:** Choosing Dictation leads to a disabled Import button. Its explanation sends the user to History, whose implemented screen has no corresponding import action. The backend adapter for this dialog imports meetings.
- **Change:** Delete the unsupported target selector and its `meeting` branches; present this as meeting import. A future dictation import needs a complete submission path.
- **Risk:** Re-approve the simpler dialog and update its keyboard/accessibility expectations. Preserve the working meeting import options.

### F18. History and Meetings carry the same search control twice

- **Kind:** simplify
- **Where:** `qt/qml/Dettivo/app/history/SearchField.qml:12` and `qt/qml/Dettivo/app/meetings/MeetingsSearchField.qml:12` begin the same property interface; both implement the same focus helpers, clear action, hit count and debounce timer.
- **Why:** These 108-line components differ principally in their placeholder and accessible strings. Every keyboard, accessibility or layout fix currently needs two edits.
- **Change:** Keep one shared search component with explicit placeholder, field name, clear-action name and container name properties. Delete the duplicate implementation.
- **Risk:** Preserve distinct accessible names and independent query state. Run both search flows through typing, Enter, clear and focus restoration.

## Keep

- `qt/qml/Dettivo/theme/theme_backend.cpp` — one theme resolver, support for palette-only themes, theme-derived fallback colours and source diagnostics.
- `qt/qml/Dettivo/tests/theme_backend_test.cpp` — concrete regression coverage for published theme files and live file changes.
- `qt/qml/Dettivo/components/Osd.qml` and `qt/qml/Dettivo/osd/osd_bars.cpp` — shared pill state handling and scene-graph bars, separated from host placement.
- `qt/qml/Dettivo/Html.qml` — explicit escaping for intentionally constructed rich text.
- `qt/qml/Dettivo/app/settings/SettingRow.qml` and `qt/qml/Dettivo/app/meetings/NotesEditor.qml` — useful shared components with meaningful responsibilities. Their extraction earns its place.
- The existing file sizes are acceptable. No further splitting is justified merely to make files shorter.

## Questions for the owner

- What record was intended by **ADR 0042**? It is absent from the checkout and decision index.
- Which round-two artboards have actually been approved? `docs/design/README.md:36` says “awaiting approval,” while ADR 0038 describes its meeting artboards as approved and `docs/design/baselines.md` records approved renders.
- Should the off-Omarchy UI use the system UI font, as ADR 0010 promises? `qt/qml/Dettivo/theme/theme_backend.cpp:351` currently assigns `"monospace"` to the built-in palettes too.