#!/usr/bin/env bash
# Exercise the package builder's namcap gate without rebuilding binaries.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/scripts/packaging" "$work/packaging/aur/dettivo-bin" "$work/bin" "$work/dist"
cp "$root/scripts/packaging/build-package.sh" "$work/scripts/packaging/"
for helper in check-namcap.sh select-package.sh; do
  if [ -f "$root/scripts/packaging/$helper" ]; then cp "$root/scripts/packaging/$helper" "$work/scripts/packaging/"; fi
done
printf 'version = "0.1.0"\n' >"$work/Cargo.toml"
printf 'pkgver=0.1.0\npkgrel=1\n' >"$work/packaging/aur/dettivo-bin/PKGBUILD"
touch "$work/dist/dettivo-0.1.0-linux-x86_64.tar.zst" "$work/dist/SHA256SUMS"
cat >"$work/bin/makepkg" <<'STUB'
#!/usr/bin/env bash
touch "$PKGDEST/dettivo-bin-0.1.0-1-x86_64.pkg.tar.zst"
STUB
cat >"$work/bin/id" <<'STUB'
#!/usr/bin/env bash
echo 1000
STUB
cat >"$work/bin/namcap" <<'STUB'
#!/usr/bin/env bash
case "$NAMCAP_CASE" in
  execution) echo 'ModuleNotFoundError: No module named Namcap' >&2; exit 7 ;;
  error) echo 'dettivo-bin E: invalid library path' ;;
  warning) echo 'dettivo-bin W: optional dependency' ;;
esac
STUB
chmod +x "$work/bin/"*
for scenario in execution error warning; do
  rc=0
  PATH="$work/bin:$PATH" NAMCAP_CASE="$scenario" bash "$work/scripts/packaging/build-package.sh" bin "$work/dist" >"$work/$scenario.log" 2>&1 || rc=$?
  case "$scenario" in
    execution) [[ "$rc" != 0 ]] && grep -q 'namcap.*exited 7' "$work/$scenario.log" ;;
    error) [[ "$rc" != 0 ]] && grep -q 'namcap reported errors' "$work/$scenario.log" ;;
    warning) [[ "$rc" = 0 ]] && grep -q 'W: optional dependency' "$work/$scenario.log" ;;
  esac || { cat "$work/$scenario.log"; echo "test-namcap: $scenario failed" >&2; exit 1; }
done
echo 'test-namcap: execution failure, error and warning policy passed'
