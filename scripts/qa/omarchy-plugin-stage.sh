#!/usr/bin/env bash
# Stage the Omarchy plugin folder for lint and test on a machine without the
# package installed: copies omarchy/ to <out-dir>, points the three files
# that import the shared module at the build tree's module instead of
# /usr/lib/dettivo/qml/Dettivo. Preserves the shipped qmldir and writes
# fixtures.js from the contract event snapshots.
#
# Usage: scripts/qa/omarchy-plugin-stage.sh <build-dir> <out-dir>
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
build_dir="${1:?build dir}"
out_dir="${2:?out dir}"
case "${build_dir}" in
    /*) ;;
    *) build_dir="${repo_root}/${build_dir}" ;;
esac
module_dir="${build_dir}/qml/Dettivo"
if [[ ! -f "${module_dir}/qmldir" ]]; then
    echo "omarchy-plugin-stage: ${module_dir}/qmldir missing; build qt first" >&2
    exit 2
fi

rm -rf "${out_dir}"
mkdir -p "${out_dir}"
cp -r "${repo_root}/omarchy/." "${out_dir}/"
installed='import "file:///usr/lib/dettivo/qml/Dettivo" as Dettivo'
staged="import \"file://${module_dir}\" as Dettivo"
for file in "${out_dir}"/*.qml; do
    if grep -qF "${installed}" "${file}"; then
        sed -i "s|${installed}|${staged}|" "${file}"
    fi
    sed -i "s|file:///usr/lib/dettivo/qml/DettivoStyle|file://${build_dir}/qml/DettivoStyle|" "${file}"
done
cp "${repo_root}"/qt/fixtures/omarchy-shell/tests/tst_*.qml "${repo_root}"/qt/fixtures/omarchy-shell/tests/*.js "${out_dir}/"
# The contract's event snapshots, as the JS library `fixtures.js` the tests
# import: `completion` is the dictation.state notification the daemon sends
# when a dictation completes (idle from inserting, with the insertion).
{
    echo ".pragma library"
    printf 'var completion = %s;\n' "$(cat "${repo_root}/crates/dettivo-proto/fixtures/events/dictation.state.event.json")"
} > "${out_dir}/fixtures.js"
echo "${out_dir}"
