# Shared C++20 host glue for every Dettivo Qt binary: argument handling,
# QGuiApplication + QQmlApplicationEngine setup, import paths, smoke mode,
# `--render <png>` with the negative style check, the frame pacing
# collector (qt/host/pacing) that DETTIVO_QA_PACING switches on, the QA
# environment and a clean exit on SIGTERM.
add_subdirectory(pacing)

add_library(dettivo-host STATIC
    host.cpp
    host.h
    qa_environment.cpp
    qa_environment.h
    render.cpp
    render.h
    style_check.cpp
    style_check.h
    unix_signals.cpp
    unix_signals.h
)

target_include_directories(dettivo-host PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}")

target_compile_definitions(dettivo-host
    PUBLIC
        DETTIVO_QML_INSTALL_DIR="${DETTIVO_QML_INSTALL_DIR}"
        DETTIVO_VERSION="${PROJECT_VERSION}")

target_link_libraries(dettivo-host
    PUBLIC
        Qt6::Core
        Qt6::Gui
        Qt6::Qml
        Qt6::Quick
        Qt6::QuickControls2
        Dettivo
        DettivoStyle
        dettivo-pacing)

# The daemon connection and path resolver every Qt client shares.
add_subdirectory(daemon)
# The OSD host pieces (fn-12): settings, control socket, placement.
add_subdirectory(osd)
# The app host pieces (fn-17): router, models, state, single instance.
add_subdirectory(app)

# QA mode parser test (ADR 0011, R5): every documented variable, bad values
# and unknown reserved names, release refusal, and the docs/qa.md table.
qt_add_executable(dettivo-qa-environment-test qa_environment_test.cpp)
target_compile_definitions(dettivo-qa-environment-test PRIVATE
    DETTIVO_REPO_ROOT="${CMAKE_SOURCE_DIR}/..")
target_link_libraries(dettivo-qa-environment-test PRIVATE dettivo-host Qt6::Test)
add_test(NAME dettivo-qa-environment-test COMMAND dettivo-qa-environment-test)

# dettivo_add_host(<target> URI <uri>)
# Declares one Qt host executable with its own tiny QML module holding Main.qml,
# links the shared Dettivo module, and registers two CTest cases:
#   <target>-version  runs `<target> --version` and expects "<target> <version>"
#   <target>-smoke    runs `<target> --smoke` headless (QT_QPA_PLATFORM=offscreen),
#                     which loads Main.qml for real and quits after objectCreated.
function(dettivo_add_host target)
    cmake_parse_arguments(ARG "" "URI" "" ${ARGN})
    qt_add_executable(${target} main.cpp)
    qt_add_qml_module(${target}
        URI ${ARG_URI}
        VERSION 1.0
        QML_FILES Main.qml)
    target_compile_definitions(${target} PRIVATE
        DETTIVO_HOST_NAME="${target}"
        DETTIVO_HOST_MODULE="${ARG_URI}")
    target_link_libraries(${target} PRIVATE dettivo-host)
    # Installed hosts link the shared Dettivo module library that lives next to
    # the QML files, so point the runtime loader at that directory.
    set_target_properties(${target} PROPERTIES
        INSTALL_RPATH "${DETTIVO_INSTALL_RPATH}")
    install(TARGETS ${target} RUNTIME DESTINATION "${CMAKE_INSTALL_BINDIR}")

    add_test(NAME ${target}-version COMMAND ${target} --version)
    set_tests_properties(${target}-version PROPERTIES
        PASS_REGULAR_EXPRESSION "^${target} ${PROJECT_VERSION}")

    add_test(NAME ${target}-smoke COMMAND ${target} --smoke)
    set_tests_properties(${target}-smoke PROPERTIES
        ENVIRONMENT "QT_QPA_PLATFORM=offscreen"
        TIMEOUT 60)

    # An unknown QA variable is refused by name before any GUI object exists
    # (ADR 0011, R5): the process exits 2 and names the variable.
    add_test(NAME ${target}-qa-refusal COMMAND ${target} --smoke)
    set_tests_properties(${target}-qa-refusal PROPERTIES
        ENVIRONMENT "QT_QPA_PLATFORM=offscreen;DETTIVO_QA_MODE=1;DETTIVO_E2E_JUMP=1"
        # PASS_REGULAR_EXPRESSION makes CTest ignore the exit code, so exit 2
        # plus the variable name in the output is the pass condition.
        PASS_REGULAR_EXPRESSION "DETTIVO_E2E_JUMP"
        TIMEOUT 60)
