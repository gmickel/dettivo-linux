# Evidence map

Every requirement of every spec under `.flow/specs/` mapped to a verification route that exists (NFR-10): 489 of 489 R-IDs over 63 specs, coverage 1.000, 955 routes (15 of them walked by a person), at commit `f75152b87999`. This is the inventory of routes, not their results: a route that resolves exists, the release gate is what runs it, and a `human` route is the receipt a person files rather than proof that they walked it. `dettivo-qa evidence-map --write` regenerates this file from `qa/evidence-map.toml`.

| Kind | Routes |
|---|---|
| `bench` | 6 |
| `contract` | 69 |
| `docs` | 144 |
| `drive` | 45 |
| `human` | 15 |
| `pack` | 60 |
| `pipeline` | 4 |
| `script` | 62 |
| `unit` | 513 |
| `visual` | 37 |

## Findings

None: every R-ID has a route and every reference resolves.

## Specs

| Spec | R-IDs | Mapped | Coverage | Human only |
|---|---|---|---|---|
| `fn-1-repository-bootstrap-workspace-cmake` | 7 | 7 | 1.000 | R5 |
| `fn-2-contract-import-ipc-v1-types-delta` | 7 | 7 | 1.000 | - |
| `fn-3-daemon-skeleton-socket-activated` | 7 | 7 | 1.000 | - |
| `fn-4-qa-rig-foundation-drivers-xvfb-ci` | 7 | 7 | 1.000 | - |
| `fn-5-design-system-theme-tokens-quick` | 7 | 7 | 1.000 | - |
| `fn-6-pipewire-mic-capture-devices-default` | 6 | 6 | 1.000 | - |
| `fn-7-engine-protocol-supervisor-and-whisper` | 7 | 7 | 1.000 | - |
| `fn-8-speech-catalogue-providers-selection` | 6 | 6 | 1.000 | - |
| `fn-9-dictation-session-state-machine-ipc` | 6 | 6 | 1.000 | - |
| `fn-10-text-insertion-virtual-keyboard-libei` | 6 | 6 | 1.000 | - |
| `fn-11-hotkeys-compositor-bindings-portal` | 6 | 6 | 1.000 | - |
| `fn-12-osd-the-qml-pill-component-dettivo-osd` | 6 | 6 | 1.000 | - |
| `fn-13-history-store-sqlite-with-fts5-the` | 6 | 6 | 1.000 | - |
| `fn-14-qa-pack-the-dictation-vertical-slice` | 6 | 6 | 1.000 | - |
| `fn-15-parakeet-engine-parakeetcpp-binding` | 6 | 6 | 1.000 | - |
| `fn-16-polish-deterministic-transforms-the` | 6 | 6 | 1.000 | - |
| `fn-17-qt-app-shell-window-routes-daemon` | 6 | 6 | 1.000 | - |
| `fn-18-mcp-server-stdio-transport-the-nineteen` | 6 | 6 | 1.000 | - |
| `fn-19-audio-import-decoding-the-chunked-long` | 6 | 6 | 1.000 | - |
| `fn-20-visual-regression-baselines-for-every` | 6 | 6 | 1.000 | - |
| `fn-21-first-run-keys-models-try-it` | 6 | 6 | 1.000 | - |
| `fn-22-history-ui-the-list-detail-search-re` | 6 | 6 | 1.000 | - |
| `fn-23-local-llm-engine-llamacpp-through-the` | 6 | 6 | 1.000 | - |
| `fn-24-meetings-dual-source-capture-the` | 6 | 6 | 1.000 | - |
| `fn-25-rest-shim-loopback-server-bearer-token` | 6 | 6 | 1.000 | - |
| `fn-26-settings-routes-as-a-config-editor` | 6 | 6 | 1.000 | - |
| `fn-27-gpu-tiers-the-doctor-report-and-the` | 6 | 6 | 1.000 | - |
| `fn-28-meeting-transcription-the-live-windowed` | 6 | 6 | 1.000 | - |
| `fn-29-omarchy-plugin-the-bar-widget-the-panel` | 7 | 7 | 1.000 | - |
| `fn-30-polish-models-gguf-conversion` | 6 | 6 | 1.000 | - |
| `fn-31-packaging-aur-recipes-release-workflow` | 6 | 6 | 1.000 | - |
| `fn-32-diarization-the-sherpa-onnx-engine-the` | 6 | 6 | 1.000 | - |
| `fn-33-meeting-notes-analysis-search-export` | 6 | 6 | 1.000 | - |
| `fn-34-meetings-gui-the-list-the-live-meeting` | 6 | 6 | 1.000 | - |
| `fn-35-qa-pack-the-gui-drives-for-onboarding` | 7 | 7 | 1.000 | - |
| `fn-36-qa-pack-meetings-end-to-end-the` | 6 | 6 | 1.000 | - |
| `fn-37-docs-the-guides-the-docs-build-the` | 6 | 6 | 1.000 | - |
| `fn-38-beauty-pass-every-surface-against-the` | 8 | 8 | 1.000 | R6 |
| `fn-39-raw-layer-protected-tokens-survive` | 5 | 5 | 1.000 | - |
| `fn-40-reduce-ci-rebuilds-and-preserve-release` | 5 | 5 | 1.000 | - |
| `fn-41-release-gate-defects-the-macros` | 4 | 4 | 1.000 | - |
| `fn-42-diarization-on-the-gpu-the-cuda-build` | 4 | 4 | 1.000 | - |
| `fn-43-cleanup-data-loss-and-persistence-17` | 18 | 18 | 1.000 | - |
| `fn-44-cleanup-ownership-races-and-lifecycle` | 24 | 24 | 1.000 | - |
| `fn-45-cleanup-delivery-and-the-target-window` | 13 | 13 | 1.000 | - |
| `fn-46-cleanup-speech-and-text-quality-7` | 8 | 8 | 1.000 | - |
| `fn-47-cleanup-backend-tier-and-configuration` | 19 | 19 | 1.000 | - |
| `fn-48-cleanup-security-boundaries-6-review` | 7 | 7 | 1.000 | - |
| `fn-49-cleanup-the-release-gate-and-its` | 17 | 17 | 1.000 | - |
| `fn-50-cleanup-qa-isolation-and-honest` | 20 | 20 | 1.000 | - |
| `fn-51-cleanup-packaging-the-plugin-and-ci-8` | 9 | 9 | 1.000 | - |
| `fn-52-cleanup-delete-and-simplify-29-review` | 30 | 30 | 1.000 | - |
| `fn-53-cleanup-other-33-review-findings` | 34 | 34 | 1.000 | - |
| `fn-54-live-desktop-qa-after-cleanup` | 4 | 4 | 1.000 | - |
| `fn-55-desktop-memory-budgets-grounded-in` | 6 | 6 | 1.000 | - |
| `fn-56-reliable-automatic-meeting-diarization` | 5 | 5 | 1.000 | - |
| `fn-57-keep-benchmark-indexing-scoped-to-suite` | 2 | 2 | 1.000 | - |
| `fn-58-classify-unavailable-gpu-workload` | 2 | 2 | 1.000 | - |
| `fn-59-working-f9-shortcuts-after-onboarding` | 8 | 8 | 1.000 | R2, R3, R8 |
| `fn-60-release-settings-and-reinsertion-ux` | 7 | 7 | 1.000 | - |
| `fn-61-select-whisper-meeting-models` | 0 | 0 | 1.000 | - |
| `fn-62-fix-meeting-timer-stuck-at-0000-in` | 0 | 0 | 1.000 | - |
| `fn-63-fix-meeting-ui-timer-and-long-meeting` | 0 | 0 | 1.000 | - |

### `fn-1-repository-bootstrap-workspace-cmake`

Repository bootstrap: workspace, CMake skeleton and CI

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1, R4 | `pack` | `release/ci_green` | pack step release/ci_green | CI runs the same just recipes (fmt, clippy, lint, unit-rust, unit-qml, docs) in the pinned container and the release gate needs them green |
| R1 | `script` | `scripts/run-step.sh` | file scripts/run-step.sh | every just recipe fans out through run-step so the first failing toolchain step is named and a partial build never reports success |
| R1 | `script` | `scripts/check-toolchain.sh` | file scripts/check-toolchain.sh | a missing toolchain or dependency fails the recipe naming the package |
| R2 | `unit` | `xtask::full_workspace_passes` | fn full_workspace_passes in tools/xtask/src/edges.rs | the declared crate dependency table covers every workspace member with no forbidden edge |
| R2 | `unit` | `xtask::violations_table` | fn violations_table in tools/xtask/src/edges.rs | a forbidden edge or a cycle fails lint-edges |
| R2 | `unit` | `dettivo-core::upstream_edges_resolve` | fn upstream_edges_resolve in crates/dettivo-core/src/lib.rs | each crate's smoke test checks its declared upstream edges resolve |
| R3 | `pack` | `release/install_test` | pack step release/install_test | the clean-machine install test builds the Qt binaries and installs the shared QML module to the system location |
| R3 | `docs` | `docs/adr/0013-qt-floor-and-packaging.md` | file docs/adr/0013-qt-floor-and-packaging.md | the Qt 6.8 floor and the installed module layout are the recorded decision |
| R4 | `human` | `.github/workflows/ci.yml` | receipt file .github/workflows/ci.yml: a route a person walks, not proof that they did | the pinned-container workflow with the fmt, clippy, unit-rust, unit-qml and docs jobs |
| R5 | `human` | `.flow/config.json` | receipt file .flow/config.json: a route a person walks, not proof that they did | pipeline.qa off and review backend none in the flow-next configuration |
| R5 | `human` | `.flow/features/README.md` | receipt file .flow/features/README.md: a route a person walks, not proof that they did | the feature map index seeded with one entry per design-coverage surface |
| R6 | `docs` | `docs/adr/README.md` | file docs/adr/README.md | the ADR index tells a contributor to copy template.md and open with what the decision does for the reader |
| R6 | `human` | `CONTRIBUTING.md` | receipt file CONTRIBUTING.md: a route a person walks, not proof that they did | the contribution guide states ADRs are the documentation, the value-first opening rule and the add-a-record steps |
| R7 | `unit` | `xtask::limits_by_extension` | fn limits_by_extension in tools/xtask/src/file_length.rs | lint-file-length applies 500 lines to Rust and C++ and 300 to QML |
| R7 | `unit` | `xtask::allow_parsing_skips_comments_and_blanks` | fn allow_parsing_skips_comments_and_blanks in tools/xtask/src/file_length.rs | the generated-code allowlist is parsed from .file-length-allow |

### `fn-2-contract-import-ipc-v1-types-delta`

Contract import: IPC v1 types, delta register and fixture suite

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1 | `script` | `scripts/check-docs.sh` | file scripts/check-docs.sh | the docs check hashes the copied IPC and REST documents against the pins and fails on drift |
| R1, R4, R5 | `docs` | `docs/api/linux-deltas.md` | file docs/api/linux-deltas.md | the delta register records the macOS commit pin, the platform and speech capability additions with their reason, and a disposition per contract item |
| R2 | `unit` | `dettivo-proto::every_fixture_round_trips_byte_stable` | fn every_fixture_round_trips_byte_stable in crates/dettivo-proto/tests/fixtures.rs | every fixture in the suite serialises back byte-stable through its typed shape |
| R2 | `unit` | `dettivo-proto::drift_is_rejected_with_the_field_named` | fn drift_is_rejected_with_the_field_named in crates/dettivo-proto/tests/fixtures.rs | an unknown field fails deserialisation naming the field |
| R2 | `unit` | `dettivo-proto::deserialize_rejects_non_uuid_and_names_error` | fn deserialize_rejects_non_uuid_and_names_error in crates/dettivo-proto/src/id.rs | a non-UUID id is refused with the error named |
| R2, R6, R7 | `contract` | `crates/dettivo-proto/fixtures` | directory crates/dettivo-proto/fixtures (18 entries) | the golden fixture suite: one directory per namespace, one file per method and error case, replayed by the QA rig |
| R3 | `unit` | `dettivo-proto::numeric_codes_match_the_contract_block` | fn numeric_codes_match_the_contract_block in crates/dettivo-proto/src/error.rs | the server codes carry the contract's numeric values |
| R3 | `unit` | `dettivo-proto::mismatched_or_unknown_codes_are_rejected` | fn mismatched_or_unknown_codes_are_rejected in crates/dettivo-proto/src/error.rs | a code outside the taxonomy or mismatched with its app code fails deserialisation |
| R3 | `unit` | `dettivo-proto::error_round_trips_with_empty_details_object` | fn error_round_trips_with_empty_details_object in crates/dettivo-proto/src/error.rs | the data block builder carries app code, retryability and an always-present details object |
| R4 | `unit` | `dettivo-proto::capability_flags_resolve_in_the_snapshot` | fn capability_flags_resolve_in_the_snapshot in crates/dettivo-proto/tests/fixtures.rs | every fixture's capability flag resolves in the capability snapshot, so a missing flag fails the suite |
| R4 | `contract` | `crates/dettivo-proto/fixtures/system/capabilities.json` | file crates/dettivo-proto/fixtures/system/capabilities.json | the capability snapshot fixture with the macOS flags and the Linux platform and speech blocks |
| R5 | `unit` | `xtask::fails_on_a_missing_item_and_an_unknown_disposition` | fn fails_on_a_missing_item_and_an_unknown_disposition in tools/xtask/src/deltas.rs | lint-deltas fails when a contract item has no row or an unknown disposition |
| R5 | `unit` | `xtask::passes_when_every_item_has_a_valid_disposition` | fn passes_when_every_item_has_a_valid_disposition in tools/xtask/src/deltas.rs | the register with every method, resource, tool, format and topic dispositioned passes the lint |
| R6 | `unit` | `dettivo-proto::reserved_methods_expect_not_implemented_and_implemented_have_success` | fn reserved_methods_expect_not_implemented_and_implemented_have_success in crates/dettivo-proto/tests/fixtures.rs | each reserved method's fixture expects NOT_IMPLEMENTED in the reserved shape |
| R6 | `unit` | `dettivo-proto::reserved_methods_match_the_reserved_module` | fn reserved_methods_match_the_reserved_module in crates/dettivo-proto/src/catalog.rs | the catalog's reserved set matches the enumerated reserved namespaces and methods |
| R7 | `unit` | `dettivo-proto::layout_names_every_namespace_and_method` | fn layout_names_every_namespace_and_method in crates/dettivo-proto/tests/fixtures.rs | a fixture directory or file without a namespace or method name fails the suite in the Rust unit job |
| R7 | `pack` | `release/contract_strict` | pack step release/contract_strict | the same fixture files replay against a live socket in the strict contract step |

### `fn-3-daemon-skeleton-socket-activated`

Daemon skeleton: socket-activated dettivod with peer auth, config and the CLI

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1 | `unit` | `dettivod::socket_activation_answers_within_300ms` | fn socket_activation_answers_within_300ms in crates/dettivod/tests/lifecycle.rs | the daemon takes the activated socket from systemd-socket-activate and answers system.ping under 300 ms |
| R1 | `script` | `scripts/qa-activation.sh` | file scripts/qa-activation.sh | measures activation latency on the installed user units |
| R1, R6 | `unit` | `dettivo-cli::unreachable_daemon_exits_2_naming_the_socket_unit` | fn unreachable_daemon_exits_2_naming_the_socket_unit in crates/dettivo-cli/tests/cli.rs | an unreachable daemon exits 2 naming dettivod.socket |
| R2 | `unit` | `dettivo-cli::token_mode_refuses_without_the_token_and_honours_the_overrides` | fn token_mode_refuses_without_the_token_and_honours_the_overrides in crates/dettivo-cli/tests/cli.rs | token mode refuses a missing or wrong token, accepts --token, the env and --token-file, and capabilities report peer_token |
| R2 | `unit` | `dettivod::peer_uid_must_match` | fn peer_uid_must_match in crates/dettivod/src/auth.rs | a connection from another uid is refused with UNAUTHORIZED_CLIENT |
| R2 | `unit` | `dettivod::peer_mode_ignores_tokens_and_token_mode_requires_a_match` | fn peer_mode_ignores_tokens_and_token_mode_requires_a_match in crates/dettivod/src/auth.rs | the auth rules per mode |
| R2 | `contract` | `crates/dettivo-proto/fixtures/system/ping.error-unauthorized.json` | file crates/dettivo-proto/fixtures/system/ping.error-unauthorized.json | the UNAUTHORIZED_CLIENT refusal shape |
| R3 | `unit` | `dettivod::implemented_fixtures_pass_against_the_live_socket` | fn implemented_fixtures_pass_against_the_live_socket in crates/dettivod/tests/contract.rs | the system.* and other implemented fixtures replay against the live socket with tolerant matching |
| R3 | `unit` | `dettivod::every_other_contract_method_is_not_implemented_in_the_reserved_shape` | fn every_other_contract_method_is_not_implemented_in_the_reserved_shape in crates/dettivod/tests/contract.rs | every unimplemented contract method answers NOT_IMPLEMENTED in the reserved shape |
| R3 | `unit` | `dettivod::malformed_and_oversized_lines_are_invalid_params_and_keep_the_connection` | fn malformed_and_oversized_lines_are_invalid_params_and_keep_the_connection in crates/dettivod/tests/contract.rs | a malformed line is INVALID_PARAMS and an oversized line is rejected without disconnecting |
| R3 | `contract` | `crates/dettivo-proto/fixtures/system` | directory crates/dettivo-proto/fixtures/system (7 entries) | the system.* fixtures the daemon replays |
| R3 | `pack` | `release/contract_strict` | pack step release/contract_strict | the strict replay of the whole suite against a fresh daemon |
| R4 | `unit` | `dettivod::a_second_start_exits_1_and_names_the_running_instance` | fn a_second_start_exits_1_and_names_the_running_instance in crates/dettivod/tests/lifecycle.rs | a second start exits 1 naming the running pid and socket |
| R4 | `unit` | `dettivod::a_stale_socket_file_does_not_prevent_a_clean_start` | fn a_stale_socket_file_does_not_prevent_a_clean_start in crates/dettivod/tests/lifecycle.rs | a stale socket file is unlinked and the daemon starts cleanly |
| R5 | `unit` | `dettivod::set_preserves_comments_and_ordering_and_writes_only_the_config_file` | fn set_preserves_comments_and_ordering_and_writes_only_the_config_file in crates/dettivod/tests/config_live.rs | config set keeps comments and ordering and never touches state.toml |
| R5 | `unit` | `dettivod::invalid_key_or_value_fails_set_and_validate_naming_the_key_and_never_writes` | fn invalid_key_or_value_fails_set_and_validate_naming_the_key_and_never_writes in crates/dettivod/tests/config_live.rs | an invalid key or value fails set and validate naming the key and leaves the file untouched |
| R5 | `unit` | `dettivod::environment_overrides_win_over_the_file_and_are_labelled` | fn environment_overrides_win_over_the_file_and_are_labelled in crates/dettivod/tests/config_live.rs | environment overrides beat the file and are labelled as such |
| R5 | `unit` | `dettivod::external_edits_are_picked_up_without_a_restart` | fn external_edits_are_picked_up_without_a_restart in crates/dettivod/tests/config_live.rs | a change to config.toml is reloaded while the daemon runs |
| R5 | `unit` | `dettivo-core::set_refuses_unknown_keys_and_bad_values_naming_the_key` | fn set_refuses_unknown_keys_and_bad_values_naming_the_key in crates/dettivo-core/src/config/edit.rs | the edit layer refuses anything the schema would not read back |
| R5 | `docs` | `docs/config.md` | file docs/config.md | every configuration key, its default and its environment override documented |
| R6 | `unit` | `dettivo-cli::status_commands_map_to_system_methods_with_human_json_and_quiet_output` | fn status_commands_map_to_system_methods_with_human_json_and_quiet_output in crates/dettivo-cli/tests/cli.rs | status verbs map 1:1 to system methods with --json and --quiet |
| R6 | `unit` | `dettivo-cli::call_reaches_any_method_one_to_one_and_reports_daemon_errors` | fn call_reaches_any_method_one_to_one_and_reports_daemon_errors in crates/dettivo-cli/tests/cli.rs | call reaches any method and reports daemon errors with the contract exit codes |
| R6 | `unit` | `dettivo-cli::unknown_commands_and_bad_flags_exit_4_with_usage` | fn unknown_commands_and_bad_flags_exit_4_with_usage in crates/dettivo-cli/tests/cli.rs | unknown commands exit 4 with usage |
| R6 | `unit` | `dettivo-cli::a_silent_server_exits_5_on_timeout` | fn a_silent_server_exits_5_on_timeout in crates/dettivo-cli/tests/cli.rs | a timeout exits 5 |
| R6 | `unit` | `dettivo-cli::rpc_codes_map_to_the_contract_exit_codes` | fn rpc_codes_map_to_the_contract_exit_codes in crates/dettivo-cli/src/exit.rs | the RPC code to exit code table |
| R7 | `unit` | `dettivod::no_log_level_leaks_content_markers` | fn no_log_level_leaks_content_markers in crates/dettivod/tests/logs.rs | every request path at trace with marker strings leaves none in the log |
| R7 | `unit` | `dettivod::log_sites_never_format_request_or_config_content` | fn log_sites_never_format_request_or_config_content in crates/dettivod/tests/logs.rs | a scan of every tracing site in the crate for content formatting |

### `fn-4-qa-rig-foundation-drivers-xvfb-ci`

QA rig foundation: drivers, Xvfb CI, virtual audio and QA mode

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1, R2 | `drive` | `placeholder_window` | scenario placeholder_window: dettivo-app shows its placeholder window with the accessible label | the placeholder window is launched on the headless display and its accessible label asserted on both drivers, writing screenshot, tree and receipt |
| R1 | `script` | `scripts/qa/xvfb-session.sh` | file scripts/qa/xvfb-session.sh | brings up Xvfb, the session bus, the accessibility bus and the window manager for the CI drive job, naming a missing piece |
| R1 | `unit` | `dettivo-qa::receipts_round_trip_and_land_in_the_scenario_dir` | fn receipts_round_trip_and_land_in_the_scenario_dir in crates/dettivo-qa/src/evidence.rs | the JSON receipt lands in the evidence directory and reads back |
| R1 | `unit` | `dettivo-qa::the_bus_address_names_its_socket` | fn the_bus_address_names_its_socket in crates/dettivo-qa/src/doctor.rs | doctor names the missing session or accessibility bus by its socket |
| R2 | `unit` | `dettivo-qa::the_shipped_pack_is_clean_and_a_driver_mention_is_caught` | fn the_shipped_pack_is_clean_and_a_driver_mention_is_caught in crates/dettivo-qa/src/lint.rs | lint-scenarios passes the shipped pack and fails a scenario that names a driver implementation |
| R2 | `unit` | `dettivo-qa::unknown_driver_names_are_refused` | fn unknown_driver_names_are_refused in crates/dettivo-qa/src/driver/mod.rs | the driver is selected by name at the runner, not inside a scenario |
| R2 | `human` | `.github/workflows/qa-weekly.yml` | receipt file .github/workflows/qa-weekly.yml: a route a person walks, not proof that they did | the weekly job runs the whole pack on the fallback driver |
| R3 | `unit` | `dettivo-qa::profile_isolates_locations_and_detects_leaks` | fn profile_isolates_locations_and_detects_leaks in crates/dettivo-qa/src/profile.rs | the isolated profile links the model directory in and the leak check names live processes and leftover sockets by path |
| R3 | `unit` | `dettivo-qa::a_root_under_the_callers_home_or_xdg_directory_is_refused_by_name` | fn a_root_under_the_callers_home_or_xdg_directory_is_refused_by_name in crates/dettivo-qa/src/profile.rs | a scenario root inside the caller's own directories is refused |
| R4 | `unit` | `dettivo-qa::fixture_round_trips_and_correlates_with_itself_under_delay` | fn fixture_round_trips_and_correlates_with_itself_under_delay in crates/dettivo-qa/src/audio.rs | the audio fixture and the lag-search correlation the rig check uses |
| R4 | `script` | `scripts/qa/audio-rig.sh` | file scripts/qa/audio-rig.sh | creates the PipeWire null sink, plays the fixture and names module-null-sink on failure |
| R4 | `unit` | `dettivo-audio::a_fixture_played_into_a_null_sink_is_captured_with_levels_and_written_as_a_take` | fn a_fixture_played_into_a_null_sink_is_captured_with_levels_and_written_as_a_take in crates/dettivo-audio/tests/rig.rs | a recorder on the null sink's monitor reproduces the fixture above the correlation tolerance |
| R4 | `unit` | `dettivo-audio::unloading_the_default_source_is_reported_within_a_second` | fn unloading_the_default_source_is_reported_within_a_second in crates/dettivo-audio/tests/rig.rs | unloading the sink mid-recording is visible to the recorder within a second |
| R5 | `unit` | `dettivo-core::every_hook_parses_and_bad_values_name_the_variable` | fn every_hook_parses_and_bad_values_name_the_variable in crates/dettivo-core/src/qa_tests.rs | every documented QA variable has a parse test and a bad value names the variable |
| R5 | `unit` | `dettivo-core::release_builds_refuse_qa_mode_unless_allowed` | fn release_builds_refuse_qa_mode_unless_allowed in crates/dettivo-core/src/qa_tests.rs | QA mode in a release build is refused unless explicitly allowed |
| R5 | `unit` | `dettivo-core::unknown_reserved_names_are_rejected_by_name` | fn unknown_reserved_names_are_rejected_by_name in crates/dettivo-core/src/qa_tests.rs | an unknown variable under the reserved prefix is rejected by name |
| R5 | `unit` | `dettivo-core::the_known_list_covers_every_prefix_and_docs_table` | fn the_known_list_covers_every_prefix_and_docs_table in crates/dettivo-core/src/qa_tests.rs | the parser's list matches the docs/qa.md table |
| R5 | `unit` | `dettivod::unknown_qa_variables_are_refused_by_name_with_exit_2` | fn unknown_qa_variables_are_refused_by_name_with_exit_2 in crates/dettivod/tests/qa_env.rs | the daemon exits 2 naming an unknown QA variable |
| R5 | `unit` | `qt::docsTableNamesEveryVariable` | docsTableNamesEveryVariable in qt/host/qa_environment_test.cpp | the Qt host parser is checked against the same docs table |
| R5 | `unit` | `qt::dettivo-app-qa-refusal` | dettivo-app-qa-refusal in qt/apps/dettivo-app/CMakeLists.txt | the Qt host refuses an unknown QA variable by name before any GUI object exists |
| R5 | `docs` | `docs/qa.md` | file docs/qa.md | the documented QA environment variables |
| R6 | `pack` | `release/contract_strict` | pack step release/contract_strict | the replay harness runs the fixture suite against a live daemon with per-method pass, fail, skip and pending |
| R6 | `unit` | `dettivo-qa::missing_flags_are_named` | fn missing_flags_are_named in crates/dettivo-qa/src/replay_flags.rs | a fixture whose capability flag the daemon does not declare is skipped and named, never passed |
| R6 | `unit` | `dettivo-qa::normalisation_levels_tolerant_and_machine_values` | fn normalisation_levels_tolerant_and_machine_values in crates/dettivo-qa/src/replay.rs | tolerant matching for ids, timestamps and machine values |
| R6 | `unit` | `dettivo-qa::report_counts_and_renders` | fn report_counts_and_renders in crates/dettivo-qa/src/replay.rs | the replay report counts pass, fail, skip and pending per method |
| R7 | `script` | `scripts/lint-accessible-names.sh` | file scripts/lint-accessible-names.sh | the accessible-name lint names the offending control |
| R7 | `script` | `scripts/lint-release-text.sh` | file scripts/lint-release-text.sh | the negative text scan names the offending text |
| R7 | `unit` | `dettivo-qa::classes_are_caught_and_clean_text_passes` | fn classes_are_caught_and_clean_text_passes in crates/dettivo-qa/src/negative_text.rs | the negative text classes are caught and clean text passes |
| R7 | `unit` | `dettivo-qa::a_planted_unnamed_button_fails_by_name_and_hidden_controls_do_not_count` | fn a_planted_unnamed_button_fails_by_name_and_hidden_controls_do_not_count in crates/dettivo-qa/src/a11y_tree.rs | an unnamed control in a captured tree fails naming the control |
| R7 | `unit` | `qt::dettivo-quick-test` | dettivo-quick-test in qt/qml/Dettivo/tests/CMakeLists.txt | the Qt Quick Test runner registered in ctest for the unit-qml job |

### `fn-5-design-system-theme-tokens-quick`

