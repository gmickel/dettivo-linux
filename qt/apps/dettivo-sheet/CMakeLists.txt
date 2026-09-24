# dettivo-sheet renders the whole design system in one scene so a person or
# the QA runner can compare it with docs/design/studio/baselines/
# design-system.png. `--render <path>` writes a PNG headlessly.
qt_add_executable(dettivo-sheet main.cpp)
qt_add_qml_module(dettivo-sheet
    URI DettivoSheet
    VERSION 1.0
    QML_FILES
        Main.qml
        SheetHeader.qml
        SheetPalette.qml
        SheetType.qml
        SheetSpacing.qml
        SheetControls.qml
        SheetIcons.qml
        SheetMotion.qml
        IconSheet.qml)
target_compile_definitions(dettivo-sheet PRIVATE
    DETTIVO_HOST_NAME="dettivo-sheet"
    DETTIVO_HOST_MODULE="DettivoSheet")
target_link_libraries(dettivo-sheet PRIVATE dettivo-host)
set_target_properties(dettivo-sheet PROPERTIES
    INSTALL_RPATH "${DETTIVO_INSTALL_RPATH}")
install(TARGETS dettivo-sheet RUNTIME DESTINATION "${CMAKE_INSTALL_BINDIR}")

add_test(NAME dettivo-sheet-version COMMAND dettivo-sheet --version)
set_tests_properties(dettivo-sheet-version PROPERTIES
    PASS_REGULAR_EXPRESSION "^dettivo-sheet ${PROJECT_VERSION}")

add_test(NAME dettivo-sheet-smoke COMMAND dettivo-sheet --smoke)
set_tests_properties(dettivo-sheet-smoke PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen;DETTIVO_OMARCHY_THEME_DIR=/nonexistent;DETTIVO_REDUCED_MOTION=0"
    TIMEOUT 60)

# The sheet renders to a PNG with the style check over its tree; the
# comparison with the approved baseline is `dettivo-qa visual` (ADR 0021).
dettivo_add_render_test(dettivo-sheet)

# The icon page renders too (docs/design/checklist.md, C-23).
add_test(NAME dettivo-sheet-render-icon
    COMMAND dettivo-sheet --render "${CMAKE_CURRENT_BINARY_DIR}/render-icon.png" --icon)
set_tests_properties(dettivo-sheet-render-icon PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen;DETTIVO_OMARCHY_THEME_DIR=/nonexistent;DETTIVO_REDUCED_MOTION=1"
    TIMEOUT 60)
dettivo_add_style_check_tests(dettivo-sheet)
