#!/usr/bin/env bash
# The docs build: the repository's documentation holds together, and a
# reader can reach every page and trust every generated block.
#
# Usage: scripts/check-docs.sh
#   DETTIVO_CLI=<path>   the `dettivo` binary the CLI-tree check renders with
#                        (default: target/debug/dettivo, built on demand)
#
# Checks, each reported with the file (and the line where one applies):
#   1. Every decision record under docs/adr/ is listed in docs/adr/README.md.
#   2. Every decision record opens with a Status line and a
#      "## What this gives you" section, the value-first shape the ADR
#      template and CONTRIBUTING.md ask for.
#   3. Every relative link in a tracked Markdown file points at a file that
#      exists.
#   4. README.md and CONTRIBUTING.md exist at the repository root.
#   5. Every copied macOS contract document listed in
#      docs/api/CONTRACT_PINS.txt still hashes to the pin recorded there
#      (fn-2 R1) - a fixed-format mismatch check, no error surface beyond
#      "file missing" or "hash does not match the recorded pin".
#   6. Every Markdown page under docs/ is reachable from README.md by
#      following links (through the guides, the ADR index and the topic
#      pages), so a page exists only where a reader can find it (ADR 0041).
#   8. Every key of the configuration schema (the config.keys fixture the
#      daemon's tests hold) is named in docs/config.md.
#   9. The CLI tree in docs/guides/agents.md, between its cli-tree markers,
#      equals `dettivo docs cli-tree`; a stale tree names the first
#      differing line.
#  10. Every migration a page names in backticks (`0007-notes-analysis`) is
#      a file of crates/dettivo-storage/migrations/, so the record names
#      the chain the daemon runs.
#  11. Every `just <recipe>` in a code span or a code block names a recipe
#      of the justfile and passes it positional arguments only: just hands
#      `engines=x` after a recipe to the recipe as the literal argument
#      `engines=x`, and an argument past the recipe's parameters is read
#      as the next recipe to run.
# Exit 0 when everything passes, 1 with one line per finding.
set -uo pipefail

root="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$root"

findings=0
report() {
  echo "docs: $1" >&2
  findings=$((findings + 1))
}

for required in README.md CONTRIBUTING.md docs/adr/README.md docs/adr/template.md; do
  [ -f "$required" ] || report "$required is missing"
done

for adr in docs/adr/[0-9][0-9][0-9][0-9]-*.md; do
  [ -f "$adr" ] || continue
  name="$(basename "$adr")"
  if ! grep -qF "($name)" docs/adr/README.md; then
    report "$adr is not listed in docs/adr/README.md"
  fi
  if ! grep -qE '^Status: ' "$adr"; then
    report "$adr has no 'Status:' line"
  fi
  first_h2="$(grep -m1 -E '^## ' "$adr" || true)"
  if [ "$first_h2" != "## What this gives you" ]; then
    report "$adr must open with '## What this gives you' (found '${first_h2:-none}')"
  fi
done

# The relative link targets of one Markdown file, one per line, resolved
# against the file's directory (anchors and URLs dropped); the reachability
# check below walks the same edges.
links_of() {
  local file="$1" dir target path
  dir="$(dirname "$file")"
  grep -oE '\]\([^) ]+\)' "$file" | sed -E 's/^\]\((.*)\)$/\1/' | while IFS= read -r target; do
    [ -n "$target" ] || continue
    case "$target" in
      http://*|https://*|mailto:*|\#*) continue ;;
    esac
    path="${target%%#*}"
    [ -n "$path" ] || continue
    printf '%s\n' "$dir/$path"
  done
}

mapfile -t tracked_md < <(git ls-files -z -- '*.md' | tr '\0' '\n')

for file in "${tracked_md[@]}"; do
  while IFS= read -r path; do
    [ -n "$path" ] || continue
    if [ ! -e "$path" ]; then
      report "$file links to ${path#"$(dirname "$file")"/} which does not exist"
    fi
  done < <(links_of "$file")
done

