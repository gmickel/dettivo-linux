"""The assignment stage: the product's speaker rule, in Rust, over cached turns and segments.

`crates/dettivo-meeting/examples/diar_assign.rs` wraps `dettivo_meeting::diarize::assign`.
Cargo rebuilds it when the crate changes, and the built binary's hash is part of every
assignment key, so editing the rule reruns assignment and scoring and nothing upstream.
All uncached jobs of a run go to one process.
"""
import json
import os
import shutil
import subprocess

from cache import key
from data import REPO


def build():
    """Builds the assigner (debug profile, like `just build`); returns its path."""
    proc = subprocess.run(["cargo", "build", "-q", "-p", "dettivo-meeting", "--example", "diar_assign",
                           "--message-format=json-render-diagnostics"],
                          cwd=REPO, capture_output=True, text=True)
    if proc.returncode:
        raise SystemExit(f"building the assigner failed:\n{proc.stderr.strip()}")
    for line in proc.stdout.splitlines():
        msg = json.loads(line)
        if msg.get("reason") == "compiler-artifact" and msg["target"]["name"] == "diar_assign":
            return msg["executable"]
    raise SystemExit("cargo reported no diar_assign executable")


class Assigner:
    def __init__(self, cache, variant, params):
        self.cache, self.variant, self.params = cache, variant, params
        built = build()
        self.program = cache.file_hash(built)
        # Run a private copy named by its hash: a rebuild during a long `--full` run must
        # not run a different rule under this run's keys.
        self.binary = cache.path("assigner", self.program, "")
        if not self.binary.exists():
            tmp = self.binary.with_suffix(".tmp")
            shutil.copy2(built, tmp)
            os.replace(tmp, self.binary)
        self.pending = {}

    def job_key(self, segments_key, engine_key, room_audio, track_ms, window):
        return key(self.program, self.variant, self.params, segments_key, engine_key, room_audio, track_ms, window)

    def request(self, k, job):
        """Queues a job unless its lines are cached; returns the cached lines or None."""
        hit = self.cache.get("assign", k)
        if hit is not None:
            self.cache.note("assign", "cached")
            return hit["lines"]
        self.pending[k] = job
        return None

    def run(self):
        """Runs every queued job in one process; returns {key: lines}."""
        if not self.pending:
            return {}
        jobs = [dict(job, id=k) for k, job in self.pending.items()]
        body = json.dumps({"variant": self.variant, "params": self.params, "jobs": jobs})
        proc = subprocess.run([self.binary], input=body, capture_output=True, text=True,
                              env=dict(os.environ, RUST_LOG="warn"))
        if proc.returncode:
            raise SystemExit(f"assigner: {proc.stderr.strip()}")
        out = {}
        for result in json.loads(proc.stdout)["results"]:
            out[result["id"]] = result["lines"]
            self.cache.put("assign", result["id"], {"lines": result["lines"]})
            self.cache.note("assign", "computed")
        self.pending = {}
        return out
