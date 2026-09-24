"""Approved rasters retain small independent colour and geometry changes."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile


def check(binary):
    with tempfile.TemporaryDirectory(prefix="dettivo-strict-") as directory:
        root = Path(directory)
        width = 512
        data = bytearray([80, 80, 80] * width * width)
        for y in range(100, 400):
            for x in range(100, 400):
                data[(y * width + x) * 3 : (y * width + x) * 3 + 3] = bytes([120] * 3)

        def write(name, pixels):
            path = root / (name + ".ppm")
            path.write_bytes(b"P6\n512 512\n255\n" + pixels)
            return path

        base = write("base", data)
        plants = {
            "unchanged": [],
            "colour": [(200, 200)],
            "type-stroke": [(201, y) for y in range(200, 204)],
            "spacing-edge": [(202, y) for y in range(200, 204)],
            "missing-control": [(x, y) for x in range(200, 202) for y in range(200, 202)],
        }
        failures = []
        for name, coordinates in plants.items():
            changed = data.copy()
            for x, y in coordinates:
                changed[(y * width + x) * 3 : (y * width + x) * 3 + 3] = bytes([220] * 3)
            candidate = write(name, changed)
            result = subprocess.run(
                [binary, "compare", str(base), str(candidate), "--strict", "--json"],
                env={**os.environ, "QT_QPA_PLATFORM": "offscreen"},
                capture_output=True, text=True, check=False,
            )
            expected = 0 if name == "unchanged" else 1
            print(name, result.returncode, result.stdout.strip())
            if result.returncode != expected or json.loads(result.stdout)["pass"] != (expected == 0):
                failures.append(name)
        assert not failures, f"strict comparison missed {failures}"


if __name__ == "__main__":
    check(sys.argv[1])
