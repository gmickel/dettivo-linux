#!/usr/bin/env bash
# Package staging and packaging must read the same selected directory.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
output="$(just --dry-run package-bin build/entrypoint-test 2>&1)"
for expected in 'scripts/package.sh build/entrypoint-test' 'scripts/packaging/build-package.sh bin build/entrypoint-test'; do
    if [[ "$output" != *"$expected"* ]]; then
        printf 'test-build-entrypoints: missing %s in dry run:\n%s\n' "$expected" "$output" >&2
        exit 1
    fi
done
printf 'test-build-entrypoints: package uses the selected directory\n'

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/scripts/packaging" "$work/build/aur" "$work/packaging/aur/dettivo-bin"
cp justfile Cargo.toml "$work/"
cp scripts/run-step.sh "$work/scripts/"
cp packaging/aur/dettivo-bin/PKGBUILD "$work/packaging/aur/dettivo-bin/"
if [ -f scripts/packaging/select-package.sh ]; then
    cp scripts/packaging/select-package.sh "$work/scripts/packaging/"
fi
cat >"$work/scripts/packaging/install-test.sh" <<'STUB'
#!/usr/bin/env bash
printf '%s\n' "$@" > selected-args
STUB
chmod +x "$work/scripts/packaging/install-test.sh"
version="$(sed -n -E 's/^pkgver=(.*)/\1/p' packaging/aur/dettivo-bin/PKGBUILD)"
release="$(sed -n -E 's/^pkgrel=(.*)/\1/p' packaging/aur/dettivo-bin/PKGBUILD)"
app="build/aur/dettivo-bin-$version-$release-x86_64.pkg.tar.zst"
touch "$work/$app" "$work/build/aur/dettivo-bin-debug-$version-$release-x86_64.pkg.tar.zst" \
    "$work/build/aur/dettivo-bin-0.0.1-1-x86_64.pkg.tar.zst"
(cd "$work" && just install-test >/dev/null)
mapfile -t selected < <(grep '\.pkg\.tar\.zst$' "$work/selected-args")
[[ "${#selected[@]}" = 1 && "${selected[0]}" = "$app" ]] || {
    echo "test-build-entrypoints: recipe must select only $app" >&2; exit 1;
}
if bash scripts/packaging/install-test.sh --root "$work/root" "$work/$app" "$work/$app" >"$work/ambiguous.log" 2>&1; then
    echo 'test-build-entrypoints: ambiguous package arguments accepted' >&2; exit 1
fi
grep -q 'exactly one package' "$work/ambiguous.log"
if bash scripts/packaging/install-test.sh --root "$work/root" "$work/build/aur/dettivo-bin-debug-$version-$release-x86_64.pkg.tar.zst" >"$work/debug.log" 2>&1; then
    echo 'test-build-entrypoints: debug package accepted' >&2; exit 1
fi
grep -q 'application package' "$work/debug.log"
echo 'test-build-entrypoints: exact application selection and argument rejection passed'
