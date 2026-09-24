# QA pack: the GUI drives for onboarding, settings, history and the Omarchy plugin, with the negative text scan and the accessibility check per surface

## Conversation Evidence

> user (turn 1): "your job is to read research and create a full masterplan/product vision document/prd that we can then use to break down into flow-next specs to get this developed as quickly as possible"
> user (turn 16): "ok go with the defaults, then do /flow-next:strategy"
> masterplan S-22 row: "`qa-pack-gui-onboarding-settings-history` | phase 3 | depends on S-18 to S-21 | 9.4 GUI packs; QA-4 | Drive packs green on Xvfb and Thor"
> masterplan QA-4: "Every GUI route passes the negative text assertion list in release builds."
> masterplan 9.4 item 9: "Negative text assertions on release routes. Every GUI route's accessible text is scanned for `exception`, stack traces, raw JSON, `debug`, `parity_gap`, absolute home paths, model file names and internal identifiers. A hit fails the drive."
> masterplan 9.4 item 5: "Bar widget QA is screenshot and IPC based. Quickshell does not expose an AT-SPI tree by default. The widget scenario asserts daemon state through the CLI, captures the bar region ... and compares against a baseline with a tolerance."
> masterplan NFR-9: "Accessibility | 100 % of interactive controls have accessible names and roles | a11y lint in CI"
> masterplan 9.2: "Every scenario at L2 and above runs in an isolated profile ... First-run profiles are used deliberately because they expose contract gaps a configured machine hides"
> masterplan phase 3 exit criteria: "GUI drive pack green on Xvfb and Thor; theme follows `omarchy theme set` live; plugin installed through `omarchy plugin add`; `dettivo setup omarchy` idempotent"

## Goal & Context

<!-- Goal & Context: 30% [user], 50% [paraphrase], 20% [strategy] -->

Phase 3 ends with one command that proves the GUI. `dettivo-qa pack gui` composes four per-surface packs, `gui-onboarding`, `gui-settings`, `gui-history` and `gui-omarchy`, each runnable alone, from the scenarios the first-run, settings, history and plugin specs landed plus the few that close their gaps, and runs them on both drivers in one evidence run with one report, `gui-pack.json` and `gui-pack.md`, built on the pack model of ADR 0017. Every route a step visits is scanned for developer text with the full 9.4 class list against release-profile binaries (QA-4) and walked for accessible names and roles on every interactive element (NFR-9), per surface, so the report says which screen leaked what. The pack is green under Xvfb in CI with the Omarchy-only steps recorded as allowed skips, and green on this Hyprland desktop with the bar, the panel and `dettivo setup omarchy` included; it is the phase 3 exit check and the second named step of the release gate. [paraphrase]

## Architecture & Data Models

<!-- Architecture & Data Models: 55% [paraphrase], 45% [inferred] -->

