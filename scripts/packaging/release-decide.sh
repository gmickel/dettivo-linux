#!/usr/bin/env bash
# The release workflow's one decision, in a script the test can drive: the
# version from Cargo.toml and whether this run publishes. A tag push of
# `v<version>` publishes; a dispatch publishes only with `dry_run=false`
# on that same tag; everything else rehearses without publishing, or
# refuses by name. A rehearsal is never turned into a publication by the
# ref it happens to run on.
#
# Usage: scripts/packaging/release-decide.sh [cargo-toml]
# Reads GITHUB_EVENT_NAME, GITHUB_REF_TYPE, GITHUB_REF_NAME and DRY_RUN
# (`true`/`false`; unset counts as true). Prints `version=<v>` and
# `publish=<true|false>`, one per line, the shape $GITHUB_OUTPUT takes.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cargo_toml="${1:-$root/Cargo.toml}"
event="${GITHUB_EVENT_NAME:-}"
ref_type="${GITHUB_REF_TYPE:-}"
ref_name="${GITHUB_REF_NAME:-}"
dry_run="${DRY_RUN:-true}"

version="$(grep -m1 -E '^version = ' "$cargo_toml" | sed -E 's/version = "([^"]+)"/\1/')"
[ -n "$version" ] || { echo "release: no version in $cargo_toml" >&2; exit 1; }
tag="v${version}"

on_tag=false
if [ "$ref_type" = tag ]; then
  if [ "$ref_name" != "$tag" ]; then
    echo "release: the tag ${ref_name} does not match Cargo.toml's version ${version}" >&2
    exit 1
  fi
  on_tag=true
fi

publish=false
case "$event" in
  push)
    [ "$on_tag" = true ] || { echo "release: a push only releases from the ${tag} tag, not from ${ref_type} ${ref_name}" >&2; exit 1; }
    publish=true
    ;;
  workflow_dispatch)
    if [ "$dry_run" = false ]; then
      [ "$on_tag" = true ] || { echo "release: a publishing dispatch needs the ${tag} tag; dispatch with dry_run on ${ref_type} ${ref_name}" >&2; exit 1; }
      publish=true
    fi
    ;;
  *)
    echo "release: ${event:-no event} is not a release event" >&2
    exit 1
    ;;
esac

echo "version=${version}"
echo "publish=${publish}"