Design system: theme tokens, Quick Controls style, type, motion and icons

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1 | `unit` | `qt::resolvesEveryTokenFromOmarchyFiles` | resolvesEveryTokenFromOmarchyFiles in qt/qml/Dettivo/tests/theme_backend_test.cpp | every role and token resolves from the Omarchy colors.toml and shell.toml |
| R1 | `unit` | `qt::reResolvesWithin100msOnThemeSwap` | reResolvesWithin100msOnThemeSwap in qt/qml/Dettivo/tests/theme_backend_test.cpp | swapping the theme files re-resolves palette, font and spacing within 100 ms without restart |
| R1 | `unit` | `qt::malformedFileFallsBackAndReportsOnce` | malformedFileFallsBackAndReportsOnce in qt/qml/Dettivo/tests/theme_backend_test.cpp | a malformed theme file falls back to the built-in palette and reports the parse error once |
| R1 | `drive` | `theme_switch` | scenario theme_switch: the pill and the app re-theme within 100 ms of the theme directory changing, frames paced | a live theme swap on the running surface re-resolves within the latency budget |
| R1 | `drive` | `app_theme_live` | scenario app_theme_live: dettivo-app re-skins on an Omarchy theme swap with no restart and reports the apply time | the app follows an Omarchy theme swap while running |
| R2 | `unit` | `qt::noOmarchyFollowsPortalScheme` | noOmarchyFollowsPortalScheme in qt/qml/Dettivo/tests/theme_backend_test.cpp | without Omarchy files the theme follows the portal scheme with the built-in dark and light palettes, reports its source, and defaults to dark on a portal error |
| R2 | `unit` | `qt::portalColourSchemeSignalFlipsThePalette` | portalColourSchemeSignalFlipsThePalette in qt/host/app/app_host_test.cpp | a portal colour-scheme change flips the palette live |
| R2 | `unit` | `qt::test_source_is_builtin_without_omarchy` | test_source_is_builtin_without_omarchy in qt/qml/Dettivo/tests/qml/tst_theme.qml | the Theme singleton reports the builtin source |
| R3 | `unit` | `qt::test_style_is_dettivo` | test_style_is_dettivo in qt/qml/Dettivo/tests/qml/tst_style.qml | the host selects the DettivoStyle before the engine and the assertion holds |
| R3 | `unit` | `qt::test_button_states_use_shell_alphas` | test_button_states_use_shell_alphas in qt/qml/Dettivo/tests/qml/tst_style.qml | normal, hover, focus, selected and pressed use the shell's alphas |
| R3 | `unit` | `qt::test_check_box_and_switch_toggle` | test_check_box_and_switch_toggle in qt/qml/Dettivo/tests/qml/tst_style.qml | the listed toggle controls render through the custom style |
| R4 | `script` | `scripts/lint-qml-tokens.sh` | file scripts/lint-qml-tokens.sh | a literal size or colour in the module fails naming the file and property |
| R4 | `unit` | `qt::test_type_scale_follows_base_size` | test_type_scale_follows_base_size in qt/qml/Dettivo/tests/qml/tst_theme.qml | the type scale is a token derived from the base size |
| R4 | `unit` | `qt::test_spacing_scale_doubles_from_xs` | test_spacing_scale_doubles_from_xs in qt/qml/Dettivo/tests/qml/tst_theme.qml | spacing, control height and radius are tokens on the scale |
| R5 | `unit` | `qt::test_motion_tokens_and_reduced_motion` | test_motion_tokens_and_reduced_motion in qt/qml/Dettivo/tests/qml/tst_theme.qml | the named easings and durations exist and duration() collapses under reduced motion |
| R5 | `unit` | `qt::reducedMotionFollowsPortalAndOverride` | reducedMotionFollowsPortalAndOverride in qt/qml/Dettivo/tests/theme_backend_test.cpp | the reduced-motion setting is read from the portal and the override |
| R5 | `unit` | `qt::test_waveform_bars_are_scene_graph_rectangles` | test_waveform_bars_are_scene_graph_rectangles in qt/qml/Dettivo/tests/qml/tst_components.qml | the waveform renders as scene-graph rectangles, no Canvas repaint |
| R5 | `unit` | `qt::test_level_meter_clips_in_urgent` | test_level_meter_clips_in_urgent in qt/qml/Dettivo/tests/qml/tst_components.qml | the level meter is a scene-graph component with its states |
| R6 | `script` | `scripts/lint-icons.sh` | file scripts/lint-icons.sh | an SVG off the 16 px grid or with another stroke width fails the icon lint |
| R6 | `visual` | `design-system` | surface design-system in qa/visual/manifest.toml | the sheet renders the icon set and the six-bar mark at their sizes against the baseline |
| R7 | `unit` | `qt::test_every_component_has_role_and_name` | test_every_component_has_role_and_name in qt/qml/Dettivo/tests/qml/tst_components.qml | every shared component carries an accessible role and name |
| R7 | `unit` | `qt::test_controls_carry_roles_and_names` | test_controls_carry_roles_and_names in qt/qml/Dettivo/tests/qml/tst_style.qml | every styled control carries an accessible role and name |
| R7 | `script` | `scripts/lint-accessible-names.sh` | file scripts/lint-accessible-names.sh | the documented accessible-name list is enforced over the module |
| R7 | `visual` | `design-system` | surface design-system in qa/visual/manifest.toml | the component sheet is compared with the approved design-system baseline |
| R7 | `human` | `docs/design/baselines.md` | receipt file docs/design/baselines.md: a route a person walks, not proof that they did | the approved design-system baseline and its review record |

### `fn-6-pipewire-mic-capture-devices-default`

PipeWire microphone capture: devices, default following, levels and takes

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1 | `unit` | `dettivo-audio::a_fixture_played_into_a_null_sink_is_captured_with_levels_and_written_as_a_take` | fn a_fixture_played_into_a_null_sink_is_captured_with_levels_and_written_as_a_take in crates/dettivo-audio/tests/rig.rs | the null sink's monitor is captured as 16 kHz mono and the take correlates above 0.99 with the fixture after delay alignment |
| R1, R2 | `drive` | `meeting_rig` | scenario meeting_rig: two null sinks feed a meeting's microphone and system tracks; the microphone sink is unloaded mid-meeting | two-source capture on the virtual rig with the device swap and the gap marker after it |
| R2 | `unit` | `dettivo-audio::unloading_the_default_source_is_reported_within_a_second` | fn unloading_the_default_source_is_reported_within_a_second in crates/dettivo-audio/tests/rig.rs | unloading the default source yields DeviceChanged within one second |
| R2 | `unit` | `dettivo-audio::a_pinned_device_that_is_absent_is_refused_by_name` | fn a_pinned_device_that_is_absent_is_refused_by_name in crates/dettivo-audio/tests/rig.rs | a pinned device that vanished ends capture naming the device |
| R2 | `unit` | `dettivo-audio::takes_are_numbered_with_offsets_gaps_and_a_sidecar` | fn takes_are_numbered_with_offsets_gaps_and_a_sidecar in crates/dettivo-audio/src/takes.rs | a second take carries its offset and gap marker in takes.json, and discard removes a take |
| R3 | `unit` | `dettivo-audio::windows_emit_rms_and_peak_and_silence_is_zero` | fn windows_emit_rms_and_peak_and_silence_is_zero in crates/dettivo-audio/src/level.rs | level windows emit RMS and peak in 0..1 with zeros during silence |
| R3 | `unit` | `dettivo-proto::notify_params_and_typed_payloads_round_trip` | fn notify_params_and_typed_payloads_round_trip in crates/dettivo-proto/src/events.rs | the audio.level payload round-trips through the typed event payloads |
| R3, R6 | `contract` | `crates/dettivo-proto/fixtures/audio/devices.json` | file crates/dettivo-proto/fixtures/audio/devices.json | the audio.devices shape the doctor and the deltas register name, including the level payload row |
| R3 | `docs` | `docs/api/linux-deltas.md` | file docs/api/linux-deltas.md | the audio.level payload appears in the additions register |
| R4 | `unit` | `dettivo-core::every_leaf_key_appears_uncommented_in_the_default_file` | fn every_leaf_key_appears_uncommented_in_the_default_file in crates/dettivo-core/src/config/schema.rs | the [audio] keys are in the schema and the default file config print-default prints |
| R4 | `unit` | `dettivo-core::default_file_parses_to_the_default_config` | fn default_file_parses_to_the_default_config in crates/dettivo-core/src/config/schema.rs | the default file parses back to the schema defaults |
| R4 | `contract` | `crates/dettivo-proto/fixtures/config/print_default.json` | file crates/dettivo-proto/fixtures/config/print_default.json | config print-default carries the [audio] section |
| R4 | `docs` | `docs/config.md` | file docs/config.md | input_device, keep_dictation_audio and level_interval_ms documented |
| R4 | `unit` | `dettivod::invalid_key_or_value_fails_set_and_validate_naming_the_key_and_never_writes` | fn invalid_key_or_value_fails_set_and_validate_naming_the_key_and_never_writes in crates/dettivod/tests/config_live.rs | an invalid audio value fails config set naming the key |
| R5 | `unit` | `dettivo-audio::the_fixture_path_matches_the_fixture_without_pipewire` | fn the_fixture_path_matches_the_fixture_without_pipewire in crates/dettivo-audio/tests/rig.rs | the R1 check runs on the DETTIVO_MOCK_MIC path without PipeWire and reports the rig path as skipped |
| R5 | `unit` | `dettivo-audio::fixture_plays_at_real_time_with_levels_and_ends` | fn fixture_plays_at_real_time_with_levels_and_ends in crates/dettivo-audio/src/mock.rs | MockCapture plays the WAV at real time through the same event stream |
| R5 | `unit` | `dettivo-core::every_hook_parses_and_bad_values_name_the_variable` | fn every_hook_parses_and_bad_values_name_the_variable in crates/dettivo-core/src/qa_tests.rs | a missing DETTIVO_MOCK_MIC file is refused at start by name |
| R6 | `unit` | `dettivo-cli::the_json_matches_the_golden_by_shape_and_the_human_report_groups_the_rows` | fn the_json_matches_the_golden_by_shape_and_the_human_report_groups_the_rows in crates/dettivo-cli/tests/doctor.rs | doctor's audio row with the default source, the pin and PipeWire reachability |
| R6 | `unit` | `dettivo-cli::doctor_reports_facts_and_exits_by_health` | fn doctor_reports_facts_and_exits_by_health in crates/dettivo-cli/tests/cli.rs | doctor reports the daemon facts end to end |

### `fn-7-engine-protocol-supervisor-and-whisper`

Engine protocol, supervisor and the Whisper engine process

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1 | `unit` | `dettivo-engine-proto::every_fixture_round_trips_byte_stable` | fn every_fixture_round_trips_byte_stable in crates/dettivo-engine-proto/tests/fixtures.rs | every protocol message and event fixture, including recognize with a PCM attachment, round-trips byte-stable |
| R1 | `unit` | `dettivo-engine-proto::every_request_and_event_name_has_a_fixture` | fn every_request_and_event_name_has_a_fixture in crates/dettivo-engine-proto/tests/fixtures.rs | the checked-in fixture suite covers every request and event name |
| R1 | `unit` | `dettivo-engine-proto::malformed_headers_name_the_field` | fn malformed_headers_name_the_field in crates/dettivo-engine-proto/src/frame.rs | a malformed frame is rejected naming the field |
| R2 | `unit` | `dettivo-speech::spawns_once_keeps_warm_and_unloads_after_idle` | fn spawns_once_keeps_warm_and_unloads_after_idle in crates/dettivo-speech/tests/supervisor.rs | spawn on first use, warm reuse across requests, unload and terminate after a short idle timeout |
| R2 | `unit` | `dettivo-speech::crashes_restart_with_backoff_and_degrade_after_three` | fn crashes_restart_with_backoff_and_degrade_after_three in crates/dettivo-speech/tests/supervisor.rs | a forced crash restarts the engine with backoff |
| R2 | `unit` | `dettivo-speech::three_load_crashes_in_a_row_mark_the_engine_degraded` | fn three_load_crashes_in_a_row_mark_the_engine_degraded in crates/dettivo-speech/tests/supervisor.rs | three consecutive crashes mark the engine degraded |
| R2 | `unit` | `dettivo-speech::a_missing_binary_names_itself_and_the_directories_searched` | fn a_missing_binary_names_itself_and_the_directories_searched in crates/dettivo-speech/tests/supervisor.rs | a missing binary names the binary and the directories searched |
| R2, R4 | `unit` | `dettivod::speech_engines_reports_the_whisper_binary` | fn speech_engines_reports_the_whisper_binary in crates/dettivod/tests/contract.rs | speech.engines reports running, model, backend, reason, crashes and degraded per engine for the doctor |
| R2, R4 | `contract` | `crates/dettivo-proto/fixtures/speech/engines.json` | file crates/dettivo-proto/fixtures/speech/engines.json | the speech.engines shape the doctor's engine rows read |
| R3 | `unit` | `dettivo-engine-whisper::cli_mode_transcribes_the_fixture_under_the_wer_threshold_and_reports_the_backend` | fn cli_mode_transcribes_the_fixture_under_the_wer_threshold_and_reports_the_backend in crates/dettivo-engine-whisper/tests/cli.rs | the CLI on jfk.wav with tiny.en scores under the WER threshold with timestamped segments |
| R3 | `unit` | `dettivo-speech::the_whisper_engine_recognizes_the_fixture_over_the_protocol` | fn the_whisper_engine_recognizes_the_fixture_over_the_protocol in crates/dettivo-speech/tests/supervisor.rs | the same fixture over the protocol through the Supervisor returns the same text |
| R3 | `unit` | `dettivo-engine-whisper::a_missing_model_names_the_path_and_exits_1` | fn a_missing_model_names_the_path_and_exits_1 in crates/dettivo-engine-whisper/tests/cli.rs | a missing model names the path |
| R3 | `pack` | `dictation/whisper_wer` | pack step dictation/whisper_wer | the pack's WER step against the reference transcript |
| R4 | `unit` | `dettivo-engine-proto::forced_cpu_and_cpu_preference_win` | fn forced_cpu_and_cpu_preference_win in crates/dettivo-engine-proto/src/backend.rs | DETTIVO_FORCE_CPU=1 or --cpu reports backend cpu |
| R4 | `unit` | `dettivo-engine-proto::auto_reports_why_when_vulkan_is_out` | fn auto_reports_why_when_vulkan_is_out in crates/dettivo-engine-proto/src/backend.rs | backend selection reports vulkan when a device answers and the reason otherwise |
| R4 | `pack` | `meetings/gpu_workload_proof` | pack step meetings/gpu_workload_proof | on the GPU tier the engine's process is proven on the Vulkan device |
| R5 | `unit` | `dettivo-speech::concurrent_sources_coalesce_into_one_load` | fn concurrent_sources_coalesce_into_one_load in crates/dettivo-speech/src/preload.rs | preload from startup, model_selection and session_start coalesce into one load |
| R5 | `unit` | `dettivo-speech::a_failed_preload_never_fails_the_caller` | fn a_failed_preload_never_fails_the_caller in crates/dettivo-speech/src/preload.rs | a preload failure is logged privacy-safe and does not fail the caller |
| R5 | `unit` | `dettivo-speech::capabilities_and_meeting_capability_per_provider` | fn capabilities_and_meeting_capability_per_provider in crates/dettivo-speech/src/engines.rs | the SttEngine capability contract per provider |
| R6 | `unit` | `dettivo-core::every_leaf_key_appears_uncommented_in_the_default_file` | fn every_leaf_key_appears_uncommented_in_the_default_file in crates/dettivo-core/src/config/schema.rs | the [engines] and [speech] keys are printed by config print-default |
| R6 | `unit` | `dettivod::external_edits_are_picked_up_without_a_restart` | fn external_edits_are_picked_up_without_a_restart in crates/dettivod/tests/config_live.rs | a config change is applied while the daemon runs |
| R6 | `unit` | `dettivo-core::wrong_types_and_variants_name_key_and_line` | fn wrong_types_and_variants_name_key_and_line in crates/dettivo-core/src/config/validate.rs | an invalid value fails validation naming the key |
| R6 | `docs` | `docs/engines.md` | file docs/engines.md | the engine directory, idle timeouts and backend selection documented |
| R6 | `docs` | `docs/config.md` | file docs/config.md | the [engines] and [speech] keys documented |
| R7 | `unit` | `dettivo-engine-whisper::cli_mode_transcribes_the_fixture_under_the_wer_threshold_and_reports_the_backend` | fn cli_mode_transcribes_the_fixture_under_the_wer_threshold_and_reports_the_backend in crates/dettivo-engine-whisper/tests/cli.rs | a marker prompt at trace level never reaches the engine's stderr while lifecycle lines still do |
| R7 | `unit` | `dettivo-speech::redaction_drops_content_and_keeps_timings` | fn redaction_drops_content_and_keeps_timings in crates/dettivo-speech/src/process.rs | the supervisor redacts engine stderr lines to timings |
| R7 | `unit` | `dettivod::no_log_level_leaks_content_markers` | fn no_log_level_leaks_content_markers in crates/dettivod/tests/logs.rs | the daemon log with the preload path carries no content markers |

### `fn-8-speech-catalogue-providers-selection`

Speech catalogue: providers, selection, verified downloads and preload

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1 | `unit` | `dettivo-speech::the_builtin_catalogue_is_complete` | fn the_builtin_catalogue_is_complete in crates/dettivo-speech/src/catalogue.rs | every Whisper model, the VAD model and the Parakeet placeholders carry every required field |
| R1 | `unit` | `dettivo-speech::a_malformed_entry_names_itself` | fn a_malformed_entry_names_itself in crates/dettivo-speech/src/catalogue.rs | a missing field, bad checksum or disallowed host fails naming the entry |
| R2 | `unit` | `dettivo-speech::a_download_lands_verified_with_progress` | fn a_download_lands_verified_with_progress in crates/dettivo-speech/tests/downloads.rs | the file lands under <models>/<provider>/<id>/ with a verified manifest and progress states through done |
| R2 | `unit` | `dettivo-speech::a_truncated_transfer_resumes_with_a_range_request` | fn a_truncated_transfer_resumes_with_a_range_request in crates/dettivo-speech/tests/downloads.rs | a truncated download resumes with a Range request and hashes to the checksum |
| R2 | `unit` | `dettivo-speech::a_cancelled_download_keeps_its_partial_file` | fn a_cancelled_download_keeps_its_partial_file in crates/dettivo-speech/tests/downloads.rs | cancellation leaves the .part file |
| R2 | `unit` | `dettivo-speech::a_wrong_checksum_is_quarantined` | fn a_wrong_checksum_is_quarantined in crates/dettivo-speech/tests/downloads.rs | a checksum mismatch quarantines the file and reports quarantined |
| R2, R5 | `drive` | `first_run_fresh` | scenario first_run_fresh: a fresh profile shows Keys, Models and Try it: the golden snippet is written, a model is picked and downloaded, the mock microphone lands in the field | the Models screen downloads from the fixture model server and follows the progress end to end |
| R3, R4 | `unit` | `dettivod::a_model_downloads_verified_and_the_selection_preloads_it` | fn a_model_downloads_verified_and_the_selection_preloads_it in crates/dettivod/tests/speech_models.rs | status, download, providers.list and selection.set answer with the contract shapes from one readiness source; selecting a downloaded model preloads the engine and never downloads; an unknown model is INVALID_PARAMS naming it |
| R3 | `unit` | `dettivod::speech_fixtures_parse_into_their_shapes` | fn speech_fixtures_parse_into_their_shapes in crates/dettivod/tests/speech_models.rs | every speech fixture parses into its typed shape |
| R3, R5 | `contract` | `crates/dettivo-proto/fixtures/speech` | directory crates/dettivo-proto/fixtures/speech (11 entries) | the speech.providers.list, selection.get/set, models.status/download/cancel/delete fixtures and their error cases |
| R4 | `unit` | `dettivo-speech::concurrent_sources_coalesce_into_one_load` | fn concurrent_sources_coalesce_into_one_load in crates/dettivo-speech/src/preload.rs | the model_selection preload coalesces with the other sources |
| R4 | `unit` | `dettivo-speech::a_failed_preload_never_fails_the_caller` | fn a_failed_preload_never_fails_the_caller in crates/dettivo-speech/src/preload.rs | a preload failure is logged privacy-safe and the selection still applies |
| R5 | `unit` | `dettivo-cli::speech_commands_map_to_the_speech_methods` | fn speech_commands_map_to_the_speech_methods in crates/dettivo-cli/tests/cli.rs | speech providers, selection, download, delete, status and the doctor's models row against the fixture server with --json shapes |
| R5 | `contract` | `crates/dettivo-proto/fixtures/speech/models.delete.error-conflict-selected.json` | file crates/dettivo-proto/fixtures/speech/models.delete.error-conflict-selected.json | deleting the selected model without --force is CONFLICT |
| R6 | `unit` | `dettivod::a_corrupted_model_on_disk_is_quarantined_at_start` | fn a_corrupted_model_on_disk_is_quarantined_at_start in crates/dettivod/tests/speech_models.rs | verify_on_start detects a changed file and marks it quarantined |
| R6 | `unit` | `dettivo-speech::a_matching_file_is_verified_and_remembered` | fn a_matching_file_is_verified_and_remembered in crates/dettivo-speech/src/models_tests.rs | verification records the checksum in the manifest |
| R6 | `unit` | `dettivo-core::every_leaf_key_appears_uncommented_in_the_default_file` | fn every_leaf_key_appears_uncommented_in_the_default_file in crates/dettivo-core/src/config/schema.rs | the [speech] and [models] keys are printed by config print-default |
| R6 | `docs` | `docs/models.md` | file docs/models.md | the model store, verification and quarantine documented |
| R6 | `docs` | `docs/config.md` | file docs/config.md | the [speech] and [models] keys documented |

### `fn-9-dictation-session-state-machine-ipc`

Dictation session: state machine, IPC methods and the event stream

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1 | `unit` | `dettivo-session::concurrent_starts_let_exactly_one_win` | fn concurrent_starts_let_exactly_one_win in crates/dettivo-session/tests/machine.rs | four concurrent starts leave one winner and the active slot is taken before the first await |
| R1 | `unit` | `dettivo-session::cancel_works_in_every_state` | fn cancel_works_in_every_state in crates/dettivo-session/tests/machine_exits.rs | cancel in recording and transcribing, engine request cancelled |
| R1 | `unit` | `dettivo-session::device_loss_ends_the_session_recoverably` | fn device_loss_ends_the_session_recoverably in crates/dettivo-session/tests/machine_exits.rs | device loss fails the session with the device name and the next start runs |
| R1 | `unit` | `dettivo-session::an_engine_crash_fails_the_session_and_the_next_one_runs` | fn an_engine_crash_fails_the_session_and_the_next_one_runs in crates/dettivo-session/tests/machine_exits.rs | the engine crash path fails the session without leaking detail |
| R1, R6 | `unit` | `dettivo-session::stop_and_cancel_without_a_session_are_not_found_and_modes_are_gated` | fn stop_and_cancel_without_a_session_are_not_found_and_modes_are_gated in crates/dettivo-session/tests/machine.rs | stop or cancel with no session is NOT_FOUND; gated modes answer NOT_IMPLEMENTED |
| R2, R3, R5 | `unit` | `dettivod::a_dictation_returns_the_fixture_words_and_the_stream_shows_every_state` | fn a_dictation_returns_the_fixture_words_and_the_stream_shows_every_state in crates/dettivod/tests/dictation.rs | mock mic plus tiny.en: transcript holds the fixture words, take removed, status active throughout, events.notify shows recording/transcribing/inserting/idle plus audio.level and engine.state |
| R2 | `unit` | `dettivod::concurrent_starts_over_two_connections_let_one_win_and_a_missing_model_is_named` | fn concurrent_starts_over_two_connections_let_one_win_and_a_missing_model_is_named in crates/dettivod/tests/dictation.rs | a missing selected model is NOT_FOUND naming the model and the download command |
| R2, R3 | `contract` | `crates/dettivo-proto/fixtures/dictation` | directory crates/dettivo-proto/fixtures/dictation (9 entries) | the dictation.* fixtures replay against the live daemon |
| R3 | `contract` | `crates/dettivo-proto/fixtures/events` | directory crates/dettivo-proto/fixtures/events (5 entries) | events.subscribe/unsubscribe fixtures and the unknown-topic INVALID_PARAMS fixture |
| R4 | `unit` | `dettivod::a_subscriber_that_stops_reading_overflows_while_another_gets_everything` | fn a_subscriber_that_stops_reading_overflows_while_another_gets_everything in crates/dettivod/tests/dictation.rs | a lazy subscriber gets events.overflow with a drop count while the eager one sees every state |
| R4 | `unit` | `dettivod::a_full_buffer_drops_and_reports_the_count_once_space_frees` | fn a_full_buffer_drops_and_reports_the_count_once_space_frees in crates/dettivod/src/events.rs | the bus counts drops and resets after overflow is delivered |
| R5 | `unit` | `dettivo-session::the_policy_is_frozen_at_start_and_the_take_stops_at_the_maximum` | fn the_policy_is_frozen_at_start_and_the_take_stops_at_the_maximum in crates/dettivo-session/tests/machine_exits.rs | a config edit mid-session applies to the next session only |
| R6 | `unit` | `dettivo-cli::dictation_and_events_verbs_map_to_their_methods` | fn dictation_and_events_verbs_map_to_their_methods in crates/dettivo-cli/tests/cli.rs | dettivo dictation start\|stop\|cancel\|status\|toggle\|reinsert-last and events --follow reach the daemon in QA mode |
| R6 | `unit` | `dettivo-language::the_golden_set_holds` | fn the_golden_set_holds in crates/dettivo-language/src/raw/tests.rs | the raw pipeline golden set: replacements, spoken punctuation and whitespace |
| R6 | `drive` | `history_roundtrip` | scenario history_roundtrip: a dictated item is found by transcripts.search, survives a daemon restart and re-inserts | a mock-mic dictation and dictation.reinsert_last run end to end through the daemon |

### `fn-10-text-insertion-virtual-keyboard-libei`

Text insertion: virtual keyboard, libei, clipboard chain and target guards

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1 | `unit` | `dettivo-insert::every_character_maps_to_exactly_one_keycode_and_compiles` | fn every_character_maps_to_exactly_one_keycode_and_compiles in crates/dettivo-insert/src/keymap.rs | ASCII, accented Latin, CJK, emoji and mixed text map one keycode each and the keymap compiles with xkbcommon |
| R1 | `unit` | `dettivo-insert::batches_split_above_the_keycode_budget_and_keep_order` | fn batches_split_above_the_keycode_budget_and_keep_order in crates/dettivo-insert/src/keymap.rs | batches split above the keycode budget |
| R1 | `unit` | `dettivo-insert::unmappable_code_points_are_reported_by_index` | fn unmappable_code_points_are_reported_by_index in crates/dettivo-insert/src/keymap.rs | an unmappable code point is reported by index |
| R1 | `unit` | `dettivo-insert::keymap_coverage_demotes_a_backend_for_that_text` | fn keymap_coverage_demotes_a_backend_for_that_text in crates/dettivo-insert/src/chain.rs | the backend demotes itself for text it cannot map |
| R2 | `unit` | `dettivo-insert::the_order_is_fr_i1_with_unavailable_backends_skipped_and_named` | fn the_order_is_fr_i1_with_unavailable_backends_skipped_and_named in crates/dettivo-insert/src/chain.rs | the FR-I1 chain order with unavailable backends skipped and named |
| R2 | `unit` | `dettivo-insert::a_pin_selects_one_backend_and_an_unavailable_pin_fails_with_its_reason` | fn a_pin_selects_one_backend_and_an_unavailable_pin_fails_with_its_reason in crates/dettivo-insert/src/chain.rs | the [insert] backend pin and the unavailable-pin failure reason |
| R2 | `unit` | `dettivo-insert::an_unverifiable_target_permits_clipboard_only_and_clipboard_only_mode_too` | fn an_unverifiable_target_permits_clipboard_only_and_clipboard_only_mode_too in crates/dettivo-insert/src/chain.rs | an unverifiable target only ever gets clipboard-only |
| R2 | `unit` | `dettivod::a_pinned_backend_that_is_unavailable_fails_with_its_reason` | fn a_pinned_backend_that_is_unavailable_fails_with_its_reason in crates/dettivod/tests/insert.rs | the live daemon refuses an unavailable pinned backend with its reason |
| R3 | `drive` | `never_into_self` | scenario never_into_self: a Dettivo window as target is refused with target_is_self and nothing is typed | the dettivo-qa-target window as target answers failed/target_is_self and nothing is typed |
| R3 | `unit` | `dettivod::guard_mismatch_is_conflict_and_nothing_is_written` | fn guard_mismatch_is_conflict_and_nothing_is_written in crates/dettivod/tests/insert.rs | expected_target mismatch is CONFLICT before any backend runs |
| R3 | `unit` | `dettivod::a_dettivo_window_as_target_fails_with_target_is_self_and_nothing_is_written` | fn a_dettivo_window_as_target_fails_with_target_is_self_and_nothing_is_written in crates/dettivod/tests/insert.rs | a Dettivo window as target is failed with target_is_self |
| R4, R6 | `drive` | `insertion_matrix` | scenario insertion_matrix: text lands in every reachable target through the real chain; insertion-matrix.json | insert into every available target, writes insertion-matrix.json with backend, latency and outcome; uninstalled targets recorded as skipped |
| R4 | `unit` | `dettivo-insert::paste_sends_the_terminal_keystroke_and_restores_the_clipboard` | fn paste_sends_the_terminal_keystroke_and_restores_the_clipboard in crates/dettivo-insert/src/backend/clipboard.rs | the clipboard path sends the app-aware paste keystroke and restores the clipboard |
| R4 | `human` | `.flow/tasks/fn-10-text-insertion-virtual-keyboard-libei.1.md` | receipt file .flow/tasks/fn-10-text-insertion-virtual-keyboard-libei.1.md: a route a person walks, not proof that they did | receipt of the Hyprland run: foot, ghostty, alacritty via virtual_keyboard, Qt target via xdotool |
| R5 | `contract` | `crates/dettivo-proto/fixtures/insert` | directory crates/dettivo-proto/fixtures/insert (6 entries) | insert.perform, insert.undo, insert.target and the NOT_FOUND source_ref fixture replay against the live daemon |
| R5 | `unit` | `dettivo-cli::doctor_reports_facts_and_exits_by_health` | fn doctor_reports_facts_and_exits_by_health in crates/dettivo-cli/tests/cli.rs | dettivo doctor prints one line per backend with availability and reason |
| R5 | `unit` | `dettivod::without_a_display_nothing_is_typed_and_a_guard_cannot_be_verified` | fn without_a_display_nothing_is_typed_and_a_guard_cannot_be_verified in crates/dettivod/tests/insert.rs | system.capabilities insertion_backend names the chain's choice |
| R6 | `unit` | `dettivo-qa::rows_compare_the_read_back_with_the_sample` | fn rows_compare_the_read_back_with_the_sample in crates/dettivo-qa/src/scenarios/support.rs | matrix rows carry target, outcome, backend, latency and reason |
| R6 | `contract` | `crates/dettivo-proto/fixtures/config/print_default.json` | file crates/dettivo-proto/fixtures/config/print_default.json | the [insert] keys are printed by config print-default |
| R6 | `docs` | `docs/insertion.md` | file docs/insertion.md | the [insert] keys and the chain are documented |

### `fn-11-hotkeys-compositor-bindings-portal`

