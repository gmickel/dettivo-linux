# dettivo-osd: the recording pill outside Omarchy (fn-12). Layer-shell
# hosting needs layer-shell-qt (Arch: layer-shell-qt); without it the
# binary still builds and always uses the window fallback, and
# `dettivo osd status` reports `window`.
find_package(LayerShellQt QUIET)

qt_add_executable(dettivo-osd
    main.cpp
    osd_host.cpp
    osd_host.h)
# The state model is the module's QML singleton, so it is compiled into
# this module rather than into the shared static library.
qt_add_qml_module(dettivo-osd
    URI DettivoOsd
    VERSION 1.0
    QML_FILES Main.qml
    SOURCES
        "${CMAKE_SOURCE_DIR}/host/osd/osd_model.h"
        "${CMAKE_SOURCE_DIR}/host/osd/osd_model.cpp")
target_compile_definitions(dettivo-osd PRIVATE
    DETTIVO_HOST_NAME="dettivo-osd"
    DETTIVO_HOST_MODULE="DettivoOsd")
target_link_libraries(dettivo-osd PRIVATE dettivo-host dettivo-osd-host Qt6::DBus)
if(LayerShellQt_FOUND)
    target_compile_definitions(dettivo-osd PRIVATE DETTIVO_HAVE_LAYER_SHELL)
    target_link_libraries(dettivo-osd PRIVATE LayerShellQt::Interface)
    message(STATUS "dettivo-osd: layer-shell-qt ${LayerShellQt_VERSION} found; the overlay host is built")
else()
    message(STATUS "dettivo-osd: layer-shell-qt not found; the pill uses the window fallback only")
endif()
set_target_properties(dettivo-osd PROPERTIES
    INSTALL_RPATH "${DETTIVO_INSTALL_RPATH}")
install(TARGETS dettivo-osd RUNTIME DESTINATION "${CMAKE_INSTALL_BINDIR}")

# The host conventions every Qt binary keeps (qt/host/CMakeLists.txt).
add_test(NAME dettivo-osd-version COMMAND dettivo-osd --version)
set_tests_properties(dettivo-osd-version PROPERTIES
    PASS_REGULAR_EXPRESSION "^dettivo-osd ${PROJECT_VERSION}")
add_test(NAME dettivo-osd-smoke COMMAND dettivo-osd --smoke)
set_tests_properties(dettivo-osd-smoke PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen;DETTIVO_OMARCHY_THEME_DIR=/nonexistent"
    TIMEOUT 60)
add_test(NAME dettivo-osd-qa-refusal COMMAND dettivo-osd --smoke)
set_tests_properties(dettivo-osd-qa-refusal PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen;DETTIVO_QA_MODE=1;DETTIVO_E2E_JUMP=1"
    PASS_REGULAR_EXPRESSION "DETTIVO_E2E_JUMP"
    TIMEOUT 60)
# A bad pill state is refused by name too.
add_test(NAME dettivo-osd-e2e-state-refusal COMMAND dettivo-osd --smoke)
set_tests_properties(dettivo-osd-e2e-state-refusal PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen;DETTIVO_QA_MODE=1;DETTIVO_QA_ALLOW_RELEASE=1;DETTIVO_E2E_OSD_STATE=sleeping"
    PASS_REGULAR_EXPRESSION "DETTIVO_E2E_OSD_STATE"
    TIMEOUT 60)
# [osd] enabled = false exits 0 with the notice.
add_test(NAME dettivo-osd-disabled-notice COMMAND dettivo-osd)
set_tests_properties(dettivo-osd-disabled-notice PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen;DETTIVO_CONFIG=${CMAKE_CURRENT_SOURCE_DIR}/tests/disabled.toml;XDG_RUNTIME_DIR=${CMAKE_BINARY_DIR}/osd-test-runtime"
    PASS_REGULAR_EXPRESSION "not showing the pill: \\[osd\\] enabled = false"
    TIMEOUT 60)
# host = "layer_shell" without a Wayland session exits 0 naming the reason.
add_test(NAME dettivo-osd-layer-shell-unavailable COMMAND dettivo-osd)
set_tests_properties(dettivo-osd-layer-shell-unavailable PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen;WAYLAND_DISPLAY=;DETTIVO_CONFIG=${CMAKE_CURRENT_SOURCE_DIR}/tests/layer-shell-only.toml;XDG_RUNTIME_DIR=${CMAKE_BINARY_DIR}/osd-test-runtime"
    PASS_REGULAR_EXPRESSION "not showing the pill: \\[osd\\] host = \"layer_shell\" but the session is not Wayland"
    TIMEOUT 60)

# The six states against the approved crops on every theme and scale are
# `dettivo-qa visual` (ADR 0021); here the render itself and the negative
# style check over the pill's tree.
dettivo_add_render_test(dettivo-osd "DETTIVO_QA_MODE=1;DETTIVO_QA_ALLOW_RELEASE=1;DETTIVO_E2E_OSD_STATE=listening;DETTIVO_CONFIG=/nonexistent/config.toml")
dettivo_add_style_check_tests(dettivo-osd "DETTIVO_E2E_OSD_STATE=listening;DETTIVO_CONFIG=/nonexistent/config.toml")
