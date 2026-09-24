#!/usr/bin/env bash
# Every icon under qt/qml/Dettivo/icons sits on the 16 px grid with one
# stroke width and square caps, and is drawn in the recolour key #000000 so
# Icon.qml can swap it for a theme token at runtime. The desktop icon
# (dettivo.svg, and dettivo-light.svg for a light desktop) is the one
# exception: it carries the mark on its own ground.
#
# Usage: scripts/lint-icons.sh
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
icon_dir="${repo_root}/qt/qml/Dettivo/icons"
status=0
count=0

shopt -s nullglob
for file in "${icon_dir}"/*.svg; do
    name="$(basename "${file}")"
    [[ "${name}" == "dettivo.svg" || "${name}" == "dettivo-light.svg" ]] && continue
    count=$((count + 1))
    rel="${file#"${repo_root}"/}"

    if ! grep -q 'viewBox="0 0 16 16"' "${file}"; then
        echo "lint-icons: ${rel}: not on the 16 px grid (viewBox=\"0 0 16 16\" expected)" >&2
        status=1
    fi
    if grep -q 'stroke="#000000"' "${file}"; then
        if grep -o 'stroke-width="[^"]*"' "${file}" | grep -qv 'stroke-width="1.5"'; then
            echo "lint-icons: ${rel}: stroke width is not 1.5" >&2
            status=1
        fi
        if ! grep -q 'stroke-linecap="square"' "${file}"; then
            echo "lint-icons: ${rel}: stroke caps are not square" >&2
            status=1
        fi
    elif ! grep -q 'fill="#000000"' "${file}"; then
        echo "lint-icons: ${rel}: neither a #000000 stroke nor a #000000 fill; Icon.qml cannot recolour it" >&2
        status=1
    fi
    if grep -oE '(stroke|fill)="#[0-9A-Fa-f]+"' "${file}" | grep -qv '#000000'; then
        echo "lint-icons: ${rel}: carries a colour other than the recolour key #000000" >&2
        status=1
    fi
done

if [[ ${count} -eq 0 ]]; then
    echo "lint-icons: no icons under ${icon_dir}" >&2
    exit 2
fi
if [[ ${status} -ne 0 ]]; then
    echo "lint-icons: FAILED" >&2
    exit "${status}"
fi
echo "lint-icons: OK (${count} icons on the 16 px grid)"
