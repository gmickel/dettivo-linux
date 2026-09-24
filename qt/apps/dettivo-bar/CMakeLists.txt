# dettivo-bar renders the Omarchy plugin's surfaces without Quickshell: the
# 340 px panel in each of its states and one bar glyph cell, so the visual
# regression job holds them to docs/design/studio/baselines/
# omarchy-bar-panel.png and the glyph strip of osd.png (ADR 0021).
# `--render <path>` writes a PNG headlessly; DETTIVO_E2E_BAR_STATE picks
# the surface and state (docs/omarchy.md).
qt_add_executable(dettivo-bar main.cpp)
qt_add_qml_module(dettivo-bar
    URI DettivoBar
    VERSION 1.0
    QML_FILES
        Main.qml
        BarCell.qml
        PanelSample.qml
    SOURCES
        bar_preview.h
        bar_preview.cpp)
target_compile_definitions(dettivo-bar PRIVATE
    DETTIVO_HOST_NAME="dettivo-bar"
    DETTIVO_HOST_MODULE="DettivoBar")
target_link_libraries(dettivo-bar PRIVATE dettivo-host)
set_target_properties(dettivo-bar PROPERTIES
    INSTALL_RPATH "${DETTIVO_INSTALL_RPATH}")
install(TARGETS dettivo-bar RUNTIME DESTINATION "${CMAKE_INSTALL_BINDIR}")

add_test(NAME dettivo-bar-version COMMAND dettivo-bar --version)
set_tests_properties(dettivo-bar-version PROPERTIES
    PASS_REGULAR_EXPRESSION "^dettivo-bar ${PROJECT_VERSION}")

add_test(NAME dettivo-bar-smoke COMMAND dettivo-bar --smoke)
set_tests_properties(dettivo-bar-smoke PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen;DETTIVO_OMARCHY_THEME_DIR=/nonexistent;DETTIVO_REDUCED_MOTION=0"
    TIMEOUT 60)

# A bad bar state is refused by name.
add_test(NAME dettivo-bar-e2e-state-refusal COMMAND dettivo-bar --smoke)
set_tests_properties(dettivo-bar-e2e-state-refusal PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen;DETTIVO_QA_MODE=1;DETTIVO_QA_ALLOW_RELEASE=1;DETTIVO_E2E_BAR_STATE=panel-asleep"
    PASS_REGULAR_EXPRESSION "DETTIVO_E2E_BAR_STATE"
    TIMEOUT 60)

# The render itself and the negative style check over the panel's tree; the
# comparison with the approved crops is `dettivo-qa visual`.
dettivo_add_render_test(dettivo-bar "DETTIVO_QA_MODE=1;DETTIVO_QA_ALLOW_RELEASE=1;DETTIVO_E2E_BAR_STATE=panel-meeting")
dettivo_add_style_check_tests(dettivo-bar "DETTIVO_E2E_BAR_STATE=panel-meeting")
