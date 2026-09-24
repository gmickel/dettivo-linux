# The OSD host pieces shared by dettivo-osd (and later the Omarchy panel):
# the [osd] settings, the control socket, Hyprland IPC, the layer-shell
# probe; the daemon connection comes from the shared dettivo-daemon-client
# library and the termination signals from dettivo-host (qt/host). The state model
# (osd_model.cpp) is a QML singleton, so it is compiled into the QML module
# of the binary that uses it rather than into this library.
add_library(dettivo-osd-host STATIC
    hyprland.cpp
    hyprland.h
    osd_control.cpp
    osd_control.h
    osd_settings.cpp
    osd_settings.h
    wayland_probe.cpp
    wayland_probe.h
)
target_include_directories(dettivo-osd-host PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}")
target_link_libraries(dettivo-osd-host
    PUBLIC Qt6::Core Qt6::Gui Qt6::Network Qt6::Qml dettivo-daemon-client
    PRIVATE tomlplusplus::tomlplusplus)

# The layer-shell probe talks to the compositor directly through
# wayland-client; without the library the probe answers "unavailable" and
# the window fallback is used.
find_package(PkgConfig QUIET)
if(PkgConfig_FOUND)
    pkg_check_modules(WAYLAND_CLIENT QUIET IMPORTED_TARGET wayland-client)
endif()
if(WAYLAND_CLIENT_FOUND)
    target_compile_definitions(dettivo-osd-host PRIVATE DETTIVO_HAVE_WAYLAND_CLIENT)
    target_link_libraries(dettivo-osd-host PRIVATE PkgConfig::WAYLAND_CLIENT)
    message(STATUS "dettivo-osd: wayland-client found; the layer-shell probe is built")
else()
    message(STATUS "dettivo-osd: wayland-client not found; the layer-shell probe answers unavailable")
endif()

# State mapping, settings parsing, the control protocol and the caret
# avoidance rule, all without a daemon, a compositor or a window.
qt_add_executable(dettivo-osd-host-test osd_host_test.cpp osd_model.cpp osd_model.h)
target_link_libraries(dettivo-osd-host-test PRIVATE dettivo-osd-host Qt6::Test)
add_test(NAME dettivo-osd-host-test COMMAND dettivo-osd-host-test)
set_tests_properties(dettivo-osd-host-test PROPERTIES ENVIRONMENT "QT_QPA_PLATFORM=offscreen")
