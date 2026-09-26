"""The diarization bench's metric definitions and its report writer; synthetic inputs only."""
import itertools
import json
from pathlib import Path
import random
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent / "diarbench"))
import metrics  # noqa: E402
import report  # noqa: E402
from diarization_score import assignment_pairs, maximum_assignment  # noqa: E402


def line(start, end, speaker, source=None):
    return {"start_ms": start, "end_ms": end, "speaker": speaker, "source": source}


def unit(start, end, speaker, words=1, source=None):
    return {"start_ms": start, "end_ms": end, "speaker": speaker, "words": words, "source": source}


class MetricTests(unittest.TestCase):
    def test_attribution_maps_labels_and_counts_unlabelled_as_wrong(self):
        lines = [line(0, 1000, "s1"), line(1000, 2000, "s0"), line(2000, 3000, None), line(3000, 4000, "s1")]
        units = [unit(100, 300, "A"), unit(400, 600, "A"), unit(1200, 1400, "B"), unit(2100, 2300, "B"),
                 unit(3100, 3300, "B"), unit(5000, 5200, "A"), unit(500, 500, "A")]
        counts = metrics.attribution(lines, units)
        # s1 -> A (3 words), s0 -> B (1 word); the fourth line's B word is wrong under that map.
        self.assertEqual(counts["words"], 6)
        self.assertEqual(counts["words_right"], 4)
        self.assertEqual(counts["words_wrong"], 1)
        self.assertEqual(counts["words_unlabelled"], 1)
        self.assertEqual(counts["words_uncovered"], 1)
        self.assertAlmostEqual(metrics.BY_NAME["attribution_error"].value(counts), 100 * 2 / 6)

    def test_short_turns_are_runs_of_three_words_or_fewer(self):
        units = [unit(i * 100, i * 100 + 50, s) for i, s in enumerate("AAAABBBAAAAAB")]
        short = metrics.short_units(units)
        self.assertEqual(sorted(short), [4, 5, 6, 12])
        self.assertEqual(metrics.short_units([unit(0, 10, "A", words=4)]), set())

    def test_units_match_lines_of_their_own_source(self):
        lines = [line(0, 5000, "you", "microphone"), line(1000, 2000, "s0", "system")]
        units = [unit(1000, 2000, "remote", source="system"), unit(0, 900, "me", source="microphone")]
        counts = metrics.attribution(lines, units)
        self.assertEqual(counts["words_right"], 2)
        view = metrics.line_view(lines, units)
        self.assertEqual((view["lines_right"], view["lines_wrong"]), (2, 0))

    def test_line_view_and_remote_lines(self):
        turns = [{"start_ms": 0, "end_ms": 1000, "speaker": "A"}, {"start_ms": 1000, "end_ms": 2000, "speaker": "B"}]
        lines = [line(0, 900, "x", "system"), line(1100, 1900, "x", "system"), line(1200, 1300, None, "system"),
                 line(5000, 6000, "y", "microphone")]
        view = metrics.line_view(lines, turns)
        self.assertEqual(dict(view), {"lines_reference": 3, "lines_right": 1, "lines_wrong": 1,
                                      "lines_reference_unlabelled": 1})
        self.assertEqual(metrics.remote_lines(lines), {"remote_lines": 3, "remote_unlabelled": 1})

    def test_pooling_intervals_and_paired_deltas(self):
        files = {f"f{i}": {"words": 100, "words_wrong": i, "words_unlabelled": 0} for i in range(1, 6)}
        summary = metrics.summarise(files.values())
        headline = summary["attribution_error"]
        self.assertAlmostEqual(headline["value"], 3.0)
        self.assertLessEqual(headline["lo"], headline["value"])
        self.assertGreaterEqual(headline["hi"], headline["value"])
        self.assertEqual(metrics.summarise([files["f1"]])["attribution_error"]["lo"], None)
        same = metrics.delta(files, files)["attribution_error"]
        self.assertEqual((same["value"], same["lo"], same["hi"]), (0, 0, 0))
        better = {k: dict(v, words_wrong=v["words_wrong"] - 1) for k, v in files.items()}
        moved = metrics.delta(better, files)["attribution_error"]
        self.assertAlmostEqual(moved["value"], -1.0)
        self.assertAlmostEqual(moved["hi"], -1.0)

    def test_assignment_pairs_reach_the_maximum(self):
        rng = random.Random(7)
        for rows, columns in itertools.product(range(1, 5), repeat=2):
            matrix = [[rng.randrange(10) for _ in range(columns)] for _ in range(rows)]
            pairs = assignment_pairs(matrix)
            self.assertEqual(len({r for r, _ in pairs}), len(pairs))
            self.assertEqual(len({c for _, c in pairs}), len(pairs))
            self.assertEqual(sum(matrix[r][c] for r, c in pairs), maximum_assignment(matrix))


class ReportTests(unittest.TestCase):
    """R7: the report under docs/ carries aggregates only."""

    SECRETS = ["EN-7", "DE-3", "ES2011a", "0000aaaa-1111-4222", "/home/someone/.local/share"]

    def board(self):
        files = {"EN-7": {"words": 10, "words_wrong": 1, "remote_lines": 5, "remote_unlabelled": 1},
                 "DE-3": {"words": 20, "words_wrong": 2, "remote_lines": 4, "remote_unlabelled": 0},
                 "ES2011a": {"words": 5, "words_unlabelled": 1, "audio_s": 60, "engine_wall_s": 2}}
        cell = {"files": files, "audio_minutes": 12.5, "metrics": metrics.summarise(files.values()),
                "dir": "/home/someone/.local/share/dettivo/meetings/0000aaaa-1111-4222"}
        return {"date": "2026-09-26", "tree": "abc123", "variant": "product", "params": {}, "heldout": False,
                "engines": {"current": {"label": "Current engine", "identity": {"program": "0" * 64}}},
                "splits": {"local-en": {"current": cell}}, "stages": {"engine": {"cached": 3}}}

    def test_the_report_holds_no_per_file_entry(self):
        with tempfile.TemporaryDirectory() as out:
            paths = report.write(self.board(), out)
            text = "".join(Path(p).read_text() for p in paths)
            data = json.loads(Path(paths[1]).read_text())
        for secret in self.SECRETS:
            self.assertNotIn(secret, text)
        cell = data["splits"]["local-en"]["current"]
        self.assertEqual(set(cell), report.ALLOWED_SPLIT_KEYS)
        self.assertEqual(cell["files"], 3)
        self.assertLessEqual(set(cell["metrics"]), set(metrics.BY_NAME))
        self.assertEqual(set(data), {"schema", "date", "tree", "variant", "params", "heldout_included",
                                     "engines", "splits"})


if __name__ == "__main__":
    unittest.main()
