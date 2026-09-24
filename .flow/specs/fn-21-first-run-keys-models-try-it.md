# First run: keys, models, try it

## Conversation Evidence

> user (turn 1): "your job is to read research and create a full masterplan/product vision document/prd that we can then use to break down into flow-next specs to get this developed as quickly as possible"
> user (turn 16): "ok go with the defaults, then do /flow-next:strategy"
> masterplan S-18 row: "`first-run-keys-models-try-it` | phase 3 | depends on S-17, S-06, S-09 | FR-U3, FR-C5, Principle 15; the three screens and the "no model or no bindings" trigger; baselines `first-run-1-keys.png`, `first-run-2-models.png`, `first-run-3-try-it.png` | Drive pack: fresh profile shows the three screens, provisioned profile shows none; bindings snippet golden; try-it inserts into the test field; visual diff against the three baselines"
> masterplan FR-U3: "First run is three screens and nothing more. **Keys**: the app detects the compositor, shows the bindings it will write (`Super+Ctrl+X` toggle, hold `F9` on Omarchy and Hyprland; the equivalent snippet on Sway and Niri; portal GlobalShortcuts elsewhere), writes them on confirm, and confirms live when the key is pressed. **Models**: choose the speech engine and model with size and a one-line recommendation, start the download, optionally tick the Enhanced model (Qwen3 4B, 2.5 GB) or point at Ollama; the step completes when a speech model is ready. **Try it**: hold the key, speak, watch the text land in a test field, see which insertion backend was used; done. There is no welcome screen."
> masterplan FR-C5: "Everything first run does is a config write plus a model download: bindings go to a snippet the compositor includes, engine and model selection to `[speech]`, the Enhanced choice to `[llm]`. A machine provisioned with a `config.toml` and a populated model directory never sees first run."
> masterplan coverage table: "First run: keys, models, try it | Designed, awaiting approval | `first-run-1-keys.png`, `first-run-2-models.png`, `first-run-3-try-it.png`"

## Goal & Context

<!-- Goal & Context: 30% [user], 50% [paraphrase], 20% [strategy] -->

A fresh machine dictates within three screens. The app's `onboarding` route becomes the first-run flow of FR-U3, built to the three designed baselines: Keys detects the compositor and shows the exact snippet `dettivo setup` will write, writes it on confirm and confirms live when the key is pressed; Models offers the speech engines and models with size and a one-line recommendation, starts the verified download with progress, and optionally ticks the Enhanced model or points at Ollama; Try it holds the key, speaks, lands the text in a test field and names the insertion backend. Every step is a config write or a model download, so a provisioned machine never sees first run, and `DETTIVO_E2E_COMPLETE` and `DETTIVO_E2E_STEP` drive it in QA. [paraphrase]

## Architecture & Data Models

<!-- Architecture & Data Models: 55% [paraphrase], 45% [inferred] -->

- Trigger: the app opens `onboarding` instead of `home` when the state file has no `first_run.completed_at` AND (no speech model is ready per `speech.models.status` OR `hotkeys.status` reports no bound actions and no sourced snippet); `DETTIVO_E2E_COMPLETE=1` marks it complete, `DETTIVO_E2E_STEP=keys|models|try` opens a step directly; `dettivo app open onboarding` reopens it. [paraphrase]
- Keys (`qt/qml/Dettivo/app/onboarding/KeysStep.qml`): the host's `SetupModel` calls the daemon for the compositor (`system.capabilities.platform`, `hotkeys.status`), renders the snippet through the same generator `dettivo setup <compositor> --stdout` uses (an `hotkeys.snippet` Linux method that returns the text and the include line), shows the hold and toggle chords as key caps, writes the snippet on confirm through `hotkeys.setup { compositor }` (a new daemon method wrapping the CLI's write with the same idempotent behaviour and `--check`), then waits for a press: the daemon's `hotkeys.status` gains `last_press_at`, and on Hyprland the step tells the user to press the hold key and confirms when a `dictation.state` event or the press marker arrives; off the compositor paths the portal backend binds and the same confirmation applies. [inferred]
- Models (`ModelsStep.qml`): the catalogue through `speech.providers.list` and `speech.models.status`, one row per model with size, languages and the one-line recommendation from the catalogue (`recommended_for`), the GPU tier from the capability snapshot deciding the default pick (Parakeet v3 on Vulkan, Whisper small on CPU), `speech.models.download` with `model.download` progress events and cancel, an Enhanced tick that selects the Qwen3 4B GGUF through the LLM catalogue (`llm.models.*`, added by the local engine spec; until then the tick offers Ollama detection through `llm.providers.list`), and completes when a speech model is ready; the selection is written through `config.set` to `[speech]` and `[llm]`. [paraphrase]
- Try it (`TryItStep.qml`): a labelled text field inside the app registered as the insertion target through the app's own app id (the chain's `self_app_ids` is bypassed for this one field by a session-scoped `insert.perform` flag `allow_self_target` valid only while the step is visible, recorded as a delta), the hold key starts a dictation through the normal path, the text lands in the field, and the result names the backend and the time to insert from the completion event's timings. Done writes `first_run.completed_at` to the state file. [inferred]
- Design: the three screens match `first-run-1-keys.png`, `first-run-2-models.png`, `first-run-3-try-it.png` region by region through the app visual diff (crops cut by `crop-baseline.py` from the manifest), on the default and the light theme; the flow has no welcome screen and no other step. [paraphrase]
- Drives: `first_run_fresh` (a fresh profile shows the three screens, the bindings snippet written matches the golden, the model row selection writes `[speech]`, the try-it dictation through the mock microphone lands the fixture words in the field) and `first_run_provisioned` (a provisioned profile with a model and a sourced snippet opens Home), both on both drivers under Xvfb. [paraphrase]

