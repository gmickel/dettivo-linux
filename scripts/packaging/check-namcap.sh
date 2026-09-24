#!/usr/bin/env bash
# A failed checker cannot certify a package. Valid warnings remain advisory.
set -euo pipefail
checker="$(command -v namcap)" || { echo 'namcap: checker is not installed (pacman -S namcap)' >&2; exit 1; }
command=("$checker")
# Arch's wrapper uses env python3; its Namcap module belongs to system Python.
if [ "$checker" = /usr/bin/namcap ]; then command=(env "PATH=/usr/bin:$PATH" "$checker"); fi
code=0
output="$("${command[@]}" "$@" 2>&1)" || code=$?
printf '%s\n' "$output" | sed 's/^/namcap: /'
if [ "$code" -ne 0 ]; then
  echo "namcap: checker exited $code" >&2
  exit 1
fi
if grep -q ' E: ' <<<"$output"; then
  echo 'namcap reported errors' >&2
  exit 1
fi
