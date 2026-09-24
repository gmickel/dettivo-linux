# Contributing

Thank you for helping build Dettivo for Linux. This page tells you how to get a green tree, how the repository documents itself, and how to add a decision so your change is understood a year from now.

## Get a green tree

```bash
just build && just test && just lint
```

Those are the recipes CI runs, in a pinned Arch Linux container. If a toolchain piece is missing, `just build` names it and stops. The full list of prerequisites is in [README.md](README.md).

## Decision records are the documentation

The architecture decision records under [docs/adr/](docs/adr/README.md) are the main documentation of this repository: what was decided, why, and what it costs. User-facing pages sit beside them. A change that alters a decision comes with a new record that supersedes the old one, and the old one gets a pointer forward; nothing is edited into silence.

Every document leads with what it does for the reader. The first section of a record is "What this gives you", and a user-facing page opens with the value the reader gets, then the mechanism. Name mechanisms, paths and numbers; leave out adjectives and lists of caveats. `just docs` checks that every record is indexed and opens with that section, that links resolve, that every page under `docs/` is reachable from `README.md`, that every configuration key is on [docs/config.md](docs/config.md) and that the command tree in the [agent guide](docs/guides/agents.md) equals `dettivo docs cli-tree` ([ADR 0041](docs/adr/0041-guides-docs-build-evidence-map-and-the-release-gate-script.md)).

A behaviour change lands with its page in the same commit: the topic page under `docs/` that describes the behaviour, `docs/config.md` for a key, the delta register for a contract change, and the agent guide's tree (paste `dettivo docs cli-tree` between its markers) for a verb. The four guides under [docs/guides/](docs/guides/user.md) link the topic pages rather than repeating them, so a fact has one home. A new requirement gets its row in `qa/evidence-map.toml` naming the test, drive, pack step or fixture that proves it; `dettivo-qa evidence-map` and `cargo test -p dettivo-qa` hold the map to the tree ([docs/guides/qa.md](docs/guides/qa.md#the-evidence-map)).

## Add a decision record

1. Copy [docs/adr/template.md](docs/adr/template.md) to `docs/adr/NNNN-short-title.md`, with `NNNN` the next number in the index.
2. Fill the four sections: What this gives you, Situation, Decision, Consequences. Set `Status: Proposed` until the decision is taken, then `Accepted YYYY-MM-DD`.
3. Add a row to the table in [docs/adr/README.md](docs/adr/README.md). Run `just docs`.
4. Commit the record with the change it explains, or ahead of it when the decision comes first.

## Code conventions

- Conventional Commits (`feat`, `fix`, `refactor`, `build`, `ci`, `chore`, `docs`, `style`, `perf`, `test`).
- Rust and C++ files stay under 500 lines, QML under 300. Generated files go in `.file-length-allow`. `just lint` enforces both limits.
- Crate dependency edges are declared in `tools/xtask/src/edges.rs`, every workspace member counted (`parakeet-cpp-sys` and `sherpa-onnx-sys` included). Client crates (`dettivo-cli`, `dettivo-mcp`, `dettivo-qa`) depend on `dettivo-proto` only; engine binaries depend on `dettivo-engine-proto` and, for Parakeet and diarization, their own native binding only. `just lint` fails on an edge outside the table.
- No telemetry, no network calls the user did not configure, no web views.
- Every interactive QML control carries an accessible name; the QA drives address controls by it.

## Way of working during the build

Specs are captured without plans or reviews and worked directly ([ADR 0012](docs/adr/0012-fast-build-then-cleanup.md)). Cheap gates run on every push; the expensive ones (plan and implementation review, the QA pipeline stage) stay off until the cleanup phase after v1.

## Security

See [SECURITY.md](SECURITY.md) for how to report a vulnerability.
