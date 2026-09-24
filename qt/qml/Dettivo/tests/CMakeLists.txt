# Tests for the design system: the theme backend under QTest, and every
# component and styled control under Qt Quick Test with the DettivoStyle
# style active. Registered with ctest so `just test-qt` runs them with the
# host smoke tests.
qt_add_executable(dettivo-theme-backend-test theme_backend_test.cpp)
target_compile_definitions(dettivo-theme-backend-test PRIVATE
    DETTIVO_THEME_FIXTURES_DIR="${CMAKE_SOURCE_DIR}/fixtures/themes")
target_link_libraries(dettivo-theme-backend-test PRIVATE
    Qt6::Core
    Qt6::Gui
    Qt6::Qml
    Qt6::Test
    Dettivo)
add_test(NAME dettivo-theme-backend-test COMMAND dettivo-theme-backend-test)
set_tests_properties(dettivo-theme-backend-test PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen;DETTIVO_OMARCHY_THEME_DIR=/nonexistent;DETTIVO_REDUCED_MOTION=0")

qt_add_executable(dettivo-quick-test quick_test_main.cpp)
target_compile_definitions(dettivo-quick-test PRIVATE
    QUICK_TEST_SOURCE_DIR="${CMAKE_CURRENT_SOURCE_DIR}/qml"
    DETTIVO_QML_BUILD_DIR="${QT_QML_OUTPUT_DIRECTORY}")
target_link_libraries(dettivo-quick-test PRIVATE
    Qt6::Core
    Qt6::Gui
    Qt6::Qml
    Qt6::Quick
    Qt6::QuickControls2
    Qt6::QuickTest
    Dettivo
    DettivoStyle)
add_test(NAME dettivo-quick-test COMMAND dettivo-quick-test)
set_tests_properties(dettivo-quick-test PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen;DETTIVO_OMARCHY_THEME_DIR=/nonexistent;DETTIVO_REDUCED_MOTION=0"
    TIMEOUT 120)

# The Omarchy plugin under the shell shim (docs/omarchy.md): the plugin
# folder staged against this build tree's module, its QML run through the
# same runner with qt/fixtures/omarchy-shell ahead of every import path.
add_test(NAME dettivo-omarchy-plugin-test
    COMMAND "${CMAKE_SOURCE_DIR}/../scripts/qa/omarchy-plugin-test.sh" "${CMAKE_BINARY_DIR}")
set_tests_properties(dettivo-omarchy-plugin-test PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen;DETTIVO_OMARCHY_THEME_DIR=/nonexistent"
    TIMEOUT 120)

# A shell host has neither Dettivo linked nor a custom import path/style.
qt_add_executable(dettivo-plugin-host-test quick_test_main.cpp)
target_compile_definitions(dettivo-plugin-host-test PRIVATE
    QUICK_TEST_SOURCE_DIR="${CMAKE_CURRENT_SOURCE_DIR}/qml"
    DETTIVO_QML_BUILD_DIR="${QT_QML_OUTPUT_DIRECTORY}")
target_link_libraries(dettivo-plugin-host-test PRIVATE
    Qt6::Core Qt6::Gui Qt6::Qml Qt6::Quick Qt6::QuickControls2 Qt6::QuickTest)
