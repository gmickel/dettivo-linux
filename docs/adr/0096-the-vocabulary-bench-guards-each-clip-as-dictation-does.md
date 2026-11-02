# 0096. The vocabulary bench guards each clip's text as dictation does

Status: Accepted 2026-11-02. Amends 0087.

## What this gives you

`dettivo-qa bench vocabulary` scores the text a dictation would paste, so a figure in its report is one a user gets: the clip is padded as a take is padded, the engine gets the vocabulary as dictation sends it, and the engine's answer passes the same output guards before it is scored.

## Situation

fn-84 found that the bench overstated Parakeet's recall because it fed each clip unpadded, while dictation pads a take to at least 1.5 s and adds 400 ms of silence for Parakeet. The bench now pads with the session's own function. Two more differences remained. The session sends Whisper the vocabulary's terms beside the prompt, so the engine can check its output (ADR 0095), and the bench sent the prompt alone. After the engine answers, the session drops decoder loops, known silence hallucinations and a whole-prompt echo (ADR 0081), and the bench scored the engine's raw text.

The guards lived in `dettivo-session`, which `dettivo-qa` may not depend on: the session is daemon-side and the QA runner reaches the daemon only through the contract types (`tools/xtask/src/edges.rs`). `dettivo-transcribe`, which the QA runner already uses for `pad_for_engine`, holds the meeting filter the guards are built on.

## Decision

- The output guards move from `dettivo-session` to `dettivo-transcribe` as `guards::clean`, unchanged. The session calls it on every take, and the bench calls it on every clip. One function means the two cannot drift apart.
- The bench parses each engine answer as a `recognize` result and scores `guards::clean` of it. With the vocabulary on, Whisper's clips are guarded with the set's terms as the prompt terms, as the session guards a take from an engine primed with a prompt. Parakeet takes no prompt, so its clips are guarded with none, as in dictation.
- With the vocabulary on, Whisper gets `--prompt` and `--vocabulary` with the set's terms, as the session sends them.
- Whisper's on row reports what the check cost per clip from the engine's `vocabulary check` log line, in the same `candidates` and `rescore_ms` fields Parakeet's rescoring fills. The Markdown column is now "Time added".

## Consequences

- Reports filed before 2026-11-02 scored unguarded text. None of the 94 Whisper large-v3-turbo transcripts of the 2026-11-02 baseline (vocabulary off and on, before the term check) is a decoder loop, a known hallucination or a whole-prompt echo, so the guards would have changed none of them. The echo "Omarchy, Hyprland." is two of five terms, and the whole-prompt guard keeps it. The Parakeet rows were not re-run for this change.
- An engine answer the bench cannot parse as a `recognize` result fails the row with the clip named, where before a missing `text` scored as an empty transcript.
- The bench still runs each clip through the engine's CLI mode, one process and one model load per clip, so a row's wall time is not the time a warm daemon spends. The per-clip time the check added is measured inside the engine and is comparable.