Hotkeys: compositor bindings, portal GlobalShortcuts, evdev and MPRIS

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1 | `unit` | `dettivo-cli::snippets_match_the_goldens_for_default_and_custom_keys` | fn snippets_match_the_goldens_for_default_and_custom_keys in crates/dettivo-cli/tests/setup.rs | hyprland, sway and niri --stdout match the checked-in goldens for default and custom keys |
| R1 | `unit` | `dettivo-cli::an_unknown_compositor_and_a_bad_chord_exit_4_naming_the_problem` | fn an_unknown_compositor_and_a_bad_chord_exit_4_naming_the_problem in crates/dettivo-cli/tests/setup.rs | unknown compositor exits 4 naming the supported ones; a bad chord names the key and notation |
| R2 | `drive` | `hotkeys_hyprland` | scenario hotkeys_hyprland: the Hyprland snippet drives hold, toggle and cancel through real key presses | hold, toggle and cancel chords drive sessions on Hyprland with every transition on the event stream; skipped with the reason without hyprctl |
| R2 | `pack` | `dictation/hotkeys_hyprland` | pack step dictation/hotkeys_hyprland | the pack runs the Hyprland hotkey scenario as its first step |
| R3 | `unit` | `dettivo-hotkeys::signals_become_presses_and_releases_for_the_bound_actions` | fn signals_become_presses_and_releases_for_the_bound_actions in crates/dettivo-hotkeys/tests/portal.rs | mock GlobalShortcuts portal on a private bus answers BindShortcuts; Activated/Deactivated become press and release |
| R3 | `unit` | `dettivo-hotkeys::a_denied_bind_is_reported_once_and_not_retried` | fn a_denied_bind_is_reported_once_and_not_retried in crates/dettivo-hotkeys/tests/portal.rs | a denied session is reported once |
| R3 | `unit` | `dettivod::without_a_portal_the_daemon_runs_and_names_the_reason` | fn without_a_portal_the_daemon_runs_and_names_the_reason in crates/dettivod/tests/hotkeys.rs | an absent portal reports unavailable with its reason and the daemon starts |
| R3, R5 | `unit` | `dettivod::portal_presses_drive_sessions_with_the_target_media_pause_and_cues` | fn portal_presses_drive_sessions_with_the_target_media_pause_and_cues in crates/dettivod/tests/hotkeys.rs | portal presses start and stop sessions; mock MPRIS player paused and resumed; start and stop cues logged through the mock audio path |
| R4 | `unit` | `dettivod::a_target_that_moved_is_refused_with_conflict_and_a_bad_pid_is_invalid_params` | fn a_target_that_moved_is_refused_with_conflict_and_a_bad_pid_is_invalid_params in crates/dettivod/tests/hotkeys.rs | dictation.status reports the target; a moved focus refuses the insertion with CONFLICT under the mock probe |
| R4 | `unit` | `dettivo-session::the_target_captured_at_start_reaches_the_snapshot_and_the_inserter` | fn the_target_captured_at_start_reaches_the_snapshot_and_the_inserter in crates/dettivo-session/tests/machine.rs | the target recorded at start reaches the status snapshot and the inserter guard |
| R5 | `unit` | `dettivo-hotkeys::playing_players_are_paused_and_resumed_and_paused_ones_left_alone` | fn playing_players_are_paused_and_resumed_and_paused_ones_left_alone in crates/dettivo-hotkeys/tests/mpris.rs | already paused players are left alone; a vanished player is skipped |
| R5 | `unit` | `dettivod::sounds_are_off_by_default_and_silent_in_meetings` | fn sounds_are_off_by_default_and_silent_in_meetings in crates/dettivod/src/feedback.rs | sounds never play during a meeting |
| R6 | `unit` | `dettivo-cli::hotkeys_status_and_doctor_name_the_backend_the_reasons_and_the_snippet` | fn hotkeys_status_and_doctor_name_the_backend_the_reasons_and_the_snippet in crates/dettivo-cli/tests/setup.rs | hotkeys status and doctor list backend, portal availability with reason, evdev requirement and snippet state |
| R6 | `contract` | `crates/dettivo-proto/fixtures/hotkeys` | directory crates/dettivo-proto/fixtures/hotkeys (4 entries) | hotkeys.status and setup fixtures |
| R6 | `docs` | `docs/hotkeys.md` | file docs/hotkeys.md | the three paths and the Omarchy defaults are documented |
| R6 | `contract` | `crates/dettivo-proto/fixtures/config/print_default.json` | file crates/dettivo-proto/fixtures/config/print_default.json | the [hotkeys] keys are printed by config print-default |

### `fn-12-osd-the-qml-pill-component-dettivo-osd`

OSD: the QML pill component, dettivo-osd layer-shell host and frame pacing

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1 | `unit` | `qt::test_states_map_to_dot_bars_border_and_icon` | test_states_map_to_dot_bars_border_and_icon in qt/qml/Dettivo/tests/qml/tst_osd.qml | the six states and hidden map to dot colour, bars, border accent and icon |
| R1 | `unit` | `qt::test_hide_timers_run_for_inserted_copied_and_error` | test_hide_timers_run_for_inserted_copied_and_error in qt/qml/Dettivo/tests/qml/tst_osd.qml | hide timers for inserted, copied and error |
| R1 | `unit` | `qt::test_reduced_motion_holds_the_bars_and_cuts_transitions` | test_reduced_motion_holds_the_bars_and_cuts_transitions in qt/qml/Dettivo/tests/qml/tst_osd.qml | reduced motion holds the bars and cuts transitions |
| R1 | `unit` | `qt::test_every_element_has_an_accessible_role_and_name` | test_every_element_has_an_accessible_role_and_name in qt/qml/Dettivo/tests/qml/tst_osd.qml | accessible names for every element |
| R1 | `unit` | `qt::test_unknown_state_warns_and_hides` | test_unknown_state_warns_and_hides in qt/qml/Dettivo/tests/qml/tst_osd.qml | an unknown state name warns and hides the pill |
| R1 | `unit` | `qt::test_every_text_is_one_line_and_elides` | test_every_text_is_one_line_and_elides in qt/qml/Dettivo/tests/qml/tst_osd.qml | one-line elision of every text |
| R2 | `drive` | `osd_dictation` | scenario osd_dictation: dettivo-osd shows Listening, Transcribing and Inserted through a mock-microphone dictation, then hides | Listening, Transcribing and Inserted read through AT-SPI in order, hidden after hide_after_ms, Copied or Error with the reason on a refused insertion |
| R2 | `pack` | `dictation/osd_dictation` | pack step dictation/osd_dictation | the pack runs the pill drive on both drivers |
| R3 | `visual` | `osd` | surface osd in qa/visual/manifest.toml | the six states under DETTIVO_E2E_OSD_STATE match the osd.png crops within the threshold; a missing crop fails by name |
| R3 | `unit` | `qt::dettivo-visual-diff-other` | dettivo-visual-diff-other in qt/tools/visual-diff/CMakeLists.txt | the diff tool fails an unrelated state pair, so the threshold discriminates |
| R3 | `visual` | `osd` | surface osd in qa/visual/manifest.toml | dettivo-qa visual renders the pill on every theme and diffs it against its baseline (optional, ADR 0066) |
| R4 | `unit` | `qt::dettivo-osd-layer-shell-unavailable` | dettivo-osd-layer-shell-unavailable in qt/apps/dettivo-osd/CMakeLists.txt | host = layer_shell without Wayland exits 0 naming it |
| R4 | `unit` | `qt::dettivo-osd-disabled-notice` | dettivo-osd-disabled-notice in qt/apps/dettivo-osd/CMakeLists.txt | [osd] enabled = false exits 0 with the notice |
| R4 | `unit` | `qt::caretAvoidanceFlipsOnlyWhenTheOtherEdgeIsFree` | caretAvoidanceFlipsOnlyWhenTheOtherEdgeIsFree in qt/host/osd/osd_host_test.cpp | the pill moves away from the focused window's band only when the other edge is free |
| R4 | `unit` | `dettivo-cli::an_absent_pill_exits_2_naming_the_unit_and_doctor_reports_it` | fn an_absent_pill_exits_2_naming_the_unit_and_doctor_reports_it in crates/dettivo-cli/tests/osd.rs | dettivo osd status and the doctor name the host or the notice |
| R4 | `human` | `.flow/tasks/fn-12-osd-the-qml-pill-component-dettivo-osd.1.md` | receipt file .flow/tasks/fn-12-osd-the-qml-pill-component-dettivo-osd.1.md: a route a person walks, not proof that they did | receipt of the live layer-shell surface, caret flip and window fallback on Hyprland |
| R5 | `unit` | `dettivo-qa::the_verdict_needs_zero_drops_and_the_render_budget` | fn the_verdict_needs_zero_drops_and_the_render_budget in crates/dettivo-qa/src/pacing.rs | the pacing verdict requires zero dropped frames and the render budget, naming each drop |
| R5 | `unit` | `qt::dettivo-pacing-test` | dettivo-pacing-test in qt/host/pacing/CMakeLists.txt | the frame summariser names drops with timestamps from QSG_RENDER_TIMING |
| R5 | `script` | `scripts/lint-qml-tokens.sh` | file scripts/lint-qml-tokens.sh | refuses Canvas and QQuickPaintedItem in the component |
| R5 | `docs` | `docs/adr/0021-visual-regression-gate-and-frame-pacing.md` | file docs/adr/0021-visual-regression-gate-and-frame-pacing.md | the frame pacing rule and the recorded per-machine figures |
| R6 | `docs` | `docs/osd.md` | file docs/osd.md | states, hosts, settings, the user service and the osd verbs |
| R6 | `docs` | `docs/design/README.md` | file docs/design/README.md | the component is mapped to its baseline |
| R6 | `contract` | `crates/dettivo-proto/fixtures/config/print_default.json` | file crates/dettivo-proto/fixtures/config/print_default.json | the [osd] keys are printed by config print-default |

### `fn-13-history-store-sqlite-with-fts5-the`

History store: SQLite with FTS5, the transcripts API, re-run and export transfer

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1 | `unit` | `dettivo-storage::an_empty_database_migrates_to_the_current_version_with_wal_and_foreign_keys` | fn an_empty_database_migrates_to_the_current_version_with_wal_and_foreign_keys in crates/dettivo-storage/tests/store.rs | migration from empty with WAL and foreign keys on |
| R1 | `unit` | `dettivo-storage::every_recorded_version_migrates_forward_and_keeps_its_rows` | fn every_recorded_version_migrates_forward_and_keeps_its_rows in crates/dettivo-storage/tests/store.rs | a fixture database per previous version migrates forward |
| R1 | `unit` | `dettivo-storage::a_failed_migration_names_itself_and_leaves_the_backup` | fn a_failed_migration_names_itself_and_leaves_the_backup in crates/dettivo-storage/tests/store.rs | a failed migration leaves the backup and names itself |
| R1 | `unit` | `dettivo-storage::every_field_reads_back_and_updates_stick` | fn every_field_reads_back_and_updates_stick in crates/dettivo-storage/tests/store.rs | every DictationItem field inserts and reads back |
| R1 | `unit` | `dettivo-storage::delete_removes_what_the_artifact_policy_says` | fn delete_removes_what_the_artifact_policy_says in crates/dettivo-storage/tests/store.rs | delete per artifact policy removes the row and the right files |
| R1 | `unit` | `dettivo-storage::the_sweep_drops_old_audio_and_rows_beyond_the_maximum_but_skips_busy_items` | fn the_sweep_drops_old_audio_and_rows_beyond_the_maximum_but_skips_busy_items in crates/dettivo-storage/tests/store.rs | the retention sweep |
| R1 | `unit` | `dettivo-storage::readers_keep_reading_while_another_connection_writes` | fn readers_keep_reading_while_another_connection_writes in crates/dettivo-storage/tests/store.rs | concurrent readers during a write |
| R2 | `unit` | `dettivo-storage::word_starts_match_across_raw_final_title_and_summary` | fn word_starts_match_across_raw_final_title_and_summary in crates/dettivo-storage/tests/search.rs | word-start matches across raw text, final text, title and summary |
| R2 | `unit` | `dettivo-storage::diacritics_fold_and_operators_are_escaped` | fn diacritics_fold_and_operators_are_escaped in crates/dettivo-storage/tests/search.rs | diacritic-insensitive matching, operator escaping, over-long query limit |
| R2 | `unit` | `dettivo-storage::snippets_cursor_paging_and_null_scores` | fn snippets_cursor_paging_and_null_scores in crates/dettivo-storage/tests/search.rs | snippets, cursor paging and null score |
| R2 | `unit` | `dettivo-storage::the_seed_is_searchable_by_app_and_date` | fn the_seed_is_searchable_by_app_and_date in crates/dettivo-storage/tests/search.rs | the twelve-item seed is searchable by app and date |
| R2 | `unit` | `dettivod::list_pages_search_limits_delete_and_stats_answer_over_the_seed` | fn list_pages_search_limits_delete_and_stats_answer_over_the_seed in crates/dettivod/tests/history.rs | an over-long query is INVALID_PARAMS at the daemon |
| R3 | `contract` | `crates/dettivo-proto/fixtures/transcripts` | directory crates/dettivo-proto/fixtures/transcripts (15 entries) | every transcripts.* fixture including delete, rerun and the error fixtures |
| R3 | `contract` | `crates/dettivo-proto/fixtures/transfer` | directory crates/dettivo-proto/fixtures/transfer (7 entries) | the transfer.* fixtures including chunk not found and rate limited |
| R3 | `unit` | `dettivod::the_stateful_transfer_fixtures_pass_over_one_upload_and_one_download` | fn the_stateful_transfer_fixtures_pass_over_one_upload_and_one_download in crates/dettivod/tests/history_export.rs | the stateful transfer fixtures pass by name over one upload and one download |
| R3 | `unit` | `dettivod::an_audio_import_transcribes_and_a_second_one_is_a_conflict` | fn an_audio_import_transcribes_and_a_second_one_is_a_conflict in crates/dettivod/tests/history_model.rs | the import-conflict fixture passes exactly |
| R4 | `drive` | `history_roundtrip` | scenario history_roundtrip: a dictated item is found by transcripts.search, survives a daemon restart and re-inserts | a mock-mic dictation lands in the store, survives a daemon restart and reinserts through dictation.reinsert_last |
| R4, R5 | `unit` | `dettivod::a_dictation_is_stored_with_audio_reinserts_after_a_restart_and_reruns_through_the_engine` | fn a_dictation_is_stored_with_audio_reinserts_after_a_restart_and_reruns_through_the_engine in crates/dettivod/tests/history_model.rs | the stored item carries transcript, insertion, app id, engine, model and retained audio; reinsert after restart; rerun through tiny.en with progress; NOT_FOUND audio_not_retained |
| R4 | `unit` | `dettivod::a_busy_database_is_reported_on_the_final_state_while_the_insertion_stands` | fn a_busy_database_is_reported_on_the_final_state_while_the_insertion_stands in crates/dettivod/tests/history_model.rs | a storage failure rides on the final state payload while the insertion stands |
| R4, R6 | `unit` | `dettivo-cli::history_verbs_list_get_latest_search_and_delete_over_the_seed` | fn history_verbs_list_get_latest_search_and_delete_over_the_seed in crates/dettivo-cli/tests/history.rs | dettivo history latest and the other verbs over the seed; the doctor history row |
| R5 | `unit` | `dettivo-cli::export_matches_the_goldens_and_an_archive_imports_into_an_empty_profile` | fn export_matches_the_goldens_and_an_archive_imports_into_an_empty_profile in crates/dettivo-cli/tests/history.rs | export json\|markdown\|txt match the goldens; zip imports back with the same ids |
| R5 | `unit` | `dettivod::an_archive_export_imports_into_an_empty_profile_with_the_same_ids` | fn an_archive_export_imports_into_an_empty_profile_with_the_same_ids in crates/dettivod/tests/history_export.rs | the archive round trip over the socket with audio adopted |
| R6 | `docs` | `docs/history.md` | file docs/history.md | store, retention, search behaviour, transfer flow and the CLI verbs |
| R6 | `contract` | `crates/dettivo-proto/fixtures/config/print_default.json` | file crates/dettivo-proto/fixtures/config/print_default.json | the [history] keys are printed by config print-default |

### `fn-14-qa-pack-the-dictation-vertical-slice`

QA pack: the dictation vertical slice, timing and the release-shaped report

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1 | `unit` | `dettivo-qa::the_dictation_pack_runs_its_steps_in_order_and_names_unknown_packs` | fn the_dictation_pack_runs_its_steps_in_order_and_names_unknown_packs in crates/dettivo-qa/src/pack/mod.rs | pack ordering, every scenario id resolves, unknown pack names the packs |
| R1 | `unit` | `dettivo-qa::a_hard_failure_stops_the_pack_and_marks_the_rest_not_run` | fn a_hard_failure_stops_the_pack_and_marks_the_rest_not_run in crates/dettivo-qa/src/pack/mod.rs | not_run marking after a hard failure |
| R1 | `unit` | `dettivo-qa::continue_runs_every_step_and_allowed_skips_pass` | fn continue_runs_every_step_and_allowed_skips_pass in crates/dettivo-qa/src/pack/mod.rs | --continue runs every step |
| R1, R3, R4 | `unit` | `dettivo-qa::the_report_renders_from_a_fixture_result` | fn the_report_renders_from_a_fixture_result in crates/dettivo-qa/src/pack/report.rs | the report from a fixture result: p50/p95 with the cold run excluded, insertion_reliability with skipped listed and failed targets with backend and reason |
| R1, R5 | `pack` | `release/pack_dictation` | pack step release/pack_dictation | the dictation pack runs as one evidence run and writes dictation-pack.json and .md |
| R2 | `pack` | `dictation/history_roundtrip` | pack step dictation/history_roundtrip | the must-pass history round trip step |
| R2 | `drive` | `history_roundtrip` | scenario history_roundtrip: a dictated item is found by transcripts.search, survives a daemon restart and re-inserts | search finds the item, it survives the restart, reinsert_last matches the read-back |
| R3 | `pack` | `dictation/first_insert_timing` | pack step dictation/first_insert_timing | ten warm runs, cold run excluded, split into capture, transcribe and insert |
| R3 | `unit` | `dettivo-qa::the_cold_run_and_failed_runs_stay_out_of_the_percentiles` | fn the_cold_run_and_failed_runs_stay_out_of_the_percentiles in crates/dettivo-qa/src/scenarios/first_insert_timing.rs | the cold run and failed runs stay out of the percentiles |
| R3 | `unit` | `dettivo-qa::each_tier_reads_its_calibrated_first_insert_target` | fn each_tier_reads_its_calibrated_first_insert_target in crates/dettivo-qa/src/pack/report.rs | p50 is compared with the NFR-1 target for the machine's tier |
| R3 | `unit` | `dettivo-proto::dictation_state_carries_the_completion_fields_only_when_present` | fn dictation_state_carries_the_completion_fields_only_when_present in crates/dettivo-proto/src/events.rs | the timings block round-trips on the completion event |
| R4 | `pack` | `dictation/insertion_matrix` | pack step dictation/insertion_matrix | the matrix rows aggregate into insertion_reliability against NFR-3 |
| R5 | `pack` | `dictation/whisper_wer` | pack step dictation/whisper_wer | the Whisper WER fixture as a pack step with its threshold |
| R5 | `unit` | `dettivo-qa::wer_is_edit_distance_over_reference_words` | fn wer_is_edit_distance_over_reference_words in crates/dettivo-qa/src/pack/wer.rs | the WER arithmetic |
| R5 | `pack` | `release/ci_green` | pack step release/ci_green | the CI drive job runs the pack after the individual drives |
| R6 | `docs` | `docs/qa.md` | file docs/qa.md | the pack, its report and the allowed skips |
| R6 | `docs` | `docs/RELEASING.md` | file docs/RELEASING.md | the pack is the first named gate step; the blocker rule |
| R6 | `docs` | `docs/adr/0017-qa-packs-and-the-dictation-report.md` | file docs/adr/0017-qa-packs-and-the-dictation-report.md | the ADR recording the pack model |
| R6 | `docs` | `docs/api/linux-deltas.md` | file docs/api/linux-deltas.md | the timings delta is registered |

### `fn-15-parakeet-engine-parakeetcpp-binding`

Parakeet engine: parakeet.cpp binding, word timestamps and the meeting-capability decision

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1 | `unit` | `parakeet-cpp-sys::the_abi_and_source_version_are_the_pinned_ones` | fn the_abi_and_source_version_are_the_pinned_ones in crates/parakeet-cpp-sys/src/lib.rs | parakeet.cpp builds from the pinned source with ABI 6 |
| R1, R3 | `unit` | `dettivo-engine-parakeet::every_fixture_row_this_machine_can_run_stays_under_its_wer_ceiling_with_word_timestamps` | fn every_fixture_row_this_machine_can_run_stays_under_its_wer_ceiling_with_word_timestamps in crates/dettivo-engine-parakeet/tests/cli.rs | loads the model, transcribes the fixture through the safe wrapper and holds every WER row under its ceiling, naming a miss |
| R1 | `unit` | `dettivo-engine-parakeet::a_missing_model_names_the_path_and_a_corrupt_file_names_the_reason` | fn a_missing_model_names_the_path_and_a_corrupt_file_names_the_reason in crates/dettivo-engine-parakeet/tests/cli.rs | missing path and corrupt file errors |
| R1 | `pack` | `release/ci_green` | pack step release/ci_green | cargo build -p dettivo-engine-parakeet succeeds in CI |
| R2 | `unit` | `dettivo-engine-parakeet::the_protocol_answers_every_request_shape_without_a_model` | fn the_protocol_answers_every_request_shape_without_a_model in crates/dettivo-engine-parakeet/tests/protocol.rs | status, cancel, recognize before load, missing model and the unknown-message error shape |
| R2 | `unit` | `dettivo-speech::the_parakeet_engine_recognizes_the_fixture_over_the_protocol_with_words` | fn the_parakeet_engine_recognizes_the_fixture_over_the_protocol_with_words in crates/dettivo-speech/tests/supervisor.rs | recognize over the protocol under the supervisor returns segments with word timestamps and confidence |
| R2 | `unit` | `dettivo-speech::crashes_restart_with_backoff_and_degrade_after_three` | fn crashes_restart_with_backoff_and_degrade_after_three in crates/dettivo-speech/tests/supervisor.rs | the supervisor's crash and timeout path |
| R2 | `unit` | `dettivo-engine-proto::every_fixture_round_trips_byte_stable` | fn every_fixture_round_trips_byte_stable in crates/dettivo-engine-proto/tests/fixtures.rs | the engine-protocol fixtures including response.recognize.words round-trip |
| R3 | `docs` | `docs/reports/parakeet-alignment-report.json` | file docs/reports/parakeet-alignment-report.json | backend and reason per run are in the filed report |
| R4 | `unit` | `dettivod::a_parakeet_dictation_returns_the_fixture_words_with_the_provider_selected_over_the_contract` | fn a_parakeet_dictation_returns_the_fixture_words_with_the_provider_selected_over_the_contract in crates/dettivod/tests/parakeet_dictation.rs | speech.selection.set picks parakeet and a mock-mic dictation returns the fixture words; skipped by name without the model |
| R4 | `unit` | `dettivo-speech::the_builtin_catalogue_is_complete` | fn the_builtin_catalogue_is_complete in crates/dettivo-speech/src/catalogue.rs | the catalogue lists both models with checksums |
| R4 | `contract` | `crates/dettivo-proto/fixtures/speech` | directory crates/dettivo-proto/fixtures/speech (11 entries) | speech.selection.set and providers.list fixtures |
| R5 | `pipeline` | `parakeet-alignment` | pipeline parakeet-alignment | both engines against the golden alignment, writes the report and applies the rule |
| R5 | `docs` | `docs/adr/0018-parakeet-engine-dictation-only.md` | file docs/adr/0018-parakeet-engine-dictation-only.md | the dictation-only verdict and the Sherpa-ONNX fallback route |
| R5 | `unit` | `dettivo-qa::words_pair_by_text_through_punctuation_and_insertions` | fn words_pair_by_text_through_punctuation_and_insertions in crates/dettivo-qa/src/alignment.rs | the alignment pairing and offset arithmetic |
| R5 | `contract` | `crates/dettivo-proto/fixtures/system/capabilities.json` | file crates/dettivo-proto/fixtures/system/capabilities.json | speech.providers[].meeting_capable is on the wire |
| R6 | `docs` | `docs/engines.md` | file docs/engines.md | the Parakeet engine, models, languages, backends, CLI mode and the capability matrix |
| R6 | `unit` | `dettivo-core::every_leaf_key_appears_uncommented_in_the_default_file` | fn every_leaf_key_appears_uncommented_in_the_default_file in crates/dettivo-core/src/config/schema.rs | [engines.parakeet] keys are in the default file |
| R6 | `contract` | `crates/dettivo-proto/fixtures/config/print_default.json` | file crates/dettivo-proto/fixtures/config/print_default.json | the keys are printed by config print-default |

### `fn-16-polish-deterministic-transforms-the`

Polish: deterministic transforms, the Enhanced provider layer and the polish.* API

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1 | `unit` | `dettivo-language::the_deterministic_pass_matches_the_macos_cases` | fn the_deterministic_pass_matches_the_macos_cases in crates/dettivo-language/tests/goldens.rs | the macOS transform, preset and style cases verbatim |
| R1 | `unit` | `dettivo-language::the_post_processor_pipeline_matches_the_registered_linux_outputs` | fn the_post_processor_pipeline_matches_the_registered_linux_outputs in crates/dettivo-language/tests/goldens.rs | the PostProcessors suite including protected tokens and marker stripping |
| R1 | `unit` | `dettivo-language::at_path_insertion_matches_the_macos_cases` | fn at_path_insertion_matches_the_macos_cases in crates/dettivo-language/tests/goldens.rs | @path insertion cases |
| R1 | `unit` | `dettivo-language::a_user_app_profile_wins_over_the_built_in_mapping` | fn a_user_app_profile_wins_over_the_built_in_mapping in crates/dettivo-language/tests/policy.rs | the resolution order with app profiles, default mappings and class overrides |
| R1 | `unit` | `dettivo-language::the_hash_is_stable_and_changes_with_every_input_to_the_resolution` | fn the_hash_is_stable_and_changes_with_every_input_to_the_resolution in crates/dettivo-language/tests/policy.rs | the frozen policy hash |
| R1 | `unit` | `dettivo-core::an_over_long_custom_rule_is_rejected_naming_the_limit` | fn an_over_long_custom_rule_is_rejected_naming_the_limit in crates/dettivo-core/src/config/polish_schema.rs | an over-long custom rule is rejected naming the limit |
| R2 | `unit` | `dettivo-language::echo_inserts_the_polish_text_and_never_notices` | fn echo_inserts_the_polish_text_and_never_notices in crates/dettivo-language/src/enhanced/mod.rs | the echo mock returns the Polish text with no notice |
| R2 | `unit` | `dettivo-language::a_rewrite_that_drops_a_path_is_rejected_and_falls_back` | fn a_rewrite_that_drops_a_path_is_rejected_and_falls_back in crates/dettivo-language/src/enhanced/mod.rs | the output guard rejects a dropped path with guard_rejected |
| R2 | `unit` | `dettivo-language::a_thinking_block_is_stripped_before_the_guard_sees_the_rewrite` | fn a_thinking_block_is_stripped_before_the_guard_sees_the_rewrite in crates/dettivo-language/src/enhanced/mod.rs | thinking blocks are stripped from the fixture provider's replay |
| R2 | `unit` | `dettivo-language::a_provider_timeout_falls_back_with_provider_unavailable` | fn a_provider_timeout_falls_back_with_provider_unavailable in crates/dettivo-language/src/enhanced/mod.rs | a provider timeout falls back with provider_unavailable inside timeout_ms |
| R3 | `contract` | `crates/dettivo-proto/fixtures/polish` | directory crates/dettivo-proto/fixtures/polish (9 entries) | every polish.* fixture including polish.test and the unknown-preset error |
| R3 | `contract` | `crates/dettivo-proto/fixtures/llm` | directory crates/dettivo-proto/fixtures/llm (9 entries) | the Linux llm.* additions have fixtures |
| R3 | `unit` | `dettivod::a_rule_write_keeps_the_comments_a_person_put_in_the_file` | fn a_rule_write_keeps_the_comments_a_person_put_in_the_file in crates/dettivod/tests/polish.rs | polish.rules.set round-trips through config.toml preserving comments |
| R3 | `unit` | `dettivod::an_over_long_rule_and_an_unknown_preset_are_refused_by_name` | fn an_over_long_rule_and_an_unknown_preset_are_refused_by_name in crates/dettivod/tests/polish.rs | unknown preset or style names are INVALID_PARAMS |
| R4 | `unit` | `dettivod::the_polish_modes_insert_the_polished_text_and_record_the_policy` | fn the_polish_modes_insert_the_polished_text_and_record_the_policy in crates/dettivod/tests/dictation.rs | deterministic_polish and enhanced with DETTIVO_MOCK_LLM=echo insert the polished text; history item and completion event carry mode, notice and hash |
| R4 | `unit` | `dettivod::enhanced_without_an_answer_inserts_the_polish_text_and_says_why` | fn enhanced_without_an_answer_inserts_the_polish_text_and_says_why in crates/dettivod/tests/dictation.rs | enhanced with no provider inserts the Polish text with the provider_unavailable notice |
| R5 | `unit` | `dettivo-language::a_model_the_endpoint_holds_is_available_and_rewrites` | fn a_model_the_endpoint_holds_is_available_and_rewrites in crates/dettivo-language/src/provider/ollama.rs | ollama is detected on a mock tags endpoint and rewrites through its chat endpoint |
| R5 | `unit` | `dettivo-language::a_local_endpoint_rewrites_through_chat_completions` | fn a_local_endpoint_rewrites_through_chat_completions in crates/dettivo-language/src/provider/openai.rs | openai_compatible rewrites through a local mock server |
| R5 | `unit` | `dettivo-language::a_remote_endpoint_needs_confirmation_until_it_is_stored` | fn a_remote_endpoint_needs_confirmation_until_it_is_stored in crates/dettivo-language/src/provider/trust.rs | a non-localhost endpoint is refused until trusted |
| R5 | `unit` | `dettivod::the_providers_are_listed_and_a_remote_endpoint_is_trusted_once` | fn the_providers_are_listed_and_a_remote_endpoint_is_trusted_once in crates/dettivod/tests/polish.rs | llm.endpoints.trust stores the endpoint once and it is then used |
| R6 | `docs` | `docs/polish.md` | file docs/polish.md | the three modes, resolution order, providers, trust gate and CLI verbs |
| R6 | `docs` | `docs/adr/0023-polish-layers-and-the-llm-provider-layer.md` | file docs/adr/0023-polish-layers-and-the-llm-provider-layer.md | the ADR for the provider layer and the trust gate |
| R6 | `contract` | `crates/dettivo-proto/fixtures/config/print_default.json` | file crates/dettivo-proto/fixtures/config/print_default.json | the [polish] and [llm] keys are printed by config print-default |

