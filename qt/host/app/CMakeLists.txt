# The app host pieces (fn-17, ADR 0020): the router, the models over the
# daemon's answers and events, the config binding, the runtime state file
# and the single-instance socket. AppHost, the QML singleton Main.qml
# reads, is compiled into the dettivo-app module (qt/apps/dettivo-app)
# like OsdModel is into dettivo-osd's.
add_library(dettivo-app-host STATIC
    agents_model.cpp
    agents_model.h
    app_control.cpp
    app_control.h
    app_journal.cpp
    app_journal.h
    app_state.cpp
    app_state.h
    cli_runner.cpp
    cli_runner.h
    config_binding.cpp
    config_binding.h
    doctor_model.cpp
    doctor_model.h
    engines_model.cpp
    engines_model.h
    first_run_activation.cpp
    first_run_model.cpp
    first_run_model.h
    first_run_models.cpp
    first_run_models.h
    history_actions.cpp
    history_actions.h
    history_detail_model.cpp
    history_detail_model.h
    history_model.cpp
    history_model.h
    history_player.cpp
    history_player.h
    wav_metadata.cpp
    wav_metadata.h
    hotkeys_setup_model.cpp
    hotkeys_setup_model.h
    live_segments_model.cpp
    live_segments_model.h
    meeting_detail_model.cpp
    meeting_detail_model.h
    meeting_detail_stages.cpp
    meeting_format.cpp
    meeting_format.h
    meeting_live_reconcile.cpp
    meeting_live_model.cpp
    meeting_live_model.h
    meetings_upload.cpp
    upload_source.cpp
    upload_source.h
    meetings_actions.cpp
    meetings_actions.h
    meetings_model.cpp
    meetings_model.h
    models_catalogue.cpp
    models_table.cpp
    models_table.h
    notes_saver.cpp
    notes_saver.h
    router.cpp
    router.h
    sample_data.cpp
    sample_data.h
    sample_meetings.cpp
    sample_settings.cpp
    settings_keys.cpp
    settings_keys.h
    settings_model.cpp
    settings_model.h
    status_format.cpp
    status_format.h
    status_model.cpp
    status_model.h
)
target_include_directories(dettivo-app-host PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}")
target_link_libraries(dettivo-app-host
    PUBLIC Qt6::Core Qt6::Gui Qt6::Network Qt6::Qml Qt6::Multimedia dettivo-daemon-client
    PRIVATE Qt6::DBus tomlplusplus::tomlplusplus)

