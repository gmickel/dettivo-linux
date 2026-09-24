# 0065. Dettivo for Linux is licensed GPL-3.0-or-later

Status: Accepted 2026-09-24, amends 0034 (the package ships the licence text at the same paths)

## What this gives you

You can use, study, change and share Dettivo for Linux, and so can everyone who gets a copy from you. Any product built on it and distributed must be released under the same licence, with its source.

## Situation

The repository carried the MIT licence while it was private, and 0.1.0 shipped under it to no public audience. The project was about to become public.

## Decision

From 2026-09-24 the repository, the packages and the Omarchy plugin folder are licensed under the GNU General Public License, version 3 or any later version (SPDX `GPL-3.0-or-later`). `LICENSE` and `omarchy/LICENSE` hold the verbatim GPLv3 text. The workspace `Cargo.toml`, the plugin's `manifest.json` and the three AUR recipes declare the SPDX identifier. `NOTICE.md` keeps listing every bundled component under its own licence.

## Consequences

Every bundled dependency's licence permits distribution under GPL-3.0-or-later. That covers the MIT, Apache-2.0, BSD, ISC, Zlib and Unicode crates, symphonia's MPL-2.0, and Qt and layer-shell-qt linked dynamically under the LGPL. The models the catalogue downloads keep their own licences and are never part of the package.

Gordon Mickel holds the copyright in all code to date. A contribution merged without a separate agreement is available under GPL-3.0-or-later only.