### `fn-17-qt-app-shell-window-routes-daemon`

Qt app shell: window, routes, daemon client, live theme and the Home surface

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1 | `drive` | `app_routes` | scenario app_routes: dettivo-app opens every route by name with its title, no developer text, and records the startup budget | opens every route through DETTIVO_E2E_OPEN and each settings sub-route, reads the accessible title and runs the negative text scan |
| R1 | `unit` | `qt::test_window_swaps_pages_by_route_with_the_title` | test_window_swaps_pages_by_route_with_the_title in qt/qml/Dettivo/tests/qml/tst_app_shell.qml | the window swaps pages by route name and carries the route title |
| R1 | `script` | `scripts/lint-accessible-names.sh` | file scripts/lint-accessible-names.sh | the a11y lint covers every element under qt/qml/Dettivo/app against docs/qa/a11y-names.md |
| R2 | `visual` | `home` | surface home in qa/visual/manifest.toml | the four regions of Home (sidebar, header, Today, rail) against the home.png crops on every theme and scale, with the sample under DETTIVO_E2E_SEED |
| R2 | `unit` | `qt::test_rail_draws_engine_states_and_progress` | test_rail_draws_engine_states_and_progress in qt/qml/Dettivo/tests/qml/tst_app_home.qml | the right rail follows engine.state and progress |
| R2 | `unit` | `qt::statusModelReadsFactsAndFollowsTheStream` | statusModelReadsFactsAndFollowsTheStream in qt/host/app/app_host_test.cpp | the status sentence reads the real hotkeys and target from the daemon |
| R3 | `drive` | `app_theme_live` | scenario app_theme_live: dettivo-app re-skins on an Omarchy theme swap with no restart and reports the apply time | swaps the Omarchy theme symlink live and measures the apply time under 100 ms |
| R3 | `unit` | `qt::portalColourSchemeSignalFlipsThePalette` | portalColourSchemeSignalFlipsThePalette in qt/host/app/app_host_test.cpp | the mocked portal color-scheme change flips the palette off Omarchy |
| R3 | `unit` | `qt::malformedFileFallsBackAndReportsOnce` | malformedFileFallsBackAndReportsOnce in qt/qml/Dettivo/tests/theme_backend_test.cpp | an unreadable theme file keeps the previous palette and reports once |
| R4 | `bench` | `startup` | bench step startup | first frame and resident set after 5 s recorded from the app drive |
| R4 | `unit` | `qt::test_style_is_dettivo` | test_style_is_dettivo in qt/qml/Dettivo/tests/qml/tst_style.qml | the Dettivo style assertion on a planted control |
| R4 | `docs` | `docs/adr/0020-app-shell-thin-host-single-instance.md` | file docs/adr/0020-app-shell-thin-host-single-instance.md | records the measured first frame and the resident-set finding on the NVIDIA machine |
| R5 | `drive` | `app_daemon` | scenario app_daemon: dettivo-app shows the daemon-unavailable state, reconnects when dettivod restarts, and a second launch raises the first window | daemon killed and restarted under the app: banner shows and clears, the stream resumes, a second launch raises the first window, dettivo app open routes it |
| R5 | `unit` | `qt::instanceSocketAnswersOneLine` | instanceSocketAnswersOneLine in qt/host/app/app_host_test.cpp | the single-instance socket answers raise, open and status |
| R5 | `unit` | `dettivo-cli::open_raise_and_status_are_one_line_each_way` | fn open_raise_and_status_are_one_line_each_way in crates/dettivo-cli/tests/app.rs | dettivo app open routes the running window |
| R6 | `docs` | `docs/app.md` | file docs/app.md | routes, QA variables, the state file, the desktop entry and the theme sources |
| R6 | `docs` | `docs/adr/0020-app-shell-thin-host-single-instance.md` | file docs/adr/0020-app-shell-thin-host-single-instance.md | the host architecture and the single-instance socket decision |
| R6 | `script` | `scripts/check-docs.sh` | file scripts/check-docs.sh | the docs build and link check pass |

### `fn-18-mcp-server-stdio-transport-the-nineteen`

MCP server: stdio transport, the nineteen tools, resources and client configuration

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1 | `unit` | `dettivo-mcp::every_step_passes_in_both_framings_against_the_seeded_daemon` | fn every_step_passes_in_both_framings_against_the_seeded_daemon in crates/dettivo-mcp/tests/harness.rs | initialize, tools/list, resources/list and the representative tool calls in both framings against a live seeded daemon |
| R1 | `unit` | `dettivo-mcp::every_tool_definition_matches_its_fixture` | fn every_tool_definition_matches_its_fixture in crates/dettivo-mcp/tests/fixtures.rs | the nineteen tool definitions match the checked-in fixtures |
| R1, R5 | `pack` | `release/mcp_harness` | pack step release/mcp_harness | the MCP harness run in the QA rig, reported per tool and framing |
| R2 | `unit` | `dettivo-mcp::oversized_payloads_are_cut_with_the_markers` | fn oversized_payloads_are_cut_with_the_markers in crates/dettivo-mcp/tests/bounds.rs | the 20 000 character, 50 item, 100 key and depth 8 bounds cut with _truncated markers |
| R2 | `unit` | `dettivo-mcp::a_message_over_the_byte_cap_is_refused_naming_the_cap_and_the_session_continues` | fn a_message_over_the_byte_cap_is_refused_naming_the_cap_and_the_session_continues in crates/dettivo-mcp/tests/bounds.rs | a message over the byte cap is refused naming the cap and the session goes on |
| R3 | `unit` | `dettivo-mcp::a_daemon_that_is_down_answers_the_unavailable_text_and_the_session_survives` | fn a_daemon_that_is_down_answers_the_unavailable_text_and_the_session_survives in crates/dettivo-mcp/tests/harness.rs | the daemon-unavailable golden text |
| R3 | `unit` | `dettivo-mcp::a_refused_token_answers_the_hardened_text` | fn a_refused_token_answers_the_hardened_text in crates/dettivo-mcp/tests/harness.rs | the hardened token-mismatch golden text |
| R3 | `unit` | `dettivo-mcp::each_failure_class_gets_its_action_line` | fn each_failure_class_gets_its_action_line in crates/dettivo-mcp/src/messages.rs | automation and polish guidance lines while the capability is off |
| R4 | `unit` | `dettivo-cli::config_matches_the_goldens_for_every_host_plain_and_hardened` | fn config_matches_the_goldens_for_every_host_plain_and_hardened in crates/dettivo-cli/tests/mcp.rs | dettivo mcp config goldens for the four hosts, plain and hardened |
| R4 | `unit` | `dettivo-cli::write_merges_into_the_existing_file_and_keeps_other_servers` | fn write_merges_into_the_existing_file_and_keeps_other_servers in crates/dettivo-cli/tests/mcp.rs | --write merges into a seeded file keeping other servers |
| R4 | `unit` | `dettivo-cli::check_reports_the_tool_count_and_the_framing_against_the_daemon` | fn check_reports_the_tool_count_and_the_framing_against_the_daemon in crates/dettivo-cli/tests/mcp.rs | dettivo mcp check prints the tool count and framing against the live daemon |
| R4 | `unit` | `dettivo-cli::an_unknown_host_exits_4_naming_the_hosts` | fn an_unknown_host_exits_4_naming_the_hosts in crates/dettivo-cli/tests/mcp.rs | an unknown host exits 4 naming the hosts |
| R5 | `unit` | `dettivo-qa::report_counts_and_renders` | fn report_counts_and_renders in crates/dettivo-qa/src/replay.rs | the contract replay report carries the MCP section |
| R6 | `docs` | `docs/mcp.md` | file docs/mcp.md | the server, tools, resources, bounds, hosts and hardened mode |
| R6 | `docs` | `docs/adr/0019-mcp-hand-rolled-protocol-and-tool-mapping.md` | file docs/adr/0019-mcp-hand-rolled-protocol-and-tool-mapping.md | the hand-rolled protocol layer and the tool mapping |
| R6 | `contract` | `crates/dettivo-proto/fixtures/config/print_default.json` | file crates/dettivo-proto/fixtures/config/print_default.json | config print-default carries the [mcp] keys |

### `fn-19-audio-import-decoding-the-chunked-long`

Audio import: decoding, the chunked long-audio pipeline and the overlap merger

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1 | `unit` | `dettivo-audio::every_content_type_decodes_to_sixteen_k_mono_in_blocks` | fn every_content_type_decodes_to_sixteen_k_mono_in_blocks in crates/dettivo-audio/tests/decode.rs | every FR-S6 content type fixture decodes to 16 kHz mono in blocks within tolerance |
| R1 | `unit` | `dettivo-audio::a_mismatched_container_is_refused_naming_the_declared_and_detected_types` | fn a_mismatched_container_is_refused_naming_the_declared_and_detected_types in crates/dettivo-audio/tests/decode.rs | a mismatched container is refused naming both types |
| R2 | `unit` | `dettivo-transcribe::the_three_chunk_overlap_fixture_matches_its_golden` | fn the_three_chunk_overlap_fixture_matches_its_golden in crates/dettivo-transcribe/tests/job.rs | the three-chunk synthetic merge matches its golden |
| R2 | `unit` | `dettivo-transcribe::windows_overlap_by_two_seconds_and_cut_at_the_quiet_point_in_the_margin` | fn windows_overlap_by_two_seconds_and_cut_at_the_quiet_point_in_the_margin in crates/dettivo-transcribe/src/chunker.rs | the macOS window, overlap and margin values with the quiet-point cut |
| R2 | `unit` | `dettivo-transcribe::cancel_between_chunks_keeps_the_finished_chunks_and_no_timestamps_names_the_capability` | fn cancel_between_chunks_keeps_the_finished_chunks_and_no_timestamps_names_the_capability in crates/dettivo-transcribe/tests/job.rs | a non-timestamp engine fails the job naming the capability |
| R2, R5 | `pipeline` | `import-merge` | pipeline import-merge | the overlap fixtures through the engine CLI and the merger against goldens in CI |
| R3 | `unit` | `dettivo-transcribe::chunks_reach_the_engine_in_order_with_progress_and_a_silent_chunk_is_skipped` | fn chunks_reach_the_engine_in_order_with_progress_and_a_silent_chunk_is_skipped in crates/dettivo-transcribe/tests/job.rs | the silent chunk is skipped, fillers dropped, mixed speech kept |
| R3 | `unit` | `dettivo-transcribe::the_known_hallucinations_and_the_blank_markers_are_dropped` | fn the_known_hallucinations_and_the_blank_markers_are_dropped in crates/dettivo-transcribe/src/filters.rs | the macOS filler hallucination list is removed |
| R4 | `unit` | `dettivod::a_twelve_minute_import_uses_speech_windows_and_a_cancel_keeps_finished_chunks` | fn a_twelve_minute_import_uses_speech_windows_and_a_cancel_keeps_finished_chunks in crates/dettivod/tests/import_long.rs | twelve-minute import through tiny.en: three chunks, progress events, golden text, retained audio, cancel, max_import_seconds refusal |
| R4 | `unit` | `dettivo-cli::import_follows_the_job_and_get_words_prints_the_segments` | fn import_follows_the_job_and_get_words_prints_the_segments in crates/dettivo-cli/tests/history.rs | dettivo history import follows the progress and get --words prints segments |
| R4 | `contract` | `crates/dettivo-proto/fixtures/transfer` | directory crates/dettivo-proto/fixtures/transfer (7 entries) | the transfer fixtures still pass |
| R5 | `pack` | `release/mcp_harness` | pack step release/mcp_harness | the MCP harness's live import_audio step with the fixture WAV |
| R6 | `docs` | `docs/history.md` | file docs/history.md | the import section: formats, limits, chunking, progress events |
| R6 | `docs` | `docs/adr/0022-chunked-import-symphonia-decoder-and-overlap-merger.md` | file docs/adr/0022-chunked-import-symphonia-decoder-and-overlap-merger.md | the decoder and the merger's rules |
| R6 | `contract` | `crates/dettivo-proto/fixtures/config/print_default.json` | file crates/dettivo-proto/fixtures/config/print_default.json | [transcribe] and max_import_seconds printed by config print-default |

### `fn-20-visual-regression-baselines-for-every`

Visual regression: baselines for every surface and theme, frame pacing in drives, a canary

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1, R2 | `visual` | `home` | surface home in qa/visual/manifest.toml | dettivo-qa visual renders every manifest surface at 1x and 2x across the themes and diffs against the baselines |
| R1 | `unit` | `dettivo-qa::the_manifest_parses_surfaces_states_and_defaults` | fn the_manifest_parses_surfaces_states_and_defaults in crates/dettivo-qa/src/visual/manifest.rs | manifest parsing |
| R1 | `unit` | `dettivo-qa::the_matrix_is_states_times_themes_times_scales_with_the_right_baseline` | fn the_matrix_is_states_times_themes_times_scales_with_the_right_baseline in crates/dettivo-qa/src/visual/matrix.rs | matrix expansion and baseline resolution |
| R1 | `unit` | `dettivo-qa::the_report_counts_outcomes_and_never_passes_a_first_approval_silently` | fn the_report_counts_outcomes_and_never_passes_a_first_approval_silently in crates/dettivo-qa/src/visual/report.rs | the report and the exit on an unapproved difference |
| R2 | `visual` | `design-system` | surface design-system in qa/visual/manifest.toml | the design system sheet baselines |
| R2 | `visual` | `osd` | surface osd in qa/visual/manifest.toml | the six pill state baselines |
| R2 | `visual` | `insert-target` | surface insert-target in qa/visual/manifest.toml | the insert target baseline |
| R2 | `docs` | `docs/design/baselines.md` | file docs/design/baselines.md | every approval recorded |
| R3 | `unit` | `dettivo-qa::the_canary_passes_only_when_every_entry_fails` | fn the_canary_passes_only_when_every_entry_fails in crates/dettivo-qa/src/visual/report.rs | the canary run fails as expected |
| R3 | `unit` | `dettivo-qa::the_canary_passes_only_when_every_entry_fails` | fn the_canary_passes_only_when_every_entry_fails in crates/dettivo-qa/src/visual/report.rs | the canary verdict logic |
| R3 | `docs` | `docs/adr/0021-visual-regression-gate-and-frame-pacing.md` | file docs/adr/0021-visual-regression-gate-and-frame-pacing.md | the blocking CI job and the throwaway-branch proof recorded |
| R4 | `drive` | `theme_switch` | scenario theme_switch: the pill and the app re-theme within 100 ms of the theme directory changing, frames paced | pill and app theme apply latency and pacing through the shared collector |
| R4 | `drive` | `osd_dictation` | scenario osd_dictation: dettivo-osd shows Listening, Transcribing and Inserted through a mock-microphone dictation, then hides | swap intervals and render cost judged for the pill's animating stretches |
| R4 | `unit` | `dettivo-qa::the_drive_budget_scales_with_frames_and_names_every_breach` | fn the_drive_budget_scales_with_frames_and_names_every_breach in crates/dettivo-qa/src/pacing.rs | dropped frames over budget are named with their timestamps |
| R5 | `unit` | `dettivo-qa::style_findings_are_the_prefixed_lines` | fn style_findings_are_the_prefixed_lines in crates/dettivo-qa/src/visual/render.rs | the style-check findings from a render are read by name |
| R5 | `unit` | `qt::everyHookParsesAndBadValuesNameTheVariable` | everyHookParsesAndBadValuesNameTheVariable in qt/host/qa_environment_test.cpp | DETTIVO_QA_PLANT parses basic_control and default_font and refuses others by name |
| R5 | `unit` | `dettivo-core::every_hook_parses_and_bad_values_name_the_variable` | fn every_hook_parses_and_bad_values_name_the_variable in crates/dettivo-core/src/qa_tests.rs | the Rust side of the plant variable parser |
| R6 | `docs` | `docs/qa.md` | file docs/qa.md | the visual regression section: manifest, thresholds, approve flow, canary, pacing |
| R6 | `docs` | `docs/design/README.md` | file docs/design/README.md | every surface mapped to its crops and themes |
| R6 | `docs` | `docs/adr/0021-visual-regression-gate-and-frame-pacing.md` | file docs/adr/0021-visual-regression-gate-and-frame-pacing.md | the perceptual metric, thresholds and approval flow |

### `fn-21-first-run-keys-models-try-it`

First run: keys, models, try it

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1, R2, R3, R4 | `drive` | `first_run_fresh` | scenario first_run_fresh: a fresh profile shows Keys, Models and Try it: the golden snippet is written, a model is picked and downloaded, the mock microphone lands in the field | a fresh profile walks the three screens: snippet golden and write, live key press, catalogue download to ready, the take into the field, Done writes completed_at |
| R1 | `drive` | `first_run_provisioned` | scenario first_run_provisioned: a profile with a model and a sourced snippet opens Home and never sees first run | a provisioned profile opens Home |
| R1 | `pack` | `gui-onboarding/first_run_steps` | pack step gui-onboarding/first_run_steps | DETTIVO_E2E_STEP opens each step by name |
| R1 | `unit` | `qt::firstRunIsNotRequiredOnAProvisionedMachine` | firstRunIsNotRequiredOnAProvisionedMachine in qt/host/app/first_run_model_test.cpp | the trigger rule's cases |
| R2 | `unit` | `dettivod::snippet_and_setup_match_the_cli_and_write_idempotently` | fn snippet_and_setup_match_the_cli_and_write_idempotently in crates/dettivod/tests/hotkeys.rs | the snippet equals dettivo setup --stdout, setup writes once, unknown compositor refused |
| R2 | `unit` | `qt::firstRunDecidesKeysAndConfirmsAPress` | firstRunDecidesKeysAndConfirmsAPress in qt/host/app/first_run_model_test.cpp | the keys step confirms a press and shows the portal path for an unknown compositor |
| R2 | `contract` | `crates/dettivo-proto/fixtures/hotkeys` | directory crates/dettivo-proto/fixtures/hotkeys (4 entries) | hotkeys.snippet, hotkeys.setup and the unknown-compositor error fixtures |
| R3 | `unit` | `qt::firstRunModelsPickWriteAndFollowTheDownload` | firstRunModelsPickWriteAndFollowTheDownload in qt/host/app/first_run_model_test.cpp | rows from the catalogue, the tier default, [speech] and [llm] writes, progress and a failed download's reason |
| R3 | `unit` | `qt::test_models_lists_rows_and_holds_continue_until_ready` | test_models_lists_rows_and_holds_continue_until_ready in qt/qml/Dettivo/tests/qml/tst_app_first_run.qml | the step completes only when a model is ready |
| R4 | `unit` | `dettivod::the_self_target_allowance_is_held_by_the_step_and_refused_outside_it` | fn the_self_target_allowance_is_held_by_the_step_and_refused_outside_it in crates/dettivod/tests/insert.rs | the self-target allowance is armed by the step and refused outside it |
| R4 | `unit` | `qt::firstRunTryItArmsTheAllowanceAndReadsTheResult` | firstRunTryItArmsTheAllowanceAndReadsTheResult in qt/host/app/first_run_model_test.cpp | Try it arms, reads backend and time to insert, Done disarms and completes |
| R4 | `contract` | `crates/dettivo-proto/fixtures/insert/allow_self_target.json` | file crates/dettivo-proto/fixtures/insert/allow_self_target.json | the insert.allow_self_target delta fixture |
| R5 | `visual` | `first-run` | surface first-run in qa/visual/manifest.toml | the three screens region by region across themes and scales |
| R5 | `script` | `scripts/lint-accessible-names.sh` | file scripts/lint-accessible-names.sh | every first-run element has a documented accessible name |
| R6 | `docs` | `docs/app.md` | file docs/app.md | the first-run section and dettivo app open onboarding |
| R6 | `docs` | `docs/adr/0024-first-run-three-screens-config-writes.md` | file docs/adr/0024-first-run-three-screens-config-writes.md | the trigger rule and the self-target allowance |
| R6 | `docs` | `docs/api/linux-deltas.md` | file docs/api/linux-deltas.md | the deltas registered with fixtures |

### `fn-22-history-ui-the-list-detail-search-re`

History UI: the list, detail, search, re-run and export

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1, R2, R3 | `drive` | `history_seeded` | scenario history_seeded: History lists the seed by day, searches with painted hits, shows a detail with its facts, re-runs, exports and deletes | day labels, twelve rows, search hits and the painted match, the detail's texts, strip and facts, the re-run to a linked item, the export against the golden, the delete, the negative text scan |
| R1 | `unit` | `dettivod::get_carries_the_facts_and_a_search_hit_paints_its_matches` | fn get_carries_the_facts_and_a_search_hit_paints_its_matches in crates/dettivod/tests/history.rs | transcripts.get carries the facts block and search hits carry match ranges |
| R1 | `unit` | `qt::listSearchesPaintsFollowsJobsAndDrops` | listSearchesPaintsFollowsJobsAndDrops in qt/host/app/app_history_test.cpp | the list model searches, paints and follows jobs |
| R2 | `unit` | `qt::actionsRerunDeleteAndExportThroughTheLink` | actionsRerunDeleteAndExportThroughTheLink in qt/host/app/app_history_test.cpp | re-run, delete and export through the daemon link; a refused re-run names the reason |
| R2 | `unit` | `dettivod::a_dictation_is_stored_with_audio_reinserts_after_a_restart_and_reruns_through_the_engine` | fn a_dictation_is_stored_with_audio_reinserts_after_a_restart_and_reruns_through_the_engine in crates/dettivod/tests/history_model.rs | the daemon-backed re-run through tiny.en |
| R2 | `unit` | `dettivo-storage::seed_exports_match_the_goldens` | fn seed_exports_match_the_goldens in crates/dettivo-storage/tests/search.rs | the store's export goldens |
| R3 | `unit` | `qt::test_detail_shows_texts_take_facts_and_asks_before_delete` | test_detail_shows_texts_take_facts_and_asks_before_delete in qt/qml/Dettivo/tests/qml/tst_app_history_detail.qml | delete asks with the urgent text action before removing |
| R3 | `contract` | `crates/dettivo-proto/fixtures/transcripts/delete.json` | file crates/dettivo-proto/fixtures/transcripts/delete.json | transcripts.delete removes per the artifact policy |
| R4 | `visual` | `history` | surface history in qa/visual/manifest.toml | list, detail and facts regions against history.png crops on every theme |
| R5 | `unit` | `qt::test_route_keeps_list_beside_detail_and_opens_rows` | test_route_keeps_list_beside_detail_and_opens_rows in qt/qml/Dettivo/tests/qml/tst_app_history.qml | / focuses search, Escape returns, Enter opens |
| R5 | `script` | `scripts/lint-accessible-names.sh` | file scripts/lint-accessible-names.sh | every history element has an accessible name |
| R6 | `docs` | `docs/app.md` | file docs/app.md | the history section: routes, keys, export directory variable, playback |
| R6 | `docs` | `docs/adr/0025-history-detail-facts-playback-and-export-file.md` | file docs/adr/0025-history-detail-facts-playback-and-export-file.md | the playback and export decisions |

### `fn-23-local-llm-engine-llamacpp-through-the`

Local LLM engine: llama.cpp through the engine protocol, the GGUF catalogue and the local provider

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1 | `unit` | `dettivo-engine-llm::the_protocol_answers_every_request_shape_without_a_model` | fn the_protocol_answers_every_request_shape_without_a_model in crates/dettivo-engine-llm/tests/protocol.rs | load, generate, cancel, status and unload shapes; a missing model named with the reason |
| R1 | `unit` | `dettivo-engine-llm::cli_mode_prints_the_generate_json_the_protocol_carries` | fn cli_mode_prints_the_generate_json_the_protocol_carries in crates/dettivo-engine-llm/tests/cli.rs | the CLI mode prints the same JSON |
| R1 | `unit` | `dettivo-engine-llm::a_corrupt_model_is_load_failed_naming_the_path_and_the_reason` | fn a_corrupt_model_is_load_failed_naming_the_path_and_the_reason in crates/dettivo-engine-llm/tests/cli.rs | a corrupt model names the path and the reason |
| R1 | `unit` | `dettivo-engine-llm::the_protocol_streams_partials_and_stops_on_cancel_between_tokens` | fn the_protocol_streams_partials_and_stops_on_cancel_between_tokens in crates/dettivo-engine-llm/tests/cli.rs | partials and cancel through the real engine |
| R1 | `unit` | `dettivo-speech::a_crash_mid_generation_is_recorded_and_the_next_request_restarts_the_engine` | fn a_crash_mid_generation_is_recorded_and_the_next_request_restarts_the_engine in crates/dettivo-speech/tests/llm.rs | the supervisor's crash handling for the LLM slot |
| R1 | `unit` | `dettivo-speech::a_generation_past_its_timeout_is_cancelled_and_the_engine_serves_the_next_request` | fn a_generation_past_its_timeout_is_cancelled_and_the_engine_serves_the_next_request in crates/dettivo-speech/tests/llm.rs | the supervisor's timeout handling for the LLM slot |
| R2 | `unit` | `dettivo-speech::the_llm_catalogue_lists_the_macos_models_in_gguf` | fn the_llm_catalogue_lists_the_macos_models_in_gguf in crates/dettivo-speech/src/catalogue.rs | the four catalogue models with checksums and roles |
| R2 | `unit` | `dettivod::the_llm_catalogue_downloads_selects_and_deletes_through_the_llm_methods` | fn the_llm_catalogue_downloads_selects_and_deletes_through_the_llm_methods in crates/dettivod/tests/llm_models.rs | download verified through the fixture server, selection, NOT_FOUND naming the catalogue |
| R2 | `contract` | `crates/dettivo-proto/fixtures/llm` | directory crates/dettivo-proto/fixtures/llm (9 entries) | the llm.models.* fixtures |
| R3 | `unit` | `dettivod::the_local_provider_rewrites_a_polish_test_through_the_real_engine` | fn the_local_provider_rewrites_a_polish_test_through_the_real_engine in crates/dettivod/tests/llm_local.rs | polish.test in enhanced mode through the real 1.7B engine |
| R3 | `unit` | `dettivod::an_enhanced_dictation_inserts_the_local_rewrite` | fn an_enhanced_dictation_inserts_the_local_rewrite in crates/dettivod/tests/llm_local.rs | an enhanced dictation inserts the local rewrite |
| R3 | `docs` | `docs/reports/llm-goldens-report.json` | file docs/reports/llm-goldens-report.json | the polish goldens run through the real engine |
| R4 | `unit` | `dettivo-speech::the_real_engine_is_unloaded_after_the_idle_timeout_and_the_device_memory_drops` | fn the_real_engine_is_unloaded_after_the_idle_timeout_and_the_device_memory_drops in crates/dettivo-speech/tests/llm.rs | the process is gone after idle, status unloaded, device memory drops |
| R4 | `unit` | `dettivo-speech::the_llm_engine_is_reaped_on_its_own_idle_timeout` | fn the_llm_engine_is_reaped_on_its_own_idle_timeout in crates/dettivo-speech/tests/supervisor.rs | the supervisor reaps the LLM slot on its idle timeout |
| R4 | `docs` | `docs/reports/llm-idle-unload-report.json` | file docs/reports/llm-idle-unload-report.json | the recorded memory numbers |
| R5 | `unit` | `dettivo-language::auto_prefers_the_local_engine_when_its_model_is_on_disk` | fn auto_prefers_the_local_engine_when_its_model_is_on_disk in crates/dettivo-language/src/provider/mod.rs | the provider order over the availability matrix |
| R5 | `unit` | `dettivod::the_llm_catalogue_downloads_selects_and_deletes_through_the_llm_methods` | fn the_llm_catalogue_downloads_selects_and_deletes_through_the_llm_methods in crates/dettivod/tests/llm_models.rs | system.capabilities.llm.local_available follows the file on disk |
| R5 | `unit` | `dettivo-cli::the_json_matches_the_golden_by_shape_and_the_human_report_groups_the_rows` | fn the_json_matches_the_golden_by_shape_and_the_human_report_groups_the_rows in crates/dettivo-cli/tests/doctor.rs | dettivo doctor's llm row and provider order |
| R6 | `docs` | `docs/engines.md` | file docs/engines.md | the engine, catalogue, provider order and idle unload |
| R6 | `docs` | `docs/adr/0026-local-llm-engine-llama-cpp-and-the-gguf-catalogue.md` | file docs/adr/0026-local-llm-engine-llama-cpp-and-the-gguf-catalogue.md | the llama.cpp binding and the catalogue |
| R6 | `contract` | `crates/dettivo-proto/fixtures/config/print_default.json` | file crates/dettivo-proto/fixtures/config/print_default.json | [llm] and [engines.llm] keys printed by config print-default |

### `fn-24-meetings-dual-source-capture-the`