# The router, the models against a fake daemon link, the formatting, the
# state file round trip, the single-instance protocol and the portal
# colour-scheme switch, all without a daemon or a window.
qt_add_executable(dettivo-app-host-test app_host_test.cpp fake_link.h)
target_link_libraries(dettivo-app-host-test PRIVATE dettivo-app-host Dettivo Qt6::Test)
add_test(NAME dettivo-app-host-test COMMAND dettivo-app-host-test)
set_tests_properties(dettivo-app-host-test PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen;DETTIVO_OMARCHY_THEME_DIR=/nonexistent;DETTIVO_REDUCED_MOTION=0")

qt_add_executable(dettivo-app-sample-data-test app_sample_data_test.cpp fake_link.h)
target_link_libraries(dettivo-app-sample-data-test PRIVATE dettivo-app-host Dettivo Qt6::Test)
add_test(NAME dettivo-app-sample-data-test COMMAND dettivo-app-sample-data-test)
set_tests_properties(dettivo-app-sample-data-test PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen;DETTIVO_OMARCHY_THEME_DIR=/nonexistent;DETTIVO_REDUCED_MOTION=0")

# First run against the same fake link (ADR 0024): the decision, the three
# steps' facts and actions, the sample.
qt_add_executable(dettivo-first-run-test first_run_model_test.cpp fake_link.h)
target_link_libraries(dettivo-first-run-test PRIVATE dettivo-app-host Dettivo Qt6::Test)
# The wire shapes come from the contract fixtures, never from the test.
target_compile_definitions(dettivo-first-run-test PRIVATE
    DETTIVO_CONTRACT_FIXTURES="${CMAKE_SOURCE_DIR}/../crates/dettivo-proto/fixtures")
add_test(NAME dettivo-first-run-test COMMAND dettivo-first-run-test)
set_tests_properties(dettivo-first-run-test PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen;DETTIVO_OMARCHY_THEME_DIR=/nonexistent;DETTIVO_REDUCED_MOTION=0")

# The history pieces (fn-22): the list's search, jobs and removals, the
# kind filter, the detail's facts, the actions through a fake link into
# the QA export directory, and the player's bars from a WAV.
qt_add_executable(dettivo-app-history-test app_history_test.cpp)
target_link_libraries(dettivo-app-history-test PRIVATE dettivo-app-host Dettivo Qt6::Test)
add_test(NAME dettivo-app-history-test COMMAND dettivo-app-history-test)
set_tests_properties(dettivo-app-history-test PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen;DETTIVO_OMARCHY_THEME_DIR=/nonexistent;DETTIVO_REDUCED_MOTION=0")

# The meetings pieces (fn-34, ADR 0038): the list's weeks, chips and
# swatches, the live transcript's tail, the live model's gate, start and
# stop, the detail's facts and rename, the actions' upload import and
# delete policy, and the formatting.
qt_add_executable(dettivo-app-cleanup-test app_cleanup_test.cpp fake_link.h)
target_link_libraries(dettivo-app-cleanup-test PRIVATE dettivo-app-host Dettivo Qt6::Test)
add_test(NAME dettivo-app-cleanup-test COMMAND dettivo-app-cleanup-test)
set_tests_properties(dettivo-app-cleanup-test PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen")

qt_add_executable(dettivo-app-meetings-test app_meetings_test.cpp fake_link.h)
target_link_libraries(dettivo-app-meetings-test PRIVATE dettivo-app-host Dettivo Qt6::Test)
add_test(NAME dettivo-app-meetings-test COMMAND dettivo-app-meetings-test)
set_tests_properties(dettivo-app-meetings-test PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen;DETTIVO_OMARCHY_THEME_DIR=/nonexistent;DETTIVO_REDUCED_MOTION=0;TZ=UTC")

# The meeting detail (fn-34 R1, ADR 0038; fn-63, ADR 0061): the facts,
# the talk shares, the speaker rename, the title and the post-processing
# stages, the notes draft, the actions and the formatting.
qt_add_executable(dettivo-app-meeting-detail-test app_meeting_detail_test.cpp fake_link.h)
target_link_libraries(dettivo-app-meeting-detail-test PRIVATE dettivo-app-host Dettivo Qt6::Test)
add_test(NAME dettivo-app-meeting-detail-test COMMAND dettivo-app-meeting-detail-test)
set_tests_properties(dettivo-app-meeting-detail-test PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen;DETTIVO_OMARCHY_THEME_DIR=/nonexistent;DETTIVO_REDUCED_MOTION=0;TZ=UTC")

# The settings routes (fn-26, ADR 0033): the key table against the
# schema's registry, the editor over the fake link (values, sources, the
# daemon's refusal per key, the environment lock, the TOML block, the
# config.changed refresh), the models table, the hotkeys snippet panel,
# the agents hosts through a fake command runner, the doctor report.
qt_add_executable(dettivo-settings-test settings_model_test.cpp fake_link.h)
target_link_libraries(dettivo-settings-test PRIVATE dettivo-app-host Dettivo Qt6::Test)
add_test(NAME dettivo-settings-test COMMAND dettivo-settings-test)
set_tests_properties(dettivo-settings-test PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen;DETTIVO_OMARCHY_THEME_DIR=/nonexistent;DETTIVO_REDUCED_MOTION=0")

qt_add_executable(dettivo-settings-pending-test settings_pending_test.cpp fake_link.h)
target_link_libraries(dettivo-settings-pending-test PRIVATE dettivo-app-host Qt6::Test)
add_test(NAME dettivo-settings-pending-test COMMAND dettivo-settings-pending-test)

qt_add_executable(dettivo-startup-work-test startup_work_test.cpp fake_link.h)
target_link_libraries(dettivo-startup-work-test PRIVATE dettivo-app-host Qt6::Test)
add_test(NAME dettivo-startup-work-test COMMAND dettivo-startup-work-test)

qt_add_executable(dettivo-meetings-upload-test meetings_upload_test.cpp fake_link.h)
target_link_libraries(dettivo-meetings-upload-test PRIVATE dettivo-app-host Qt6::Test)
add_test(NAME dettivo-meetings-upload-test COMMAND dettivo-meetings-upload-test)
