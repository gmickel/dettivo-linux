# The frame pacing collector every Qt host carries (fn-20 R4): enabled by
# DETTIVO_QA_PACING=<file> in any binary, and by `dettivo-osd --pacing`.
add_library(dettivo-pacing STATIC
    pacing_collector.cpp
    pacing_collector.h
    first_frame_timer.h)
target_include_directories(dettivo-pacing PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}")
target_link_libraries(dettivo-pacing PUBLIC Qt6::Core Qt6::Gui Qt6::Qml Qt6::Quick)

# The summariser: dropped frames are named with their timestamps.
qt_add_executable(dettivo-pacing-test pacing_collector_test.cpp)
target_link_libraries(dettivo-pacing-test PRIVATE dettivo-pacing Qt6::Test)
add_test(NAME dettivo-pacing-test COMMAND dettivo-pacing-test)
set_tests_properties(dettivo-pacing-test PROPERTIES ENVIRONMENT "QT_QPA_PLATFORM=offscreen")