## API Contracts

<!-- API Contracts: 70% [paraphrase], 30% [inferred] -->

- Linux additions with fixtures in `docs/api/linux-deltas.md`: `hotkeys.snippet { compositor } -> { text, include_line, path }`, `hotkeys.setup { compositor, write: bool } -> { written, path, include_line, sourced }`, `hotkeys.status.last_press_at`, `insert.perform.allow_self_target` (session-scoped, app only). [inferred]
- State file: `first_run.completed_at`, `first_run.step` in `state.toml`. [paraphrase]
- QA: `DETTIVO_E2E_COMPLETE=1`, `DETTIVO_E2E_STEP=<step>` (already parsed), `dettivo app open onboarding`. [paraphrase]
- No config keys of its own; it writes `[speech]`, `[llm]` and the snippet. [paraphrase]

## Edge Cases & Constraints

- Compositor unknown (GNOME, KDE): the Keys step binds through the portal and shows the portal's outcome instead of a snippet. [paraphrase]
- Download fails or is cancelled: the row shows the reason and the step waits; a model that was already on disk completes the step at once. [inferred]
- No microphone: Try it says so with the device list and lets the user finish anyway. [inferred]
- The user closes the window mid-flow: the step is remembered in the state file and reopens there. [inferred]
- Provisioned by a dotfile manager: never shown; `dettivo app open onboarding` still works. [paraphrase]

## Acceptance Criteria

- **R1:** A fresh profile opens the three screens in order and a provisioned profile opens Home (`first_run_fresh` and `first_run_provisioned` drives on both drivers under Xvfb, plus `DETTIVO_E2E_STEP` opening each step by name). Errors: as stated. [paraphrase]
- **R2:** Keys: the shown snippet equals `dettivo setup <compositor> --stdout` (golden), confirm writes it idempotently and reports the include line, and the step confirms live when the hold key is pressed (the drive presses it through the daemon's action path in QA mode). Errors: an unknown compositor shows the portal path. [paraphrase]
- **R3:** Models: the rows come from the catalogue with size and recommendation, the tier picks the default, download shows progress and can be cancelled, the selection is written to `[speech]` (and `[llm]` for the Enhanced tick), and the step completes when a model is ready (drive with the fixture model server). Errors: a failed download names the reason. [paraphrase]
- **R4:** Try it: the hold key dictates the mock-microphone fixture into the field through the real chain with the self-target allowance, the result names the backend and the time to insert, and Done writes `first_run.completed_at`; the allowance is refused outside the step (daemon test). Errors: as stated. [paraphrase]
- **R5:** The three screens match their baselines region by region on the default and light themes in CI (manifest entries and crops), the negative text scan passes, and every element has an accessible name. Errors: a missing crop fails by name. [paraphrase]
- **R6:** `docs/app.md` gains the first-run section, the deltas are registered with fixtures, `dettivo app open onboarding` is documented, and an ADR records the trigger rule and the self-target allowance. Errors: as stated. [paraphrase]

## Boundaries

- No settings editors (S-19); the steps write config directly. [paraphrase]
- No LLM catalogue download until the local engine spec lands; the Enhanced tick offers Ollama until then and the GGUF once `llm.models.*` exists. [inferred]
- No welcome, tour or account screens. [paraphrase]

## Decision Context

### Motivation
<!-- scope: business -->

- Three screens between install and the first dictation is the promise of Principle 15; every step being a config write keeps dotfile provisioning first-class. [paraphrase]

## Strategy Alignment

- **Omarchy-native, beautiful by default:** the flow is the approved baseline and writes Omarchy's own bindings. [strategy:Omarchy-native, beautiful by default]
- **Complete speech workflows, proven by drives:** the fresh-profile drive proves the whole path to the first inserted words. [strategy:Complete speech workflows, proven by drives]

## Requirement coverage

| R-ID | Task |
|---|---|
| R1 | fn-N.M (TBD) |
| R2 | fn-N.M (TBD) |
| R3 | fn-N.M (TBD) |
| R4 | fn-N.M (TBD) |
| R5 | fn-N.M (TBD) |
| R6 | fn-N.M (TBD) |
