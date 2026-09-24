#!/usr/bin/env bash
# The Omarchy plugin folder holds together: the manifest carries the fields
# the shell needs (schemaVersion 1, id, both kinds, both entry points that
# exist, keepLoaded, the category, the three settings entries, the minimum
# daemon version), `omarchy plugin validate` accepts it where the shell is
# installed, no symlink hides inside, the version follows Cargo.toml, and
# every QML file lints and is formatted against the shell shim under
# qt/fixtures/omarchy-shell with the module from the build tree.
#
# Usage: scripts/lint-omarchy-plugin.sh [build-dir]   (default: build/qt)
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
build_dir="${1:-${repo_root}/build/qt}"
case "${build_dir}" in
    /*) ;;
    *) build_dir="${repo_root}/${build_dir}" ;;
esac
plugin="${repo_root}/omarchy"
manifest="${plugin}/manifest.json"
status=0
fail() {
    echo "lint-omarchy-plugin: $*" >&2
    status=1
}

[[ -f "${manifest}" ]] || { echo "lint-omarchy-plugin: ${manifest} missing" >&2; exit 2; }
command -v jq >/dev/null || { echo "lint-omarchy-plugin: jq not found" >&2; exit 2; }
jq -e . "${manifest}" >/dev/null 2>&1 || fail "manifest.json is not valid JSON"

check() {
    local filter="$1" message="$2"
    jq -e "${filter}" "${manifest}" >/dev/null 2>&1 || fail "manifest: ${message}"
}
check '.schemaVersion == 1' "schemaVersion must be 1"
check '.id == "gmickel.dettivo"' "id must be gmickel.dettivo"
check '(.kinds | index("bar-widget")) != null' "kinds must include bar-widget"
check '(.kinds | index("panel")) != null' "kinds must include panel"
check '.keepLoaded == true' "keepLoaded must be true (the panel hosts the pill)"
check '.entryPoints.barWidget == "BarWidget.qml"' "entryPoints.barWidget must be BarWidget.qml"
check '.entryPoints.panel == "Panel.qml"' "entryPoints.panel must be Panel.qml"
check '.barWidget.category == "Developer Tools"' "barWidget.category must be Developer Tools"
check '.barWidget.defaultSection == "right"' "barWidget.defaultSection must be right"
check '[.barWidget.schema[].key] == ["glyph", "levelMeter", "osd"]' "barWidget.schema must carry glyph, levelMeter and osd"
check '.omarchy.minDettivo | type == "string"' "omarchy.minDettivo must name the daemon version the plugin needs"
for entry in $(jq -r '.entryPoints[]' "${manifest}"); do
    [[ -f "${plugin}/${entry}" ]] || fail "entry point ${entry} does not exist"
done
version="$(grep -m1 -E '^version = ' "${repo_root}/Cargo.toml" | sed -E 's/version = "([^"]+)"/\1/')"
[[ "$(jq -r .version "${manifest}")" == "${version}" ]] || fail "manifest version $(jq -r .version "${manifest}") differs from Cargo.toml ${version}"
min_dettivo="$(jq -r '.omarchy.minDettivo' "${manifest}")"
grep -qF "readonly property string minDettivo: \"${min_dettivo}\"" "${plugin}/DettivoState.qml" \
    || fail "DettivoState.qml minDettivo differs from manifest omarchy.minDettivo ${min_dettivo}"
link="$(find "${plugin}" -type l -print -quit)"
[[ -z "${link}" ]] || fail "symlink inside the plugin folder: ${link}"

if command -v omarchy-plugin-validate >/dev/null 2>&1; then
    omarchy-plugin-validate "${plugin}" || fail "omarchy plugin validate refused ${plugin}"
    echo "lint-omarchy-plugin: omarchy plugin validate OK"
else
    echo "lint-omarchy-plugin: omarchy not installed; the manifest checks above mirror its validator"
fi

qt_bins="$(qmake6 -query QT_HOST_BINS 2>/dev/null || true)"
qt_qml="$(qmake6 -query QT_INSTALL_QML 2>/dev/null || true)"
qmllint="${qt_bins:+${qt_bins}/}qmllint"
qmlformat="${qt_bins:+${qt_bins}/}qmlformat"
if [[ -f "${build_dir}/qml/Dettivo/qmldir" ]] && command -v "${qmllint}" >/dev/null 2>&1; then
    staged="$(mktemp -d)"
    trap 'rm -rf "${staged}"' EXIT
    "${repo_root}/scripts/qa/omarchy-plugin-stage.sh" "${build_dir}" "${staged}/plugin" >/dev/null
    shim="${repo_root}/qt/fixtures/omarchy-shell"
    for file in "${plugin}"/*.qml; do
        name="$(basename "${file}")"
        # --bare drops the default import paths so the shim answers for
        # Quickshell and the shell's modules on every machine, whether or
        # not the real shell is installed; Qt's own modules come back
        # through the explicit path behind it.
        if ! "${qmllint}" --max-warnings 0 --bare -I "${shim}" -I "${qt_qml}" -I "${build_dir}/qml" -i "${staged}/plugin/qmldir" "${staged}/plugin/${name}"; then
            fail "qmllint FAILED: omarchy/${name}"
        fi
        if ! diff -u "${file}" <("${qmlformat}" "${file}") >/dev/null; then
            fail "qmlformat FAILED (run: ${qmlformat} -i ${file})"
        fi
    done
else
    echo "lint-omarchy-plugin: build tree or qmllint missing; QML lint skipped (build qt first)" >&2
fi

if [[ ${status} -ne 0 ]]; then
    echo "lint-omarchy-plugin: FAILED" >&2
    exit "${status}"
fi
echo "lint-omarchy-plugin: OK"