Meetings: dual-source capture, the session journal and recovery

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1, R2 | `drive` | `meeting_rig` | scenario meeting_rig: two null sinks feed a meeting's microphone and system tracks; the microphone sink is unloaded mid-meeting | two null sinks on real PipeWire: both tracks with correlated offsets, the mid-meeting mic switch to microphone-2.wav with the gap while the system track continues |
| R1 | `unit` | `dettivod::a_meeting_writes_both_tracks_streams_segments_and_finalises_and_the_fixtures_replay` | fn a_meeting_writes_both_tracks_streams_segments_and_finalises_and_the_fixtures_replay in crates/dettivod/tests/meetings.rs | mock fixtures: both WAVs, takes, journal, metadata, levels per source, stopping then stopped idempotently, the start/stop/cancel/status fixtures by name |
| R1, R2 | `unit` | `dettivo-meeting::a_loss_with_no_device_keeps_the_system_track_and_no_monitor_is_room_audio` | fn a_loss_with_no_device_keeps_the_system_track_and_no_monitor_is_room_audio in crates/dettivo-meeting/tests/session.rs | no monitor is microphone-only; a loss with no device keeps the system track and records the gap |
| R2 | `unit` | `dettivo-meeting::two_tracks_switch_stop_and_the_journal_says_what_happened` | fn two_tracks_switch_stop_and_the_journal_says_what_happened in crates/dettivo-meeting/tests/session.rs | the device switch restarts the microphone as a second take with its offset and the gap marker |
| R3 | `drive` | `meeting_recovery` | scenario meeting_recovery: a meeting killed mid-capture comes back partial with its takes; recover keeps it, discard removes it | SIGKILL mid-meeting, restart, partial with counts and takes, recover and discard |
| R3 | `unit` | `dettivod::a_killed_daemon_promotes_the_meeting_to_partial_and_recover_and_discard_settle_it` | fn a_killed_daemon_promotes_the_meeting_to_partial_and_recover_and_discard_settle_it in crates/dettivod/tests/meetings_recovery.rs | partial with is_partial and chunk counts from the checkpoint, recover keeps, discard removes, an unreadable checkpoint reports its reason |
| R3 | `unit` | `dettivo-meeting::a_killed_meeting_is_promoted_with_its_takes_repaired` | fn a_killed_meeting_is_promoted_with_its_takes_repaired in crates/dettivo-meeting/src/recovery.rs | WAV headers repaired from file length on recovery |
| R4 | `unit` | `dettivod::the_gates_refuse_in_order_and_the_disclosure_fixtures_answer` | fn the_gates_refuse_in_order_and_the_disclosure_fixtures_answer in crates/dettivod/tests/meetings.rs | disclosureRequired, the acknowledgement with timestamp, engineWithoutTimestamps and sessionActive in order |
| R4 | `contract` | `crates/dettivo-proto/fixtures/meetings/start.error-conflict-disclosure-required.json` | file crates/dettivo-proto/fixtures/meetings/start.error-conflict-disclosure-required.json | the disclosure gate fixture |
| R4 | `contract` | `crates/dettivo-proto/fixtures/meetings/start.error-conflict-engine-without-timestamps.json` | file crates/dettivo-proto/fixtures/meetings/start.error-conflict-engine-without-timestamps.json | the non-timestamp engine gate fixture |
| R5 | `contract` | `crates/dettivo-proto/fixtures/meetings` | directory crates/dettivo-proto/fixtures/meetings (34 entries) | every meetings.* fixture replays against the live daemon |
| R5 | `pack` | `release/contract_strict` | pack step release/contract_strict | the strict contract replay with the MEETING_PENDING list gone |
| R5 | `unit` | `dettivod::an_audio_import_into_the_meeting_kind_transcribes_into_a_meeting_row` | fn an_audio_import_into_the_meeting_kind_transcribes_into_a_meeting_row in crates/dettivod/tests/meetings_import.rs | the meeting-kind transcripts.* fixtures against the live daemon |
| R5 | `unit` | `dettivo-storage::the_timeline_interleaves_both_kinds_and_pages_through_a_tie` | fn the_timeline_interleaves_both_kinds_and_pages_through_a_tie in crates/dettivo-storage/src/timeline.rs | the migration's meetings table and the timeline listing both kinds |
| R5 | `pack` | `release/mcp_harness` | pack step release/mcp_harness | the MCP meeting tools answer the capture-level fields |
| R6 | `docs` | `docs/meetings.md` | file docs/meetings.md | capture, the journal, recovery and the gates |
| R6 | `docs` | `docs/adr/0027-two-stream-meeting-capture-journal-and-checkpoint.md` | file docs/adr/0027-two-stream-meeting-capture-journal-and-checkpoint.md | the two-stream capture and the checkpoint schema |
| R6 | `contract` | `crates/dettivo-proto/fixtures/config/print_default.json` | file crates/dettivo-proto/fixtures/config/print_default.json | [meetings] and [audio] system_source printed by config print-default |

### `fn-25-rest-shim-loopback-server-bearer-token`

REST shim: loopback server, bearer token, streaming import and export, the VS Code check

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1, R2, R3, R4 | `unit` | `dettivo-rest::the_daemon_hosts_the_shim_and_every_fixture_passes` | fn the_daemon_hosts_the_shim_and_every_fixture_passes in crates/dettivo-rest/tests/harness.rs | the daemon-hosted shim replays every route, auth, error and stream fixture against a live seeded daemon |
| R1 | `contract` | `crates/dettivo-rest/fixtures/routes` | directory crates/dettivo-rest/fixtures/routes (24 entries) | GET query and POST body-overlay route fixtures per implemented namespace |
| R1 | `contract` | `crates/dettivo-rest/fixtures/errors` | directory crates/dettivo-rest/fixtures/errors (10 entries) | 404 unknown route, 501 reserved shape and the app_code to status map fixtures |
| R1 | `unit` | `dettivo-rest::the_status_map_is_the_contracts` | fn the_status_map_is_the_contracts in crates/dettivo-rest/src/status.rs | the app_code to HTTP status map matches the contract |
| R2 | `contract` | `crates/dettivo-rest/fixtures/auth` | directory crates/dettivo-rest/fixtures/auth (5 entries) | missing, wrong, bearer, X-Dettivo-Token and non-loopback host fixtures |
| R2 | `unit` | `dettivo-core::the_three_sources_resolve_in_order` | fn the_three_sources_resolve_in_order in crates/dettivo-core/src/token.rs | the token resolves from env, Secret Service and the file in the macOS order |
| R2, R4 | `unit` | `dettivo-rest::no_token_means_no_listener_and_a_bad_bind_is_refused_at_validation` | fn no_token_means_no_listener_and_a_bad_bind_is_refused_at_validation in crates/dettivo-rest/tests/harness.rs | a non-loopback bind is refused at validation and no token means no listener |
| R3 | `contract` | `crates/dettivo-rest/fixtures/stream` | directory crates/dettivo-rest/fixtures/stream (9 entries) | WAV import stream, export stream by format and the refused variants |
| R3 | `unit` | `dettivo-rest::a_request_is_parsed_with_its_body_and_the_cap_is_enforced` | fn a_request_is_parsed_with_its_body_and_the_cap_is_enforced in crates/dettivo-rest/src/http.rs | a body over the cap is answered 413 |
| R3 | `contract` | `crates/dettivo-rest/fixtures/goldens` | directory crates/dettivo-rest/fixtures/goldens (2 entries) | export stream output matches the store's txt and md goldens |
| R4 | `unit` | `dettivo-rest::a_process_hosted_shim_over_the_socket_passes_the_same_fixtures` | fn a_process_hosted_shim_over_the_socket_passes_the_same_fixtures in crates/dettivo-rest/tests/harness.rs | dettivo rest serve hosts the shim as a process and the second server on the port fails naming the first |
| R4 | `unit` | `dettivo-cli::token_names_its_source_and_status_reports_no_listener` | fn token_names_its_source_and_status_reports_no_listener in crates/dettivo-cli/tests/rest.rs | dettivo rest status reports the listener and its token source |
| R4 | `contract` | `crates/dettivo-proto/fixtures/system/capabilities.json` | file crates/dettivo-proto/fixtures/system/capabilities.json | system.capabilities.rest reports the listener |
| R5 | `contract` | `crates/dettivo-rest/fixtures/vscode` | directory crates/dettivo-rest/fixtures/vscode (7 entries) | the VS Code extension's captured request shapes replay through the harness |
| R5 | `unit` | `dettivo-rest::every_fixture_parses_and_names_a_known_set` | fn every_fixture_parses_and_names_a_known_set in crates/dettivo-rest/src/harness.rs | dettivo-qa rest loads every fixture set including vscode in the contract replay |
| R6 | `docs` | `docs/rest.md` | file docs/rest.md | routes, auth, streaming, statuses and the VS Code setup |
| R6 | `docs` | `docs/adr/0028-rest-shim-hand-rolled-server-and-status-map.md` | file docs/adr/0028-rest-shim-hand-rolled-server-and-status-map.md | the ADR records the hand-rolled server and the status map |
| R6 | `contract` | `crates/dettivo-proto/fixtures/config/print_default.json` | file crates/dettivo-proto/fixtures/config/print_default.json | the [rest] keys are printed by config print-default |

### `fn-26-settings-routes-as-a-config-editor`

Settings routes as a config editor

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1, R3 | `drive` | `settings_roundtrip` | scenario settings_roundtrip: every settings key round-trips through its control and config.get, the comment survives, unset restores the default | every editable key of every section round-trips through the control and config.get, the comment survives, unset restores the default |
| R1 | `pack` | `gui-settings/lint_settings_keys` | pack step gui-settings/lint_settings_keys | the coverage lint proves every schema key sits on a route or is excused |
| R1 | `script` | `scripts/lint-settings-keys.sh` | file scripts/lint-settings-keys.sh | the lint fails on an undeclared key |
| R1 | `unit` | `qt::editorReadsWritesAndKeepsTheRefusal` | editorReadsWritesAndKeepsTheRefusal in qt/host/app/settings_model_test.cpp | an invalid value shows the daemon's message inline |
| R2 | `visual` | `settings` | surface settings in qa/visual/manifest.toml | Models, Hotkeys and Agents crops against the baselines on the themes |
| R2 | `visual` | `settings-pattern` | surface settings-pattern in qa/visual/manifest.toml | the general section follows the pattern |
| R3 | `drive` | `settings_env_override` | scenario settings_env_override: a key set through the environment shows its source badge and a disabled control naming the variable | an environment override shows the source badge and the disabled control |
| R3 | `unit` | `qt::environmentLocksARowAndTheBlockReadsLikeTheFile` | environmentLocksARowAndTheBlockReadsLikeTheFile in qt/host/app/settings_model_test.cpp | the environment source locks the row and the route shows its keys |
| R4 | `unit` | `qt::agentsHostsRunTheCommandLine` | agentsHostsRunTheCommandLine in qt/host/app/settings_model_test.cpp | the MCP host entries write through the same path as dettivo mcp config |
| R4 | `unit` | `qt::doctorRunsAndCopies` | doctorRunsAndCopies in qt/host/app/settings_model_test.cpp | Diagnostics renders the doctor report with copy |
| R5 | `contract` | `crates/dettivo-proto/fixtures/config/keys.json` | file crates/dettivo-proto/fixtures/config/keys.json | config.keys fixture passes and dettivo config keys --json follows the schema |
| R5 | `contract` | `crates/dettivo-proto/fixtures/events/subscribe.json` | file crates/dettivo-proto/fixtures/events/subscribe.json | config.changed is a subscribable topic |
| R5 | `unit` | `qt::aFileChangeRefreshesAndNotices` | aFileChangeRefreshesAndNotices in qt/host/app/settings_model_test.cpp | an editor refreshes when config.toml changes |
| R6 | `docs` | `docs/app.md` | file docs/app.md | the settings section |
| R6 | `docs` | `docs/adr/0033-settings-routes-config-editor-key-registry.md` | file docs/adr/0033-settings-routes-config-editor-key-registry.md | the ADR records the registry and the write path |

### `fn-27-gpu-tiers-the-doctor-report-and-the`

GPU tiers, the doctor report and the benchmark suite

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1 | `unit` | `dettivo-cli::the_json_matches_the_golden_by_shape_and_the_human_report_groups_the_rows` | fn the_json_matches_the_golden_by_shape_and_the_human_report_groups_the_rows in crates/dettivo-cli/tests/doctor.rs | dettivo doctor --json matches the fixture by shape and the human report groups the rows with the tier |
| R1 | `unit` | `dettivo-cli::an_unreachable_daemon_still_prints_the_machine_rows_and_exits_1` | fn an_unreachable_daemon_still_prints_the_machine_rows_and_exits_1 in crates/dettivo-cli/tests/doctor.rs | an unreachable daemon prints the machine rows and exits 1 |
| R2 | `bench` | `first_insert` | bench step first_insert | the suite runs on the GPU tier and under --cpu and writes the report |
| R2 | `unit` | `dettivo-qa::quick_runs_three_iterations` | fn quick_runs_three_iterations in crates/dettivo-qa/src/bench/mod.rs | --quick runs the CI shape of every step |
| R2 | `unit` | `dettivo-qa::models_resolve_the_preferred_file_and_name_the_download` | fn models_resolve_the_preferred_file_and_name_the_download in crates/dettivo-qa/src/bench/host.rs | a missing model skips the step naming the download |
| R2, R3 | `docs` | `docs/reports/benchmarks/README.md` | file docs/reports/benchmarks/README.md | the rendered README table over the checked-in reports |
| R3 | `docs` | `docs/reports/benchmarks/2026-09-05-thor-gpu.json` | file docs/reports/benchmarks/2026-09-05-thor-gpu.json | the first GPU report from this desktop |
| R3 | `docs` | `docs/reports/benchmarks/2026-09-05-thor-cpu.json` | file docs/reports/benchmarks/2026-09-05-thor-cpu.json | the first CPU report from this desktop |
| R3 | `docs` | `docs/adr/0029-nfr-calibration-and-the-benchmark-suite.md` | file docs/adr/0029-nfr-calibration-and-the-benchmark-suite.md | the ADR records the calibrated NFR targets against the initial ones |
| R3 | `unit` | `dettivo-qa::each_tier_reads_its_calibrated_first_insert_target` | fn each_tier_reads_its_calibrated_first_insert_target in crates/dettivo-qa/src/pack/report.rs | the dictation pack reads the calibrated targets per tier |
| R4 | `bench` | `idle_footprint` | bench step idle_footprint | the daemon's RSS after the idle window with the engines unloaded, against NFR-6 |
| R4 | `unit` | `dettivo-speech::spawns_once_keeps_warm_and_unloads_after_idle` | fn spawns_once_keeps_warm_and_unloads_after_idle in crates/dettivo-speech/tests/supervisor.rs | the engines' unload after the idle timeout is observed |
| R5 | `contract` | `crates/dettivo-proto/fixtures/system/capabilities.json` | file crates/dettivo-proto/fixtures/system/capabilities.json | system.capabilities.platform.tier is a registered delta with a fixture |
| R5 | `unit` | `dettivo-speech::the_tier_follows_the_override_the_device_and_the_engines` | fn the_tier_follows_the_override_the_device_and_the_engines in crates/dettivo-speech/src/tier.rs | DETTIVO_FORCE_CPU=1 forces the CPU tier |
| R5 | `docs` | `docs/api/linux-deltas.md` | file docs/api/linux-deltas.md | the tier and engine memory deltas are registered |
| R6 | `docs` | `docs/qa.md` | file docs/qa.md | the suite and the report schema |
| R6 | `docs` | `docs/daemon.md` | file docs/daemon.md | the doctor shape |
| R6 | `docs` | `docs/RELEASING.md` | file docs/RELEASING.md | the benchmark step of the release gate |

### `fn-28-meeting-transcription-the-live-windowed`

Meeting transcription: the live windowed path, the two-source merger and live events

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1 | `unit` | `dettivo-transcribe::windows_cut_per_tick_with_the_overlap_and_never_exceed_the_target` | fn windows_cut_per_tick_with_the_overlap_and_never_exceed_the_target in crates/dettivo-transcribe/src/live/windower.rs | window cuts and overlaps per tick with the macOS values |
| R1 | `unit` | `dettivo-transcribe::a_lagging_engine_skips_the_oldest_audio_and_silence_is_gated` | fn a_lagging_engine_skips_the_oldest_audio_and_silence_is_gated in crates/dettivo-transcribe/src/live/windower.rs | silent windows are gated and a lagging engine skips |
| R1 | `unit` | `dettivo-transcribe::provisional_tails_are_replaced_and_finalise_after_the_boundary_gap` | fn provisional_tails_are_replaced_and_finalise_after_the_boundary_gap in crates/dettivo-transcribe/src/live/merger.rs | provisional replacement and finalisation after the boundary gap |
| R1 | `unit` | `dettivo-transcribe::segments_interleave_by_time_with_the_remote_side_first_on_a_tie` | fn segments_interleave_by_time_with_the_remote_side_first_on_a_tie in crates/dettivo-transcribe/src/interleave.rs | the interleave with the cross-source suppression against the two-source golden |
| R1 | `unit` | `dettivo-meeting::a_result_with_text_but_no_segments_names_the_engine` | fn a_result_with_text_but_no_segments_names_the_engine in crates/dettivo-meeting/src/live.rs | a window from a non-timestamp result fails naming the engine |
| R2 | `drive` | `meeting_live` | scenario meeting_live: the mock tracks play two overlapping clips; meeting.segment streams provisional then final per source and the finalised transcript matches the two-source golden | meeting.segment events arrive provisional then final in time order and the finalised transcript matches the golden |
| R2 | `pack` | `meetings/meeting_live` | pack step meetings/meeting_live | the live transcript against the interleave golden in the meetings pack |
| R3 | `drive` | `meeting_rig` | scenario meeting_rig: two null sinks feed a meeting's microphone and system tracks; the microphone sink is unloaded mid-meeting | a device switch mid-meeting yields a transcript over both takes with the gap marker |
| R3 | `drive` | `meeting_recovery` | scenario meeting_recovery: a meeting killed mid-capture comes back partial with its takes; recover keeps it, discard removes it | recovery of a partial meeting finalises the same way |
| R3 | `unit` | `dettivod::a_meeting_writes_both_tracks_streams_segments_and_finalises_and_the_fixtures_replay` | fn a_meeting_writes_both_tracks_streams_segments_and_finalises_and_the_fixtures_replay in crates/dettivod/tests/meetings.rs | finalisation stores segments and transcript on the row and meetings.stop reports transcribing progress |
| R4 | `contract` | `crates/dettivo-proto/fixtures/meetings/search.json` | file crates/dettivo-proto/fixtures/meetings/search.json | meetings.search finds meeting text |
| R4 | `contract` | `crates/dettivo-proto/fixtures/transcripts/search.json` | file crates/dettivo-proto/fixtures/transcripts/search.json | transcripts.search covers meeting segments through FTS |
| R4 | `contract` | `crates/dettivo-proto/fixtures/meetings/get.json` | file crates/dettivo-proto/fixtures/meetings/get.json | meetings.get carries the segments and transcript |
| R4 | `pack` | `meetings/meeting_export_goldens` | pack step meetings/meeting_export_goldens | the exports carry meeting segments with source and timestamps in every format |
| R5 | `contract` | `crates/dettivo-proto/fixtures/events/meeting.segment.event.json` | file crates/dettivo-proto/fixtures/events/meeting.segment.event.json | the meeting.segment topic and payload snapshot |
| R5 | `unit` | `qt::meetingShowsListeningWithItsElapsedTime` | meetingShowsListeningWithItsElapsedTime in qt/host/osd/osd_host_test.cpp | the pill shows the meeting elapsed time while a meeting runs |
| R6 | `docs` | `docs/meetings.md` | file docs/meetings.md | the transcription section: live tuning, interleave rule, finalisation, recovery, CLI verbs |
| R6 | `docs` | `docs/adr/0031-live-windowed-meeting-transcription-and-cross-source-suppression.md` | file docs/adr/0031-live-windowed-meeting-transcription-and-cross-source-suppression.md | the ADR records the live path and the suppression rule |
| R6 | `contract` | `crates/dettivo-proto/fixtures/config/print_default.json` | file crates/dettivo-proto/fixtures/config/print_default.json | the live transcription keys are printed by config print-default |

### `fn-29-omarchy-plugin-the-bar-widget-the-panel`

Omarchy plugin: the bar widget, the panel that hosts the pill, and `dettivo setup omarchy`

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1 | `pack` | `gui-omarchy/omarchy_validate` | pack step gui-omarchy/omarchy_validate | the manifest and omarchy plugin validate pass |
| R1 | `script` | `scripts/lint-omarchy-plugin.sh` | file scripts/lint-omarchy-plugin.sh | a missing manifest field or absent entry point fails the lint naming it |
| R1 | `unit` | `dettivo-cli::the_manifest_carries_the_plugin_contract` | fn the_manifest_carries_the_plugin_contract in crates/dettivo-cli/src/omarchy.rs | the manifest carries the FR-B1 fields |
| R1 | `script` | `scripts/omarchy/export-plugin.sh` | file scripts/omarchy/export-plugin.sh | the exported folder validates and matches omarchy/ byte for byte |
| R2 | `unit` | `qt::test_glyph_states` | test_glyph_states in qt/qml/Dettivo/tests/qml/tst_bar.qml | the four widget states and the meeting timer on Dettivo.BarGlyph |
| R2 | `unit` | `qt::test_panel_rows_actions_and_signals` | test_panel_rows_actions_and_signals in qt/qml/Dettivo/tests/qml/tst_bar.qml | every panel row and action |
| R2 | `unit` | `qt::test_every_element_has_an_accessible_name` | test_every_element_has_an_accessible_name in qt/qml/Dettivo/tests/qml/tst_bar.qml | the accessible names on the bar and panel |
| R2 | `pack` | `gui-omarchy/omarchy_shim_load` | pack step gui-omarchy/omarchy_shim_load | the plugin QML loads under the shell shim with zero qmllint warnings |
| R2 | `unit` | `qt::test_style_is_dettivo` | test_style_is_dettivo in qt/qml/Dettivo/tests/qml/tst_style.qml | a control rendered by a non-Dettivo style fails the assertion |
| R3 | `drive` | `omarchy_bar` | scenario omarchy_bar: a CLI-started dictation turns the Omarchy bar widget to listening; the daemon's state and the bar's pixels agree | a CLI-started dictation drives the glyph and panel, history rows and dictation status agree with the crop |
| R3 | `pack` | `gui-omarchy/omarchy_bar` | pack step gui-omarchy/omarchy_bar | the bar widget's pixels against the daemon's state in the gui pack |
| R4 | `drive` | `omarchy_osd_host` | scenario omarchy_osd_host: the panel's bus-name claim makes dettivo-osd step aside; [omarchy] osd = "service" gives the pill back | the panel-hosted pill, the OmarchyPanel notice and the service restore |
| R5 | `unit` | `dettivo-cli::setup_omarchy_prints_its_plan_and_checks_every_step` | fn setup_omarchy_prints_its_plan_and_checks_every_step in crates/dettivo-cli/tests/setup.rs | dettivo setup omarchy runs every step and --check reports them |
| R5 | `drive` | `omarchy_setup_idempotent` | scenario omarchy_setup_idempotent: dettivo setup omarchy run twice changes nothing by hash and --check reports every step ok | setup omarchy on a profile twice changes nothing |
| R5 | `pack` | `gui-omarchy/omarchy_version_skew` | pack step gui-omarchy/omarchy_version_skew | the upgrade hint against an old system.version and the install hint with the module absent |
| R5 | `unit` | `qt::test_2_upgrade_hint_against_an_old_daemon` | test_2_upgrade_hint_against_an_old_daemon in qt/fixtures/omarchy-shell/tests/tst_plugin_hints.qml | the version-skew upgrade hint |
| R6 | `visual` | `panel` | surface panel in qa/visual/manifest.toml | idle, recording, transcribing, meeting and hint states on the palettes at 1x and 2x |
| R6 | `visual` | `bar-glyph` | surface bar-glyph in qa/visual/manifest.toml | the five glyph states on the palettes |
| R6 | `docs` | `docs/design/baselines.md` | file docs/design/baselines.md | the approved baselines are recorded |
| R7 | `docs` | `docs/omarchy.md` | file docs/omarchy.md | install, the panel, the settings keys, the mirror repository, setup omarchy |
| R7 | `docs` | `docs/adr/0030-omarchy-plugin-in-repo-folder-mirror-and-panel-hosted-pill.md` | file docs/adr/0030-omarchy-plugin-in-repo-folder-mirror-and-panel-hosted-pill.md | the ADR records the in-repo plugin folder, the mirror and the panel-hosted pill |
| R7 | `script` | `scripts/check-docs.sh` | file scripts/check-docs.sh | just docs fails on a missing key or an unindexed ADR |

### `fn-30-polish-models-gguf-conversion`

Polish models: GGUF conversion, the sideloaded fine-tune, the eval harness port and the hard-gate report

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1 | `unit` | `dettivo-speech::the_macos_manifest_shape_resolves_to_a_fused_gguf` | fn the_macos_manifest_shape_resolves_to_a_fused_gguf in crates/dettivo-speech/src/llm/sideload.rs | the sideload resolver against the macOS manifest shape and the fused GGUF form |
| R1 | `unit` | `dettivo-speech::the_lora_form_names_its_base_and_needs_one` | fn the_lora_form_names_its_base_and_needs_one in crates/dettivo-speech/src/llm/sideload.rs | the LoRA-over-qwen3-1.7b form |
| R1 | `unit` | `dettivod::a_fused_sideload_is_listed_selected_and_never_downloadable` | fn a_fused_sideload_is_listed_selected_and_never_downloadable in crates/dettivod/tests/llm_sideload.rs | llm.models.status lists the sideload with source = sideload and never among download ids |
| R1 | `unit` | `dettivod::an_enhanced_polish_test_names_the_sideloaded_model` | fn an_enhanced_polish_test_names_the_sideloaded_model in crates/dettivod/tests/llm_sideload.rs | Enhanced with polish_experiment = current runs on the tuned model |
| R1 | `unit` | `dettivod::an_unresolvable_manifest_is_reported_by_name_and_enhanced_stays_on_the_model` | fn an_unresolvable_manifest_is_reported_by_name_and_enhanced_stays_on_the_model in crates/dettivod/tests/llm_sideload.rs | an unresolvable manifest reports manifest_error and Enhanced falls back to [llm] model |
| R2 | `unit` | `dettivo-qa::the_sample_set_with_the_recorded_rewrites_matches_the_golden` | fn the_sample_set_with_the_recorded_rewrites_matches_the_golden in crates/dettivo-qa/tests/polish_eval.rs | polish-eval on the sample set with the fixture LLM matches the checked-in golden |
| R2 | `unit` | `dettivo-qa::every_metric_has_a_passing_and_a_failing_row` | fn every_metric_has_a_passing_and_a_failing_row in crates/dettivo-qa/src/polish_eval/tests.rs | the scorer against cases ported from score_results.py |
| R2 | `unit` | `dettivo-qa::the_gate_states_every_hard_gate_and_names_a_failure` | fn the_gate_states_every_hard_gate_and_names_a_failure in crates/dettivo-qa/src/polish_eval/tests.rs | the gate against cases ported from check_promotion.py |
| R3 | `docs` | `docs/reports/polish-eval/2026-09-05-qwen3-4b-instruct-2507-eval-private-whisper-heldout-v1.json` | file docs/reports/polish-eval/2026-09-05-qwen3-4b-instruct-2507-eval-private-whisper-heldout-v1.json | the 4B held-out report |
| R3 | `docs` | `docs/reports/polish-eval/2026-09-05-qwen3-1.7b-converted-eval-private-parakeet-heldout-v1.json` | file docs/reports/polish-eval/2026-09-05-qwen3-1.7b-converted-eval-private-parakeet-heldout-v1.json | the sideloaded 1.7B held-out report |
| R3 | `docs` | `docs/reports/polish-eval/macos-baseline.json` | file docs/reports/polish-eval/macos-baseline.json | the macOS comparison with every hard gate stated |
| R3 | `unit` | `dettivo-qa::the_checked_in_reports_carry_metrics_and_row_ids_only` | fn the_checked_in_reports_carry_metrics_and_row_ids_only in crates/dettivo-qa/tests/polish_eval.rs | the reports carry no row text |
| R4 | `unit` | `dettivo-language::the_raw_layer_scores_the_macos_dictionary_set_as_the_report_records` | fn the_raw_layer_scores_the_macos_dictionary_set_as_the_report_records in crates/dettivo-language/tests/dictionary_eval.rs | the dictionary eval reports the macOS accuracy figure for the shared rows |
| R4 | `docs` | `docs/reports/polish-eval/dictionary-eval-v1.json` | file docs/reports/polish-eval/dictionary-eval-v1.json | the filed dictionary-correction eval |
| R5 | `script` | `scripts/models/convert-polish-finetune.sh` | file scripts/models/convert-polish-finetune.sh | --dry-run validates the toolchain and paths; the full conversion writes the GGUF and current.json |
| R5 | `human` | `.flow/tasks/fn-30-polish-models-gguf-conversion.1.md` | receipt file .flow/tasks/fn-30-polish-models-gguf-conversion.1.md: a route a person walks, not proof that they did | the full conversion of Qwen3-1.7B and the engine load were run on this machine |
| R6 | `docs` | `docs/polish-models.md` | file docs/polish-models.md | the runbook, the manifest, the opt-in key, the eval command and how to read a report |
| R6 | `docs` | `docs/adr/0032-polish-fine-tune-sideload-and-the-eval-harness.md` | file docs/adr/0032-polish-fine-tune-sideload-and-the-eval-harness.md | the ADR records the sideload path, the harness port and the optional-first rule |
| R6 | `docs` | `docs/api/linux-deltas.md` | file docs/api/linux-deltas.md | the new fields are registered deltas |

### `fn-31-packaging-aur-recipes-release-workflow`

Packaging: the AUR recipes, the release workflow, engine and QML install layout, systemd and desktop files

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1 | `script` | `scripts/packaging/test-check-manifest.sh` | file scripts/packaging/test-check-manifest.sh | a missing or extra path against packaging/manifest.txt fails naming it |
| R1 | `script` | `scripts/package.sh` | file scripts/package.sh | just package produces the tree and tarball with the module and plugin folder |
| R1 | `unit` | `dettivo-speech::the_tier_follows_the_override_the_device_and_the_engines` | fn the_tier_follows_the_override_the_device_and_the_engines in crates/dettivo-speech/src/tier.rs | the engines report Vulkan and cpu under DETTIVO_FORCE_CPU=1 |
| R2 | `script` | `scripts/packaging/build-package.sh` | file scripts/packaging/build-package.sh | both PKGBUILDs build with makepkg, namcap and .SRCINFO checked |
| R2 | `script` | `scripts/packaging/lint.sh` | file scripts/packaging/lint.sh | namcap and .SRCINFO against makepkg --printsrcinfo |
| R2, R4 | `script` | `scripts/packaging/install-test.sh` | file scripts/packaging/install-test.sh | the clean-container install with every check green in install-test.json |
| R2, R4 | `pack` | `release/install_test` | pack step release/install_test | the packaging step of the release pack |
| R3 | `script` | `scripts/packaging/release-notes.sh` | file scripts/packaging/release-notes.sh | the notes extracted from CHANGELOG.md, failing on a missing section |
| R3 | `docs` | `docs/RELEASING.md` | file docs/RELEASING.md | the release workflow, dry_run and the tag run with the three files |
| R3 | `human` | `.flow/tasks/fn-31-packaging-aur-recipes-release-workflow.1.md` | receipt file .flow/tasks/fn-31-packaging-aur-recipes-release-workflow.1.md: a route a person walks, not proof that they did | the dry-run workflow run on the branch is recorded |
| R4 | `script` | `scripts/packaging/install-test-session.sh` | file scripts/packaging/install-test-session.sh | the --session arm: socket activation, setup omarchy --check, the OSD unit and one mock-mic dictation |
| R5 | `unit` | `xtask::a_missing_and_a_stale_crate_are_named` | fn a_missing_and_a_stale_crate_are_named in tools/xtask/src/notice.rs | lint-notice fails when Cargo.lock gains a dependency the notice does not cover |
| R5 | `unit` | `dettivo-cli::the_three_shells_get_a_script_that_names_the_subcommands` | fn the_three_shells_get_a_script_that_names_the_subcommands in crates/dettivo-cli/tests/completions.rs | dettivo completions for the three shells install and load |
| R6 | `docs` | `docs/RELEASING.md` | file docs/RELEASING.md | the whole release gate |
| R6 | `docs` | `docs/install.md` | file docs/install.md | dettivo-bin, dettivo, the layout and the engine discovery order |
| R6 | `docs` | `docs/adr/0034-install-layout-cuda-drop-in-and-release-workflow.md` | file docs/adr/0034-install-layout-cuda-drop-in-and-release-workflow.md | the ADR records the install layout, the CUDA drop-in and the workflow |