pins_file="docs/api/CONTRACT_PINS.txt"
pinned=()
is_pinned() {
  local p
  for p in "${pinned[@]}"; do
    [ "$p" = "$1" ] && return 0
  done
  return 1
}
if [ -f "$pins_file" ]; then
  while IFS= read -r line; do
    case "$line" in
      ""|"#"*) continue ;;
    esac
    path="$(echo "$line" | awk '{print $1}')"
    want_hash="$(echo "$line" | awk '{print $2}')"
    # Only a tracked file under docs/api/ may be pinned: the register is
    # repository input, so a path outside that directory (or one git does
    # not track) is refused before anything is read or hashed.
    case "$path" in
      docs/api/*) ;;
      *) report "$path is listed in $pins_file but pins must name files under docs/api/"; continue ;;
    esac
    case "$path" in
      *..*) report "$path is listed in $pins_file but contains a parent-directory segment"; continue ;;
    esac
    if ! git ls-files --error-unmatch -- "$path" >/dev/null 2>&1; then
      report "$path is listed in $pins_file but is not a tracked file"
      continue
    fi
    # A symlink would make sha256sum read whatever it points at, inside the
    # repository or not, so a pinned path has to be a regular file.
    if [ -L "$path" ] || [ ! -f "$path" ]; then
      report "$path is listed in $pins_file but is not a regular file (symlinks are refused)"
      continue
    fi
    pinned+=("$path")
    got_hash="$(sha256sum -- "$path" | awk '{print $1}')"
    if [ "$got_hash" != "$want_hash" ]; then
      report "$path does not match the pin recorded in $pins_file (got $got_hash, want $want_hash) - it must only be refreshed by re-copying from a new macOS commit, never hand-edited"
    fi
  done < "$pins_file"
fi

# 6. Reachability: a breadth-first walk over the links from README.md.
declare -A reached=()
queue=(README.md)
reached[README.md]=1
while [ "${#queue[@]}" -gt 0 ]; do
  file="${queue[0]}"
  queue=("${queue[@]:1}")
  [ -f "$file" ] || continue
  while IFS= read -r path; do
    [ -n "$path" ] || continue
    norm="$(realpath --relative-to="$root" -m -- "$path" 2>/dev/null || printf '%s' "$path")"
    case "$norm" in
      *.md) ;;
      *) continue ;;
    esac
    if [ -z "${reached[$norm]:-}" ]; then
      reached[$norm]=1
      queue+=("$norm")
    fi
  done < <(links_of "$file")
done
for file in "${tracked_md[@]}"; do
  case "$file" in
    docs/*) ;;
    *) continue ;;
  esac
  if [ -z "${reached[$file]:-}" ]; then
    report "$file is not reachable from README.md by links; link it from a guide, a topic page or the ADR index, or remove it"
  fi
done

# 8. Every schema key is documented.
keys_fixture="crates/dettivo-proto/fixtures/config/keys.json"
if [ -f "$keys_fixture" ] && [ -f docs/config.md ]; then
  if command -v jq >/dev/null 2>&1; then
    while IFS= read -r key; do
      [ -n "$key" ] || continue
      leaf="${key##*.}"
      if ! grep -qF "\`$key\`" docs/config.md && ! grep -qF "\`$leaf\`" docs/config.md; then
        report "docs/config.md does not name the schema key $key"
      fi
    done < <(jq -r '.. | objects | select(has("key") and has("kind")) | .key' "$keys_fixture" | sort -u)
  else
    report "jq is missing (pacman -S jq); the config-key check cannot run"
  fi
fi

# 9. The CLI tree golden.
guide="docs/guides/agents.md"
if [ -f "$guide" ]; then
  cli="${DETTIVO_CLI:-target/debug/dettivo}"
  if [ ! -x "$cli" ] && [ -z "${DETTIVO_CLI:-}" ]; then
    cargo build -q -p dettivo-cli >/dev/null 2>&1 || report "cargo build -p dettivo-cli failed; the CLI-tree check needs the binary"
  fi
  if [ -x "$cli" ]; then
    expected="$(mktemp)"
    actual="$(mktemp)"
    if "$cli" docs cli-tree --register docs/api/linux-deltas.md >"$expected" 2>/dev/null; then
      begin="$(grep -n -F '<!-- cli-tree:begin -->' "$guide" | head -1 | cut -d: -f1)"
      end="$(grep -n -F '<!-- cli-tree:end -->' "$guide" | head -1 | cut -d: -f1)"
      if [ -z "$begin" ] || [ -z "$end" ] || [ "$end" -le "$begin" ]; then
        report "$guide has no cli-tree markers around the generated tree"
      else
        # The lines between the markers minus the code fence lines.
        sed -n "$((begin + 1)),$((end - 1))p" "$guide" | grep -v '^```' >"$actual"
        if ! cmp -s "$expected" "$actual"; then
          first="$(diff "$actual" "$expected" | grep -m1 -E '^[0-9]' | grep -oE '^[0-9]+')"
          [ "${first:-0}" -ge 1 ] || first=1
          offset="$(sed -n "$((begin + 1)),$((end - 1))p" "$guide" | grep -n -v '^```' | sed -n "${first}p" | cut -d: -f1)"
          report "$guide:$((begin + ${offset:-1})) the CLI tree differs from \`dettivo docs cli-tree\` at that line; paste the command's output between the markers"
        fi
      fi
    else
      report "$cli docs cli-tree failed; the CLI-tree check cannot run"
    fi
    rm -f "$expected" "$actual"
  else
    report "$cli is not an executable; set DETTIVO_CLI or build dettivo-cli"
  fi
fi

# 10. Every migration a page names exists in the daemon's chain.
for file in "${tracked_md[@]}"; do
  case "$file" in
    docs/reports/reviews/*) continue ;;
    docs/*) ;;
    *) continue ;;
  esac
  while IFS=: read -r line_no name; do
    [ -n "$name" ] || continue
    if [ ! -f "crates/dettivo-storage/migrations/$name.sql" ]; then
      report "$file:$line_no names migration \`$name\`, which is not in crates/dettivo-storage/migrations/"
    fi
  done < <(grep -n -oE '`[0-9]{4}-[a-z-]+`' "$file" | tr -d '`' | sort -u)
done

# 11. Every just invocation in code names a recipe with positional arguments.
recipe_table="$(sed -n -E 's/^([a-z][a-z0-9-]*)( [^:]*)?:.*/\1 \2/p' justfile)"
for file in "${tracked_md[@]}"; do
  case "$file" in
    docs/reports/reviews/*) continue ;;
    docs/*|README.md|CONTRIBUTING.md) ;;
    *) continue ;;
  esac
  is_pinned "$file" && continue
  # A token past a recipe's parameters is the next recipe (`just build
  # test lint`), so the parse walks recipe by recipe.
  while IFS=$'\t' read -r line_no recipe args; do
    [ -n "$recipe" ] || continue
    for arg in $recipe $args; do
      case "$arg" in
        [a-z_]*=*)
          report "$file:$line_no passes \`$arg\` to \`just $recipe\`; just reads it as the literal argument, so write the value alone (\`just $recipe ${arg#*=}\`)"
          continue
          ;;
      esac
      if [ "$arg" = "$recipe" ] || { [ "${variadic:-0}" -eq 0 ] && [ "${count:-0}" -ge "${max:-0}" ]; }; then
        # A recipe name: the one the line started with, or the next one.
        params="$(printf '%s\n' "$recipe_table" | awk -v r="$arg" '$1 == r { $1 = ""; print; found = 1; exit } END { if (!found) print "-" }')"
        if [ "$params" = "-" ]; then
          if [ "$arg" = "$recipe" ]; then
            report "$file:$line_no runs \`just $recipe\`, which is not a recipe of the justfile"
          else
            report "$file:$line_no passes \`$arg\` to \`just $recipe\`, which takes $max argument(s); just would read it as the next recipe"
          fi
          break
        fi
        recipe="$arg"
        variadic=0
        max=0
        count=0
        for param in $params; do
          case "$param" in
            \**|+*) variadic=1 ;;
            *) max=$((max + 1)) ;;
          esac
        done
        continue
      fi
      count=$((count + 1))
    done
  done < <(awk '
    # Every `just <recipe> <args>` in a code span or a fenced block, as
    # "<line>\t<recipe>\t<args>"; the arguments end at a shell operator,
    # a comment or a closing bracket.
    function emit(snippet,    rest, recipe, args, w, n, i, stop) {
      while (match(snippet, /(^|[ (;|&])just [a-z][a-z0-9-]*/)) {
        rest = substr(snippet, RSTART + RLENGTH)
        recipe = substr(snippet, RSTART, RLENGTH)
        sub(/^.*just /, "", recipe)
        snippet = rest
        args = ""
        n = split(rest, w, " ")
        for (i = 1; i <= n; i++) {
          if (w[i] ~ /^(#|;|&&|\|\||\||\))/) break
          stop = 0
          if (w[i] ~ /[;)]$/) { sub(/[;)]+$/, "", w[i]); stop = 1 }
          args = args (args == "" ? "" : " ") w[i]
          if (stop) break
        }
        printf "%d\t%s\t%s\n", NR, recipe, args
      }
    }
    /^```/ { fence = !fence; next }
    fence { emit($0); next }
    {
      n = split($0, parts, "`")
      for (i = 2; i <= n; i += 2) emit(parts[i])
    }
  ' "$file")
done

if [ "$findings" -eq 0 ]; then
  echo "docs: ok"
  exit 0
fi
echo "docs: $findings finding(s)" >&2
exit 1
