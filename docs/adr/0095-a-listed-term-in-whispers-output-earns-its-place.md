# 0095. A listed term in Whisper's output earns its place against the take decoded without the prompt

Status: Accepted 2026-11-02. Amends 0081 and 0088.

## What this gives you

On Whisper, a vocabulary helps you get the names you say spelt as you listed them, and it no longer writes them into sentences where you never said them. Both engines now hold one bar: a listed term comes out when it was said, and not when it was not.

## Situation

Whisper takes the vocabulary as its decoder prompt, so the model decides whether to write a term, and until now nothing checked the result. On the term-recall benchmark, Whisper large-v3-turbo with the vocabulary on wrote 10 of 15 terms. It also wrote 4 terms into 3 of the 25 distractor sentences. "Omar keeps the spare keys in his desk." came back as "Omarchy, Hyprland.", "Clay and I went to the cinema" as "Clef and I went", and "Omar, he said" as "Omarchy said". The prompt-echo guard of ADR 0081 drops a segment only when it is the whole prompt, so these partial echoes were pasted. Parakeet decides each replacement from the audio and wrote no false term (ADR 0088).

whisper.cpp reports a log probability for every token it emits, and one engine state can decode a take twice, once with the prompt and once without it.

## Decision

- **What is checked.** A dictation request to Whisper carries the prompt's terms in `vocabulary` (no aliases, boost 0, both ignored). A term span is a run of one to four words of one segment whose letters and digits, lower-cased, spell a term, or the term with `s` after it. Only a term in the prompt can be read back from it, so aliases are never looked for, and a take whose output spells no term is returned exactly as decoded, with no second decode.
- **Echo shape.** A span is a suspect, however sure the decoder was, when it sits in a run of term spans that closes its segment and either opens the segment or follows a word ending in `.`, `!` or `?`. "Omarchy, Hyprland." and "Book the room. Omarchy." are echo-shaped. "I bound the key in Hyprland." is not.
- **Token confidence.** Any other span is a suspect when the mean log probability of the tokens under its letters is under -0.3 nats (`MIN_MEAN_LOGPROB`, a geometric mean probability of 0.74). Tokens that do not spell the segment's text make the span a suspect.
- **Settling a suspect.** When any span is a suspect, the engine decodes the take once more on the same state with no prompt, in the language the first decode used. Words both decodes share, matched by their longest common subsequence, anchor the two. A suspect stays when that decode has its words too, or when the gap between its anchors spells the same letters in both ("Sapien XT" for SapienXT). Otherwise the gap becomes the words that decode heard there, or nothing when it heard nothing there. A segment the check rewrote returns no word timings.
- **The level** was chosen as fn-84 chose Parakeet's boost: the best level that keeps zero false insertions with no guard WER rise, with recall at least at its earlier figure. Every level from -0.45 to -0.2 does that, with 10 of 15 terms and no false insertion. -0.3 is the round level nearest the middle of that window ([vocabulary-2026-11-02-thor-whisper-threshold](../reports/benchmarks/vocabulary-2026-11-02-thor-whisper-threshold.md)). One level serves both recall and safety on this set, so there is no new setting.
- The engine logs `vocabulary check candidates=<n> suspects=<n> replaced=<n> check_ms=<ms>` per request and each span's mean log probability at debug level, never a word. In CLI mode `--vocabulary '[{"term":"Clef"}]'` beside `--prompt` does the same.

## Consequences

- On the 47-clip set, padded and guarded as dictation does (ADR 0096), Whisper with the vocabulary on writes 10 of 15 terms and no false insertion, guard WER 0.000 against 0.011 with it off. The kept level has 0.12 nats above the least sure target (target-hyprland-1, -0.177) and 0.16 nats below the surest distractor term (distractor-clay-name, -0.458). Above -0.2, target-hyprland-1 gives way to "Piper Lan". Below -0.45, the Clef distractor stays, and below -0.66 the "Omarchy said" distractor stays too.
- The two echo-shaped terms (-0.96 and -1.18) are caught by shape at any level. A confident echo would be caught the same way.
- 33 clips with no term came back byte for byte as before. 10 of the 13 clips with a term needed no second decode. The three that did took 78 to 85 ms more on an RTX 4090 over Vulkan, against a target of 150 ms, so the median over clips with a candidate is 0 ms.
- A take that is only one listed name ("Dettivo.") is echo-shaped. It keeps the term only when the decode without the prompt also spells it, and otherwise pastes what Whisper hears without the vocabulary. A silent take primed with one term reads the same way, so the shape cannot tell the two apart.
- When a gap holds a rejected term and other words the two decodes disagree on, the whole gap takes the prompt-free words, including a confident term in it.
- File transcription, re-runs and meetings still send the prompt without the terms and are not checked.
