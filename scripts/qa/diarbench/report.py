"""The bench's outputs: the terminal table, and the aggregate-only report for git.

A scoreboard holds per-file counts keyed by recording (AMI names, meeting aliases), and
those stay in the protected eval directory. `aggregate` keeps only what a split pools:
metric values with their intervals, file counts, audio minutes and engine identities.
It is the only thing `write` puts under docs/reports/benchmarks/, and a test holds it to
that (scripts/qa/test_diarbench.py).
"""
import json
from pathlib import Path

from data import HELDOUT
from metrics import BY_NAME, METRICS

ALLOWED_SPLIT_KEYS = {"files", "audio_minutes", "metrics"}


def aggregate(board):
    """The report's content: pooled numbers and identities only, no per-file entry."""
    splits = {}
    for split, engines in board["splits"].items():
        splits[split] = {name: {"files": len(cell["files"]), "audio_minutes": cell["audio_minutes"],
                                "metrics": cell["metrics"]} for name, cell in engines.items()}
    return {"schema": 1, "date": board["date"], "tree": board["tree"], "variant": board["variant"],
            "params": board["params"], "heldout_included": board["heldout"],
            "engines": {n: {"label": e["label"], "identity": e["identity"]} for n, e in board["engines"].items()},
            "splits": splits}


def fmt(entry, percent=True):
    if entry is None:
        return "-"
    unit = "%" if percent else "x"
    text = f"{entry['value']:.2f}{unit}"
    if entry.get("lo") is not None:
        text += f" [{entry['lo']:.2f}, {entry['hi']:.2f}]"
    return text


def fmt_delta(entry, percent=True):
    if entry is None:
        return ""
    text = f"{entry['value']:+.2f}"
    if entry.get("lo") is not None:
        text += f" [{entry['lo']:+.2f}, {entry['hi']:+.2f}]"
    return text


def rows(board, split, deltas=None):
    engines = list(board["splits"][split])
    for m in METRICS:
        cells = [board["splits"][split][e]["metrics"].get(m.name) for e in engines]
        if not any(cells):
            continue
        row = [m.label]
        for e, cell in zip(engines, cells):
            row.append(fmt(cell, m.percent))
            if deltas is not None:
                row.append(fmt_delta((deltas.get(split, {}).get(e) or {}).get(m.name), m.percent))
        yield row


def terminal(board, deltas=None):
    lines = []
    for split, engines in board["splits"].items():
        first = next(iter(engines.values()))
        tag = " (held out)" if split in HELDOUT else ""
        lines.append(f"\n{split}{tag}: {len(first['files'])} files, {first['audio_minutes']:.0f} min")
        header = ["metric"]
        for e in engines:
            header.append(board["engines"][e]["label"])
            if deltas is not None:
                header.append(f"delta vs {deltas['_name']}")
        table = [header, *rows(board, split, deltas)]
        widths = [max(len(r[i]) for r in table) for i in range(len(header))]
        for r in table:
            lines.append("  ".join(c.ljust(w) for c, w in zip(r, widths)).rstrip())
    return "\n".join(lines)


def markdown(report):
    out = [f"# Diarization bench, {report['date']}", "",
           f"Tree `{report['tree']}`, assignment variant `{report['variant']}`"
           + (f" with `{json.dumps(report['params'])}`" if report["params"] else "")
           + ". Values are pooled over each split's files; brackets are 95% bootstrap intervals over files.", ""]
    for split, engines in report["splits"].items():
        names = list(engines)
        first = engines[names[0]]
        out += [f"## {split}{' (held out)' if split in HELDOUT else ''}", "",
                f"{first['files']} files, {first['audio_minutes']:.0f} minutes.", "",
                "| Metric | " + " | ".join(report["engines"][n]["label"] for n in names) + " |",
                "|---|" + "---:|" * len(names)]
        for m in METRICS:
            cells = [engines[n]["metrics"].get(m.name) for n in names]
            if any(cells):
                out.append(f"| {m.label.strip()} | " + " | ".join(fmt(c, m.percent) for c in cells) + " |")
        out.append("")
    out += ["Metric definitions: [docs/diarization-bench.md](../../diarization-bench.md). "
            "Engine identities are in the JSON beside this page.", ""]
    return "\n".join(out)


def write(board, out_dir):
    """Writes diarization-bench-<date>.{md,json}; returns the two paths."""
    report = aggregate(board)
    for cells in report["splits"].values():
        for cell in cells.values():
            assert set(cell) == ALLOWED_SPLIT_KEYS and set(cell["metrics"]) <= set(BY_NAME)
    out_dir = Path(out_dir)
    stem = out_dir / f"diarization-bench-{board['date']}"
    stem.with_suffix(".json").write_text(json.dumps(report, indent=1) + "\n")
    stem.with_suffix(".md").write_text(markdown(report))
    return stem.with_suffix(".md"), stem.with_suffix(".json")