### `fn-32-diarization-the-sherpa-onnx-engine-the`

Diarization: the sherpa-onnx engine process, the post-meeting speaker pass and speaker names

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1 | `pipeline` | `diarization` | pipeline diarization | two-speakers.wav diarizes under DER 0.20 with two speakers on the CPU; --bench records the realtime factor |
| R1 | `pack` | `meetings/diarization_der` | pack step meetings/diarization_der | two speakers, DER under 0.20 |
| R1 | `unit` | `dettivo-engine-diarize::the_fixture_diarizes_into_the_expected_turns` | fn the_fixture_diarizes_into_the_expected_turns in crates/dettivo-engine-diarize/tests/cli.rs | CLI mode diarizes the fixture |
| R1 | `unit` | `dettivo-engine-diarize::a_pass_reports_chunk_progress_and_a_cancel_is_answered_at_once` | fn a_pass_reports_chunk_progress_and_a_cancel_is_answered_at_once in crates/dettivo-engine-diarize/tests/protocol.rs | load, diarize with progress, cancel, status and unload through the protocol |
| R1 | `unit` | `dettivo-engine-diarize::a_missing_model_names_its_path_and_a_bad_wav_is_refused` | fn a_missing_model_names_its_path_and_a_bad_wav_is_refused in crates/dettivo-engine-diarize/tests/cli.rs | model_missing names the path and a bad attachment is bad_request |
| R2 | `unit` | `dettivod::a_two_track_meeting_learns_its_speakers_and_a_rerun_replaces_them` | fn a_two_track_meeting_learns_its_speakers_and_a_rerun_replaces_them in crates/dettivod/tests/meetings_diarize.rs | the auto post-pass labels remote segments under the coverage and share rule, microphone segments are You, talk_ms and diarizing progress |
| R2 | `unit` | `dettivo-meeting::the_rule_labels_covered_segments_and_leaves_straddling_ones_alone` | fn the_rule_labels_covered_segments_and_leaves_straddling_ones_alone in crates/dettivo-meeting/src/diarize.rs | the 0.25 coverage and 0.60 share rule leaves the straddling segment unlabeled |
| R2 | `unit` | `dettivod::a_room_audio_import_diarizes_the_whole_track_and_renames_reach_everything` | fn a_room_audio_import_diarizes_the_whole_track_and_renames_reach_everything in crates/dettivod/tests/meetings_diarize.rs | a microphone-only meeting diarizes the whole track |
| R2 | `unit` | `dettivod::a_missing_model_set_leaves_the_meeting_completed_and_unavailable` | fn a_missing_model_set_leaves_the_meeting_completed_and_unavailable in crates/dettivod/tests/meetings_diarize.rs | model absent leaves diarization.status = unavailable with the command and the transcript intact |
| R2 | `contract` | `crates/dettivo-proto/fixtures/meetings/diarize.error-not-found-model-missing.json` | file crates/dettivo-proto/fixtures/meetings/diarize.error-not-found-model-missing.json | the model-missing shape |
| R3 | `contract` | `crates/dettivo-proto/fixtures/meetings/diarize.error-conflict-running.json` | file crates/dettivo-proto/fixtures/meetings/diarize.error-conflict-running.json | meetings.diarize refuses while running with the named CONFLICT |
| R3 | `contract` | `crates/dettivo-proto/fixtures/meetings/diarize.error-conflict-not-completed.json` | file crates/dettivo-proto/fixtures/meetings/diarize.error-conflict-not-completed.json | meetings.diarize refuses before finalisation |
| R3 | `unit` | `dettivod::a_two_track_meeting_learns_its_speakers_and_a_rerun_replaces_them` | fn a_two_track_meeting_learns_its_speakers_and_a_rerun_replaces_them in crates/dettivod/tests/meetings_diarize.rs | a re-run on a completed meeting replaces the assignment with progress notices |
| R4 | `unit` | `dettivod::an_engine_crash_fails_the_pass_keeps_the_daemon_and_degrades_after_three` | fn an_engine_crash_fails_the_pass_keeps_the_daemon_and_degrades_after_three in crates/dettivod/tests/meetings_diarize_crash.rs | an injected crash fails the job with the redacted reason, the daemon answers ping, the meeting stays completed, a third crash degrades the engine |
| R5 | `contract` | `crates/dettivo-proto/fixtures/meetings/speakers.rename.json` | file crates/dettivo-proto/fixtures/meetings/speakers.rename.json | meetings.speakers.rename passes against the live daemon |
| R5 | `contract` | `crates/dettivo-proto/fixtures/meetings/speakers.suggest.json` | file crates/dettivo-proto/fixtures/meetings/speakers.suggest.json | meetings.speakers.suggest ordered by recency |
| R5 | `contract` | `crates/dettivo-proto/fixtures/meetings/speakers.rename.error-invalid-params-too-long.json` | file crates/dettivo-proto/fixtures/meetings/speakers.rename.error-invalid-params-too-long.json | an over-long name is INVALID_PARAMS naming the limit |
| R5 | `unit` | `dettivod::a_room_audio_import_diarizes_the_whole_track_and_renames_reach_everything` | fn a_room_audio_import_diarizes_the_whole_track_and_renames_reach_everything in crates/dettivod/tests/meetings_diarize.rs | a rename updates every segment, the five exports and FTS |
| R5 | `unit` | `dettivo-speech::the_diarization_model_set_lists_both_files` | fn the_diarization_model_set_lists_both_files in crates/dettivo-speech/src/catalogue.rs | the catalogue lists the two-file model set |
| R5 | `unit` | `dettivo-speech::a_wrong_checksum_is_quarantined` | fn a_wrong_checksum_is_quarantined in crates/dettivo-speech/tests/downloads.rs | a checksum mismatch from the fixture model server quarantines the download |
| R6 | `docs` | `docs/meetings.md` | file docs/meetings.md | the diarization section: the rule, the labels, re-run, renames, the CLI verbs |
| R6 | `docs` | `docs/engines.md` | file docs/engines.md | the fourth engine |
| R6 | `docs` | `docs/models.md` | file docs/models.md | the diarization model set |
| R6 | `docs` | `docs/adr/0035-sherpa-onnx-diarization-engine-and-the-speaker-pass.md` | file docs/adr/0035-sherpa-onnx-diarization-engine-and-the-speaker-pass.md | the ADR records the sherpa-onnx engine, the binding choice and the assignment rule |
| R6 | `contract` | `crates/dettivo-proto/fixtures/config/print_default.json` | file crates/dettivo-proto/fixtures/config/print_default.json | the diarization keys are printed by config print-default |

### `fn-33-meeting-notes-analysis-search-export`

Meeting notes, analysis, search, export, delete and the disclosure

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1 | `unit` | `dettivod::notes_land_on_the_row_and_in_notes_md_and_the_search_names_the_column` | fn notes_land_on_the_row_and_in_notes_md_and_the_search_names_the_column in crates/dettivod/tests/meetings_notes.rs | notes.set writes the row and notes.md, notes.get reads them, FTS finds them with matched_field = notes, the 1 MiB body is INVALID_PARAMS |
| R1 | `unit` | `dettivo-storage::meetings_are_searchable_and_stale_jobs_fail` | fn meetings_are_searchable_and_stale_jobs_fail in crates/dettivo-storage/src/timeline.rs | the storage FTS index over notes and analysis returns the matched column |
| R2 | `unit` | `dettivo-language::the_analysis_cases_hold` | fn the_analysis_cases_hold in crates/dettivo-language/tests/analysis_goldens.rs | the seven meeting_analysis.json goldens: clean, fenced, camel-case, repair, double failure, empty, timeout, map then reduce |
| R2 | `unit` | `dettivod::the_analysis_runs_map_then_reduce_from_the_fixture_and_keeps_the_old_one_on_failure` | fn the_analysis_runs_map_then_reduce_from_the_fixture_and_keeps_the_old_one_on_failure in crates/dettivod/tests/meetings_notes.rs | DETTIVO_MOCK_LLM=fixture:<dir> over chunk_chars runs parts=2, force regenerates, timeout is failed with provider_unavailable, a degenerate answer fails after one repair |
| R2 | `unit` | `dettivod::the_seeded_meeting_is_analysed_through_the_real_engine` | fn the_seeded_meeting_is_analysed_through_the_real_engine in crates/dettivod/tests/llm_local.rs | the model-backed run on llm/qwen3-1.7b passes by name here and skips without the model |
| R3 | `unit` | `dettivo-storage::the_rich_seed_meeting_renders_byte_equal_to_the_goldens` | fn the_rich_seed_meeting_renders_byte_equal_to_the_goldens in crates/dettivo-storage/tests/meeting_export.rs | the seed meeting renders byte-equal to goldens/meeting.{txt,md,srt,vtt,json} |
| R3 | `unit` | `dettivo-storage::the_live_notes_win_and_raw_renders_the_engine_words` | fn the_live_notes_win_and_raw_renders_the_engine_words in crates/dettivo-storage/tests/meeting_export.rs | notes_override beats saved notes beats analysis in md; raw = true renders the raw segments |
| R3 | `unit` | `dettivod::a_meeting_export_renders_the_seed_in_every_format_over_a_download` | fn a_meeting_export_renders_the_seed_in_every_format_over_a_download in crates/dettivod/tests/meetings_import.rs | the daemon export over a download; an unknown format is INVALID_PARAMS listing the five |
| R4 | `unit` | `dettivod::transcript_only_keeps_the_row_the_notes_and_the_audio` | fn transcript_only_keeps_the_row_the_notes_and_the_audio in crates/dettivod/tests/meetings_delete.rs | transcript_only clears texts, segments, speakers, analysis and the FTS row and keeps row, notes and audio |
| R4 | `unit` | `dettivod::transcript_and_audio_removes_the_takes_and_sidecars_and_keeps_the_facts_and_notes` | fn transcript_and_audio_removes_the_takes_and_sidecars_and_keeps_the_facts_and_notes in crates/dettivod/tests/meetings_delete.rs | transcript_and_audio removes the WAVs and sidecars and keeps the facts and notes |
| R4 | `unit` | `dettivod::all_removes_the_row_and_the_directory_and_a_missing_directory_is_fine` | fn all_removes_the_row_and_the_directory_and_a_missing_directory_is_fine in crates/dettivod/tests/meetings_delete.rs | all removes the row and the meeting directory |
| R4 | `unit` | `dettivod::the_active_meeting_is_a_conflict` | fn the_active_meeting_is_a_conflict in crates/dettivod/tests/meetings_delete.rs | deleting the active meeting is CONFLICT |
| R5 | `contract` | `crates/dettivo-proto/fixtures/meetings` | directory crates/dettivo-proto/fixtures/meetings (34 entries) | every meetings.* fixture (notes, analyze, analysis.get, the seed-backed list and search) replays against the live daemon |
| R5 | `unit` | `dettivod::implemented_fixtures_pass_against_the_live_socket` | fn implemented_fixtures_pass_against_the_live_socket in crates/dettivod/tests/contract.rs | the daemon contract test replays every implemented fixture and reports no pending meeting method |
| R5 | `unit` | `dettivo-cli::notes_analysis_export_delete_and_the_disclosure_copy_work_end_to_end` | fn notes_analysis_export_delete_and_the_disclosure_copy_work_end_to_end in crates/dettivo-cli/tests/meetings.rs | the CLI --json snapshots for the new verbs and disclosure --copy landing the macOS text in the mock sink |
| R5 | `unit` | `dettivod::the_disclosure_variable_seeds_or_clears_the_acknowledgement` | fn the_disclosure_variable_seeds_or_clears_the_acknowledgement in crates/dettivod/tests/meetings_notes.rs | DETTIVO_E2E_DISCLOSURE=acknowledged\|pending seeds or clears the acknowledgement |
| R5 | `contract` | `crates/dettivo-proto/fixtures/system/capabilities.json` | file crates/dettivo-proto/fixtures/system/capabilities.json | system.capabilities.meetings.methods lists the new verbs |
| R6 | `docs` | `docs/meetings.md` | file docs/meetings.md | notes, analysis, search, export, delete and disclosure sections with the precedence rule |
| R6 | `contract` | `crates/dettivo-proto/fixtures/config/print_default.json` | file crates/dettivo-proto/fixtures/config/print_default.json | config print-default prints [meetings.analysis] and delete_artifact_policy |
| R6 | `docs` | `docs/adr/0036-meeting-notes-analysis-search-export-and-the-delete-policy.md` | file docs/adr/0036-meeting-notes-analysis-search-export-and-the-delete-policy.md | the ADR records the analysis path, the polished segments and the delete policy |
| R6 | `script` | `scripts/check-docs.sh` | file scripts/check-docs.sh | the docs check indexes the ADR and the qa.md variable table |

### `fn-34-meetings-gui-the-list-the-live-meeting`

Meetings GUI: the list, the live meeting, the detail, speakers and notes

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1 | `drive` | `meetings_seeded` | scenario meetings_seeded: Meetings lists the seed by week with chips and swatches, shows the detail with its bar, tabs and rail, exports, renames a speaker and deletes | the week labels, seeded rows with chips and swatches, detail facts, tabs, Raw/Polished, rename popover, md export against the golden, delete after confirmation, negative text scan |
| R1 | `pack` | `meetings/meetings_seeded` | pack step meetings/meetings_seeded | the same drive on both drivers inside the meetings pack |
| R2 | `drive` | `meetings_live_gui` | scenario meetings_live_gui: the disclosure dialog on a fresh profile, Acknowledge and start, both meters, provisional then final segments, Stop into the detail | disclosure dialog on a fresh profile, Not now, Acknowledge and start, both meters, provisional then final rows, Stop through Stopping and Transcribing into the detail |
| R2 | `pack` | `meetings/meetings_live_gui` | pack step meetings/meetings_live_gui | the live drive in the pack with the mock fixtures on the CI path |
| R3 | `drive` | `meetings_import_gui` | scenario meetings_import_gui: the import dialog rejects invalid audio and transcribes the jfk clip; row Cancel and Recover preserve the imported meeting's ID, audio and notes | the refused non-audio file names the daemon's reason, then the jfk clip imports through transcripts.import and opens the row |
| R3 | `pack` | `meetings/meetings_import_gui` | pack step meetings/meetings_import_gui | the import drive on both drivers inside the meetings pack |
| R4 | `visual` | `meetings` | surface meetings in qa/visual/manifest.toml | the list column and the rail against the meetings-list artboard crops on five palettes at 1x and 2x |
| R4 | `visual` | `meetings-empty` | surface meetings-empty in qa/visual/manifest.toml | the list without rows against its approved renders |
| R4 | `visual` | `meeting-live` | surface meeting-live in qa/visual/manifest.toml | the live screen's header, transcript and rail against the artboard crops |
| R4 | `visual` | `meeting-detail` | surface meeting-detail in qa/visual/manifest.toml | the detail's header, transcript and rail against the artboard crops |
| R4 | `visual` | `meeting-detail-tabs` | surface meeting-detail-tabs in qa/visual/manifest.toml | the Notes and Analysis tabs and the rename popover against approved renders |
| R4 | `visual` | `meeting-dialogs` | surface meeting-dialogs in qa/visual/manifest.toml | the import and disclosure boxes through render_crop against the artboard |
| R4 | `unit` | `dettivo-qa::the_report_counts_outcomes_and_never_passes_a_first_approval_silently` | fn the_report_counts_outcomes_and_never_passes_a_first_approval_silently in crates/dettivo-qa/src/visual/report.rs | a missing crop or fixture fails by name; a first approval is reported with the command, never passed |
| R5 | `unit` | `qt::dettivo-app-meetings-test` | dettivo-app-meetings-test in qt/host/app/CMakeLists.txt | tst_app_meetings.qml and tst_app_meetings_detail.qml cover every meetings component's states over the shared fakes |
| R5 | `unit` | `qt::test_detail_tabs_toggle_rename_export_and_delete` | test_detail_tabs_toggle_rename_export_and_delete in qt/qml/Dettivo/tests/qml/tst_app_meetings_detail.qml | the detail's tabs, Raw/Polished toggle, rename popover, export sheet and delete confirmation |
| R5 | `script` | `scripts/lint-accessible-names.sh` | file scripts/lint-accessible-names.sh | every element carries an accessible name listed in accessible-names.txt |
| R5 | `unit` | `dettivo-core::every_hook_parses_and_bad_values_name_the_variable` | fn every_hook_parses_and_bad_values_name_the_variable in crates/dettivo-core/src/qa_tests.rs | DETTIVO_E2E_OPEN=meetings\|meetings.live\|meetings.detail and DETTIVO_E2E_MEETING_STATE parse in the QA hook list |
| R5 | `unit` | `dettivo-cli::open_raise_and_status_are_one_line_each_way` | fn open_raise_and_status_are_one_line_each_way in crates/dettivo-cli/tests/app.rs | dettivo app open routes through the router |
| R6 | `docs` | `docs/app.md` | file docs/app.md | the Meetings section: routes, keys, the state variable, the rail's config keys, the history chip |
| R6 | `docs` | `docs/qa/a11y-names.md` | file docs/qa/a11y-names.md | the meetings accessible names |
| R6 | `docs` | `docs/adr/0038-meetings-gui-live-events-pause-gap-and-inline-dialogs.md` | file docs/adr/0038-meetings-gui-live-events-pause-gap-and-inline-dialogs.md | the ADR records the live model's event handling, the pause gap and the inline-designed popover |
| R6 | `unit` | `dettivo-core::the_known_list_covers_every_prefix_and_docs_table` | fn the_known_list_covers_every_prefix_and_docs_table in crates/dettivo-core/src/qa_tests.rs | the QA variable table in docs/qa.md carries DETTIVO_E2E_MEETING_STATE |

### `fn-35-qa-pack-the-gui-drives-for-onboarding`

QA pack: the GUI drives for onboarding, settings, history and the Omarchy plugin, with the negative text scan and the accessibility check per surface

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1 | `unit` | `dettivo-qa::the_gui_pack_composes_the_four_surfaces_in_order_from_real_steps` | fn the_gui_pack_composes_the_four_surfaces_in_order_from_real_steps in crates/dettivo-qa/src/pack/gui.rs | pack gui composes gui-onboarding, gui-settings, gui-history, gui-omarchy in order from the real scenario and command tables |
| R1 | `unit` | `dettivo-qa::the_surface_filter_narrows_and_names_the_four` | fn the_surface_filter_narrows_and_names_the_four in crates/dettivo-qa/src/pack/gui.rs | --surface narrows a GUI pack; an unknown surface exits 2 naming the four |
| R1 | `unit` | `dettivo-qa::a_gui_report_renders_its_surfaces_with_the_findings_located` | fn a_gui_report_renders_its_surfaces_with_the_findings_located in crates/dettivo-qa/src/pack/markdown.rs | gui-pack.md renders per-surface steps, findings and coverage and round-trips with gui-pack.json |
| R1 | `unit` | `dettivo-qa::a_root_under_the_callers_home_or_xdg_directory_is_refused_by_name` | fn a_root_under_the_callers_home_or_xdg_directory_is_refused_by_name in crates/dettivo-qa/src/profile.rs | the profile preflight refuses a root under $HOME or XDG_DATA_HOME by the variable's name |
| R2 | `pack` | `gui-onboarding/first_run_fresh` | pack step gui-onboarding/first_run_fresh | the fresh profile shows the three screens on atspi (pass) and cua (allowed skip) |
| R2 | `pack` | `gui-onboarding/first_run_provisioned` | pack step gui-onboarding/first_run_provisioned | the provisioned profile opens Home |
| R2 | `pack` | `gui-onboarding/first_run_steps` | pack step gui-onboarding/first_run_steps | each DETTIVO_E2E_STEP opens by name; no fourth screen or welcome title |
| R2 | `pack` | `gui-onboarding/first_run_resume` | pack step gui-onboarding/first_run_resume | a flow closed on Models reopens on Models |
| R3 | `drive` | `settings_roundtrip.general` | scenario settings_roundtrip.general: one settings section's keys round-trip through their controls, with the section's own checks | one section round trip with the planted comment intact; the pack runs one step per section, a failing section named with its key |
| R3 | `pack` | `gui-settings/settings_env_override` | pack step gui-settings/settings_env_override | the env badge and the disabled control keep their value and source, nothing reaches config.toml |
| R3 | `pack` | `gui-settings/lint_settings_keys` | pack step gui-settings/lint_settings_keys | the key-coverage lint as a command step |
| R3 | `script` | `scripts/lint-settings-keys.sh` | file scripts/lint-settings-keys.sh | every schema key sits on a settings route or is excused |
| R4 | `pack` | `gui-history/history_seeded` | pack step gui-history/history_seeded | the seed by day, search, detail, export, re-run and delete |
| R4 | `pack` | `gui-history/app_routes` | pack step gui-history/app_routes | every route by name with its title |
| R4 | `pack` | `gui-history/history_states` | pack step gui-history/history_states | the empty and no-hits states without a raw table, raw path or generic placeholder |
| R5 | `pack` | `gui-omarchy/omarchy_setup_idempotent` | pack step gui-omarchy/omarchy_setup_idempotent | dettivo setup omarchy twice, nothing changes by SHA-256, --check reports every step ok; an allowed skip off Hyprland |
| R5 | `unit` | `dettivo-qa::a_changed_file_is_named_and_the_reports_are_judged` | fn a_changed_file_is_named_and_the_reports_are_judged in crates/dettivo-qa/src/scenarios/omarchy_setup_idempotent.rs | the hash judgement names a changed path and reads the --check report |
| R5 | `pack` | `gui-omarchy/omarchy_bar` | pack step gui-omarchy/omarchy_bar | the bar widget's pixels against the daemon's state on a live desktop |
| R5 | `pack` | `gui-omarchy/omarchy_validate` | pack step gui-omarchy/omarchy_validate | the plugin manifest lint |
| R5 | `pack` | `gui-omarchy/omarchy_shim_load` | pack step gui-omarchy/omarchy_shim_load | the plugin loads under the shell shim and answers events |
| R5 | `pack` | `gui-omarchy/omarchy_version_skew` | pack step gui-omarchy/omarchy_version_skew | the install and upgrade hints under version skew |
| R5 | `script` | `scripts/qa/omarchy-plugin-test.sh` | file scripts/qa/omarchy-plugin-test.sh | runs the shim-load and version-skew Quick cases |
| R6 | `unit` | `dettivo-qa::a_planted_parity_gap_and_an_unnamed_button_fail_their_routes_by_name` | fn a_planted_parity_gap_and_an_unnamed_button_fail_their_routes_by_name in crates/dettivo-qa/src/pack/scan.rs | the pack scan fails a planted parity_gap string and a planted unnamed button by route |
| R6 | `unit` | `dettivo-qa::a_planted_unnamed_button_fails_by_name_and_hidden_controls_do_not_count` | fn a_planted_unnamed_button_fails_by_name_and_hidden_controls_do_not_count in crates/dettivo-qa/src/a11y_tree.rs | the a11y tree walk counts on-screen interactive elements for the 1.0 coverage figure |
| R6 | `unit` | `dettivo-qa::classes_are_caught_and_clean_text_passes` | fn classes_are_caught_and_clean_text_passes in crates/dettivo-qa/src/negative_text.rs | the full negative text class list |
| R6 | `script` | `scripts/lint-release-text.sh` | file scripts/lint-release-text.sh | the eight scan classes over QML literals |
| R6 | `unit` | `dettivo-core::release_builds_refuse_qa_mode_unless_allowed` | fn release_builds_refuse_qa_mode_unless_allowed in crates/dettivo-core/src/qa_tests.rs | release-profile binaries run the pack only with qa_allow_release |
| R7 | `docs` | `docs/qa.md` | file docs/qa.md | The GUI pack section: steps, allowed skips, scan classes, coverage, the release-build rule, the profile guard |
| R7 | `docs` | `docs/RELEASING.md` | file docs/RELEASING.md | the GUI pack as the second named gate step |
| R7 | `docs` | `docs/adr/0037-gui-packs-per-surface-and-the-tree-checks.md` | file docs/adr/0037-gui-packs-per-surface-and-the-tree-checks.md | the ADR records the per-surface pack model and the tree checks |
| R7 | `script` | `scripts/check-docs.sh` | file scripts/check-docs.sh | fails on an unindexed ADR |

### `fn-36-qa-pack-meetings-end-to-end-the`

QA pack: meetings end to end on the rig and with mock fixtures, the NFR-4 and NFR-5 throughput report and the CPU-only benchmark path

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1 | `unit` | `dettivo-qa::the_pack_runs_the_lane_in_order_from_real_steps` | fn the_pack_runs_the_lane_in_order_from_real_steps in crates/dettivo-qa/src/pack/meetings.rs | pack meetings runs the fifteen steps in order from real scenarios and commands |
| R1 | `unit` | `dettivo-qa::cpu_forces_the_engines_and_picks_the_nfr5_model` | fn cpu_forces_the_engines_and_picks_the_nfr5_model in crates/dettivo-qa/src/pack/meetings.rs | --cpu sets DETTIVO_FORCE_CPU=1 and picks base.en |
| R1 | `unit` | `dettivo-qa::the_block_lands_in_the_latest_report_in_place_and_never_under_a_new_date` | fn the_block_lands_in_the_latest_report_in_place_and_never_under_a_new_date in crates/dettivo-qa/src/pack/meeting_measure.rs | --record writes the block into the latest report in place, never a copy under a new date |
| R1 | `unit` | `dettivo-qa::the_measurements_read_back_from_the_evidence_and_render` | fn the_measurements_read_back_from_the_evidence_and_render in crates/dettivo-qa/src/pack/meeting_measure.rs | meetings-pack.md renders the measurements block and the blockers |
| R1 | `unit` | `dettivo-qa::an_unknown_model_is_refused_naming_the_catalogue` | fn an_unknown_model_is_refused_naming_the_catalogue in crates/dettivo-qa/src/pack/meetings.rs | an unknown model id exits 2 naming the catalogue's Whisper ids |
| R2 | `pack` | `meetings/meeting_rig` | pack step meetings/meeting_rig | two-source capture with the gap marker after the sink swap |
| R2 | `pack` | `meetings/meeting_recovery` | pack step meetings/meeting_recovery | SIGKILL and recovery with the transcript covering both takes |
| R2 | `pack` | `meetings/meeting_live` | pack step meetings/meeting_live | the live transcript against the interleave golden |
| R2 | `pack` | `meetings/meeting_token_coverage` | pack step meetings/meeting_token_coverage | every alpha token on its source with two named speakers in the md export; a missing token fails naming it |
| R2 | `drive` | `meeting_token_coverage` | scenario meeting_token_coverage: the two-voice alpha fixture through capture, finalisation and the speaker pass; every token on its source, both names in the md export | the two-voice alpha fixture scenario on the rig and on the mock path |
| R2 | `unit` | `dettivo-meeting::two_tracks_switch_stop_and_the_journal_says_what_happened` | fn two_tracks_switch_stop_and_the_journal_says_what_happened in crates/dettivo-meeting/tests/session.rs | the rig step's metadata.json bug: a row update no longer drops the takes |
| R3 | `pack` | `meetings/diarization_der` | pack step meetings/diarization_der | two speakers under DER 0.20 |
| R3 | `pipeline` | `diarization` | pipeline diarization | the diarization pipeline against the turns golden |
| R3 | `pack` | `meetings/meeting_export_goldens` | pack step meetings/meeting_export_goldens | the five formats byte-equal through cargo test -p dettivo-storage --test meeting_export |
| R3 | `pack` | `meetings/meetings_seeded` | pack step meetings/meetings_seeded | the three meetings screens on atspi (pass) and cua (allowed skip); live and import are their own steps in the same pack |
| R4 | `bench` | `meeting_throughput` | bench step meeting_throughput | the GPU tier's realtime factor against NFR-4 with engine, model and backend named |
| R4 | `bench` | `diarization_throughput` | bench step diarization_throughput | the diarization realtime factor against its target |
| R4 | `bench` | `gpu_workload_proof` | bench step gpu_workload_proof | the engine pid seen on the GPU in every sample |
| R4 | `unit` | `dettivo-qa::the_verdict_needs_the_engine_pid_on_the_gpu_tier_only` | fn the_verdict_needs_the_engine_pid_on_the_gpu_tier_only in crates/dettivo-qa/src/pack/gpu_proof.rs | the GPU verdict on the gpu tier, a skip on cpu |
| R4 | `docs` | `docs/reports/benchmarks/2026-09-05-thor-gpu.json` | file docs/reports/benchmarks/2026-09-05-thor-gpu.json | the recorded GPU-tier meetings block with targets beside the factors |
| R5 | `docs` | `docs/reports/benchmarks/2026-09-05-thor-cpu.json` | file docs/reports/benchmarks/2026-09-05-thor-cpu.json | the --cpu row with base.en against NFR-5; the CPU-only VM row is not yet present |
| R5 | `unit` | `dettivo-qa::the_gpu_proof_and_cua_may_skip_while_the_rest_must_pass` | fn the_gpu_proof_and_cua_may_skip_while_the_rest_must_pass in crates/dettivo-qa/src/pack/meetings.rs | the GPU proof is the only allowed skip on the CPU path; the other steps must pass |
| R5 | `unit` | `dettivo-qa::cpu_forces_the_engines_and_picks_the_nfr5_model` | fn cpu_forces_the_engines_and_picks_the_nfr5_model in crates/dettivo-qa/src/pack/meetings.rs | --cpu forces every daemon and engine onto the CPU |
| R6 | `docs` | `docs/qa.md` | file docs/qa.md | The meetings pack section: steps, fixtures, the token rule, the throughput method, the CPU path, the report schema |
| R6 | `docs` | `docs/RELEASING.md` | file docs/RELEASING.md | the meetings pack as the third named gate step with its benchmark rows |
| R6 | `docs` | `docs/adr/0039-meetings-pack-finalisation-path-throughput-and-the-gpu-proof.md` | file docs/adr/0039-meetings-pack-finalisation-path-throughput-and-the-gpu-proof.md | the ADR records the finalisation-path throughput measurement and the GPU proof |
| R6 | `script` | `scripts/check-docs.sh` | file scripts/check-docs.sh | fails on an unindexed ADR |

