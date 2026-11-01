# Dettivo on more Linux distributions, with updates through the package manager

## Goal & Context

Dettivo ships one pacman package attached to each GitHub release. The AUR packages exist in the repository, but publishing is switched off (ADR 0069), because AUR account registration closed in September 2026.
- **Arch.** As of 2026-11-01, third-party reports say registration is still closed with no announced date. The register page sits behind a JavaScript proof-of-work check, so it couldn't be confirmed directly.
- **Updates.** Arch and Omarchy users install with `pacman -U` and get no updates from pacman.
- **Other distros.** Users on Debian, Ubuntu, Fedora, openSUSE and Nix have no package at all.

This spec ships updatable packages for Arch without waiting for the AUR, and adds packages for the other major distributions. It keeps the AUR path ready to switch on the day an account can be made.

## Approach

**A signed pacman repository (Arch and Omarchy, now).**
- Each release adds `dettivo-bin` to a pacman repository database (`repo-add`) signed with a dedicated GPG key, and publishes it at a stable URL: a GitHub Pages site or a `pacman` release asset set.
- Users add the repository once, and from then on `pacman -Syu` updates Dettivo.

```ini
[dettivo]
SigLevel = Required DatabaseOptional
Server = https://gmickel.github.io/dettivo-linux/pacman/$arch
```

- `dettivo setup omarchy` can offer to add it.
- The `pacman -U` asset stays for one-off installs.
- The key's fingerprint is published in the README and `docs/install.md`.

**Debian/Ubuntu and Fedora/openSUSE.**
- Build `.deb` and `.rpm` from the same release binaries and install layout (ADR 0034), with nfpm or the native tools; plan decides.
- Dependencies are expressed per distro: PipeWire, Qt 6 at the floor ADR 0013 sets, and the Vulkan loader.
- Publish signed apt and dnf repositories next to the pacman one, so updates come through the distro's tool.
- CI installs each package in a clean container of its distro (as the rig's `install` job does for Arch) and runs `dettivo doctor --json` and a mock-mic dictation before the release goes out.

```yaml
# release matrix sketch
strategy:
  matrix:
    target: [arch, debian-13, ubuntu-24.04, fedora-43, opensuse-tumbleweed]
steps:
  - run: scripts/packaging/build-${{ matrix.target }}.sh "$VERSION"
  - run: scripts/packaging/install-test.sh "${{ matrix.target }}"   # clean container: install, doctor, mock dictation
```

**Nix.** A flake in the repository builds the daemon, CLI, engines (Vulkan) and app. CI runs `nix flake check`. Submission to nixpkgs is a later step.

**Install script.** `curl -fsSL <url>/install.sh | sh` detects the distro and adds the right repository. It never builds from source and never runs as root without saying why. It is checked by shellcheck and run in each CI container.

**AUR, when possible.**
- Keep `packaging/aur/` and the `distribute.yml` job ready.
- Add `scripts/packaging/aur-status.sh`, which reports whether the AUR accepts registrations and whether the names `dettivo` and `dettivo-bin` are free (RPC `info`). Run it in a weekly scheduled job that opens one issue when registration reopens.
- The ADR 0069 switch-on steps stay the procedure. Making the account is a human step.
- A co-maintainer route (an existing AUR maintainer publishes and adds Gordon later) is documented as an option, not taken without Gordon's go-ahead.

**Docs.**
- `docs/install.md` leads with the repository for each distro, then the one-off package.
- The README's install table follows.
- The strategy's "no Flatpak or Snap in v1" stays.

## Acceptance

- **R1:** A release publishes a signed pacman repository. After adding it, `pacman -Syu` on a clean Arch container installs and later upgrades Dettivo. CI proves both against the previous release.
- **R2:** A release publishes `.deb` and `.rpm` packages and signed apt and dnf repositories. Each installs in a clean Debian, Ubuntu, Fedora and openSUSE container, passes `dettivo doctor --json` (no failures beyond missing hardware), and completes a mock-mic dictation in CI.
- **R3:** `nix build` and `nix flake check` pass in CI, and the flake's package runs `dettivo --version`.
- **R4:** The install script adds the right repository on each supported distro in CI, refuses unsupported ones with a clear message, and passes shellcheck.
- **R5:** `scripts/packaging/aur-status.sh` reports registration status and name availability. A weekly job opens a single issue when registration reopens, and `distribute.yml` stays switched off until `AUR_PUBLISH` is set.
- **R6:** An ADR records the repository hosting, the signing keys and their custody, and the supported distro matrix. `docs/install.md`, `docs/RELEASING.md` and the README are updated.

## Boundaries

- No Flatpak or Snap.
- No change to the Arch release asset or the plugin mirror.
- Creating the AUR account and adopting a co-maintainer are Gordon's decisions.

## Quick commands

- `scripts/packaging/lint.sh`
- `just build test lint`
