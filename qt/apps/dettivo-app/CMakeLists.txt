# dettivo-app: the desktop app (fn-17, ADR 0020). The host pieces come
# from qt/host/app; AppHost, the QML singleton Main.qml reads, is compiled
# into this module. `--open <route>` and DETTIVO_E2E_OPEN pick the first
# route, `--render <png>` writes a render headlessly for the visual matrix
# (qa/visual/manifest.toml), `--smoke` and `--version` are the host
# conventions of ADR 0001.
qt_add_executable(dettivo-app
    main.cpp
    app_main.cpp
    app_main.h)
qt_add_qml_module(dettivo-app
    URI DettivoApp
    VERSION 1.0
    QML_FILES Main.qml
    SOURCES
        "${CMAKE_SOURCE_DIR}/host/app/app_host.h"
        "${CMAKE_SOURCE_DIR}/host/app/app_host.cpp")
target_compile_definitions(dettivo-app PRIVATE
    DETTIVO_HOST_NAME="dettivo-app"
    DETTIVO_HOST_MODULE="DettivoApp")
target_link_libraries(dettivo-app PRIVATE dettivo-host dettivo-app-host Qt6::DBus)
set_target_properties(dettivo-app PROPERTIES
    INSTALL_RPATH "${DETTIVO_INSTALL_RPATH}")
install(TARGETS dettivo-app RUNTIME DESTINATION "${CMAKE_INSTALL_BINDIR}")

# The desktop entry; the icon is the six-bar mark the Dettivo module installs.
install(FILES dettivo.desktop DESTINATION "${CMAKE_INSTALL_DATADIR}/applications")

# The host conventions every Qt binary keeps (qt/host/CMakeLists.txt).
add_test(NAME dettivo-app-version COMMAND dettivo-app --version)
set_tests_properties(dettivo-app-version PROPERTIES
    PASS_REGULAR_EXPRESSION "^dettivo-app ${PROJECT_VERSION}")
add_test(NAME dettivo-app-smoke COMMAND dettivo-app --smoke)
set_tests_properties(dettivo-app-smoke PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen;DETTIVO_OMARCHY_THEME_DIR=/nonexistent;XDG_RUNTIME_DIR=${CMAKE_BINARY_DIR}/app-test-runtime;XDG_STATE_HOME=${CMAKE_BINARY_DIR}/app-test-state"
    TIMEOUT 60)
add_test(NAME dettivo-app-qa-refusal COMMAND dettivo-app --smoke)
set_tests_properties(dettivo-app-qa-refusal PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen;DETTIVO_QA_MODE=1;DETTIVO_E2E_JUMP=1"
    PASS_REGULAR_EXPRESSION "DETTIVO_E2E_JUMP"
    TIMEOUT 60)
# An unknown route exits 2 naming the routes (fn-17 R1), through the QA
# variable and through --open alike.
add_test(NAME dettivo-app-route-refusal COMMAND dettivo-app --smoke)
set_tests_properties(dettivo-app-route-refusal PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen;DETTIVO_QA_MODE=1;DETTIVO_QA_ALLOW_RELEASE=1;DETTIVO_E2E_OPEN=garage"
    PASS_REGULAR_EXPRESSION "DETTIVO_E2E_OPEN"
    TIMEOUT 60)
add_test(NAME dettivo-app-open-refusal COMMAND dettivo-app --smoke --open garage)
set_tests_properties(dettivo-app-open-refusal PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen"
    PASS_REGULAR_EXPRESSION "settings.hotkeys"
    TIMEOUT 60)
# Every route loads headlessly with the route's title on its page.
foreach(route home history history.detail meetings meetings.live meetings.detail
        settings.general settings.hotkeys settings.models settings.polish
        settings.insertion settings.meetings settings.agents settings.diagnostics onboarding)
    add_test(NAME dettivo-app-smoke-${route} COMMAND dettivo-app --smoke --open ${route})
    set_tests_properties(dettivo-app-smoke-${route} PROPERTIES
        ENVIRONMENT "QT_QPA_PLATFORM=offscreen;DETTIVO_OMARCHY_THEME_DIR=/nonexistent;XDG_RUNTIME_DIR=${CMAKE_BINARY_DIR}/app-test-runtime;XDG_STATE_HOME=${CMAKE_BINARY_DIR}/app-test-state"
        PASS_REGULAR_EXPRESSION "route ${route} "
        TIMEOUT 60)
endforeach()

# The render and the negative style check (ADR 0021) the other hosts
# register, on Home with the sample: a planted Basic control and a
# planted default-font text each fail the render by name.
set(app_render_env "DETTIVO_E2E_OPEN=home;DETTIVO_E2E_SEED=1;DETTIVO_CONFIG=/nonexistent/config.toml;XDG_RUNTIME_DIR=${CMAKE_BINARY_DIR}/app-test-runtime;XDG_STATE_HOME=${CMAKE_BINARY_DIR}/app-test-state")
dettivo_add_render_test(dettivo-app "DETTIVO_QA_MODE=1;DETTIVO_QA_ALLOW_RELEASE=1;${app_render_env}")
dettivo_add_style_check_tests(dettivo-app "${app_render_env}")