### `fn-37-docs-the-guides-the-docs-build-the`

Docs: the user, agent, Omarchy and QA guides, the docs build in CI, the evidence map, the executed release gate and the Voxtype reuse review

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1 | `docs` | `docs/guides/agents.md` | file docs/guides/agents.md | the agent guide with the generated CLI tree between its markers |
| R1 | `docs` | `docs/guides/user.md` | file docs/guides/user.md | the user guide |
| R1 | `docs` | `docs/guides/omarchy.md` | file docs/guides/omarchy.md | the Omarchy guide |
| R1, R6 | `docs` | `docs/guides/qa.md` | file docs/guides/qa.md | the QA guide with the evidence map and the gate |
| R1 | `unit` | `dettivo-cli::the_register_rows_mark_the_verb_and_everything_beneath_it` | fn the_register_rows_mark_the_verb_and_everything_beneath_it in crates/dettivo-cli/src/docs.rs | dettivo docs cli-tree marks every verb the register lists |
| R1, R2, R6 | `script` | `scripts/check-docs.sh` | file scripts/check-docs.sh | the docs build: the tree golden, the config keys, reachability, value-first openings, the ADR index |
| R2 | `script` | `scripts/test-check-docs.sh` | file scripts/test-check-docs.sh | the plants: an unlinked page, a missing config key, a changed CLI verb, each named by file |
| R1, R2, R3, R4, R5, R6 | `pack` | `release/docs_build` | pack step release/docs_build | the docs build as a release gate step |
| R3 | `unit` | `dettivo-qa::full_coverage_passes_and_an_unmapped_rid_fails_by_name` | fn full_coverage_passes_and_an_unmapped_rid_fails_by_name in crates/dettivo-qa/src/evidence_map/tests.rs | an unmapped R-ID exits 1 naming it |
| R3 | `unit` | `dettivo-qa::an_unresolvable_ref_names_the_kind_and_the_reference` | fn an_unresolvable_ref_names_the_kind_and_the_reference in crates/dettivo-qa/src/evidence_map/tests.rs | an unresolvable ref names the kind and the reference |
| R3 | `unit` | `dettivo-qa::the_checked_in_map_covers_every_rid_of_every_spec` | fn the_checked_in_map_covers_every_rid_of_every_spec in crates/dettivo-qa/src/evidence_map/tests.rs | this map at 1.0 coverage with every ref resolving, on every cargo test |
| R3 | `docs` | `docs/reports/evidence-map.json` | file docs/reports/evidence-map.json | the checked-in report |
| R3 | `pack` | `release/evidence_map` | pack step release/evidence_map | the evidence map as a release gate step |
| R4 | `unit` | `dettivo-qa::the_release_pack_names_every_step_and_the_report_names_the_external_blockers` | fn the_release_pack_names_every_step_and_the_report_names_the_external_blockers in crates/dettivo-qa/src/pack/release.rs | every named step, the embedded reports and the external blockers |
| R4 | `docs` | `docs/reports/release-gate/0.1.0.json` | file docs/reports/release-gate/0.1.0.json | the executed run for the v1 candidate on this desktop |
| R5 | `docs` | `docs/reports/voxtype-reuse-review.md` | file docs/reports/voxtype-reuse-review.md | every Appendix B area with the files and the verdict |
| R5 | `unit` | `xtask::a_marker_without_a_row_and_a_row_without_a_marker_fail_by_path` | fn a_marker_without_a_row_and_a_row_without_a_marker_fail_by_path in tools/xtask/src/notice_markers.rs | lint-notice fails on a planted marker without a row and a planted row without a marker |
| R5 | `pack` | `release/notice_lint` | pack step release/notice_lint | the notice lint as a release gate step |
| R6 | `docs` | `docs/RELEASING.md` | file docs/RELEASING.md | the whole gate step by step with the report it quotes |
| R6 | `docs` | `docs/adr/0041-guides-docs-build-evidence-map-and-the-release-gate-script.md` | file docs/adr/0041-guides-docs-build-evidence-map-and-the-release-gate-script.md | the ADR recording the guides layout, the docs build checks, the evidence map and the gate script |
| R6 | `docs` | `README.md` | file README.md | the guides linked under one heading |
| R6 | `docs` | `CONTRIBUTING.md` | file CONTRIBUTING.md | the same-commit rule |

### `fn-38-beauty-pass-every-surface-against-the`

Beauty pass: every surface walked against the design checklist, the copy, the states, the light theme and the icon, with Gordon's review and re-approved baselines

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R8 | `script` | `scripts/packaging/install-test.sh` | file scripts/packaging/install-test.sh | The installed application proves its strict manifest, native QML modules and required checks. |
| R8 | `script` | `scripts/test-build-entrypoints.sh` | file scripts/test-build-entrypoints.sh | The real install recipe selects the current application beside debug and older packages and rejects ambiguous arguments. |
| R8 | `script` | `scripts/packaging/test-namcap.sh` | file scripts/packaging/test-namcap.sh | Namcap execution failures and errors fail the package build; valid warnings remain advisory. |
| R8 | `script` | `scripts/packaging/test-install-receipt.sh` | file scripts/packaging/test-install-receipt.sh | Skipped required checks and failed natural backend probes fail the install receipt. |
| R1 | `docs` | `docs/design/README.md` | file docs/design/README.md | the design record the checklist and contact sheets hang off; docs/design/checklist.md and the beauty verb are not at HEAD (fn-38 is in flight in another worktree) |
| R1 | `unit` | `dettivo-qa::the_manifest_parses_surfaces_states_and_defaults` | fn the_manifest_parses_surfaces_states_and_defaults in crates/dettivo-qa/src/visual/manifest.rs | every manifest surface is enumerated by name, which the contact sheets must cover |
| R2 | `visual` | `osd` | surface osd in qa/visual/manifest.toml | the pill's states render through the state variable and pass against their boxes on five palettes at both scales |
| R2 | `visual` | `panel` | surface panel in qa/visual/manifest.toml | the panel's states through the same visual job |
| R2 | `visual` | `bar-glyph` | surface bar-glyph in qa/visual/manifest.toml | the glyph states |
| R2 | `unit` | `dettivo-qa::the_matrix_is_states_times_themes_times_scales_with_the_right_baseline` | fn the_matrix_is_states_times_themes_times_scales_with_the_right_baseline in crates/dettivo-qa/src/visual/matrix.rs | the visual matrix is states x themes x scales; a missing crop fails by name |
| R3 | `script` | `scripts/lint-release-text.sh` | file scripts/lint-release-text.sh | the existing copy lint over QML literals in just lint; scripts/lint-copy.sh is not at HEAD |
| R3 | `script` | `scripts/qml-lint.sh` | file scripts/qml-lint.sh | the qml lint and format step of just lint-qml the copy lint joins |
| R4 | `drive` | `app_routes` | scenario app_routes: dettivo-app opens every route by name with its title, no developer text, and records the startup budget | every route opens by name; the keyboard_only scenario is not at HEAD, this is the route sweep it extends |
| R4 | `script` | `scripts/lint-accessible-names.sh` | file scripts/lint-accessible-names.sh | every focusable element carries an accessible name Tab can land on |
| R5 | `visual` | `design-system` | surface design-system in qa/visual/manifest.toml | the sheet against re-approved baselines on catppuccin-latte and builtin-light |
| R5 | `visual` | `first-run` | surface first-run in qa/visual/manifest.toml | the onboarding surfaces on the light palettes |
| R5 | `visual` | `history` | surface history in qa/visual/manifest.toml | the history surfaces on the light palettes |
| R5 | `visual` | `settings` | surface settings in qa/visual/manifest.toml | the settings surfaces on the light palettes |
| R5 | `visual` | `settings-pattern` | surface settings-pattern in qa/visual/manifest.toml | the settings pattern on the light palettes |
| R5 | `visual` | `insert-target` | surface insert-target in qa/visual/manifest.toml | the QA insert-target window on the light palettes |
| R5 | `visual` | `meetings` | surface meetings in qa/visual/manifest.toml | the meetings list on the light palettes |
| R5 | `visual` | `meetings-empty` | surface meetings-empty in qa/visual/manifest.toml | the empty list on the light palettes |
| R5 | `visual` | `meeting-live` | surface meeting-live in qa/visual/manifest.toml | the live meeting on the light palettes |
| R5 | `visual` | `meeting-detail` | surface meeting-detail in qa/visual/manifest.toml | the detail on the light palettes |
| R5 | `visual` | `meeting-detail-tabs` | surface meeting-detail-tabs in qa/visual/manifest.toml | the tabs and popover on the light palettes |
| R5 | `visual` | `meeting-dialogs` | surface meeting-dialogs in qa/visual/manifest.toml | the dialogs on the light palettes |
| R5 | `script` | `scripts/lint-icons.sh` | file scripts/lint-icons.sh | the icon set against its rules; no icon surface exists in the manifest yet |
| R5 | `script` | `scripts/packaging/install-test-checks.sh` | file scripts/packaging/install-test-checks.sh | the install test renders the installed app and checks the packaged PNGs |
| R5 | `unit` | `dettivo-qa::the_canary_passes_only_when_every_entry_fails` | fn the_canary_passes_only_when_every_entry_fails in crates/dettivo-qa/src/visual/report.rs | the canary proves the visual job still fails a deliberate token regression |
| R5 | `unit` | `dettivo-qa::the_canary_passes_only_when_every_entry_fails` | fn the_canary_passes_only_when_every_entry_fails in crates/dettivo-qa/src/visual/report.rs | the canary verdict |
| R5 | `script` | `scripts/lint-qml-tokens.sh` | file scripts/lint-qml-tokens.sh | every surface resolves from Omarchy theme tokens |
| R5 | `unit` | `dettivo-qa::style_findings_are_the_prefixed_lines` | fn style_findings_are_the_prefixed_lines in crates/dettivo-qa/src/visual/render.rs | the style check's findings are read from the render log |
| R6 | `human` | `docs/design/baselines.md` | receipt file docs/design/baselines.md: a route a person walks, not proof that they did | Gordon's re-approval rows carry his name and the date per surface; the checklist receipt lands here |
| R7 | `docs` | `docs/design/README.md` | file docs/design/README.md | round-two surfaces move to Approved with their baselines |
| R7 | `docs` | `docs/app.md` | file docs/app.md | the state variable documented |
| R7 | `docs` | `docs/qa.md` | file docs/qa.md | the beauty verb and the copy lint documented |
| R7 | `docs` | `docs/adr/0012-fast-build-then-cleanup.md` | file docs/adr/0012-fast-build-then-cleanup.md | the cleanup-phase decision the checklist gate and the human checkpoint rule are recorded against; the fn-38 ADR is not at HEAD |
| R7 | `script` | `scripts/check-docs.sh` | file scripts/check-docs.sh | fails on an unindexed ADR |

### `fn-39-raw-layer-protected-tokens-survive`

Raw layer: protected tokens survive sentence casing and replacements stay out of URLs

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1 | `unit` | `dettivo-language::every_token_class_is_found` | fn every_token_class_is_found in crates/dettivo-language/src/raw/protect.rs | protect.rs finds file names, paths, URLs, emails, versions, dotted and :: identifiers and backtick spans |
| R1 | `unit` | `dettivo-language::trailing_punctuation_and_wrappers_stay_outside` | fn trailing_punctuation_and_wrappers_stay_outside in crates/dettivo-language/src/raw/protect.rs | brackets and quotes before and sentence punctuation after a token stay outside the span |
| R1 | `unit` | `dettivo-language::every_protected_token_class_survives_the_raw_layer` | fn every_protected_token_class_survives_the_raw_layer in crates/dettivo-language/src/raw/tests.rs | each class through the whole raw pass with a spoken sentence end after it |
| R1 | `unit` | `dettivo-language::a_leading_token_keeps_its_case_and_a_sentence_can_end_with_one` | fn a_leading_token_keeps_its_case_and_a_sentence_can_end_with_one in crates/dettivo-language/src/raw/tests.rs | the sentence-really-ends cases: open index.ts. Then run it; see example.com. |
| R1 | `unit` | `dettivo-language::protected_tokens_survive_raw_and_deterministic_polish` | fn protected_tokens_survive_raw_and_deterministic_polish in crates/dettivo-language/src/pipeline.rs | each class byte-identical through raw and deterministic_polish, failing naming the token |
| R2 | `unit` | `dettivo-language::replacements_stay_out_of_protected_spans` | fn replacements_stay_out_of_protected_spans in crates/dettivo-language/src/raw/tests.rs | example -> sample changes the plain word, never inside https://example.com/example or gordon@example.org |
| R2 | `unit` | `dettivo-language::a_rule_may_cover_a_span_whole_but_never_cut_into_it` | fn a_rule_may_cover_a_span_whole_but_never_cut_into_it in crates/dettivo-language/src/raw/protect.rs | a rule naming index.ts whole applies; ts against index.ts does not |
| R2 | `unit` | `dettivo-language::protection_off_restores_the_old_sentence_rule` | fn protection_off_restores_the_old_sentence_rule in crates/dettivo-language/src/raw/tests.rs | protect_tokens = false restores the old behaviour |
| R3 | `unit` | `dettivo-language::the_raw_layer_scores_the_macos_dictionary_set_as_the_report_records` | fn the_raw_layer_scores_the_macos_dictionary_set_as_the_report_records in crates/dettivo-language/tests/dictionary_eval.rs | the dictionary eval holds against docs/reports/polish-eval/dictionary-eval-v1.json; token-class rows in the sample set are still the open part of R3 (fn-30 landed after fn-39) |
| R3 | `docs` | `docs/reports/polish-eval/dictionary-eval-v1.json` | file docs/reports/polish-eval/dictionary-eval-v1.json | the checked-in dictionary eval report the golden is scored against |
| R3 | `unit` | `dettivo-qa::the_model_argument_names_every_candidate_kind` | fn the_model_argument_names_every_candidate_kind in crates/dettivo-qa/src/polish_eval/runner.rs | dettivo-qa polish-eval's runner; the sample set carries index.ts rows with protected_spans and the golden records protected_token_preservation_rate 1.0 |
| R4 | `contract` | `crates/dettivo-proto/fixtures/polish/test.json` | file crates/dettivo-proto/fixtures/polish/test.json | polish.test dictates src/app/index.ts, https://example.com/docs and v1.2.3 through the code preset and replays |
| R4 | `contract` | `crates/dettivo-proto/fixtures/config/print_default.json` | file crates/dettivo-proto/fixtures/config/print_default.json | [dictation] protect_tokens = true in the default TOML config print-default prints |
| R4 | `contract` | `crates/dettivo-proto/fixtures/config/keys.json` | file crates/dettivo-proto/fixtures/config/keys.json | dictation.protect_tokens in the schema key list |
| R5 | `docs` | `docs/dictation.md` | file docs/dictation.md | the protected spans paragraph and the protect_tokens row |
| R5 | `docs` | `docs/polish.md` | file docs/polish.md | the modes opening names the tokens kept whole |
| R5 | `docs` | `docs/adr/0023-polish-layers-and-the-llm-provider-layer.md` | file docs/adr/0023-polish-layers-and-the-llm-provider-layer.md | the polish layers ADR records the token rule |
| R5 | `script` | `scripts/check-docs.sh` | file scripts/check-docs.sh | the docs check |

### `fn-40-reduce-ci-rebuilds-and-preserve-release`

Reduce CI rebuilds and preserve release evidence

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1 | `docs` | `docs/adr/0040-fast-ci-loop-proves-the-receipt-the-rig-runs-nightly-and-at-release.md` | file docs/adr/0040-fast-ci-loop-proves-the-receipt-the-rig-runs-nightly-and-at-release.md | the ADR records the cache key (toolchain inventory, configuration, revision) and its sole restore prefix, and the Qt cache assessment |
| R1 | `script` | `scripts/check-toolchain.sh` | file scripts/check-toolchain.sh | the installed toolchain the cache identity is derived from |
| R2 | `docs` | `docs/adr/0040-fast-ci-loop-proves-the-receipt-the-rig-runs-nightly-and-at-release.md` | file docs/adr/0040-fast-ci-loop-proves-the-receipt-the-rig-runs-nightly-and-at-release.md | the fast gate, the fail-closed aggregate, timeouts and concurrency, nightly rig and weekly drift, the advisory benchmark |
| R2 | `pack` | `release/ci_green` | pack step release/ci_green | the release pack reads the stable aggregate |
| R3 | `script` | `scripts/packaging/test-installed-revision.sh` | file scripts/packaging/test-installed-revision.sh | the clean-install gate rejects an older binary of the same version (three cases) |
| R3 | `script` | `scripts/packaging/install-test-checks.sh` | file scripts/packaging/install-test-checks.sh | the exact-revision and clean-install checks the release consumes on the artifact |
| R3 | `script` | `scripts/packaging/install-test.sh` | file scripts/packaging/install-test.sh | the installed package proves itself: manifest, units, CLI version, rendered app |
| R3 | `script` | `scripts/packaging/test-check-manifest.sh` | file scripts/packaging/test-check-manifest.sh | the manifest check names a missing and an extra path |
| R3 | `docs` | `docs/RELEASING.md` | file docs/RELEASING.md | the release consumes the rig's checksum-bound package, publishing serialised, dry_run rehearsal |
| R4 | `docs` | `docs/adr/0040-fast-ci-loop-proves-the-receipt-the-rig-runs-nightly-and-at-release.md` | file docs/adr/0040-fast-ci-loop-proves-the-receipt-the-rig-runs-nightly-and-at-release.md | Copilot on ready PRs and explicit rereview after substantial changes; both instruction files carry the same paragraph |
| R5 | `script` | `scripts/packaging/lint.sh` | file scripts/packaging/lint.sh | the packaging lint inside just lint |
| R5 | `script` | `scripts/check-docs.sh` | file scripts/check-docs.sh | the docs check inside the full gate |
| R5 | `human` | `.flow/tasks/fn-40-reduce-ci-rebuilds-and-preserve-release.1.md` | receipt file .flow/tasks/fn-40-reduce-ci-rebuilds-and-preserve-release.1.md: a route a person walks, not proof that they did | the receipt: local gate green, actionlint and workflow contracts green, remote cold/warm and the rig rehearsal blocked by disabled workflows, no release published |

### `fn-41-release-gate-defects-the-macros`

Release gate defects: the macros capability flag, the seeded row count and the QA profile's models directory

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1 | `contract` | `crates/dettivo-proto/fixtures/system/capabilities.json` | file crates/dettivo-proto/fixtures/system/capabilities.json | the snapshot declares dictation_macros and dictation_macro_audit false |
| R1 | `unit` | `dettivo-qa::a_false_flag_admits_a_pending_row_and_a_true_flag_promises_it` | fn a_false_flag_admits_a_pending_row_and_a_true_flag_promises_it in crates/dettivo-qa/src/replay_flags.rs | a pending row behind a false flag passes --strict, one behind a true flag fails naming the method |
| R1 | `unit` | `dettivo-proto::capability_flags_resolve_in_the_snapshot` | fn capability_flags_resolve_in_the_snapshot in crates/dettivo-proto/tests/fixtures.rs | every required flag resolves in the snapshot or is an admitted false |
| R1 | `pack` | `release/contract_strict` | pack step release/contract_strict | the strict replay with the six macros methods pending behind a false flag |
| R1 | `docs` | `docs/api/linux-deltas.md` | file docs/api/linux-deltas.md | the six automation.macros.* rows deferred behind automation.dictation_macros = false |
| R2 | `drive` | `history_seeded` | scenario history_seeded: History lists the seed by day, searches with painted hits, shows a detail with its facts, re-runs, exports and deletes | waits for as many rows as the seeded daemon's transcripts.list answers |
| R2 | `drive` | `history_states` | scenario history_states: History shows the designed empty state in an empty profile and names the query when a search has no hits | the no-hits state over the seed with the same seed-derived count |
| R2 | `docs` | `docs/qa.md` | file docs/qa.md | the two scenario rows say the count comes from the seed |
| R3 | `unit` | `dettivo-qa::the_tree_carries_only_the_named_models_as_hard_links_and_never_the_real_directory` | fn the_tree_carries_only_the_named_models_as_hard_links_and_never_the_real_directory in crates/dettivo-qa/src/profile_models.rs | a private tree of hard links, a named model linked on request, a missing one refused by name, the real files untouched by a delete |
| R3 | `unit` | `dettivo-qa::a_tree_that_is_the_real_directory_or_under_it_is_refused_by_path` | fn a_tree_that_is_the_real_directory_or_under_it_is_refused_by_path in crates/dettivo-qa/src/profile_models.rs | the preflight refuses the real directory, a path under it and a symlink to it |
| R3 | `unit` | `dettivo-qa::a_profile_carries_a_private_models_tree_and_links_by_name` | fn a_profile_carries_a_private_models_tree_and_links_by_name in crates/dettivo-qa/src/profile.rs | the profile builds its tree, links by name and removes it on drop |
| R3 | `human` | `.flow/tasks/fn-41-release-gate-defects-the-macros.1.md` | receipt file .flow/tasks/fn-41-release-gate-defects-the-macros.1.md: a route a person walks, not proof that they did | the gui and dictation packs under Xvfb bracketed by a listing of the real models directory, byte-identical |
| R4 | `docs` | `docs/reports/release-gate/0.1.0.json` | file docs/reports/release-gate/0.1.0.json | the re-run whose only blockers are external |
| R4 | `docs` | `docs/adr/0041-guides-docs-build-evidence-map-and-the-release-gate-script.md` | file docs/adr/0041-guides-docs-build-evidence-map-and-the-release-gate-script.md | the gate record carrying the outcome of the re-run |
| R1, R2, R3 | `docs` | `docs/adr/0043-qa-profiles-carry-a-private-models-directory-and-the-gate-passes-on-what-the-code-does.md` | file docs/adr/0043-qa-profiles-carry-a-private-models-directory-and-the-gate-passes-on-what-the-code-does.md | the decision record for the three fixes |

### `fn-42-diarization-on-the-gpu-the-cuda-build`

Diarization on the GPU: the CUDA build of the diarize engine as a drop-in, with the CPU build as the fallback

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1, R2, R3, R4 | `docs` | `docs/adr/0034-install-layout-cuda-drop-in-and-release-workflow.md` | file docs/adr/0034-install-layout-cuda-drop-in-and-release-workflow.md | captured 2026-09-06; the worker replaces this with the routes of each fix or rejection when the spec lands |

### `fn-43-cleanup-data-loss-and-persistence-17`

Cleanup: data loss and persistence (17 review findings)

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1, R4 | `unit` | `dettivo-storage::a_whole_row_update_keeps_the_notes_and_the_analysis_saved_meanwhile` | fn a_whole_row_update_keeps_the_notes_and_the_analysis_saved_meanwhile in crates/dettivo-storage/src/meetings_tests.rs | a checkpoint written from the worker's stale row keeps the notes, the analysis, the summary and the search hit saved meanwhile |
| R2 | `unit` | `dettivod::an_omitted_policy_is_the_configured_one` | fn an_omitted_policy_is_the_configured_one in crates/dettivod/tests/meetings_delete.rs | meetings.delete without artifact_policy applies [meetings] delete_artifact_policy; a named policy still wins |
| R2 | `unit` | `dettivo-cli::a_delete_without_a_policy_leaves_the_policy_to_the_daemon` | fn a_delete_without_a_policy_leaves_the_policy_to_the_daemon in crates/dettivo-cli/tests/meetings.rs | dettivo meetings delete sends no policy and reads no key |
| R2 | `contract` | `crates/dettivo-proto/fixtures/meetings/delete.json` | file crates/dettivo-proto/fixtures/meetings/delete.json | the request without artifact_policy |
| R2 | `docs` | `docs/api/linux-deltas.md` | file docs/api/linux-deltas.md | the additive row for meetings.delete without artifact_policy |
| R3 | `unit` | `dettivo-session::every_stopper_of_a_take_gets_that_takes_result` | fn every_stopper_of_a_take_gets_that_takes_result in crates/dettivo-session/tests/machine.rs | a stop during the source open waits for the take, two stoppers get its result, the next start has its own |
| R5 | `unit` | `dettivo-meeting::a_store_that_refuses_the_final_row_keeps_the_takes_and_ends_failed_not_completed` | fn a_store_that_refuses_the_final_row_keeps_the_takes_and_ends_failed_not_completed in crates/dettivo-meeting/tests/transcription.rs | a refused final row keeps the takes and the checkpoint, ends failed with the storage error, and a retry finishes |
| R6 | `unit` | `dettivod::an_old_job_neither_removes_nor_commits_over_a_newer_one` | fn an_old_job_neither_removes_nor_commits_over_a_newer_one in crates/dettivod/src/meeting_jobs.rs | a late job neither removes nor commits over the newer job on the meeting |
| R6 | `unit` | `dettivod::a_cancel_keeps_the_entry_and_an_invalidation_drops_the_result` | fn a_cancel_keeps_the_entry_and_an_invalidation_drops_the_result in crates/dettivod/src/meeting_jobs.rs | a cancel lets the job record its end; an invalidation drops the result |
| R6 | `unit` | `dettivod::an_invalidation_during_a_commit_waits_for_the_write` | fn an_invalidation_during_a_commit_waits_for_the_write in crates/dettivod/src/meeting_jobs.rs | a delete that lands during a commit waits for the write and clears after it |
| R7 | `unit` | `dettivo-storage::an_unreadable_directory_is_an_error_and_a_missing_one_is_fine` | fn an_unreadable_directory_is_an_error_and_a_missing_one_is_fine in crates/dettivo-storage/src/meeting_artifacts.rs | an unreadable meeting directory is an error, a missing one nothing to remove |
| R7 | `unit` | `dettivod::a_directory_that_cannot_be_cleaned_keeps_the_facts_and_the_row_until_a_retry` | fn a_directory_that_cannot_be_cleaned_keeps_the_facts_and_the_row_until_a_retry in crates/dettivod/tests/meetings_delete.rs | a directory the daemon cannot clean keeps the audio facts and the row; a retry through meetings.delete finishes |
| R8 | `unit` | `dettivo-storage::the_row_and_its_speakers_change_together_or_not_at_all` | fn the_row_and_its_speakers_change_together_or_not_at_all in crates/dettivo-storage/src/meetings_tests.rs | a speaker insert that fails rolls the row update and the speaker table back |
| R9 | `unit` | `dettivo-storage::unreadable_json_is_an_error_and_null_is_absent` | fn unreadable_json_is_an_error_and_null_is_absent in crates/dettivo-storage/src/meetings_tests.rs | unreadable segments, diarization or analysis JSON is an error naming the meeting and the column; NULL is absent; an older payload reads |
| R10 | `unit` | `dettivo-qa::the_baseline_keeps_the_hotkey_backend_off_unless_the_scenario_names_one` | fn the_baseline_keeps_the_hotkey_backend_off_unless_the_scenario_names_one in crates/dettivo-qa/src/profile.rs | a scenario configuration without [hotkeys] parses with backend none; a scenario's own table stays |
| R11 | `unit` | `dettivo-qa::typed_shapes_catch_a_wrong_type_and_a_malformed_element` | fn typed_shapes_catch_a_wrong_type_and_a_malformed_element in crates/dettivo-qa/src/replay_shape.rs | a wrong scalar type and a malformed row fail the shape comparison; an empty list and a null optional field pass |
| R12 | `unit` | `dettivo-qa::a_real_candidate_needs_its_checksum_while_the_file_exists` | fn a_real_candidate_needs_its_checksum_while_the_file_exists in crates/dettivo-qa/src/polish_eval/tests.rs | a real candidate's checksum is taken while the file exists; a file that cannot be hashed fails the report |
| R13 | `unit` | `qt::stateFileRoundTrips` | stateFileRoundTrips in qt/host/app/app_host_test.cpp | an acknowledgement the daemon records while the app is open survives the app's save |
| R14 | `unit` | `qt::actionsRerunDeleteAndExportThroughTheLink` | actionsRerunDeleteAndExportThroughTheLink in qt/host/app/app_history_test.cpp | a transfer that fails at the first or a later chunk leaves the previous export untouched |
| R15 | `unit` | `qt::anOldTakesCompletionNeverOverwritesANewerTake` | anOldTakesCompletionNeverOverwritesANewerTake in qt/host/osd/osd_host_test.cpp | an old take's completion inside the beat never overwrites the newer take; a hide drops a pending completion |
| R16 | `unit` | `qt::modelsTableOrdersRowsAndFollowsADownload` | modelsTableOrdersRowsAndFollowsADownload in qt/host/app/settings_model_test.cpp | a refused download keeps its reason through the refresh until the next attempt succeeds |
| R17 | `script` | `scripts/models/tests/convert-polish-finetune-staging.sh` | file scripts/models/tests/convert-polish-finetune-staging.sh | a failed converter or quantiser leaves the previous model, checksum and manifest as they were |
| R18 | `docs` | `docs/adr/0045-a-write-owns-its-fields-and-nothing-announces-what-it-did-not-store.md` | file docs/adr/0045-a-write-owns-its-fields-and-nothing-announces-what-it-did-not-store.md | the decision record for the seventeen findings, indexed in docs/adr/README.md |

### `fn-44-cleanup-ownership-races-and-lifecycle`

Cleanup: ownership, races and lifecycle (23 review findings)

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1, R2, R3, R4, R5, R6, R7, R8, R9, R10, R11, R12, R13, R14, R15, R16, R17, R18, R19, R20, R21, R22, R23, R24 | `docs` | `docs/reports/reviews/2026-09-06-astra/DIGEST.md` | file docs/reports/reviews/2026-09-06-astra/DIGEST.md | captured 2026-09-06; the worker replaces this with the routes of each fix or rejection when the spec lands |

