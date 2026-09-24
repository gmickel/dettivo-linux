#!/usr/bin/env bash
# No release route carries developer text (ADR 0011, R7; ADR 0037): QML
# string literals under qt/apps and the shared module may not contain
# exception names, stack-trace frames, serialised JSON, raw paths, model
# file names, internal identifiers, the word `debug` or hashes and
# addresses, the same classes `dettivo-qa` scans on every captured tree
# (crates/dettivo-qa/src/negative_text.rs). Test files and the design
# sheet (a developer tool) are exempt.
#
# Usage: scripts/lint-release-text.sh
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mapfile -t files < <(find "${repo_root}/qt/apps" "${repo_root}/qt/qml" -name '*.qml' -type f \
    ! -path '*/tests/*' ! -path '*/dettivo-sheet/*' | sort)
[[ ${#files[@]} -gt 0 ]] || { echo "lint-release-text: no QML files found" >&2; exit 2; }

# Each pattern is checked inside double-quoted string literals only.
patterns=(
    'Exception|Traceback|panicked|unwrap\(\)|NullPointer'          # exceptions
    '\.(rs|cpp|qml|h):[0-9]+'                                       # stack-trace frames (a source location)
    '"[[:space:]]*(\{[[:space:]]*\\"|\[[[:space:]]*[\{\["0-9])'    # raw JSON (a literal that opens an object or array)
    '/home/|/tmp/|/usr/|/var/|(^|[^~"])/\.(local|config)/|(^|[^/])\.(local|config)/'   # raw paths (a ~/ path is user text)
    '\.gguf|\.bin\b|\.onnx|ggml-|\.safetensors'                    # model file names
    'fn-[0-9]+|TODO|FIXME|XXX|lorem ipsum|placeholder text|parity_gap'   # internal identifiers
    '(^|[^A-Za-z0-9_])[Dd][Ee][Bb][Uu][Gg]([^A-Za-z0-9_]|$)'        # the word debug
    '0x[0-9A-Fa-f]{6,}|[0-9a-f]{32,}'                              # hashes and addresses
)
labels=("exception text" "stack trace" "raw JSON" "raw path" "model file name" "internal identifier" "debug text" "hash or address")

status=0
for file in "${files[@]}"; do
    rel="${file#"${repo_root}"/}"
    while IFS=: read -r lineno rest; do
        [[ -z "${lineno}" ]] && continue
        literals="$(grep -oE '"[^"]*"' <<<"${rest}" || true)"
        [[ -z "${literals}" ]] && continue
        for i in "${!patterns[@]}"; do
            candidates="${literals}"
            # A literal that is nothing but the word is a value the
            # product enumerates (the `debug` log level), not a sentence.
            if [[ "${labels[$i]}" == "debug text" ]]; then
                candidates="$(grep -viE '^"debug"$' <<<"${literals}" || true)"
                [[ -z "${candidates}" ]] && continue
            fi
            if grep -qE "${patterns[$i]}" <<<"${candidates}"; then
                echo "lint-release-text: ${rel}:${lineno}: ${labels[$i]} in a user-visible string -> ${rest}" >&2
                status=1
            fi
        done
    done < <(grep -nE '"[^"]*"' "${file}" || true)
done

if [[ ${status} -ne 0 ]]; then
    echo "lint-release-text: FAILED" >&2
    exit ${status}
fi
echo "lint-release-text: OK (${#files[@]} files, ${#patterns[@]} classes)"
