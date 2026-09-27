"""The labelling page's server: 127.0.0.1 only, one secret path per run, no external asset.

Routes under /<token>/: the page, `session` (the session JSON), `audio/<line>` (the
line's span of its own track as WAV) and `save` (POST the page's labels). The token and
the Host check keep other local users and DNS-rebinding pages from reading the
confidential transcript, and the Content-Security-Policy lets the page load nothing
from anywhere but this server.
"""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import io
import json
import secrets
import threading
import wave
import webbrowser
from pathlib import Path

import label

HOST = "127.0.0.1"
PAGE = Path(__file__).resolve().parent / "label.html"
PAD_MS = 250
CSP = "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; connect-src 'self'; media-src 'self'"


class Labelling:
    """What one run serves: the session, where it and the labels go, the meeting's audio."""

    def __init__(self, state, session_path, labels_path, meeting_dir):
        self.state, self.session_path, self.labels_path = state, Path(session_path), Path(labels_path)
        self.meeting_dir = Path(meeting_dir)
        self.token = secrets.token_urlsafe(18)
        self.lock = threading.Lock()

    def save(self, update):
        with self.lock:
            label.apply(self.state, update)
            label.write_private(self.session_path, self.state)
            done = label.complete(self.state)
            if done:
                label.write_private(self.labels_path, label.labels(self.state))
            else:
                self.labels_path.unlink(missing_ok=True)  # only a complete session is scored
            return {"complete": done}

    def clip(self, i):
        """The line's span, padded, from its own track (the microphone on one-track meetings)."""
        line = self.state["lines"][i]
        track = self.meeting_dir / "system.wav"
        if line["source"] != "system" or not track.exists():
            track = self.meeting_dir / "microphone.wav"
        with wave.open(str(track)) as w:
            rate = w.getframerate()
            first = max(0, (line["start_ms"] - PAD_MS) * rate // 1000)
            last = min(w.getnframes(), (line["end_ms"] + PAD_MS) * rate // 1000)
            w.setpos(min(first, w.getnframes()))
            frames = w.readframes(max(0, last - first))
            params = w.getparams()
        out = io.BytesIO()
        with wave.open(out, "wb") as w:
            w.setparams(params)
            w.writeframes(frames)
        return out.getvalue()


def handler(app):
    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *args):
            pass

        def route(self):
            """The path below the token, or None (answered 404) for a wrong token or Host."""
            port = self.server.server_address[1]
            if self.headers.get("Host") not in {f"{HOST}:{port}", f"localhost:{port}"}:
                return None
            prefix = f"/{app.token}/"
            return self.path[len(prefix):] if self.path.startswith(prefix) else None

        def send(self, code, body, kind):
            self.send_response(code)
            self.send_header("Content-Type", kind)
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Cache-Control", "no-store")
            self.send_header("Content-Security-Policy", CSP)
            self.end_headers()
            self.wfile.write(body)

        def fail(self, code, message):
            self.send(code, json.dumps({"error": message}).encode(), "application/json")

        def do_GET(self):
            path = self.route()
            if path == "":
                self.send(200, PAGE.read_bytes(), "text/html; charset=utf-8")
            elif path == "session":
                self.send(200, json.dumps(app.state).encode(), "application/json")
            elif path is not None and path.startswith("audio/") and path[6:].isdigit() \
                    and int(path[6:]) < len(app.state["lines"]):
                self.send(200, app.clip(int(path[6:])), "audio/wav")
            else:
                self.fail(404, "not found")

        def do_POST(self):
            if self.route() != "save":
                return self.fail(404, "not found")
            try:
                length = int(self.headers.get("Content-Length", "0"))
                answer = app.save(json.loads(self.rfile.read(length)))
            except ValueError as error:
                return self.fail(400, str(error))
            self.send(200, json.dumps(answer).encode(), "application/json")

    return Handler


def make_server(app, port=0):
    """The page's server, bound to 127.0.0.1 and nothing else."""
    return ThreadingHTTPServer((HOST, port), handler(app))


def run(state, session_path, labels_path, meeting_dir, port=0):
    app = Labelling(state, session_path, labels_path, meeting_dir)
    server = make_server(app, port)
    url = f"http://{HOST}:{server.server_address[1]}/{app.token}/"
    print(f"labelling {state['alias']} at {url}\nprogress saves to {session_path}; Ctrl-C stops the server", flush=True)
    webbrowser.open(url)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
    if label.complete(app.state):
        print(f"labels written to {labels_path}; `just diar-bench` scores them")
