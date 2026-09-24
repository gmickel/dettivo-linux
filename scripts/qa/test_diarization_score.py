"""Strict DER regressions; synthetic inputs contain no transcript data."""
import itertools
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from diarization_score import maximum_assignment, score


class ScoreTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.rttm = Path(self.directory.name) / "reference.rttm"
        self.uem = Path(self.directory.name) / "reference.uem"
        self.rttm.write_text("SPEAKER fixture 1 0 1 <NA> <NA> A <NA> <NA>\n")
        self.uem.write_text("fixture 1 0 2\n")

    def evaluate(self, reference, hypothesis, regions=((0, 2),)):
        self.rttm.write_text("".join(
            f"SPEAKER fixture 1 {a} {b-a} <NA> <NA> {s} <NA> <NA>\n"
            for a, b, s in reference))
        self.uem.write_text("".join(f"fixture 1 {a} {b}\n" for a, b in regions))
        return score("fixture", [{"start_ms": a * 1000, "end_ms": b * 1000,
                                  "speaker": s} for a, b, s in hypothesis],
                     self.rttm, self.uem)

    def test_components_and_overlap(self):
        cases = [
            ([(0, 1, "A"), (1, 2, "B")], [(0, 1, "Y"), (1, 2, "X")], (0, 0, 0)),
            ([(0, 2, "A"), (1, 2, "B")], [(0, 2, "X")], (1/3, 0, 0)),
            ([(0, 1, "A")], [(0, 2, "X")], (0, 1, 0)),
            ([(0, 1, "A"), (1, 2, "B")], [(0, 2, "X")], (0, 0, .5)),
            ([(0, 1, "A")], [], (1, 0, 0)),
            ([(0, 1, "A"), (.5, 1.5, "A")], [(0, 1.5, "X")], (0, 0, 0)),
            ([(0, 1, "A"), (.5, 1.5, "B")],
             [(0, 1, "X"), (.5, 1.5, "Y")], (0, 0, 0)),
        ]
        for reference, hypothesis, expected in cases:
            with self.subTest(reference=reference, hypothesis=hypothesis):
                result = self.evaluate(reference, hypothesis)
                for key, value in zip(("missed", "false_alarm", "confusion"), expected):
                    self.assertAlmostEqual(result[key], value)
                self.assertAlmostEqual(result["der"], sum(expected))

    def test_uem_union_and_gaps(self):
        result = self.evaluate([(0, 1, "A"), (2, 3, "B")],
                               [(0, 1, "X"), (1, 2, "Z"), (2, 3, "Y")],
                               [(0, .75), (.5, 1), (2, 3)])
        self.assertEqual(result["der"], 0)
        self.assertEqual(result["reference_speaker_seconds"], 2)
        self.assertEqual(result["hypothesis_speakers"], 2)

    def test_microseconds_and_identity(self):
        result = self.evaluate([(.000123, .876543, "A")], [(.000123, .876543, "X")])
        self.assertEqual(result["der"], 0)
        self.assertAlmostEqual(result["reference_speaker_seconds"], .876420)
        self.assertEqual(len(result["rttm_sha256"]), 64)
        self.assertEqual(len(result["uem_sha256"]), 64)
        self.assertEqual(result["collar_seconds"], 0)
        self.assertTrue(result["overlap_included"])

    def test_invalid_reference_files(self):
        original_rttm, original_uem = self.rttm.read_text(), self.uem.read_text()
        cases = [
            ("rttm", "garbage", "RTTM"),
            ("rttm", "SPEAKER fixture 1 0 -1 <NA> <NA> A <NA> <NA>", "duration"),
            ("rttm", "SPEAKER fixture 1 NaN 1 <NA> <NA> A <NA> <NA>", "finite"),
            ("rttm", "SPEAKER other 1 0 1 <NA> <NA> A <NA> <NA>", "recording"),
            ("rttm", "SPEAKER fixture 1 0 1 <NA> <NA> <NA> <NA> <NA>", "speaker"),
            ("uem", "fixture 1 0", "UEM"),
            ("uem", "fixture 1 2 1", "interval"),
            ("uem", "fixture 1 -1 2", "negative"),
            ("uem", "fixture 1 0 inf", "finite"),
            ("uem", "other 1 0 2", "recording"),
            ("uem", "fixture 2 0 2", "channel"),
            ("uem", "fixture 1 2 3", "reference speech"),
        ]
        for target, content, message in cases:
            with self.subTest(content=content):
                self.rttm.write_text(original_rttm)
                self.uem.write_text(original_uem)
                getattr(self, target).write_text(content)
                with self.assertRaisesRegex(ValueError, message):
                    score("fixture", [], self.rttm, self.uem)

    def test_invalid_hypotheses(self):
        for turns in ({}, [None], [{}], [{"start_ms": 1, "end_ms": 0, "speaker": 0}],
                      [{"start_ms": 0, "end_ms": 0, "speaker": 0}],
                      [{"start_ms": -1, "end_ms": 2, "speaker": 0}],
                      [{"start_ms": True, "end_ms": 2, "speaker": 0}],
                      [{"start_ms": 0, "end_ms": float("nan"), "speaker": 0}],
                      [{"start_ms": 0, "end_ms": 2, "speaker": []}]):
            with self.subTest(turns=turns), self.assertRaises(ValueError):
                score("fixture", turns, self.rttm, self.uem)

    def test_assignment_matches_exhaustive_oracle(self):
        for size in (1, 2, 3):
            for entries in itertools.product((0, 1), repeat=size * size):
                matrix = [entries[i * size:(i + 1) * size] for i in range(size)]
                expected = max(sum(matrix[i][p[i]] for i in range(size))
                               for p in itertools.permutations(range(size)))
                self.assertEqual(maximum_assignment(matrix), expected)
        self.assertEqual(maximum_assignment([[1, 4, 2], [3, 2, 1]]), 7)
        self.assertEqual(maximum_assignment([[1], [4], [2]]), 4)

    def test_cli(self):
        hypothesis = Path(self.directory.name) / "hypothesis.json"
        command = [sys.executable, str(Path(__file__).with_name("diarization_score.py")),
                   str(hypothesis), "--rttm", str(self.rttm), "--uem", str(self.uem),
                   "--recording", "fixture"]
        for payload, expected in (({"turns": []}, 0), ({"turns": "bad"}, 2), ({}, 2)):
            hypothesis.write_text(json.dumps(payload))
            result = subprocess.run(command, capture_output=True, text=True)
            self.assertEqual(result.returncode, expected, result.stderr)
            if expected == 0:
                self.assertEqual(json.loads(result.stdout)["der"], 1)
            else:
                self.assertEqual(result.stdout, "")


if __name__ == "__main__":
    unittest.main()
