# Vocabulary on Parakeet

## Goal & Context

Parakeet is Dettivo's default and fastest dictation engine, and it ignores the vocabulary entirely: the engine accepts the prompt and drops it. Names and jargon are what dictation gets wrong most often, so a user on Parakeet gets no help from the feature that exists to fix that.

This spec biases Parakeet towards the vocabulary using the audio itself, so a term replaces what was heard only when the acoustic evidence supports it, and ordinary speech is left alone.

## Approach

Parakeet is a transducer: an encoder over audio frames, a prediction network (decoder) over emitted tokens, and a joint network that scores a token at a frame. Greedy decoding picks the best token frame by frame. A vocabulary term is a short token sequence that greedy decoding missed. Ask the joint network directly whether the term explains the same frames better than the heard words do.

**Stage 1: post-decode rescoring (ship first; works for TDT models and later for streams).**

1. Decode greedily as today, keeping per word the frame span and the decoder state at its start.
2. Find candidate spans: runs of 1–4 heard words that could be a term. A span qualifies when any of these hold:
   - it shares most letters with the term;
   - it matches a `sounds_like` alias;
   - it is within a small edit distance;
   - it is one consonant sound off (phonetic key), only for terms of 5 or more sounds.
3. For each candidate, re-run the decoder and joint network over the span's frames twice, from the span's starting decoder state: once forced to the heard tokens, once forced to the term's tokens. Compare total log-probabilities.
4. Replace when `log P(term) + boost × pieces(term) ≥ log P(heard)`. The boost is in nats per token piece: off 0, light 2.0, normal 2.5 (default), strong 4.0.
5. Skip:
   - case-only differences;
   - inflections of the term (plural, possessive);
   - spans inside paths, URLs or addresses;
   - terms under 5 letters with no alias.

   Caps: 12 pieces per term and 16 candidate spans per decode, so cost stays bounded.

```cpp
// engine-side (parakeet.cpp patch or our shim): score a forced token path over a frame span
float score_forced_path(const Model& m, const EncoderOut& enc, int frame_lo, int frame_hi,
                        DecoderState state, std::span<const int> tokens);  // sum of log-probs, TDT durations respected

struct Candidate { int word_lo, word_hi; int frame_lo, frame_hi; DecoderState state; int term; };

std::optional<int> best_replacement(const Candidate& c, const Terms& terms, float boost) {
    float heard = score_forced_path(m, enc, c.frame_lo, c.frame_hi, c.state, heard_tokens(c));
    float term  = score_forced_path(m, enc, c.frame_lo, c.frame_hi, c.state, terms[c.term].tokens);
    return term + boost * terms[c.term].tokens.size() >= heard ? std::optional{c.term} : std::nullopt;
}
```

```rust
// engine protocol: recognize carries the terms, not a prompt string, for Parakeet
pub struct VocabularyBias { pub terms: Vec<BiasTerm>, pub boost: f32 }
pub struct BiasTerm { pub term: String, pub aliases: Vec<String> }
```

**Stage 2: decode-time biasing (only if stage 1's recall falls short in the benchmark).**
- Whole-clip RNN-T decodes carry "shadow" hypotheses beside the greedy path. Wherever a term's first piece is plausible at a word start, a hypothesis spells the term through a token trie.
- Each piece gets the full boost if the greedy words sound like the term, and 0.6 × the boost otherwise.
- At the word boundary, the term replaces the greedy span only if it scores higher.
- Rejected designs:
  - boosting term tokens at every greedy step, which can't be gated tightly enough to protect ordinary speech;
  - subtracting an internal language model estimate, which over-fires;
  - plain shallow-fusion beam search, which is slower and drops phrases.

**Config.** `[dictation] vocabulary_boost = "normal"` (`off`, `light`, `normal`, `strong`). The boost applies to Parakeet only; Whisper keeps its prompt.

**Cost.** Stage 1 adds work only when a clip has a candidate span. The target is a median under 150 ms extra on such clips on the reference GPU and nothing on clips without candidates. The benchmark measures it.

## Acceptance

- **R1:** A Parakeet `recognize` with a vocabulary applies stage 1 rescoring. With `vocabulary_boost = "off"`, or with an empty list, output is byte-identical to today's.
- **R2:** On the term-recall benchmark at `normal`, term recall rises clearly over `off` (target: at least two thirds of missed terms recovered). False insertions on distractor clips stay at zero, and guard-set WER does not rise. The numbers are recorded in the benchmark report.
- **R3:** Added latency is reported per clip. The median on clips with candidates is under 150 ms on the reference GPU, and clips without candidates add no measurable time.
- **R4:** Aliases widen candidate matching. A term with an alias is found where its spelling alone would not be.
- **R5:** Case-only differences, inflections, paths, URLs and addresses are never rewritten, and terms under 5 letters without an alias are skipped. Each has a unit test.
- **R6:** Stage 2 is built only if R2's target is missed with stage 1, and that decision is recorded in an ADR either way.
- **R7:** `docs/engines.md` and `docs/dictation.md` describe how the vocabulary reaches Parakeet and what the boost does. A changelog entry is added.

## Parked unknowns

- Whether parakeet.cpp exposes encoder frames, decoder state and the joint network to callers, or whether a patch is needed. This is resolved by reading the vendored source in the first task.

## Boundaries

- Rescoring never invents a term the audio does not support. The comparison is always against the heard words on the same frames.
- No change to Whisper's handling of the vocabulary.

## Quick commands

- `cargo test -p dettivo-engine-parakeet -p parakeet-cpp-sys`
- `dettivo-qa bench vocabulary`
- `just build test lint`
