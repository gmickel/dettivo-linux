# 0069. Releases ship the pacman package until the AUR account exists

Status: Accepted 2026-09-25, amends 0064 (the AUR job runs only when switched on)

## What this gives you

Arch and Omarchy users install Dettivo with one `pacman -U` from the latest GitHub release: the same package CI installed and tested in a clean container before the release went out. Releases stay green while the AUR packages cannot be published.

## Situation

ADR 0064 made every release push `dettivo-bin` and `dettivo` to the AUR. That needs an AUR account holding Dettivo's publishing key. On 2026-09-25 the AUR's registration page answered "New account registration is temporarily closed" (HTTP 503) because of a wave of automated sign-ups, with no date for reopening. Without the account, the `aur` job would fail on every tag, and the documented `yay -S dettivo-bin` would find no package.

## Decision

- `release.yml`'s `publish` job uploads `dettivo-bin-<version>-<pkgrel>-x86_64.pkg.tar.zst` and its `.sha256` beside the tarball and `SHA256SUMS`. It is the package the rig's `install` job installed in a clean container, bound to the tag's revision by `ARTIFACT_SHA256SUMS`, and the job re-downloads it and verifies its checksum after publishing. Its checksum stays out of `SHA256SUMS`, which the `dettivo-bin` recipe verifies against the tarball.
- The README, `docs/install.md`, the user guide and the plugin README lead with a `pacman -U` of that package from the latest release, and say why the AUR is not there yet.
- `distribute.yml`'s `aur` job runs only when the repository variable `AUR_PUBLISH` is `true`. The key and the `AUR_SSH_PRIVATE_KEY` secret are already in place, and the plugin mirror publishes as before.

## Consequences

A package installed with `pacman -U` gets no updates from pacman, so a user installs each new release the same way. The docs say so. Releases still happen only when a version is bumped and tagged, so an ordinary pull request builds nothing for users.

When AUR registration reopens: create the account, paste `~/.ssh/aur-dettivo.pub` into it, run `gh variable set AUR_PUBLISH -b true -R gmickel/dettivo-linux`, then `gh workflow run distribute.yml -f version=<current>` to publish the current release. The docs then go back to `yay -S dettivo-bin`, with the release package as the fallback.
