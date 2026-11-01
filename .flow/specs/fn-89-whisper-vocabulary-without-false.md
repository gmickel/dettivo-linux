# Whisper vocabulary without false insertions

## Goal & Context

A vocabulary should help Whisper spell the words a user listed, never make Whisper say them. Today it does both: on the term-recall benchmark, Whisper large-v3-turbo with the vocabulary on gets 10 of 15 targets right but writes listed terms into 2 to 3 clips where nobody said them ("Omar keeps the spare keys in his desk." became "Omarchy, Hyprland."). Parakeet now reaches 13 of 15 with zero false insertions because Dettivo decides each replacement from the audio (fn-84). Whisper takes the vocabulary as a decoder prompt, so the model decides, and nothing checks it afterwards. fn-79's prompt-echo guard drops a segment only when it is the whole prompt, so a partial echo is pasted.

This spec makes a listed term in Whisper's output earn its place, so both engines hold the same bar: listed terms come out when they were said, and never when they were not.

Origin: Gordon, 2026-11-02, goal fn-88 ("a" - add it to this release), under his standing bar of best-in-class performance.

## Approach

Two checks after Whisper decodes, before the text pipeline. Both act only on spans that match a vocabulary term or alias, so text without listed terms is untouched.

- **Echo shape.** A segment, or a trailing run inside one, that consists only of listed terms with separators and punctuation ("Omarchy, Hyprland.") is an echo unless the same span appears when the take is decoded without the prompt. Drop it.
- **Token confidence.** For each listed term in the output, read the decoder's probabilities for its tokens (whisper.cpp exposes per-token probabilities). Keep the term only when its mean log-probability clears a threshold. Otherwise fall back to what the same span decodes to without the prompt, or drop the span if that decode has nothing there.

The no-prompt decode runs only when a check needs it, that is when the output contains a listed term. Its cost is measured; the target is a median under 150 ms extra on clips with a candidate on the reference GPU, and nothing on clips without one.

The threshold is chosen from the benchmark the same way fn-84 chose Parakeet's boost: the best level that keeps zero false insertions and no guard WER rise, with its margins recorded.

## Acceptance

- **R1:** On the term-recall benchmark (fn-83's set as extended by fn-84, padded as dictation pads), Whisper large-v3-turbo with the vocabulary on writes zero false insertions on the distractor clips, and guard-set WER does not rise over the vocabulary-off run.
- **R2:** Term recall with the vocabulary on stays at least at the current figure (10 of 15) on the same run; any loss is a failure.
- **R3:** A take whose output contains no listed term is byte-identical to today's output and gets no extra decode.
- **R4:** Added latency is reported per clip; the median on clips with a candidate is under 150 ms on the reference GPU.
- **R5:** The threshold and the echo rule are recorded in an ADR with the measurement and margins, and `docs/engines.md` and `docs/dictation.md` describe how the vocabulary reaches Whisper and what guards it. A changelog entry is added.

## Boundaries

- Parakeet's path (fn-84) is not changed.
- No new user-facing setting unless the measurement shows one level cannot serve both recall and safety; any new setting goes in `config.toml` first.
- The benchmark measures the real dictation path, as fn-84 established.

## Quick commands

- `cargo test -p dettivo-session -p dettivo-engine-whisper -p dettivo-qa`
- `dettivo-qa bench vocabulary`
- `just build test lint`
