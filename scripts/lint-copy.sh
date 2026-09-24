#!/usr/bin/env bash
# Every user-visible string reads the way the sheet says it should
# (docs/design/checklist.md, C-15 and C-16; states-and-hint-sheet.png):
# over the same QML literals the release-text lint reads, but only the
# translated ones (`qsTr("...")`), a sentence starts with a capital and
# ends with a full stop, nothing shouts (`!`), nothing trails off (`...`,
# `…`), nobody says `Please`, `Oops` or `Error:`, and a string the pill
# shows (components/Osd*.qml) is one sentence. A string that quotes the
# user, a device or a command verbatim is exempt through a `//: verbatim`
# marker on its line; the summary counts them. Each finding names the
# file, the line and the rule.
#
# Usage: scripts/lint-copy.sh [--root DIR]   (default: the repository's QML)
#        scripts/lint-copy.sh --self-test    (plants three bad strings and
#                                             proves each is named)
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
root="${repo_root}"
self_test=0
while [[ $# -gt 0 ]]; do
    case "$1" in
        --root) root="$2"; shift 2 ;;
        --self-test) self_test=1; shift ;;
        *) echo "lint-copy: unknown argument $1" >&2; exit 2 ;;
    esac
done

# The rules, each a name and what it flags.
#   exclamation    an exclamation mark anywhere in the string
#   ellipsis       three dots or the ellipsis character
#   forbidden      Please, Oops, or an Error: prefix
#   sentence-case  a sentence (two words or more, ending in a full stop)
#                  that starts lowercase; a first word that is a file,
#                  a key, a command or the wordmark is a name, not a word
#   full-stop      a string that carries a sentence boundary inside it
#                  but no full stop at its end
#   pill-sentences a string in the pill's components with two sentences
lint_tree() {
    local tree="$1"
    local -a files
    mapfile -t files < <(find "${tree}" -name '*.qml' -type f ! -path '*/tests/*' ! -path '*/dettivo-sheet/*' \
        ! -path '*/fixtures/*' | sort)
    if [[ ${#files[@]} -eq 0 ]]; then
        echo "lint-copy: no QML files under ${tree}" >&2
        return 2
    fi
    local status=0 strings=0 verbatim=0
    local file rel lineno rest literal text pill
    for file in "${files[@]}"; do
        rel="${file#"${tree}"/}"
        pill=0
        [[ "${rel}" == *components/Osd*.qml ]] && pill=1
        while IFS=: read -r lineno rest; do
            [[ -z "${lineno}" ]] && continue
            if [[ "${rest}" == *"//: verbatim"* ]]; then
                verbatim=$((verbatim + 1))
                continue
            fi
            while IFS= read -r literal; do
                [[ -z "${literal}" ]] && continue
                strings=$((strings + 1))
                text="${literal}"
                local finding=""
                if [[ "${text}" == *"!"* ]]; then
                    finding="exclamation"
                elif [[ "${text}" == *"..."* || "${text}" == *"…"* ]]; then
                    finding="ellipsis"
                elif grep -qiE '(^|[^A-Za-z])(please|oops)([^A-Za-z]|$)|^Error:' <<<"${text}"; then
                    finding="forbidden"
                elif [[ "${text}" =~ ^[a-z][^[:space:]]*[[:space:]].*\.$ ]] \
                    && ! [[ "${text%% *}" =~ [._/\-] || "${text%% *}" == "dettivo" ]]; then
                    finding="sentence-case"
                elif [[ "${text}" =~ [.\?][[:space:]]+[A-Z] && ! "${text}" =~ ([.\?]|%[0-9])$ ]]; then
                    finding="full-stop"
                elif [[ ${pill} -eq 1 ]] && [[ "${text}" =~ [.\?][[:space:]]+[A-Za-z%] ]]; then
                    finding="pill-sentences"
                fi
                if [[ -n "${finding}" ]]; then
                    echo "lint-copy: ${rel}:${lineno}: ${finding} -> \"${text}\"" >&2
                    status=1
                fi
            done < <(grep -oE 'qsTr\("([^"\\]|\\.)*"\)' <<<"${rest}" | sed -E 's/^qsTr\("//; s/"\)$//; s/\\"/"/g')
        done < <(grep -nE 'qsTr\("' "${file}" || true)
    done
    if [[ ${status} -ne 0 ]]; then
        echo "lint-copy: FAILED" >&2
        return 1
    fi
    echo "lint-copy: OK (${#files[@]} files, ${strings} strings, ${verbatim} verbatim)"
}

if [[ ${self_test} -eq 1 ]]; then
    plant="$(mktemp -d -t lint-copy-self-test.XXXXXX)"
    trap 'rm -r -f -- "${plant}"' EXIT
    mkdir -p "${plant}/app" "${plant}/components"
    printf 'Item {\n    property string a: qsTr("Oops! Something went wrong.")\n    property string b: qsTr("the daemon is away.")\n    property string c: qsTr("Fine.") //: verbatim\n}\n' >"${plant}/app/Planted.qml"
    printf 'Item {\n    property string t: qsTr("Inserted into ghostty. Add a regression test.")\n}\n' >"${plant}/components/OsdPlanted.qml"
    findings="$(lint_tree "${plant}" 2>&1 >/dev/null || true)"
    expected=(
        "app/Planted.qml:2: exclamation"
        "app/Planted.qml:3: sentence-case"
        "components/OsdPlanted.qml:2: pill-sentences"
    )
    for want in "${expected[@]}"; do
        if ! grep -qF "${want}" <<<"${findings}"; then
            echo "lint-copy self-test: expected a finding \"${want}\"; got:" >&2
            echo "${findings}" >&2
            exit 1
        fi
    done
    if grep -qF "Planted.qml:4" <<<"${findings}"; then
        echo "lint-copy self-test: the verbatim line was flagged" >&2
        exit 1
    fi
    echo "lint-copy self-test: OK (${#expected[@]} planted findings named by file, line and rule)"
    exit 0
fi

lint_tree "${root}/qt" 2>&1 | sed -E "s#^lint-copy: qml/#lint-copy: qt/qml/#; s#^lint-copy: apps/#lint-copy: qt/apps/#"
exit "${PIPESTATUS[0]}"