- Packs (`crates/dettivo-qa/src/pack/gui.rs`): `gui-onboarding` runs `first_run_fresh` and `first_run_provisioned` on `atspi` then `cua`, a `first_run_steps` step (each `DETTIVO_E2E_STEP` value opens its screen by title) and a new `first_run_resume` scenario (close the window on Models, reopen, the state file's `first_run.step` lands on Models). `gui-settings` runs `settings_roundtrip` once per section as eight steps so a failing section is named, a new `settings_env_override` scenario (a key set through the environment shows the source badge and the disabled control naming the variable), and `scripts/lint-settings-keys.sh` as a step. `gui-history` runs `history_seeded`, `app_routes` and a new `history_states` scenario (an empty profile shows the designed empty state, a search with no hits names the query, both from `states-and-hint-sheet.png`). `gui-omarchy` runs `omarchy plugin validate omarchy/` and the shim load test everywhere, the version-skew test, and on this machine `omarchy_bar` plus a new `omarchy_setup_idempotent` step (`dettivo setup omarchy` twice: the second `--check` reports every step `ok` and the snippet and units unchanged by hash). `pack gui` runs the four in that order; `pack list` names all five. [inferred]
- Scans per route: the runner's `negative_text::classify` grows the 9.4 classes it lacks today, `debug`, `parity_gap`, stack-trace frames (`at ` plus a source location, `src/...rs:NN`) and raw JSON (a name that parses as an object or array), and every step's `tree-<route>.json` is scanned after the scenario's own assertions; `scripts/lint-release-text.sh` mirrors the same classes over QML literals. A new `a11y_tree` check walks each tree: an element whose role is interactive (button, check box, radio, text, combo, slider, tab, list item, menu item, link) must carry a non-empty accessible name and a role; static text and images are exempt; the result lands as `a11y-<route>.json` with named, interactive and the offenders, and the report carries `a11y_coverage` per surface. Both checks fail the step naming the route, the class or the element path. [inferred]
- Release builds: the pack's preflight records the binaries' profile; the CI drive job adds `just build-release` (the `--release` profile for `dettivod`, `dettivo`, `dettivo-app`, `dettivo-osd` and `dettivo-qa`, the Qt build is Release already) before `just qa-pack gui`, and the pack sets `DETTIVO_QA_ALLOW_RELEASE=1` for release binaries and says so in the report, so QA-4's condition holds on every push. [inferred]
- Profiles: every step's scenario root is `qa-evidence/<run>/<scenario>.<driver>/profile/` with its own `HOME`, XDG directories, `DETTIVO_IPC_SOCKET` and `config.toml`, the model directory linked in and nothing else; a new preflight in `profile.rs` refuses a scenario whose config, state or data path resolves under the caller's `$HOME` or a real `XDG_*` directory, so a first-run drive never sees the developer's snippet or models. [paraphrase]
- Report: `qa-evidence/<run>/pack-gui/gui-pack.json` and `gui-pack.md` with the ADR 0017 shape plus `surfaces: [{ name, steps, negative_text: { routes, findings }, a11y_coverage }]`; the per-surface packs write `gui-<surface>-pack.json` in the same shape. [inferred]

## API Contracts

<!-- API Contracts: 70% [paraphrase], 30% [inferred] -->

- `dettivo-qa pack gui [--surface onboarding|settings|history|omarchy] [--driver atspi|cua] [--continue] [--json] [--out <dir>]` and `dettivo-qa pack gui-<surface>`; exit 0, 1 and 2 as ADR 0017 defines them. [inferred]
- `just qa-pack gui`, `just build-release`; the CI drive job runs the pack after the dictation pack in the same Xvfb session and uploads the report with the evidence; `qa-weekly` runs it on the fallback driver. [inferred]
- No new daemon methods and no new QA variables; `DETTIVO_E2E_STEP`, `DETTIVO_E2E_SEED`, `DETTIVO_E2E_OPEN`, `DETTIVO_E2E_ROUTE` and `DETTIVO_QA_ALLOW_RELEASE` are reused as documented. [paraphrase]

## Edge Cases & Constraints

- No `omarchy` CLI, no Hyprland or no `grim` on the machine: `omarchy_bar` and `omarchy_setup_idempotent` skip with the reason; the validate, shim and version-skew steps must pass everywhere. [paraphrase]
- `cua-driver` absent: the `cua` steps skip with the reason and the `atspi` steps must pass, so a cua regression never hides a GUI failure. [paraphrase]
- Developer text that arrives from the daemon (a model path in an error sentence): the scan fails the route naming the element; the fix is in the product, and the scan carries no allowlist. [inferred]
- A section whose round trip fails: the other seven still run; the report names the section and the key. [inferred]
- A hard failure mid-pack: later steps are `not_run` and the report is still written, per ADR 0017. [paraphrase]

## Acceptance Criteria

- **R1:** `dettivo-qa pack gui` runs the four per-surface packs in order in one evidence run and writes `gui-pack.json` plus `gui-pack.md` with per-surface steps, scan findings and coverage; each `gui-<surface>` pack runs alone into the same shape; unit tests cover the composition, the surface filter, the report and the profile preflight (a root under the caller's `$HOME` is refused by name). Errors: an unknown surface exits 2 naming the four. [inferred]
- **R2:** `gui-onboarding` is green on both drivers under Xvfb and here: the fresh profile shows the three screens, the provisioned profile opens Home, each step opens by name, and the resume scenario reopens on the remembered step. Errors: a fourth screen or a welcome title fails naming it. [paraphrase]
- **R3:** `gui-settings` is green on both drivers under Xvfb and here: the eight section round trips with the planted comment intact, the environment override badge with the disabled control, and the key-coverage lint. Errors: a failing section is named with its key. [paraphrase]
- **R4:** `gui-history` is green on both drivers under Xvfb and here: `history_seeded`, every route through `app_routes`, and the two designed states. Errors: a state that shows a raw table, a raw path or a generic placeholder fails naming the route. [paraphrase]
- **R5:** `gui-omarchy` is green here with `omarchy_bar`, the idempotent setup (second run changes nothing by hash and `--check` reports every step `ok`), the validate, shim and version-skew steps; under CI the desktop steps are recorded as allowed skips with their reasons. Errors: a changed file on the second setup run fails naming the path. [paraphrase]
- **R6:** Every route of every step passes the negative text scan with the full class list against release-profile binaries in CI (`just build-release` precedes the pack) and `a11y_coverage` is 1.0 per surface; a planted `parity_gap` string and a planted unnamed button (unit tests with the plant under QA mode) fail their route by name. Errors: as stated. [paraphrase]
- **R7:** `docs/qa.md` gains the GUI pack section (steps, allowed skips, the scan classes, the coverage figure, the release-build rule) and the profile guard; `docs/RELEASING.md` adds the pack as the second named gate step; the CI and weekly workflows run it; an ADR records the per-surface pack model and the tree checks. Errors: `just docs` fails on an unindexed ADR. [paraphrase]

## Boundaries

- No new GUI behaviour; a gap a step exposes is filed against the owning spec and fixed there. [inferred]
- No meetings drives (S-28), no visual re-approval or design changes (S-36), no soak. [paraphrase]
- No release gate beyond its second step; the release spec completes it. [inferred]

## Decision Context

### Motivation
<!-- scope: business -->

- The GUI is where an agent-driven build drifts first; one pack that walks every screen on both drivers, in fresh profiles, with the text and accessibility checks per surface, is what lets phase 3 close on evidence. [paraphrase]

## Strategy Alignment

- **Complete speech workflows, proven by drives:** onboarding, settings, history and the bar each get their drive pack and their report. [strategy:Complete speech workflows, proven by drives]
- **Omarchy-native, beautiful by default:** the plugin, the panel and `dettivo setup omarchy` are proven on the desktop they ship for. [strategy:Omarchy-native, beautiful by default]

## Requirement coverage

| R-ID | Task |
|---|---|
| R1 | fn-N.M (TBD) |
| R2 | fn-N.M (TBD) |
| R3 | fn-N.M (TBD) |
| R4 | fn-N.M (TBD) |
| R5 | fn-N.M (TBD) |
| R6 | fn-N.M (TBD) |
| R7 | fn-N.M (TBD) |
