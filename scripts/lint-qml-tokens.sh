#!/usr/bin/env bash
# No component, styled control or app screen carries a literal colour or
# pixel size beyond the structural ones: every other value comes from a
# Theme or Motion token (ADR 0010). Heuristic (not a full QML parse): flags
# hex colour literals, a small denylist of CSS named colours, and bare
# numeric literals assigned to a curated set of size/geometry properties.
# It deliberately lets `color: "transparent"` and the literal `0` through,
# and Theme.qml, Motion.qml and StyleHelpers.qml are the token
# *definitions* and are exempt (that is where the literals live).
# It also refuses `Canvas` in any component and `QQuickPaintedItem` in the
# module's C++ items: every surface, the recording pill first of all, draws
# on the scene graph (ADR 0014).
#
# Usage: scripts/lint-qml-tokens.sh
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
module_dirs=("${repo_root}/qt/qml/Dettivo" "${repo_root}/qt/qml/DettivoStyle" "${repo_root}/qt/apps")

mapfile -t files < <(find "${module_dirs[@]}" -name '*.qml' -type f \
    ! -name 'Theme.qml' ! -name 'Motion.qml' ! -name 'StyleHelpers.qml' ! -path '*/tests/*' | sort)

if [[ ${#files[@]} -eq 0 ]]; then
    echo "lint-qml-tokens: no .qml files to check" >&2
    exit 2
fi

status=0
named_colors='aqua|black|blue|cyan|fuchsia|gray|green|grey|lime|magenta|maroon|navy|olive|orange|pink|purple|red|silver|teal|white|yellow'
size_props='radius|pixelSize|border\.width|implicitWidth|implicitHeight|spacing|leftPadding|rightPadding|topPadding|bottomPadding'

for file in "${files[@]}"; do
    rel="${file#"${repo_root}"/}"

    # Hex colour literal anywhere: "#abc", "#aabbcc", "#aabbccdd". `defaultColor`
    # is exempt: it is QQC2's ColorImage colour-key property (the pixel value
    # SVG icons are drawn in so ColorImage can swap it for a real token colour
    # at runtime), not a rendered colour choice of its own.
    while IFS=: read -r lineno rest; do
        [[ -z "${lineno}" ]] && continue
        prop="$(echo "${rest}" | sed -E 's/^[[:space:]]*//;s/:.*//')"
        [[ "${prop}" == "defaultColor" ]] && continue
        echo "lint-qml-tokens: ${rel}:${lineno}: literal hex colour in '${prop}' (use a Theme.* token) -> ${rest}" >&2
        status=1
    done < <(grep -nE '"#[0-9A-Fa-f]{3}([0-9A-Fa-f]{3}([0-9A-Fa-f]{2})?)?"' "${file}" || true)

    # Named CSS colour literal in a value position, e.g. `color: "red"`.
    while IFS=: read -r lineno rest; do
        [[ -z "${lineno}" ]] && continue
        prop="$(echo "${rest}" | sed -E 's/^[[:space:]]*//;s/:.*//')"
        echo "lint-qml-tokens: ${rel}:${lineno}: literal named colour in '${prop}' (use a Theme.* token) -> ${rest}" >&2
        status=1
    done < <(grep -nEi ":[[:space:]]*\"(${named_colors})\"" "${file}" || true)

    # Bare non-zero numeric literal assigned to a curated size/geometry
    # property, directly (`radius: 4`) or through a group (`font.pixelSize:
    # 12`, `border.width: 2`). A literal 0 is a structural "no offset/no
    # gap", not a design choice, and is allowed (e.g. `spacing: 0`).
    while IFS=: read -r lineno rest; do
        [[ -z "${lineno}" ]] && continue
        prop="$(echo "${rest}" | sed -E 's/^[[:space:]]*//;s/:.*//')"
        value="$(echo "${rest}" | sed -E "s/^[^:]*:[[:space:]]*//;s/^([0-9]+(\.[0-9]+)?).*/\1/")"
        # Only an exact zero passes; 0.5 is a size choice like any other.
        [[ "${value}" =~ ^0(\.0+)?$ ]] && continue
        echo "lint-qml-tokens: ${rel}:${lineno}: literal pixel size in '${prop}' (use a Theme.* token) -> ${rest}" >&2
        status=1
    done < <(grep -nE "^[[:space:]]*([A-Za-z_]+\.)?(${size_props})[[:space:]]*:[[:space:]]*[0-9]" "${file}" || true)

    # A Canvas repaints on the CPU every frame; the scene graph is the rule.
    while IFS=: read -r lineno rest; do
        [[ -z "${lineno}" ]] && continue
        echo "lint-qml-tokens: ${rel}:${lineno}: Canvas is not allowed (draw on the scene graph) -> ${rest}" >&2
        status=1
    done < <(grep -nE '(^|[^A-Za-z])Canvas[[:space:]]*\{' "${file}" || true)
done

# The module's C++ items draw with scene-graph nodes, never QQuickPaintedItem.
while IFS=: read -r cppfile lineno rest; do
    [[ -z "${cppfile}" ]] && continue
    echo "lint-qml-tokens: ${cppfile#"${repo_root}"/}:${lineno}: QQuickPaintedItem is not allowed (use scene-graph nodes) -> ${rest}" >&2
    status=1
done < <(grep -rnE 'QQuickPaintedItem' "${repo_root}/qt/qml/Dettivo" "${repo_root}/qt/host" "${repo_root}/qt/apps" --include='*.h' --include='*.cpp' || true)

if [[ ${status} -ne 0 ]]; then
    echo "lint-qml-tokens: FAILED" >&2
    exit "${status}"
fi
echo "lint-qml-tokens: OK (${#files[@]} files checked)"
