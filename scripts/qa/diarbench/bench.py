#!/usr/bin/env python3
"""`just diar-bench`: score the current tree's speaker assignment on cached inputs.

Stages: audio (hashed), engine turns, transcript segments (Whisper for AMI, the product's
stored segments for meetings), assignment (the product's Rust rule), scoring. Each is
cached by content, so a changed rule reruns only assignment and scoring. `--full` also
runs the engines and Whisper wherever their inputs changed. The scoreboard goes to
<eval>/runs/; `--save` keeps it as a baseline, `--baseline` prints deltas against one.
See docs/diarization-bench.md.
"""
import argparse
import datetime
import json
import os
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))


def venv_python(root):
    return root / ".venv/bin/python"


def ensure_numpy(root):
    """Re-runs this script under the bench's environment when numpy is missing here."""
    try:
        import numpy  # noqa: F401
    except ImportError:
        python = venv_python(root)
        if not python.exists() or Path(sys.executable) == python:
            raise SystemExit("the bench environment is missing: run `just diar-bench-setup`")
        os.execv(python, [str(python), *sys.argv])


def parse():
    parser = argparse.ArgumentParser(prog="just diar-bench", description=__doc__.splitlines()[0])
    parser.add_argument("--full", action="store_true", help="also run the engines and Whisper where inputs changed")
    parser.add_argument("--heldout", action="store_true", help="also score the held-out AMI test split")
    parser.add_argument("--baseline", metavar="NAME", help="print per-metric deltas against a saved baseline")
    parser.add_argument("--save", metavar="NAME", help="store this run as a baseline")
    parser.add_argument("--report", action="store_true", help="write an aggregate-only report under docs/")
    parser.add_argument("--variant", default="product", help="assignment variant (examples/diar_assign.rs)")
    parser.add_argument("--set", action="append", default=[], nargs=2, metavar=("KEY", "VALUE"),
                        help="a variant parameter, repeatable")
    return parser.parse_args()


def params_of(pairs):
    out = {}
    for k, v in pairs:
        try:
            out[k] = json.loads(v)
        except ValueError:
            out[k] = v
    return out


def main():
    args = parse()
    from data import eval_dir
    root = eval_dir()
    if not (root / "selection.json").exists():
        raise SystemExit(f"{root} is not set up: run `just diar-bench-setup`")
    root.chmod(0o700)
    ensure_numpy(root)
    import stages
    from cache import Cache
    from data import config
    import metrics
    import report

    started = time.perf_counter()
    cfg = config(root)
    cache = Cache(root)
    board = stages.run(root, cfg, cache, args.variant, params_of(args.set), args.full, args.heldout,
                       str(venv_python(root)))
    cache.save()
    board["date"] = datetime.date.today().isoformat()
    board["tree"] = subprocess.run(["git", "rev-parse", "--short=12", "HEAD"], cwd=HERE, capture_output=True,
                                   text=True).stdout.strip()
    for split in board["splits"].values():
        for cell in split.values():
            cell["metrics"] = metrics.summarise(cell["files"].values())
    deltas = None
    if args.baseline:
        path = root / "baselines" / f"{args.baseline}.json"
        if not path.exists():
            raise SystemExit(f"no baseline {args.baseline} (saved: "
                             f"{', '.join(p.stem for p in (root / 'baselines').glob('*.json')) or 'none'})")
        base = json.loads(path.read_text())
        deltas = {"_name": args.baseline}
        for split, engines in board["splits"].items():
            for engine, cell in engines.items():
                then = base["splits"].get(split, {}).get(engine)
                if then:
                    deltas.setdefault(split, {})[engine] = metrics.delta(cell["files"], then["files"])
    board["elapsed_seconds"] = round(time.perf_counter() - started, 1)
    print(report.terminal(board, deltas))
    print("\nstages: " + " | ".join(f"{stage} " + ", ".join(f"{n} {what}" for what, n in sorted(c.items()))
                                    for stage, c in board["stages"].items()))
    missing = sum(c.get("missing", 0) + c.get("stale", 0) for c in board["stages"].values())
    if missing:
        print("some engine or Whisper results are stale or missing: `just diar-bench --full` computes them")
    print(f"elapsed {board['elapsed_seconds']} s")
    runs = root / "runs"
    runs.mkdir(exist_ok=True)
    stamp = datetime.datetime.now().strftime("%Y%m%dT%H%M%S")
    (runs / f"{stamp}.json").write_text(json.dumps(board))
    if args.save:
        (root / "baselines").mkdir(exist_ok=True)
        (root / "baselines" / f"{args.save}.json").write_text(json.dumps(board))
        print(f"saved baseline {args.save}")
    if args.report:
        for path in report.write(board, HERE.parents[2] / "docs/reports/benchmarks"):
            print(f"wrote {path.relative_to(HERE.parents[2])}")


if __name__ == "__main__":
    main()
