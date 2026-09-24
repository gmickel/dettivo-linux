#!/usr/bin/env bash
# Lint and format-check every .qml file under qt/.
#
# Usage: scripts/qml-lint.sh [build-dir]   (default: build/qt)
#
# qmllint needs the generated qmldir/qmltypes of the shared `Dettivo` module to
# resolve `import Dettivo` and the `Theme` singleton. Qt writes those into the
# build tree at <build-dir>/qml/<URI>/ (QT_QML_OUTPUT_DIRECTORY in
# qt/CMakeLists.txt); the source tree has no qmldir, so the build directory is
# the import root: `qmllint -I <build-dir>/qml <file>`. Configure and build
# first (cmake -S qt -B build/qt -G Ninja && cmake --build build/qt).
#
# Tools come from the Qt 6 host bin directory (qmake6 -query QT_HOST_BINS,
# /usr/lib/qt6/bin on Arch). /usr/bin/qmllint and /usr/bin/qmlformat may be the
# Qt 5 tools on multi-Qt systems, so they are deliberately not used.
#
# qmlformat has no --check flag; a file passes when its formatted output is
# byte-identical to the file on disk.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
build_dir="${1:-${repo_root}/build/qt}"
case "${build_dir}" in
    /*) ;;
    *) build_dir="${repo_root}/${build_dir}" ;;
esac
import_root="${build_dir}/qml"

qt_bins="$(qmake6 -query QT_HOST_BINS 2>/dev/null || qmake -query QT_HOST_BINS 2>/dev/null || true)"
qmllint="${qt_bins:+${qt_bins}/}qmllint"
qmlformat="${qt_bins:+${qt_bins}/}qmlformat"
for tool in "${qmllint}" "${qmlformat}"; do
    if ! command -v "${tool}" >/dev/null 2>&1; then
        echo "qml-lint: ${tool} not found (install qt6-declarative)" >&2
        exit 2
    fi
done

if [[ ! -f "${import_root}/Dettivo/qmldir" ]]; then
    echo "qml-lint: ${import_root}/Dettivo/qmldir missing; configure and build first:" >&2
    echo "  cmake -S qt -B build/qt -G Ninja -DCMAKE_BUILD_TYPE=Release && cmake --build build/qt" >&2
    exit 2
fi

mapfile -t files < <(find "${repo_root}/qt" -name '*.qml' -type f | sort)
if [[ ${#files[@]} -eq 0 ]]; then
    echo "qml-lint: no .qml files under ${repo_root}/qt" >&2
    exit 2
fi

status=0

# The Omarchy shell shim under qt/fixtures/omarchy-shell stands in for
# Quickshell and the shell's own modules in tests; qmllint would resolve
# its `import Quickshell` to the real module where one is installed and
# flag the shim's own additions, so the shim is format-checked only. The
# plugin QML itself is linted against it by scripts/lint-omarchy-plugin.sh.
shim_dir="${repo_root}/qt/fixtures/omarchy-shell"
lint_files=()
for file in "${files[@]}"; do
    case "${file}" in
        "${shim_dir}"/*) ;;
        *) lint_files+=("${file}") ;;
    esac
done

echo "qmllint (${#lint_files[@]} files, import root ${import_root})"
for file in "${lint_files[@]}"; do
    # A file anywhere under a module directory (its root, components/,
    # style/, theme/) is linted with that module's generated qmldir, so its
    # singletons and sibling types resolve the way they do at runtime rather
    # than as an implicit directory import. The first path segment after
    # qt/qml/ names the module.
    module_args=()
    case "${file}" in
        "${repo_root}"/qt/qml/*)
            module="${file#"${repo_root}"/qt/qml/}"; module="${module%%/*}"
            [[ -f "${import_root}/${module}/qmldir" ]] && module_args=(-i "${import_root}/${module}/qmldir")
            ;;
    esac
    # --max-warnings 0: qmllint exits 0 on plain warnings by default.
    if ! "${qmllint}" --max-warnings 0 -I "${import_root}" "${module_args[@]}" "${file}"; then
        echo "qml-lint: qmllint FAILED: ${file}" >&2
        status=1
    fi
done

echo "qmlformat --check (${#files[@]} files)"
for file in "${files[@]}"; do
    if ! diff -u "${file}" <("${qmlformat}" "${file}") >/dev/null; then
        echo "qml-lint: qmlformat FAILED (run: ${qmlformat} -i ${file}):" >&2
        diff -u "${file}" <("${qmlformat}" "${file}") >&2 || true
        status=1
    fi
done

if [[ ${status} -ne 0 ]]; then
    echo "qml-lint: FAILED" >&2
    exit "${status}"
fi
echo "qml-lint: OK"
