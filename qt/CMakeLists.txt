# Dettivo Qt surface: the shared `Dettivo` QML module, the Qt hosts
# (dettivo-app, dettivo-osd, dettivo-insert-target, dettivo-sheet) and the
# visual-diff tool the baselines are checked with. See ADR 0001 (Qt Quick UI,
# one shared QML module at /usr/lib/dettivo/qml) and ADR 0013 (Qt 6.8 floor).
#
# From the repository root:
#   cmake -S qt -B build/qt -G Ninja -DCMAKE_BUILD_TYPE=Release
#   cmake --build build/qt
#   ctest --test-dir build/qt --output-on-failure
cmake_minimum_required(VERSION 3.21)

project(dettivo-qt
    VERSION 0.1.0
    DESCRIPTION "Dettivo Qt Quick user interface"
    LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

if(NOT CMAKE_BUILD_TYPE AND NOT CMAKE_CONFIGURATION_TYPES)
    set(CMAKE_BUILD_TYPE Release CACHE STRING "Build type" FORCE)
endif()

include(GNUInstallDirs)

# ADR 0013: Qt 6.8 is the minimum. find_package enforces it; the explicit check
# below exists only to give a clearer message than CMake's generic one.
find_package(Qt6 6.8 REQUIRED COMPONENTS Core Gui Qml Quick QuickControls2 QuickTest DBus Network Multimedia Test)

# Theme resolution (fn-5 R1/R2) parses Omarchy's colors.toml/shell.toml with
# tomlplusplus (header-only, packaged as tomlplusplus on Arch/Omarchy).
find_package(tomlplusplus REQUIRED)
if(Qt6_VERSION VERSION_LESS 6.8)
    message(FATAL_ERROR
        "Dettivo requires Qt 6.8 or newer (ADR 0013); found Qt ${Qt6_VERSION} at ${Qt6_DIR}. "
        "Install qt6-base/qt6-declarative >= 6.8 or point Qt6_DIR / CMAKE_PREFIX_PATH at a newer Qt.")
endif()

qt_standard_project_setup(REQUIRES 6.8)

# Every QML module (shared and per-app) lands under build/qt/qml/<URI>, which is
# what scripts/qml-lint.sh points qmllint at.
set(QT_QML_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/qml")

# Where `cmake --install` puts the shared module; the hosts add this as an
# extra QML import path so installed binaries find it without QML_IMPORT_PATH.
set(DETTIVO_QML_INSTALL_DIR "${CMAKE_INSTALL_FULL_LIBDIR}/dettivo/qml")
# Installed binaries link libDettivo.so and libDettivoStyle.so from the module
# directories, so every host carries both as its runtime path; the style
# library itself links libDettivo.so and carries the first.
set(DETTIVO_INSTALL_RPATH "${DETTIVO_QML_INSTALL_DIR}/Dettivo;${DETTIVO_QML_INSTALL_DIR}/DettivoStyle")

add_compile_options(-Wall -Wextra -Wpedantic -Wshadow -Wconversion)

enable_testing()

add_subdirectory(qml/Dettivo)
add_subdirectory(qml/DettivoStyle)
add_subdirectory(host)
add_subdirectory(apps/dettivo-app)
add_subdirectory(apps/dettivo-osd)
add_subdirectory(apps/dettivo-insert-target)
add_subdirectory(apps/dettivo-sheet)
add_subdirectory(apps/dettivo-bar)
add_subdirectory(tools/visual-diff)
