#!/usr/bin/env bash
# The C++ under qt/ compiles at the Qt floor: a Qt API newer than 6.8
# (ADR 0013) appears only inside a `#if QT_VERSION >= QT_VERSION_CHECK(...)`
# block that names at least the version the API arrived in, with the
# floor's own call on the `#else` side. find_package(Qt6 6.8) accepts a
# 6.8 install that a bare call would then fail to compile against.
#
# Usage: scripts/lint-qt-floor.sh
# Exit 0 when every use is guarded, 1 with one line per unguarded use.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

# API name and the Qt version that introduced it, from the Qt documentation.
apis=(
    "beginFilterChange 6.9"
    "endFilterChange 6.10"
)

status=0
while IFS= read -r file; do
    for entry in "${apis[@]}"; do
        api="${entry%% *}"
        version="${entry#* }"
        grep -q "\b${api}(" "${file}" || continue
        # Walk the preprocessor conditionals: a stack of the Qt version each
        # open `#if QT_VERSION >= QT_VERSION_CHECK(major, minor, patch)`
        # guarantees (0 for any other conditional and for the `#else` side).
        out="$(awk -v api="${api}" -v need="${version}" '
            function guaranteed(    i, best) {
                best = 0
                for (i = 1; i <= depth; i++)
                    if (stack[i] > best) best = stack[i]
                return best
            }
            function version_of(line,    m, a) {
                if (match(line, /QT_VERSION *>= *QT_VERSION_CHECK\( *[0-9]+ *, *[0-9]+ *, *[0-9]+ *\)/)) {
                    m = substr(line, RSTART, RLENGTH)
                    gsub(/[^0-9,]/, "", m)
                    split(m, a, ",")
                    return a[1] + a[2] / 100
                }
                return 0
            }
            /^[ \t]*#[ \t]*if/ { depth++; stack[depth] = version_of($0); next }
            /^[ \t]*#[ \t]*(else|elif)/ { if (depth > 0) stack[depth] = 0; next }
            /^[ \t]*#[ \t]*endif/ { if (depth > 0) depth--; next }
            $0 ~ ("\\<" api "\\(") {
                split(need, n, ".")
                if (guaranteed() < n[1] + n[2] / 100)
                    printf "%s:%d: %s() needs Qt %s; the floor is 6.8, so guard it with QT_VERSION_CHECK and keep the floor call on the #else side\n", FILENAME, NR, api, need
            }
        ' "${file}")"
        if [[ -n "${out}" ]]; then
            echo "${out}" >&2
            status=1
        fi
    done
done < <(git ls-files 'qt/*.cpp' 'qt/*.h')

if [[ "${status}" -eq 0 ]]; then
    echo "qt-floor: ok"
fi
exit "${status}"
