---
satisfies: [R1, R2, R3, R4, R5]
---
# fn-89-whisper-vocabulary-without-false.1 Implement Whisper vocabulary without false insertions

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
Whisper checks every listed term in a prompted dictation transcript (ADR 0095). Two kinds of term are suspects: an echo-shaped run of two or more terms, and a term whose mean token log probability is under -0.3. Each suspect is settled against the same take decoded without the prompt. The output guards moved to dettivo-transcribe, and `dettivo-qa bench vocabulary` passes Whisper the terms and guards each clip as dictation does (ADR 0096). After QA, a lone listed name the decoder was sure of now pastes, and raw spoken punctuation keeps nouns and ends a sentence once.

QA fixes (QA verdict at 9b1f92e, receipt qa-fn-89-whisper-vocabulary-without-false.json). Merged origin/fn-84-vocabulary-on-parakeet at 92c0729 first (e32311c, clean).
- QA-fn89-1 (fixed, b562a13): echo shape now needs a run of two or more listed terms, and one name alone is judged by confidence. Measured on the single-name set through the isolated daemon (Whisper large-v3-turbo, Vulkan, RTX 4090, shared GPU). With the five-term vocabulary 5/10 takes paste the name, against 1/10 at 9b1f92e and 7/10 at base e0071ee. With a one-term vocabulary 0/10 paste it (base 1/10: Hyprland at -0.605 is unsure). Options measured on the same decodes: a lone-term level of -0.45 or -0.6 gives 7/10. It was rejected because the two names it adds (Clef -0.416, Hyprland -0.411) have no distractor to calibrate against: ten rendered lone sound-alikes ("Clay.", "Omar.", "Hyper.", "Sapient.", "Detective.", two voices) never came out as a term, with or without the fix. Non-speech primed with the vocabulary (silence, click, breath, hum, noise) gives filler, never a term. Bench at f04e248's engine: 10/15, 0 false insertions, guard WER 0.000, every transcript identical to the shipped run. docs/dictation.md lines 48 and 82, docs/engines.md and ADR 0095 agree with the rule.
- QA-fn89-2 (cause found; one-time cost documented, no code change): the 0.8 to 1.2 s second decode is GPU pipeline compilation on a cold NVIDIA shader cache. The QA profile sets a fresh XDG_CACHE_HOME per take, so every take compiled. Through the daemon with a persistent driver cache, single-omarchy-amy drops from 862 ms to 97.7 ms (warm-up 10.9 s to 167 ms). With the fix the single-name second decodes take 69 to 76 ms, and the bench's three take 78.4 to 84.5 ms. It is not padding, re-encoding or a cold code path: the same audio through the engine CLI with a warm cache is 99 to 101 ms. Silent warm-ups with or without a prompt cannot precompile it: cold-cache check_ms stays 898 to 1176. Only a prompt-free speech decode does, at 100.3 ms. A user pays it once per driver shader cache (driver update or cache wipe), recorded in ADR 0095.
- QA-fn89-3 (partly fixed, 2b527e6 and f04e248): a punctuation word after a/an/the/each/every/my/your/our/their/its stays a word ("It took a period of three weeks", "a dash of salt"). A spoken stop beside Whisper's own ends the sentence once, over a preceding comma, with the next sentence capitalised. "Send me the report period." used to paste "report.."; it now pastes "Send me the report.". Not fixed, by design: "…during the exam period." still pastes "…during the exam." (was "exam.."). Measured through Whisper, the command "Send me the report period" comes back "Send me the report period." and the word use "during the exam period." Likewise "Open the file period then run it" and "The trial period ends on Friday" have the same shape. No text rule separates noun from command without breaking the command. "this/that period" stays a command because "Look at this period" means "Look at this.".

Tests: check_tests.rs a_lone_listed_name_stands_when_sure plus updated echo cases; raw/tests.rs a_punctuation_word_after_a_determiner_stays_a_word and a_spoken_stop_beside_the_engines_own_ends_the_sentence_once (each failed first). Focused crates: 386 passed. `just lint` green.

Follow-ups (not done):
- correctness:2 from review (in fn-84's merged Parakeet code, crates/dettivo-engine-parakeet/src/bias/mod.rs:190): a candidate already spelling `C++.` replaced by `C++` may write `C++++.`. Belongs to fn-84 / PR #45.
- A lone-name threshold looser than -0.3 would recover 2 more single-name takes; it needs lone distractors that actually produce a term before it can be calibrated.
- The silent warm-up (ADR 0084) leaves speech-decode pipelines uncompiled on a cold driver cache. This predates fn-89: a no-vocabulary user's first cold-cache take costs about 1.2 s.
- The review range from 0225288 included the merged fn-84 commits.
- From the first pass: file transcription, re-runs and meetings still prompt Whisper unchecked. Parakeet bench rows were not re-run (engine not built here). No mapped feature route changed.

Tier: implementer (project routing block: claude-opus-5-5) (model: claude-opus-5-5)

stage: impl-review - ran [QA-fix round 1: 3 reviewers gpt-6.1-sol SHIP (duplicate-stop capitalisation P2 x3 collapsed; Parakeet C++ P2 kept as fn-84 follow-up) -> fix f04e248 -> re-review 1 reviewer gpt-6.1-sol SHIP at f04e248]

stage: plan-sync - skipped(config: planSync.enabled != true)
## Evidence
- Commits: e32311cae763, b562a13, 2b527e6, f04e248, 60efadb890e7ecfd28936d4c11161185396b87a4, 2618cc980656668df74cbe889e253be468971db8, 841270997d2ecd94a7ab9812d5e0c8b22438d55c
- Tests: cargo test --no-fail-fast -p dettivo-engine-whisper -p dettivo-session -p dettivo-transcribe -p dettivo-language -p dettivo-qa (386 passed), just lint (green), dettivo-qa bench vocabulary --engines target/release (whisper on 10/15, 0 false insertions, guard WER 0.000; second decodes 78.4-84.5 ms), single-name set through isolated daemon (xvfb-session): five-term 5/10, one-term 0/10, lone sound-alikes 0/10 insertions, second decodes 69-76 ms with a warm driver cache, cold vs warm driver shader cache: check 862 ms -> 97.7 ms (single-omarchy-amy, daemon), period clips through daemon: 'Send me the report.', 'It took a period of three weeks.', guard-library 'during the exam.'
- PRs: