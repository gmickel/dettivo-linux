#!/usr/bin/env bash
# Verify the toolchain the build needs and name every missing piece.
#
# Usage: scripts/check-toolchain.sh [--engines]
#
# Without flags: what every crate and Qt binary in this repository needs today
# (Rust, Cargo, CMake, a C++20 compiler, Qt 6.8 or newer with the Quick and
# QuickControls2 modules and their qmllint and qmlformat tools) and what the
# lints of `just lint` read fixtures with (jq).
# With --engines: additionally the Vulkan headers and the shader compiler that
# the ggml engine builds need (ADR 0004). The engine specs turn this on once an
# engine links ggml; until then the bootstrap crates compile without them.
#
# Exit 0 when everything is present, 1 with one line per missing dependency.
set -uo pipefail

check_engines=0
for arg in "$@"; do
  case "$arg" in
    --engines) check_engines=1 ;;
    *) echo "usage: $0 [--engines]" >&2; exit 2 ;;
  esac
done

missing=()

need_cmd() {
  # need_cmd <command> <what it provides> <how to get it>
  if ! command -v "$1" >/dev/null 2>&1; then
    missing+=("$1 ($2): $3")
  fi
}

need_cmd git "git; the repository lints enumerate tracked files through it" "pacman -S git"
need_cmd cargo "Rust build tool" "install Rust 1.85 or newer, e.g. pacman -S rust or rustup"
need_cmd rustc "Rust compiler" "install Rust 1.85 or newer"
need_cmd cargo-fmt "rustfmt" "pacman -S rust, or rustup component add rustfmt"
need_cmd cargo-clippy "clippy" "pacman -S rust, or rustup component add clippy"
need_cmd cmake "CMake" "pacman -S cmake"
need_cmd ninja "Ninja build backend" "pacman -S ninja"
need_cmd patch "GNU patch; both diarization providers build the calibrated C API" "pacman -S patch"
need_cmd jq "JSON tool; the plugin, settings and docs lints read the fixtures with it" "pacman -S jq"
if ! command -v c++ >/dev/null 2>&1 && ! command -v g++ >/dev/null 2>&1 && ! command -v clang++ >/dev/null 2>&1; then
  missing+=("c++ (C++20 compiler): pacman -S gcc or clang")
fi

qmake_bin=""
for candidate in qmake6 qmake /usr/lib/qt6/bin/qmake; do
  if command -v "$candidate" >/dev/null 2>&1; then
    qmake_bin="$candidate"
    break
  fi
done
if [ -z "$qmake_bin" ]; then
  missing+=("Qt 6.8 or newer (qmake6 not found): pacman -S qt6-base qt6-declarative qt6-svg qt6-wayland qt6-multimedia")
else
  qt_version="$("$qmake_bin" -query QT_VERSION 2>/dev/null || echo 0)"
  qt_major="${qt_version%%.*}"
  qt_rest="${qt_version#*.}"
  qt_minor="${qt_rest%%.*}"
  if [ "${qt_major:-0}" -lt 6 ] || { [ "${qt_major:-0}" -eq 6 ] && [ "${qt_minor:-0}" -lt 8 ]; }; then
    missing+=("Qt 6.8 or newer (found $qt_version): upgrade qt6-base and qt6-declarative")
  fi
  # /usr/bin/qmllint can be the Qt 5 tool; the Qt 6 tools live in QT_HOST_BINS.
  host_bins="$("$qmake_bin" -query QT_HOST_BINS 2>/dev/null || true)"
  for tool in qmllint qmlformat; do
    if [ ! -x "$host_bins/$tool" ]; then
      missing+=("$tool (Qt 6 QML tool, looked in $host_bins): pacman -S qt6-declarative")
    fi
  done
  qml_dir="$("$qmake_bin" -query QT_INSTALL_QML 2>/dev/null || true)"
  for module in QtQuick QtQuick/Controls; do
    if [ ! -d "$qml_dir/$module" ]; then
      missing+=("Qt module $module (looked in $qml_dir): pacman -S qt6-declarative")
    fi
  done
  # The history detail plays a retained take through Qt Multimedia (ADR 0025).
  lib_dir="$("$qmake_bin" -query QT_INSTALL_LIBS 2>/dev/null || true)"
  if [ ! -d "$lib_dir/cmake/Qt6Multimedia" ]; then
    missing+=("Qt Multimedia (looked in $lib_dir/cmake/Qt6Multimedia): pacman -S qt6-multimedia qt6-multimedia-ffmpeg")
  fi
fi

# The recording pill's layer-shell host needs layer-shell-qt; without it the
# build still succeeds and dettivo-osd always uses the window fallback.
if [ ! -d /usr/lib/cmake/LayerShellQt ] && [ -z "$(find /usr/lib*/cmake -maxdepth 1 -name 'LayerShellQt' 2>/dev/null)" ]; then
  echo "toolchain: note: layer-shell-qt not found; dettivo-osd builds with the window fallback only (pacman -S layer-shell-qt)"
fi

if [ "$check_engines" -eq 1 ]; then
  if [ ! -f /usr/include/vulkan/vulkan.h ] && [ -z "${VULKAN_SDK:-}" ]; then
    missing+=("Vulkan headers (/usr/include/vulkan/vulkan.h): pacman -S vulkan-headers")
  fi
  need_cmd glslc "shader compiler" "pacman -S shaderc"
  if [ ! -f /usr/include/spirv/unified1/spirv.h ] && [ -z "${VULKAN_SDK:-}" ]; then
    missing+=("SPIR-V headers (/usr/include/spirv/unified1/spirv.h, for parakeet.cpp's ggml Vulkan build): pacman -S spirv-headers")
  fi
fi

if [ "${#missing[@]}" -eq 0 ]; then
  echo "toolchain: ok (Qt ${qt_version:-unknown}, $(rustc --version 2>/dev/null || echo rustc unknown), $(cmake --version | head -1))"
  exit 0
fi

echo "toolchain: missing ${#missing[@]} dependenc$([ "${#missing[@]}" -eq 1 ] && echo y || echo ies)" >&2
for item in "${missing[@]}"; do
  echo "  - $item" >&2
done
exit 1
