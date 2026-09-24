#!/usr/bin/env bash
# Every component, app screen and styled control sets an accessible role
# and name, every fixed name it uses is listed in
# qt/qml/Dettivo/accessible-names.txt, the one list the QA drives read when
# they look a control up, and every listed name is documented in
# docs/qa/a11y-names.md so a drive author knows what a name is.
#
# Usage: scripts/lint-accessible-names.sh
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
list="${repo_root}/qt/qml/Dettivo/accessible-names.txt"
status=0

if [[ ! -f "${list}" ]]; then
    echo "lint-accessible-names: ${list} missing" >&2
    exit 2
fi

doc="${repo_root}/docs/qa/a11y-names.md"
if [[ ! -f "${doc}" ]]; then
    echo "lint-accessible-names: ${doc} missing" >&2
    exit 2
fi

mapfile -t files < <(find "${repo_root}/qt/qml/Dettivo/components" "${repo_root}/qt/qml/Dettivo/app" \
    "${repo_root}/qt/qml/DettivoStyle" -name '*.qml' -type f | sort)

for file in "${files[@]}"; do
    rel="${file#"${repo_root}"/}"
    if ! grep -q 'Accessible.role:' "${file}"; then
        echo "lint-accessible-names: ${rel}: no Accessible.role" >&2
        status=1
    fi
    if ! grep -q 'Accessible.name:' "${file}"; then
        echo "lint-accessible-names: ${rel}: no Accessible.name" >&2
        status=1
    fi
    # Fixed names are the qsTr("...") literals bound to Accessible.name or to
    # a label property; each must appear in the documented list.
    while IFS= read -r name; do
        [[ -z "${name}" ]] && continue
        if ! grep -qxF "${name}" "${list}"; then
            echo "lint-accessible-names: ${rel}: fixed accessible name \"${name}\" is not in ${list#"${repo_root}"/}" >&2
            status=1
        fi
    done < <(grep -oE '(Accessible\.name|property string (label|accessibleName)): qsTr\("[^"]+"\)' "${file}" \
        | sed -E 's/.*qsTr\("([^"]+)"\)/\1/')
done

# The documented list and the drive author's table stay in step: every
# listed name appears in docs/qa/a11y-names.md.
while IFS= read -r name; do
    [[ -z "${name}" || "${name}" == \#* ]] && continue
    if ! grep -qF "\`${name}\`" "${doc}"; then
        echo "lint-accessible-names: \"${name}\" is listed in accessible-names.txt but not documented in ${doc#"${repo_root}"/}" >&2
        status=1
    fi
done < "${list}"

if [[ ${status} -ne 0 ]]; then
    echo "lint-accessible-names: FAILED" >&2
    exit "${status}"
fi
echo "lint-accessible-names: OK (${#files[@]} files, $(grep -cvE '^(#|$)' "${list}") documented names)"
