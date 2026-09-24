#!/usr/bin/env bash
set -euo pipefail
root="$(cd -- "$(dirname -- "$0")/../.." && pwd -P)"
binary="$(mktemp /tmp/dettivo-clustering-test.XXXXXX)"
trap 'rm -f -- "$binary"' EXIT
c++ -std=c++17 -O2 -Wall -Wextra -Werror \
    "${root}/crates/sherpa-onnx-sys/tests/calibrated_clustering.cc" -o "$binary"
"$binary"