endfunction()

# dettivo_add_render_test(<target> [extra-env])
# `--render` writes the window headlessly and the negative style check
# passes on the host's own tree (fn-20 R5); every surface in
# qa/visual/manifest.toml registers it.
function(dettivo_add_render_test target)
    set(extra_env "")
    if(ARGC GREATER 1)
        set(extra_env ";${ARGV1}")
    endif()
    add_test(NAME ${target}-render COMMAND ${target} --render "${CMAKE_BINARY_DIR}/${target}-render.png")
    set_tests_properties(${target}-render PROPERTIES
        ENVIRONMENT "QT_QPA_PLATFORM=offscreen;DETTIVO_OMARCHY_THEME_DIR=/nonexistent;DETTIVO_REDUCED_MOTION=1${extra_env}"
        TIMEOUT 120)
endfunction()

# The negative style check (fn-20 R5): a planted Basic-style control and a
# planted default-font text each fail a render by name; exit 3 with the
# finding on stderr is the pass condition.
function(dettivo_add_style_check_tests target)
    set(extra_env "")
    if(ARGC GREATER 1)
        set(extra_env ";${ARGV1}")
    endif()
    add_test(NAME ${target}-style-check-basic-control COMMAND ${target} --render "${CMAKE_BINARY_DIR}/${target}-plant-control.png")
    set_tests_properties(${target}-style-check-basic-control PROPERTIES
        ENVIRONMENT "QT_QPA_PLATFORM=offscreen;DETTIVO_OMARCHY_THEME_DIR=/nonexistent;DETTIVO_REDUCED_MOTION=1;DETTIVO_QA_MODE=1;DETTIVO_QA_ALLOW_RELEASE=1;DETTIVO_QA_PLANT=basic_control${extra_env}"
        PASS_REGULAR_EXPRESSION "style-check: Button \"qaPlant\": (background|contentItem) from .*Basic.*, not the Dettivo style"
        TIMEOUT 120)
    # One delegate replaced at a time: the check has to name the delegate
    # whichever one is the style's.
    add_test(NAME ${target}-style-check-basic-background COMMAND ${target} --render "${CMAKE_BINARY_DIR}/${target}-plant-background.png")
    set_tests_properties(${target}-style-check-basic-background PROPERTIES
        ENVIRONMENT "QT_QPA_PLATFORM=offscreen;DETTIVO_OMARCHY_THEME_DIR=/nonexistent;DETTIVO_REDUCED_MOTION=1;DETTIVO_QA_MODE=1;DETTIVO_QA_ALLOW_RELEASE=1;DETTIVO_QA_PLANT=basic_background${extra_env}"
        PASS_REGULAR_EXPRESSION "style-check: Button \"qaPlant\": background from qa-plant.qml, not the Dettivo style"
        TIMEOUT 120)
    add_test(NAME ${target}-style-check-basic-content COMMAND ${target} --render "${CMAKE_BINARY_DIR}/${target}-plant-content.png")
    set_tests_properties(${target}-style-check-basic-content PROPERTIES
        ENVIRONMENT "QT_QPA_PLATFORM=offscreen;DETTIVO_OMARCHY_THEME_DIR=/nonexistent;DETTIVO_REDUCED_MOTION=1;DETTIVO_QA_MODE=1;DETTIVO_QA_ALLOW_RELEASE=1;DETTIVO_QA_PLANT=basic_content${extra_env}"
        PASS_REGULAR_EXPRESSION "style-check: Button \"qaPlant\": contentItem from qa-plant.qml, not the Dettivo style"
        TIMEOUT 120)
    add_test(NAME ${target}-style-check-default-font COMMAND ${target} --render "${CMAKE_BINARY_DIR}/${target}-plant-font.png")
    set_tests_properties(${target}-style-check-default-font PROPERTIES
        ENVIRONMENT "QT_QPA_PLATFORM=offscreen;DETTIVO_OMARCHY_THEME_DIR=/nonexistent;DETTIVO_REDUCED_MOTION=1;DETTIVO_QA_MODE=1;DETTIVO_QA_ALLOW_RELEASE=1;DETTIVO_QA_PLANT=default_font${extra_env}"
        PASS_REGULAR_EXPRESSION "style-check: QQuickText \"qaPlant\": font family \".*\", the theme's is \"monospace\""
        TIMEOUT 120)
endfunction()
