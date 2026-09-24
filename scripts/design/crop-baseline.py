#!/usr/bin/env python3
"""Cut the approved Black Gold crops out of the artboards the visual
manifest names (ADR 0021).

Reads qa/visual/manifest.toml and, for every surface with an artboard under
docs/design/studio/baselines/, writes docs/design/studio/baselines/<surface>/
<state>.png: with `crop = "boxes"` the bordered boxes touching the left
edge, top to bottom, one per state in manifest order (the six 2x pills of
osd.png); with a per-state `crop = [x, y, w, h]` that rectangle, cut from
the state's own `artboard` when it names one (the three first-run screens)
and from the surface's otherwise. A surface without a crop rule keeps the
artboard itself as its crop and gets no file.
Run it again after a re-approved artboard lands; `dettivo-qa visual`
compares the renders against these crops.

Usage: scripts/design/crop-baseline.py [surface ...]   (default: every surface)
Needs Pillow (Arch: python-pillow).
"""
from __future__ import annotations

import sys
import tomllib
from pathlib import Path

from PIL import Image

LEFT_EDGE = 48  # every box starts on this column in a sheet
MIN_HEIGHT = 40  # a box is taller than the divider hairlines


def find_boxes(image: Image.Image) -> list[tuple[int, int, int, int]]:
    """The bordered boxes touching LEFT_EDGE, top to bottom, as (x, y, w, h)."""
    rgb = image.convert("RGB")
    pixels = rgb.load()
    width, height = rgb.size
    background = pixels[0, 0]

    def is_border(x: int, y: int) -> bool:
        r, g, b = pixels[x, y]
        br, bg, bb = background
        return abs(r - br) + abs(g - bg) + abs(b - bb) > 60

    boxes = []
    y = 0
    while y < height:
        if is_border(LEFT_EDGE, y):
            top = y
            while y < height and is_border(LEFT_EDGE, y):
                y += 1
            box_height = y - top
            if box_height >= MIN_HEIGHT:
                x = LEFT_EDGE
                while x < width and is_border(x, top):
                    x += 1
                boxes.append((LEFT_EDGE, top, x - LEFT_EDGE, box_height))
        else:
            y += 1
    return boxes


def crop_surface(root: Path, surface: dict) -> int:
    """Writes the crops of one surface; returns the number written."""
    artboard = surface.get("artboard")
    states = surface.get("state", [])
    if artboard is None and not any("artboard" in state for state in states):
        return 0
    baselines = root / "docs/design/studio/baselines"
    out_dir = baselines / surface["name"]
    rects: list[tuple[str, Path, tuple[int, int, int, int]]] = []
    if surface.get("crop") == "boxes":
        sheet = baselines / artboard
        if not sheet.is_file():
            print(f"crop-baseline: {surface['name']}: artboard missing: {sheet.relative_to(root)}", file=sys.stderr)
            return -1
        boxes = find_boxes(Image.open(sheet))
        if len(boxes) < len(states):
            print(
                f"crop-baseline: {surface['name']}: found {len(boxes)} boxes in {sheet.relative_to(root)}, expected {len(states)}",
                file=sys.stderr,
            )
            return -1
        rects = [(state["name"], sheet, box) for state, box in zip(states, boxes)]
    else:
        # A state may carry its own artboard (one artboard per first-run step).
        for state in states:
            if "crop" not in state:
                continue
            sheet = baselines / state.get("artboard", artboard)
            if not sheet.is_file():
                print(f"crop-baseline: {surface['name']}/{state['name']}: artboard missing: {sheet.relative_to(root)}", file=sys.stderr)
                return -1
            rects.append((state["name"], sheet, tuple(state["crop"])))
    if not rects:
        return 0
    out_dir.mkdir(parents=True, exist_ok=True)
    for name, sheet, (x, y, w, h) in rects:
        crop = Image.open(sheet).crop((x, y, x + w, y + h))
        target = out_dir / f"{name}.png"
        crop.save(target, optimize=True)
        print(f"{target.relative_to(root)}: {w}x{h}+{x}+{y}")
    return len(rects)


def main(argv: list[str]) -> int:
    root = Path(__file__).resolve().parents[2]
    manifest = tomllib.loads((root / "qa/visual/manifest.toml").read_text())
    wanted = set(argv)
    surfaces = [s for s in manifest.get("surface", []) if not wanted or s["name"] in wanted]
    if wanted and len(surfaces) != len(wanted):
        known = ", ".join(s["name"] for s in manifest.get("surface", []))
        print(f"crop-baseline: unknown surface in {sorted(wanted)}; the manifest has {known}", file=sys.stderr)
        return 2
    status = 0
    for surface in surfaces:
        if crop_surface(root, surface) < 0:
            status = 1
    return status


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
