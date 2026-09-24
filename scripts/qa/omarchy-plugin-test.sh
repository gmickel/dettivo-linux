#!/usr/bin/env bash
# Run the Omarchy plugin's Qt Quick Test under the shell shim: stage the
# plugin folder against the build tree's module, then run the tst_*.qml
# files of qt/fixtures/omarchy-shell/tests/ through a plain Qt host
# without Dettivo linked or its style or import path selected. The shim
# comes first so Quickshell and the shell's modules resolve to the shim whether or not
# the real shell is installed. Any argument after the build directory is
# handed to the runner, so one test case or function runs alone
# (`OmarchyPluginHints::test_2_upgrade_hint_against_an_old_daemon`), the
# way the GUI pack's shim-load and version-skew steps call it.
#
# Usage: scripts/qa/omarchy-plugin-test.sh [build-dir] [runner args...]   (default: build/qt)
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
build_dir="${1:-${repo_root}/build/qt}"
if [[ $# -gt 0 ]]; then shift; fi
case "${build_dir}" in
    /*) ;;
    *) build_dir="${repo_root}/${build_dir}" ;;
esac
runner="${build_dir}/qml/Dettivo/tests/dettivo-plugin-host-test"
if [[ ! -x "${runner}" ]]; then
    echo "omarchy-plugin-test: ${runner} missing; build qt first" >&2
    exit 2
fi
staged="${build_dir}/omarchy-plugin-staged"
"${repo_root}/scripts/qa/omarchy-plugin-stage.sh" "${build_dir}" "${staged}" >/dev/null
export QT_QPA_PLATFORM="${QT_QPA_PLATFORM:-offscreen}"
export DETTIVO_OMARCHY_THEME_DIR="${DETTIVO_OMARCHY_THEME_DIR:-/nonexistent}"
export DETTIVO_REDUCED_MOTION=1
export DETTIVO_TEST_SHELL_HOST=1
unset QML_IMPORT_PATH QML2_IMPORT_PATH
exec "${runner}" -import "${repo_root}/qt/fixtures/omarchy-shell" -input "${staged}" "$@"
