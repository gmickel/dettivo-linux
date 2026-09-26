"""The bench's content-keyed cache and the ledger of what each run reused or recomputed.

Every stage result lives at <eval>/cache/<stage>/<key>.json, where the key hashes
everything the result depends on (audio hash, engine identity, parameters, the keys of
the stages it consumed). A changed input is a new key, so nothing is invalidated by hand.
File hashes are remembered by (path, size, mtime, inode), so a run hashes the multi-
gigabyte audio only when it changes.
"""
from collections import Counter, defaultdict
import hashlib
import json
import os
from pathlib import Path


def key(*parts):
    return hashlib.sha256(json.dumps(parts, sort_keys=True, default=str).encode()).hexdigest()[:40]


class Cache:
    def __init__(self, root):
        self.root = Path(root) / "cache"
        self.root.mkdir(parents=True, exist_ok=True)
        self.ledger = defaultdict(Counter)
        self._hashes_path = self.root / "file-hashes.json"
        try:
            self._hashes = json.loads(self._hashes_path.read_text())
        except (OSError, ValueError):
            self._hashes = {}
        self._hashes_dirty = False

    def path(self, stage, k, suffix=".json"):
        d = self.root / stage
        d.mkdir(exist_ok=True)
        return d / f"{k}{suffix}"

    def get(self, stage, k):
        p = self.path(stage, k)
        try:
            return json.loads(p.read_text())
        except (OSError, ValueError):
            return None

    def put(self, stage, k, value, index=None):
        """Stores `value`; `index` names a slot ("<audio>:<engine name>") whose newest key a
        cache-only run falls back to when the exact key is missing."""
        p = self.path(stage, k)
        tmp = p.with_suffix(".tmp")
        tmp.write_text(json.dumps(value))
        os.replace(tmp, p)
        if index:
            slots = self.get("index", stage) or {}
            slots.setdefault(index, [])
            if k in slots[index]:
                slots[index].remove(k)
            slots[index].append(k)
            self.put("index", stage, slots)

    def newest(self, stage, index):
        slots = self.get("index", stage) or {}
        for k in reversed(slots.get(index, [])):
            value = self.get(stage, k)
            if value is not None:
                return k, value
        return None, None

    def note(self, stage, outcome):
        """Records one stage outcome: cached, computed, stale or missing."""
        self.ledger[stage][outcome] += 1

    def file_hash(self, path):
        path = Path(path)
        real = path.resolve()
        st = real.stat()
        stamp = [st.st_size, st.st_mtime_ns, st.st_ino]
        entry = self._hashes.get(str(real))
        if entry and entry["stamp"] == stamp:
            self.note("hash", "cached")
            return entry["sha256"]
        self.note("hash", "computed")
        h = hashlib.sha256()
        with open(real, "rb") as f:
            for block in iter(lambda: f.read(1 << 22), b""):
                h.update(block)
        self._hashes[str(real)] = {"stamp": stamp, "sha256": h.hexdigest()}
        self._hashes_dirty = True
        return h.hexdigest()

    def tree_hash(self, path):
        """A model directory (or file with its sidecar data) as one hash of its files."""
        path = Path(path)
        files = sorted(p for p in path.rglob("*") if p.is_file()) if path.is_dir() else \
            [path, *sorted(path.parent.glob(f"{path.name}_data"))]
        return key([(p.name, self.file_hash(p)) for p in files])

    def save(self):
        if self._hashes_dirty:
            tmp = self._hashes_path.with_suffix(".tmp")
            tmp.write_text(json.dumps(self._hashes))
            os.replace(tmp, self._hashes_path)
            self._hashes_dirty = False
