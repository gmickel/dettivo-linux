#!/usr/bin/env python3
"""Run the fn-64 side-by-side diarization jobs: one fresh, niced process per job.

A job file (JSON list) names each job: {"id", "wav", "system", "speakers"?}. Systems
come from --systems (JSON object: name -> {"argv": [...], "env": {...}, "gpu": bool}),
where "{wav}" in argv is replaced by the job's WAV. Each job's stdout lands in
<out>/<id>.<system>[.k<n>].json and one record per job (wall time, CPU time, peak RSS,
exit code, whether a meeting was recording or finalising) is appended to <out>/runs.jsonl. Jobs
already recorded with exit 0 are skipped, so the runner resumes after an interruption.
GPU jobs wait while a Dettivo meeting records or finalises, so they never contend with it.
"""
import argparse
import json
import os
import sqlite3
import subprocess
import time
from pathlib import Path

DB = Path.home() / ".local/share/dettivo/dettivo.db"


BUSY = """select count(*) from meetings where status in ('recording', 'stopping', 'stopped', 'transcribing')
    or json_extract(diarization, '$.status') in ('queued', 'running') or analysis_status in ('queued', 'running')"""


def busy():
    """A meeting is recording or finalising (transcript, speaker pass, analysis)."""
    try:
        with sqlite3.connect(f"file:{DB}?mode=ro", uri=True, timeout=5) as db:
            return db.execute(BUSY).fetchone()[0] > 0
    except sqlite3.Error:
        return True  # unknown counts as busy


def done(runs):
    seen = set()
    if runs.exists():
        for line in runs.read_text().splitlines():
            r = json.loads(line)
            if r["exit"] == 0:
                seen.add(r["key"])
    return seen


def run(job, system, out):
    key = f"{job['id']}.{job['system']}" + (f".k{job['speakers']}" if job.get("speakers") else "")
    argv = [a.replace("{wav}", job["wav"]) for a in system["argv"]]
    if job.get("speakers"):
        argv += ["--speakers", str(job["speakers"])]
    env = dict(os.environ, **system.get("env", {}))
    busy_at_start = busy()
    started = time.perf_counter()
    with open(out / f"{key}.json", "wb") as stdout, open(out / f"{key}.stderr", "wb") as stderr:
        proc = subprocess.Popen(["nice", "-n", "19", "ionice", "-c3", *argv], stdout=stdout, stderr=stderr, env=env)
        _, status, usage = os.wait4(proc.pid, 0)
    wall = time.perf_counter() - started
    return key, {
        "key": key, "id": job["id"], "system": job["system"], "speakers": job.get("speakers"),
        "exit": os.waitstatus_to_exitcode(status), "wall_seconds": wall,
        "cpu_seconds": usage.ru_utime + usage.ru_stime, "max_rss_kib": usage.ru_maxrss,
        "meeting_busy_at_start": busy_at_start, "meeting_busy_at_end": busy(),
        "finished_unix": time.time(),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--jobs", type=Path, required=True)
    parser.add_argument("--systems", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--only", help="comma-separated systems to run")
    args = parser.parse_args()
    systems = json.loads(args.systems.read_text())
    jobs = json.loads(args.jobs.read_text())
    only = set(args.only.split(",")) if args.only else None
    args.out.mkdir(parents=True, exist_ok=True)
    runs = args.out / "runs.jsonl"
    for job in jobs:
        if only and job["system"] not in only:
            continue
        system = systems[job["system"]]
        key = f"{job['id']}.{job['system']}" + (f".k{job['speakers']}" if job.get("speakers") else "")
        if key in done(runs):
            continue
        while system.get("gpu") and busy():
            time.sleep(60)
        key, record = run(job, system, args.out)
        with runs.open("a") as f:
            f.write(json.dumps(record) + "\n")
        print(f"{key} exit={record['exit']} wall={record['wall_seconds']:.1f}s", flush=True)


if __name__ == "__main__":
    main()