### `fn-45-cleanup-delivery-and-the-target-window`

Cleanup: delivery and the target window (12 review findings)

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1 | `unit` | `dettivo-insert::the_window_guard_tells_two_windows_of_one_process_apart` | fn the_window_guard_tells_two_windows_of_one_process_apart in crates/dettivo-insert/src/guards.rs | the session's guard carries the window identity, so a second window of the same process is a conflict and a probe that cannot name the window any more is one too |
| R1 | `unit` | `dettivod::a_failed_probe_at_start_is_an_unverified_origin_not_an_unguarded_one` | fn a_failed_probe_at_start_is_an_unverified_origin_not_an_unguarded_one in crates/dettivod/src/actions.rs | a probe that saw nothing when the key went down is an unverified origin, distinct from a re-insert without a guard |
| R1 | `unit` | `dettivod::an_unverified_origin_is_clipboard_only_and_says_so` | fn an_unverified_origin_is_clipboard_only_and_says_so in crates/dettivod/src/inserter.rs | an unverified origin's take goes to the clipboard with reason origin_unverified rather than into whatever window has the focus later |
| R2 | `unit` | `dettivo-insert::the_next_backend_runs_only_when_nothing_was_delivered` | fn the_next_backend_runs_only_when_nothing_was_delivered in crates/dettivo-insert/src/delivery.rs | a backend that may have typed a prefix ends the insertion as partial_delivery and no later backend types the text again |
| R2 | `unit` | `dettivo-insert::a_failing_command_says_whether_text_may_have_been_delivered` | fn a_failing_command_says_whether_text_may_have_been_delivered in crates/dettivo-insert/src/backend/commands.rs | a helper that could not start delivered nothing; one that exited with an error or timed out is a delivery that cannot be ruled out |
| R3 | `unit` | `dettivo-insert::the_origin_is_checked_again_right_before_delivery` | fn the_origin_is_checked_again_right_before_delivery in crates/dettivo-insert/src/service.rs | the window that has the focus right before the backend types must be the one the chain decided on; a move is target_changed with nothing typed |
| R3 | `unit` | `dettivo-insert::a_focus_that_moved_before_delivery_refuses_every_backend` | fn a_focus_that_moved_before_delivery_refuses_every_backend in crates/dettivo-insert/src/delivery.rs | a focus that moved before the first key refuses every backend of the chain, not only the first |
| R4 | `unit` | `dettivo-proto::a_target_makes_no_secure_field_claim` | fn a_target_makes_no_secure_field_claim in crates/dettivo-proto/src/methods/insert.rs | no probe detects a password field, so the target carries no secure_field bit and a document with one is rejected |
| R4 | `contract` | `crates/dettivo-proto/fixtures/insert/target.json` | file crates/dettivo-proto/fixtures/insert/target.json | the insert.target fixture without the secure_field claim, replayed against the live daemon |
| R5 | `unit` | `dettivo-cli::a_start_without_a_mode_leaves_the_mode_to_the_daemon` | fn a_start_without_a_mode_leaves_the_mode_to_the_daemon in crates/dettivo-cli/src/commands.rs | dettivo dictation start and toggle send no mode they were not given |
| R5 | `unit` | `dettivod::a_start_without_a_mode_runs_in_the_configured_mode` | fn a_start_without_a_mode_runs_in_the_configured_mode in crates/dettivod/tests/dictation_mode.rs | a start without a mode runs in the configured [dictation] mode and an explicit raw still overrides it |
| R6 | `unit` | `dettivo-meeting::a_source_that_opens_later_keeps_its_origin_live_and_in_the_takes` | fn a_source_that_opens_later_keeps_its_origin_live_and_in_the_takes in crates/dettivo-meeting/tests/session.rs | a system track that opens 300 ms after the microphone lands 300 ms later in the take manifest and in the live segments alike |
| R7 | `unit` | `dettivo-qa::reserved_chords_skip_existing_ctrl_alt_shift_bindings` | fn reserved_chords_skip_existing_ctrl_alt_shift_bindings in crates/dettivo-qa/src/scenarios/hotkeys_hyprland_chords.rs | the Hyprland scenario reserves chords free of a Ctrl+Alt+Shift binding, so a binding of the user's is never unbound |
| R7 | `unit` | `dettivo-qa::still_bound_names_the_chords_the_cleanup_left_behind` | fn still_bound_names_the_chords_the_cleanup_left_behind in crates/dettivo-qa/src/scenarios/hotkeys_hyprland_chords.rs | the cleanup is checked against the compositor and a chord left bound fails the scenario |
| R7 | `drive` | `hotkeys_hyprland` | scenario hotkeys_hyprland: the Hyprland snippet drives hold, toggle and cancel through real key presses | the reserved chords drive hold, toggle and cancel through real key presses and come back out afterwards |
| R8 | `drive` | `first_run_fresh` | scenario first_run_fresh: a fresh profile shows Keys, Models and Try it: the golden snippet is written, a model is picked and downloaded, the mock microphone lands in the field | the take's own insertion must land in the Try it field, and after Done an insertion into the app is refused as target_is_self with no backend run |
| R9 | `unit` | `dettivo-qa::a_row_qualifies_only_with_every_warm_run_completed` | fn a_row_qualifies_only_with_every_warm_run_completed in crates/dettivo-qa/src/bench/first_insert.rs | one, nine and ten of ten warm runs: only the complete row carries a figure, the others keep their failures and partial spread and fail the step |
| R10 | `unit` | `qt::test_minimum_width_leaves_every_route_room` | test_minimum_width_leaves_every_route_room in qt/qml/Dettivo/tests/qml/tst_app_shell.qml | at the window's minimum width History's detail and the meeting detail still have their columns |
| R11 | `unit` | `qt::test_keyboard_rename_targets_the_focused_rows_speaker` | test_keyboard_rename_targets_the_focused_rows_speaker in qt/qml/Dettivo/tests/qml/tst_app_meetings_detail.qml | j and k move the transcript row and r renames the speaker of that row |
| R12 | `unit` | `qt::test_try_it_names_the_outcome_not_the_presence_of_a_result` | test_try_it_names_the_outcome_not_the_presence_of_a_result in qt/qml/Dettivo/tests/qml/tst_app_first_run.qml | inserted reads Inserted, a clipboard fallback says the words are on the clipboard, a failure claims no clipboard |
| R13 | `pack` | `release/ci_green` | pack step release/ci_green | just build test lint green at the final commit |
| R13 | `docs` | `docs/adr/0047-delivery-guards-the-window-itself-and-a-take-is-never-typed-twice.md` | file docs/adr/0047-delivery-guards-the-window-itself-and-a-take-is-never-typed-twice.md | the record of the decisions this theme changed, indexed in docs/adr/README.md |

### `fn-46-cleanup-speech-and-text-quality-7`

Cleanup: speech and text quality (7 review findings)

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1 | `unit` | `dettivo-transcribe::one_second_of_speech_in_five_minutes_of_silence_is_not_silent` | fn one_second_of_speech_in_five_minutes_of_silence_is_not_silent in crates/dettivo-transcribe/src/filters.rs | the floor is judged per 200 ms frame; the whole-chunk average of that fixture is under it |
| R1 | `unit` | `dettivo-transcribe::a_brief_utterance_in_a_long_silence_reaches_the_engine` | fn a_brief_utterance_in_a_long_silence_reaches_the_engine in crates/dettivo-transcribe/tests/job.rs | through the job: the utterance reaches the engine and the all-silent fixture still does not |
| R2 | `unit` | `dettivo-transcribe::a_repetition_across_a_gap_or_outside_the_shared_audio_stays` | fn a_repetition_across_a_gap_or_outside_the_shared_audio_stays in crates/dettivo-transcribe/src/merger.rs | disjoint chunks are appended untouched and a word outside the overlap is no seam candidate |
| R2 | `unit` | `dettivo-transcribe::a_duplicate_inside_the_shared_audio_is_dropped_once` | fn a_duplicate_inside_the_shared_audio_is_dropped_once in crates/dettivo-transcribe/src/merger.rs | the seam run still drops what both chunks heard |
| R3 | `unit` | `dettivo-engine-llm::a_stop_string_that_spans_tokens_is_never_streamed` | fn a_stop_string_that_spans_tokens_is_never_streamed in crates/dettivo-engine-llm/src/stop.rs | the release probe's `3\n`: the partials concatenate to the answer |
| R3 | `unit` | `dettivo-engine-llm::a_held_prefix_is_released_when_the_match_fails_or_at_the_end` | fn a_held_prefix_is_released_when_the_match_fails_or_at_the_end in crates/dettivo-engine-llm/src/stop.rs | a held prefix flushes when it cannot match and at the end of generation |
| R4 | `unit` | `dettivo-language::protected_tokens_are_not_prose` | fn protected_tokens_are_not_prose in crates/dettivo-language/src/polish/mod.rs | a filler inside backticks, an `i` in a path and the case of a file name survive every style |
| R5 | `unit` | `dettivo-language::a_changed_number_is_rejected_even_when_its_digits_survive` | fn a_changed_number_is_rejected_even_when_its_digits_survive in crates/dettivo-language/src/enhanced/guard.rs | 42 in 420, 12 in 1 and 2, 1.5 in 15 and one 3 for two are rejected; 10:30 and a trailing full stop pass |
| R6 | `unit` | `dettivo-session::silence_is_read_from_the_samples_not_the_meter` | fn silence_is_read_from_the_samples_not_the_meter in crates/dettivo-session/tests/machine.rs | a 20 ms voiced take with no level event reaches the engine; a level event over silent samples does not |
| R7 | `unit` | `qt::notesStayDirtyUntilTheDaemonAnswers` | notesStayDirtyUntilTheDaemonAnswers in qt/host/app/app_meeting_detail_test.cpp | a refused save is retried, a reload keeps the draft, an edit during a save follows its answer, a parked draft goes out on reconnect, the live model shares it |
| R8 | `docs` | `docs/adr/0048-speech-and-text-hold-under-test-silence-seams-stop-strings-tokens-numbers-and-notes.md` | file docs/adr/0048-speech-and-text-hold-under-test-silence-seams-stop-strings-tokens-numbers-and-notes.md | the decision record for the seven fixes, indexed in the ADR README |
| R8 | `human` | `.flow/tasks/fn-46-cleanup-speech-and-text-quality-7.1.md` | receipt file .flow/tasks/fn-46-cleanup-speech-and-text-quality-7.1.md: a route a person walks, not proof that they did | the gate at the final commit and the summary naming every finding fixed or rejected |

### `fn-47-cleanup-backend-tier-and-configuration`

Cleanup: backend, tier and configuration truth (18 review findings)

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1, R2, R3, R4, R5, R6, R7, R8, R9, R10, R11, R12, R13, R14, R15, R16, R17, R18, R19 | `docs` | `docs/reports/reviews/2026-09-06-astra/DIGEST.md` | file docs/reports/reviews/2026-09-06-astra/DIGEST.md | captured 2026-09-06; the worker replaces this with the routes of each fix or rejection when the spec lands |

### `fn-48-cleanup-security-boundaries-6-review`

Cleanup: security boundaries (6 review findings)

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1, R2, R3, R4, R5, R6, R7 | `docs` | `docs/reports/reviews/2026-09-06-astra/DIGEST.md` | file docs/reports/reviews/2026-09-06-astra/DIGEST.md | captured 2026-09-06; the worker replaces this with the routes of each fix or rejection when the spec lands |

### `fn-49-cleanup-the-release-gate-and-its`

Cleanup: the release gate and its evidence (16 review findings)

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1 | `unit` | `dettivod::an_empty_search_result_never_passes_as_the_fixture` | fn an_empty_search_result_never_passes_as_the_fixture in crates/dettivod/tests/contract/normalise.rs | an empty search answer fails instead of becoming the fixture (daemon/F11) |
| R1 | `unit` | `dettivod::a_malformed_search_row_never_passes_as_the_fixture` | fn a_malformed_search_row_never_passes_as_the_fixture in crates/dettivod/tests/contract/normalise.rs | a mistyped search row fails the typed deserialisation (daemon/F11) |
| R2 | `unit` | `parakeet-cpp-sys::a_supplied_tree_is_checked_and_never_written` | fn a_supplied_tree_is_checked_and_never_written in crates/parakeet-cpp-sys/tests/patch.rs | PARAKEET_CPP_SOURCE_DIR is read, never patched in place (engines/F15) |
| R2 | `unit` | `parakeet-cpp-sys::an_owned_tree_is_patched_once_and_a_tree_without_a_patch_builds_as_it_is` | fn an_owned_tree_is_patched_once_and_a_tree_without_a_patch_builds_as_it_is in crates/parakeet-cpp-sys/tests/patch.rs | the fetched tree is patched exactly once (engines/F15) |
| R3 | `unit` | `dettivo-qa::a_control_whose_geometry_failed_to_read_still_has_to_carry_a_name` | fn a_control_whose_geometry_failed_to_read_still_has_to_carry_a_name in crates/dettivo-qa/src/a11y_tree.rs | unknown geometry never excludes a control from the walk (qa-rig/F8) |
| R4, R13 | `script` | `scripts/packaging/test-release-scripts.sh` | file scripts/packaging/test-release-scripts.sh | the decision and the receipt: tag push, rehearsals on branch and tag, missing/failed/stale/corrupt/mismatched receipts (qa-packs/F1, ops-and-record/F1) |
| R4, R13 | `script` | `scripts/packaging/release-receipt.sh` | file scripts/packaging/release-receipt.sh | the receipt check the workflow runs before publishing |
| R4, R13 | `script` | `scripts/packaging/release-decide.sh` | file scripts/packaging/release-decide.sh | the publish decision the workflow runs |
| R5, R14 | `unit` | `dettivo-qa::a_receipt_without_evidence_is_refused` | fn a_receipt_without_evidence_is_refused in crates/dettivo-qa/src/pack/gate.rs | empty steps, omitted, skipped, unknown, nonzero, duplicate rows and another version's receipt fail (qa-packs/F2, ops-and-record/F7) |
| R5, R14 | `unit` | `dettivo-qa::a_missing_report_is_missing_proof` | fn a_missing_report_is_missing_proof in crates/dettivo-qa/src/pack/gate.rs | a missing install-test report fails as unproven |
| R6 | `unit` | `dettivo-qa::a_row_that_fails_on_one_driver_fails_the_step_whatever_the_other_did` | fn a_row_that_fails_on_one_driver_fails_the_step_whatever_the_other_did in crates/dettivo-qa/src/pack/release_steps.rs | the cua inference is gone (qa-packs/F3) |
| R6 | `unit` | `dettivo-qa::missing_ci_evidence_is_external_only_when_the_api_confirms_actions_are_disabled` | fn missing_ci_evidence_is_external_only_when_the_api_confirms_actions_are_disabled in crates/dettivo-qa/src/pack/release_checks.rs | ci_green confirms disabled workflows through the API (qa-packs/F3) |
| R7 | `unit` | `dettivo-qa::the_profile_chooses_the_binary_and_a_newer_file_in_the_other_profile_never_does` | fn the_profile_chooses_the_binary_and_a_newer_file_in_the_other_profile_never_does in crates/dettivo-qa/src/scenarios/mod.rs | DETTIVO_BUILD_PROFILE replaces the mtime choice (qa-packs/F4) |
| R7 | `unit` | `dettivo-qa::the_release_gate_refuses_a_debug_or_stale_binary_set` | fn the_release_gate_refuses_a_debug_or_stale_binary_set in crates/dettivo-qa/src/pack/binaries.rs | the release gate refuses a debug or stale binary set by its embedded commit (qa-packs/F4) |
| R8, R15 | `unit` | `dettivo-qa::a_report_passes_on_its_contents_and_never_on_its_name` | fn a_report_passes_on_its_contents_and_never_on_its_name in crates/dettivo-qa/src/pack/release_bench.rs | the benchmark report is judged typed: parse, tier, steps, history, meetings block (qa-packs/F5, ops-and-record/F8) |
| R8, R15 | `unit` | `dettivo-qa::a_cpu_only_host_clears_the_forced_desktop_row_and_a_corrupt_newest_file_never_stands_in` | fn a_cpu_only_host_clears_the_forced_desktop_row_and_a_corrupt_newest_file_never_stands_in in crates/dettivo-qa/src/pack/release_bench.rs | a CPU-only host's report clears the forced row; a corrupt newest file fails the tier |
| R8 | `unit` | `dettivo-qa::the_block_lands_in_the_latest_report_in_place_and_never_under_a_new_date` | fn the_block_lands_in_the_latest_report_in_place_and_never_under_a_new_date in crates/dettivo-qa/src/pack/meeting_measure.rs | --record never copies a benchmark under a new date (qa-packs/F5) |
| R9 | `unit` | `dettivo-qa::a_unit_ref_needs_a_test_attribute_and_a_visual_ref_needs_a_real_state` | fn a_unit_ref_needs_a_test_attribute_and_a_visual_ref_needs_a_real_state in crates/dettivo-qa/src/evidence_map/tests.rs | unit refs need #[test], visual refs resolve their state, human refs are receipts (qa-packs/F7) |
| R10 | `unit` | `dettivo-qa::an_unreadable_or_empty_tree_is_a_finding_and_the_default_tree_is_scanned` | fn an_unreadable_or_empty_tree_is_a_finding_and_the_default_tree_is_scanned in crates/dettivo-qa/src/pack/scan.rs | empty and unreadable captures are findings, tree.json is scanned (qa-packs/F16) |
| R10 | `unit` | `dettivo-qa::the_block_locates_every_finding_and_sums_the_coverage_per_surface` | fn the_block_locates_every_finding_and_sums_the_coverage_per_surface in crates/dettivo-qa/src/pack/surfaces.rs | a surface with no inspected tree reports coverage as null (qa-packs/F16) |
| R11 | `unit` | `dettivo-qa::the_startup_clock_excludes_the_profile_preparation` | fn the_startup_clock_excludes_the_profile_preparation in crates/dettivo-qa/src/bench/footprint.rs | the startup figure times the spawn alone (qa-packs/F17) |
| R12 | `unit` | `qt::test_style_is_dettivo` | test_style_is_dettivo in qt/qml/Dettivo/tests/qml/tst_style.qml | the shared style check every host runs; the CTest cases *-style-check-basic-background and *-style-check-basic-content prove each delegate fails by name (qml/F15) |
| R12 | `docs` | `docs/qa.md` | file docs/qa.md | the four plants and the delegate-by-delegate rule, under the render pass (qml/F15) |
| R16 | `docs` | `docs/omarchy.md` | file docs/omarchy.md | the mirror is the releaser's hand step after the tag; no workflow publishes it (ops-and-record/F13) |
| R16 | `docs` | `docs/RELEASING.md` | file docs/RELEASING.md | the runbook names the mirror export as the last hand step |
| R17 | `docs` | `docs/adr/0051-the-release-gate-passes-on-validated-evidence-bound-to-the-tested-binaries.md` | file docs/adr/0051-the-release-gate-passes-on-validated-evidence-bound-to-the-tested-binaries.md | the decision record for the theme, indexed |
| R17 | `docs` | `docs/RELEASING.md` | file docs/RELEASING.md | the gate's steps as the code now judges them |

### `fn-50-cleanup-qa-isolation-and-honest`

Cleanup: qa isolation and honest assertions (19 review findings)

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1, R2, R3, R4, R5, R6, R7, R8, R9, R10, R11, R12, R13, R14, R15, R16, R17, R18, R19, R20 | `docs` | `docs/reports/reviews/2026-09-06-astra/DIGEST.md` | file docs/reports/reviews/2026-09-06-astra/DIGEST.md | captured 2026-09-06; the worker replaces this with the routes of each fix or rejection when the spec lands |

### `fn-51-cleanup-packaging-the-plugin-and-ci-8`

Cleanup: packaging, the plugin and ci (8 review findings)

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1, R2, R3, R4, R5, R6, R7, R8, R9 | `docs` | `docs/reports/reviews/2026-09-06-astra/DIGEST.md` | file docs/reports/reviews/2026-09-06-astra/DIGEST.md | captured 2026-09-06; the worker replaces this with the routes of each fix or rejection when the spec lands |

### `fn-52-cleanup-delete-and-simplify-29-review`

Cleanup: delete and simplify (29 review findings)

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1, R2, R3, R4, R5, R6, R7, R8, R9, R10, R11, R12, R13, R14, R15, R16, R17, R18, R19, R20, R21, R22, R23, R24, R25, R26, R27, R28, R29, R30 | `docs` | `docs/reports/reviews/2026-09-06-astra/DIGEST.md` | file docs/reports/reviews/2026-09-06-astra/DIGEST.md | captured 2026-09-06; the worker replaces this with the routes of each fix or rejection when the spec lands |

### `fn-53-cleanup-other-33-review-findings`

Cleanup: other (33 review findings)

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1, R2, R3, R4, R5, R6, R7, R8, R9, R10, R11, R12, R13, R14, R15, R16, R17, R18, R19, R20, R21, R22, R23, R24, R25, R26, R27, R28, R29, R30, R31, R32, R33, R34 | `docs` | `docs/reports/reviews/2026-09-06-astra/DIGEST.md` | file docs/reports/reviews/2026-09-06-astra/DIGEST.md | captured 2026-09-06; the worker replaces this with the routes of each fix or rejection when the spec lands |

### `fn-54-live-desktop-qa-after-cleanup`

Live desktop QA after cleanup

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1 | `drive` | `app_routes` | scenario app_routes: dettivo-app opens every route by name with its title, no developer text, and records the startup budget | Every application route is captured from a running native window. |
| R1, R2 | `drive` | `meetings_live_gui` | scenario meetings_live_gui: the disclosure dialog on a fresh profile, Acknowledge and start, both meters, provisional then final segments, Stop into the detail | Home starts through disclosure, cancellation starts nothing, and a meeting reaches its saved detail. |
| R3 | `visual` | `meeting-dialogs` | surface meeting-dialogs in qa/visual/manifest.toml | Dialog chrome resolves from the shared style and retains the approved render. |
| R4 | `docs` | `docs/qa.md` | file docs/qa.md | The live pass records the tested head, captured evidence, failures and remaining approvals. |

### `fn-55-desktop-memory-budgets-grounded-in`

Desktop memory budgets grounded in measured usage

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1, R3, R5 | `unit` | `dettivo-qa::desktop_memory_contract_is_320_and_128_mib_inclusive` | fn desktop_memory_contract_is_320_and_128_mib_inclusive in crates/dettivo-qa/src/scenarios/app_support.rs | Both exact memory limits pass and one KiB above either fails; PSS remains uncapped and MiB conversion is exact. |
| R2, R3, R5 | `unit` | `dettivo-qa::partial_or_malformed_memory_is_unavailable` | fn partial_or_malformed_memory_is_unavailable in crates/dettivo-qa/src/scenarios/app_support.rs | Missing, malformed and partial procfs metrics cannot become successful zero samples. |
| R2, R3, R5 | `unit` | `dettivo-qa::exited_process_and_unreadable_metrics_are_failures` | fn exited_process_and_unreadable_metrics_are_failures in crates/dettivo-qa/src/scenarios/app_memory.rs | An exited app and unreadable metric sources produce failures. |
| R1, R2, R3, R5 | `drive` | `app_memory.wayland` | scenario app_memory.wayland: five fresh release Home launches with connected five-second idle memory and startup gates | Five native Wayland release Home launches retain every memory and startup result; a route existing does not certify that its measured gate passed. |
| R1, R2, R3, R5 | `drive` | `app_memory.xvfb` | scenario app_memory.xvfb: five fresh release Home launches with connected five-second idle memory and startup gates | Five private Xvfb release Home launches record individual values, maxima and actual display metadata. |
| R5 | `drive` | `app_theme_live` | scenario app_theme_live: dettivo-app re-skins on an Omarchy theme swap with no restart and reports the apply time | The existing synchronized-generation theme event retains the 100 ms gate. |
| R4, R6 | `docs` | `docs/adr/0059-desktop-memory-budgets-follow-measured-reference-profiles.md` | file docs/adr/0059-desktop-memory-budgets-follow-measured-reference-profiles.md | The replacement contract preserves the mixed-build snapshots, original failed verdict and rejected experiments. |
| R2, R3, R4, R5 | `docs` | `docs/reports/qa/2026-09-10-desktop-memory.md` | file docs/reports/qa/2026-09-10-desktop-memory.md | Fresh five-launch release evidence per mode records the approved contract alongside historical failures and separate outstanding approvals. |

### `fn-56-reliable-automatic-meeting-diarization`

Reliable automatic meeting diarization

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1, R2, R3, R4, R5 | `docs` | `docs/reports/benchmarks/diarization-accuracy-2026-09-09.md` | file docs/reports/benchmarks/diarization-accuracy-2026-09-09.md | Strict scorer and development calibration are recorded. Native implementation and held-out CPU/CUDA validation remain in progress; this route is not completion proof. Add regression and measured final-result routes as the task completes. |

### `fn-57-keep-benchmark-indexing-scoped-to-suite`

Keep the benchmark index scoped to suite reports

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1, R2 | `unit` | `dettivo-qa::the_readme_ignores_accuracy_reports_but_rejects_malformed_suite_reports` | fn the_readme_ignores_accuracy_reports_but_rejects_malformed_suite_reports in crates/dettivo-qa/src/bench/render.rs | Unrelated accuracy JSON is excluded while malformed suite JSON still fails explicitly. |
| R1, R2 | `unit` | `dettivo-qa::the_readme_keeps_the_latest_report_per_host_and_tier` | fn the_readme_keeps_the_latest_report_per_host_and_tier in crates/dettivo-qa/src/bench/render.rs | The latest valid report for each host and tier retains its measured values and target misses. |

### `fn-58-classify-unavailable-gpu-workload`

Classify unavailable GPU workload attribution at release

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1 | `unit` | `dettivo-qa::workload_attribution_skips_are_external_only_for_the_gpu_proof_step` | fn workload_attribution_skips_are_external_only_for_the_gpu_proof_step in crates/dettivo-qa/src/pack/release_blockers.rs | Only the two sampler attribution limitations on the GPU proof step become named external prerequisites. |
| R2 | `unit` | `dettivo-qa::an_external_gpu_counter_blocker_never_excuses_a_failed_proof_row` | fn an_external_gpu_counter_blocker_never_excuses_a_failed_proof_row in crates/dettivo-qa/src/pack/release_blockers.rs | A failed proof row still fails the release step even when the same report names an external counter blocker. |

### `fn-59-working-f9-shortcuts-after-onboarding`

Working F9 shortcuts after onboarding

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1, R4 | `unit` | `dettivo-hotkeys::lua_setup_sources_after_defaults_and_is_repeatable` | fn lua_setup_sources_after_defaults_and_is_repeatable in crates/dettivo-hotkeys/src/install.rs | The isolated installer fixture verifies a missing include is appended after defaults and repeated setup preserves unrelated configuration. Native F9 acceptance remains separate. |
| R5 | `unit` | `dettivo-hotkeys::failures_are_actionable_and_never_success` | fn failures_are_actionable_and_never_success in crates/dettivo-hotkeys/src/install.rs | Reload and configuration failures remain actionable errors. |
| R1, R5 | `unit` | `qt::shortcutActivationWaitsForSuccessAndRetriesUnsourced` | shortcutActivationWaitsForSuccessAndRetriesUnsourced in qt/host/app/first_run_model_test.cpp | Onboarding waits for successful activation and retries a written but unsourced snippet. |
| R6 | `script` | `scripts/qa/omarchy-plugin-test.sh` | file scripts/qa/omarchy-plugin-test.sh | The plugin fixture verifies packaged module lookup and shared-state registration in an isolated shell. |
| R7 | `unit` | `qt::test_panel_mode_track_and_keyboard` | test_panel_mode_track_and_keyboard in qt/qml/Dettivo/tests/qml/tst_bar_controls.qml | Panel control geometry and keyboard behavior have isolated regression coverage; native visual acceptance remains separate. |
| R2, R3, R4, R6, R7, R8 | `human` | `.flow/tasks/fn-59-working-f9-shortcuts-after-onboarding.1.md` | receipt file .flow/tasks/fn-59-working-f9-shortcuts-after-onboarding.1.md: a route a person walks, not proof that they did | The receipt records clean shortcut setup and package checks plus incomplete controlled native insertion, panel interaction and restart persistence. This route inventories required human acceptance and does not claim those checks passed. |

### `fn-60-release-settings-and-reinsertion-ux`

Release settings and reinsertion UX

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
| R1 | `unit` | `qt::test_copy_transcript_uses_final_then_raw_without_insertion` | test_copy_transcript_uses_final_then_raw_without_insertion in qt/qml/Dettivo/tests/qml/tst_app_history_detail.qml | History copies the final or raw displayed text, explains pasting, and disables an empty transcript. |
| R2, R5 | `unit` | `qt::test_simple_advanced_reveals_without_writes_and_reset_respects_locks` | test_simple_advanced_reveals_without_writes_and_reset_respects_locks in qt/qml/Dettivo/tests/qml/tst_release_settings.qml | Presentation toggles preserve values; reset uses unset and respects environment locks. |
| R3 | `unit` | `qt::test_model_choices_preserve_unknown_and_inherited_values` | test_model_choices_preserve_unknown_and_inherited_values in qt/qml/Dettivo/tests/qml/tst_settings_layout.qml | Labeled model controls retain custom and inherited selections with empty catalogues. |
| R3, R4, R6 | `unit` | `qt::test_vocabulary_preserves_whole_terms_and_transform_custom_values` | test_vocabulary_preserves_whole_terms_and_transform_custom_values in qt/qml/Dettivo/tests/qml/tst_release_settings.qml | Vocabulary retains punctuation within terms, transform toggles preserve additional values, and experiments require Advanced. |
| R7 | `script` | `scripts/packaging/check-manifest.sh` | file scripts/packaging/check-manifest.sh | The release manifest requires the binary-generated commented config reference; user documentation explains both editing workflows. |

### `fn-61-select-whisper-meeting-models`



| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|

### `fn-62-fix-meeting-timer-stuck-at-0000-in`

Fix meeting timer stuck at 00:00

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|

### `fn-63-fix-meeting-ui-timer-and-long-meeting`

Fix meeting live fragments and stop progress feedback

| R-IDs | Kind | Ref | Resolves | Proves |
|---|---|---|---|---|
