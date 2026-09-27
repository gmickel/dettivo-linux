"""fn-72's labelling kit: blended drafts, excerpt picking, the labels file the bench reads,
and a server that only localhost can reach. Synthetic inputs only."""
import http.client
import json
from pathlib import Path
import re
import sys
import tempfile
import threading
import unittest
import wave

sys.path.insert(0, str(Path(__file__).resolve().parent / "diarbench"))
import data  # noqa: E402
import label  # noqa: E402
import label_server  # noqa: E402


def seg(start, end, source="system", text="one two three"):
    return {"start_ms": start, "end_ms": end, "source_type": source, "text": text}


def turn(start, end, speaker):
    return {"start_ms": start, "end_ms": end, "speaker": speaker}


# Engine b names the same two people differently and hears the third line as the first person.
SEGMENTS = [seg(0, 1000), seg(1000, 2000), seg(2000, 3000, "microphone"), seg(3000, 4000)]
OUTPUTS = {"a": [turn(0, 1000, "0"), turn(1000, 2000, "1"), turn(2000, 3000, "1"), turn(3000, 4000, "0")],
           "b": [turn(0, 1000, "y"), turn(1000, 2000, "x"), turn(2000, 3000, "y"), turn(3000, 4000, "y")]}


class DraftTests(unittest.TestCase):
    def test_a_draft_never_comes_from_one_system(self):
        with self.assertRaises(ValueError):
            label.blend(label.meeting_lines(SEGMENTS), {"a": OUTPUTS["a"]})
        with self.assertRaisesRegex(ValueError, "--draft blank"):
            label.session("EN-1", "en", SEGMENTS, {"a": OUTPUTS["a"]}, "blend", 15)
        blank = label.session("EN-1", "en", SEGMENTS, {"a": OUTPUTS["a"]}, "blank", 15)
        self.assertEqual(blank["draft"], "blank")
        self.assertEqual({x["speaker"] for x in blank["lines"]}, {None})

    def test_the_blend_maps_engines_onto_one_letter_space_and_flags_disagreement(self):
        drafts = label.blend(label.meeting_lines(SEGMENTS), OUTPUTS)
        self.assertEqual([d["status"] for d in drafts], ["agree", "agree", "disagree", "agree"])
        self.assertEqual([d["speaker"] for d in drafts], ["A", "B", None, "A"])
        self.assertEqual(drafts[2]["candidates"], ["A", "B"])
        state = label.session("EN-1", "en", SEGMENTS, OUTPUTS, "blend", 15)
        self.assertEqual(state["draft"], "blend:a+b")
        self.assertIsNone(state["window_ms"])

    def test_the_excerpt_is_the_window_with_the_most_speaker_changes(self):
        minute = 60_000
        lines = [{"start_ms": i * minute, "end_ms": i * minute + 1000} for i in range(40)]
        guesses = ["A"] * 30 + ["A", "B"] * 3 + ["A"] * 4  # six changes, at minutes 31 to 36
        self.assertEqual(label.pick_window(lines, guesses, 10), [27 * minute, 37 * minute])
        self.assertIsNone(label.pick_window(lines[:5], guesses[:5], 10))


class LabelsFileTests(unittest.TestCase):
    def test_a_finished_session_is_a_labels_file_the_bench_reads(self):
        state = label.session("DE-2", "de", SEGMENTS, OUTPUTS, "blend", 15)
        with self.assertRaises(ValueError):
            label.apply(state, {"speakers": ["A"], "names": {}, "lines": []})
        label.apply(state, {"speakers": ["A", "B"], "names": {"A": " Gordon "},
                            "lines": [{"speaker": s, "confirmed": True} for s in ("A", "B", None, "A")]})
        self.assertTrue(label.complete(state))
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "labels" / "DE-2.json"
            label.write_private(path, label.labels(state))
            self.assertEqual(path.stat().st_mode & 0o777, 0o600)
            units = data.reference_units({"kind": "labels", "path": path})
            self.assertEqual(json.loads(path.read_text())["draft"], "blend:a+b")
        self.assertEqual([(u["speaker"], u["source"], u["words"]) for u in units],
                         [("Gordon", "system", 3), ("B", "system", 3), ("Gordon", "system", 3)])

    def test_starting_over_drops_the_earlier_labels(self):
        state = label.session("DE-2", "de", SEGMENTS, OUTPUTS, "blend", 15)
        with tempfile.TemporaryDirectory() as tmp:
            session_path, labels_path = Path(tmp) / "labelling" / "DE-2.json", Path(tmp) / "labels" / "DE-2.json"
            label.write_private(labels_path, {"schema": 1})
            label.start(session_path, labels_path, state)
            self.assertTrue(session_path.exists())
            self.assertFalse(labels_path.exists())


class ServerTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        root = Path(self.tmp.name)
        with wave.open(str(root / "system.wav"), "wb") as w:
            w.setnchannels(1)
            w.setsampwidth(2)
            w.setframerate(16000)
            w.writeframes(b"\0\0" * 16000 * 5)
        state = label.session("EN-1", "en", SEGMENTS, OUTPUTS, "blend", 15)
        self.app = label_server.Labelling(state, root / "labelling/EN-1.json", root / "labels/EN-1.json", root)
        self.server = label_server.make_server(self.app)
        self.port = self.server.server_address[1]
        threading.Thread(target=self.server.serve_forever, daemon=True).start()

    def tearDown(self):
        self.server.shutdown()
        self.server.server_close()
        self.tmp.cleanup()

    def ask(self, method, path, body=None, host=None):
        conn = http.client.HTTPConnection("127.0.0.1", self.port, timeout=5)
        conn.putrequest(method, path, skip_host=True)
        conn.putheader("Host", host or f"127.0.0.1:{self.port}")
        payload = json.dumps(body).encode() if body is not None else b""
        conn.putheader("Content-Length", str(len(payload)))
        conn.endheaders(payload)
        r = conn.getresponse()
        out = (r.status, r.getheader("Content-Security-Policy"), r.read())
        conn.close()
        return out

    def test_the_server_binds_to_localhost_only(self):
        self.assertEqual(label_server.HOST, "127.0.0.1")
        self.assertEqual(self.server.server_address[0], "127.0.0.1")

    def test_only_the_token_and_a_local_host_name_reach_the_page(self):
        base = f"/{self.app.token}/"
        status, csp, _ = self.ask("GET", base)
        self.assertEqual(status, 200)
        self.assertIn("default-src 'none'", csp)
        self.assertEqual(self.ask("GET", "/")[0], 404)
        self.assertEqual(self.ask("GET", base, host="attacker.example:80")[0], 404)
        status, _, clip = self.ask("GET", base + "audio/1")
        self.assertEqual(status, 200)
        self.assertEqual(len(clip) - 44, 2 * 16 * (1000 + 2 * label_server.PAD_MS))
        lines = [{"speaker": "A", "confirmed": True}] * 4
        self.assertEqual(self.ask("POST", base + "save", {"speakers": ["A"], "names": {}, "lines": lines[:3]})[0], 400)
        status, _, body = self.ask("POST", base + "save", {"speakers": ["A", "B"], "names": {}, "lines": lines})
        self.assertEqual((status, json.loads(body)), (200, {"complete": True}))
        self.assertTrue(self.app.labels_path.exists())
        reopened = [{"speaker": "A", "confirmed": False}] + lines[1:]
        status, _, body = self.ask("POST", base + "save", {"speakers": ["A", "B"], "names": {}, "lines": reopened})
        self.assertEqual((status, json.loads(body)), (200, {"complete": False}))
        self.assertFalse(self.app.labels_path.exists())

    def test_the_page_loads_no_external_asset(self):
        page = label_server.PAGE.read_text()
        self.assertNotIn("http", page)
        self.assertIsNone(re.search(r"<[^>]*\b(src|href)\s*=|@import|url\(", page))


if __name__ == "__main__":
    unittest.main()
