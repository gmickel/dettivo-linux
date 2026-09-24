# 0064. A release publishes itself to the AUR and the plugin mirror

Status: Accepted 2026-09-24, amends 0034 (CI never touched the AUR) and 0030 (the mirror was written by hand)

## What this gives you

`yay -S dettivo-bin` and `omarchy plugin add` install the version a tag just released, minutes after the release exists. A version reaches every channel it ships through or the release run shows which channel failed, and nobody has to remember a manual step afterwards.

## Situation

ADR 0034 made the tag publish the GitHub release and left `packaging/aur/publish.sh` and `scripts/omarchy/export-plugin.sh` as hand steps, and `publish.sh` refused to run under CI. By 2026-09-24 the 0.1.0 release had existed for ten days and neither step had run: `dettivo-bin` and `dettivo` were not on the AUR, `gmickel/omarchy-dettivo` did not exist, and the install guide's first command failed for anyone who tried it. A hand step whose only reminder is a paragraph in `docs/RELEASING.md` gets skipped. The repository was private too, so the AUR recipes could not have downloaded the tarball anyway.

## Decision

`.github/workflows/distribute.yml` publishes a released version to both channels. The release workflow calls it after `publish` has uploaded the tarball and verified the download, and a manual dispatch with a version runs it again for a release that already exists.

- The `aur` job runs in the pinned Arch container. It takes the recipes from the tag, so the AUR gets what the release shipped, and runs `publish.sh` from the workflow's own revision, so a fix to the script also reaches releases tagged before it. The script runs as an unprivileged user, because `makepkg` refuses root. The key comes from the `AUR_SSH_PRIVATE_KEY` secret. The AUR host key is pinned to its published fingerprint and never learned on first use. `publish.sh` keeps its `DETTIVO_AUR_PUBLISH=1` guard and its `--dry-run` mode, and the CI refusal is gone.
- The `plugin-mirror` job clones `gmickel/omarchy-dettivo` with the write deploy key in `OMARCHY_MIRROR_DEPLOY_KEY`, runs `export-plugin.sh` so the mirror's root equals `omarchy/` at the tag, commits `Release v<version>` when the folder changed, and pushes `main` and the `v<version>` tag.
- A missing secret fails its own job with the secret named. The GitHub release is already out by then, and the other channel still publishes.

## Consequences

Two credentials now sit in the repository's Actions secrets. The AUR key can push to every package its AUR account maintains, so it belongs to an account that maintains Dettivo's packages only, or to a key used for nothing else. The deploy key reaches the mirror only. Both jobs run only from `release.yml` or a manual dispatch, never on a pull request, so a fork's pull request never sees either secret.

`dettivo-engines-cuda` stays off the AUR. Its recipe builds against CUDA 13 and cuDNN 9, and no CI runner has a GPU to prove it on.

Re-running the jobs is safe. The AUR push commits only when the sums or the version changed, and a failed push fails the job. The mirror force-moves its `v<version>` tag to the released folder, so the tag always names what the release shipped.
