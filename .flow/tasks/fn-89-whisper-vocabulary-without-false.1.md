---
satisfies: [R1, R2, R3, R4, R5]
---
# fn-89-whisper-vocabulary-without-false.1 Implement Whisper vocabulary without false insertions

## Description
TBD

## Acceptance
Every R-ID in the parent spec's ## Acceptance Criteria is satisfied; judge this task against the spec's criteria directly.

## Done summary
Whisper checks every listed term in a prompted dictation transcript (ADR 0095). Two kinds of term are suspects: an echo-shaped run of two or more terms, and a term whose mean token log probability is under -0.3. Each suspect is settled against the same take decoded without the prompt. The output guards moved to dettivo-transcribe, and `dettivo-qa bench vocabulary` passes Whisper the terms and guards each clip as dictation does (ADR 0096). It now also measures Whisper's lone names on every run. After QA, a lone listed name the decoder was sure of pastes, and raw spoken punctuation keeps nouns and ends a sentence once. Parakeet no longer writes `C++++.` for a heard `C++.`.

Round 3 (conductor decision 2026-11-02: a lone-name level of -0.45, with the order to stop rather than ship it on any false insertion). Merged origin/goal/fn-88-better-dictation-shipped-as-one-release first (2684ac6). The conflicts came only from fn-84 being squashed as 379e18b, whose crates/docs tree equals the 92c0729 already merged here. Each conflict resolved to ours, and the merge changes no crates/docs file against c8546c7.
- Lone-name level -0.45: measured and NOT shipped. The new single-name bench section (10 lone names, 10 lone sound-alikes, two Piper voices, rendered by scripts/qa/make-vocabulary-fixture.py) gave 7/10 names but 1 false insertion. The newly rendered lone "Clay." (amy) decodes as "Clef," at -0.344. That is surer than three real names: Clef-ryan -0.416, SapienXT-amy -0.415, Hyprland-amy -0.411. No level on this set keeps those names and refuses it. At -0.3 the take goes to the prompt-free decode and pastes "Clyde.". The level stays -0.3, and a test pins the measured case (a5f5cf9). The -0.45 report is filed as docs/reports/benchmarks/vocabulary-2026-11-02-thor-lone-level-0.45.json, from an uncommitted build on ff79297.
- Bench at ce35eea (Whisper large-v3-turbo / Parakeet v3, Vulkan, RTX 4090, GPU 2-10 % busy from other users, warm private driver cache). Whisper on 10/15, 0 false insertions, guard WER 0.000 (off 0.011). Parakeet on (normal) 13/15, 0 false insertions, guard WER 0.000, 13.2 ms median. Whisper single names: 5/10 names, 0/10 sound-alikes as terms. Main-set transcripts are identical to the filed fn-89 report. Second decodes take 70-85 ms. Filed as vocabulary-2026-11-02-thor-single-name.{json,md}.
- Daemon (isolated debug dettivod under xvfb-session, mock mic/insert, persistent private driver cache): the 20 single-set clips paste the same text as the bench, take for take. Names 5/10, sound-alikes 0/10, check 74-153 ms on the four suspects.
- Parakeet C++ (review follow-up from round 2): `written()` drops the term's own trailing marks from the heard word's marks, so a heard `C++.` stays `C++.`. Failing test first (ff79297), then fix f7b3dd4.
- Docs: ADR 0095 (single-name measurement, rejected lone level), ADR 0088 (marks rule), docs/dictation.md, docs/qa.md, CHANGELOG. docs/engines.md is unchanged because the rule it states (-0.3, one name judged by confidence) is unchanged.

QA fixes (QA verdict at 9b1f92e, receipt qa-fn-89-whisper-vocabulary-without-false.json). Merged origin/fn-84-vocabulary-on-parakeet at 92c0729 first (e32311c, clean).
- QA-fn89-1 (fixed, b562a13): echo shape now needs a run of two or more listed terms, and one name alone is judged by confidence.
- QA-fn89-2 (cause found, one-time cost documented): the 0.8-1.2 s second decode is GPU pipeline compilation on a cold NVIDIA shader cache. With a warm driver cache the second decode takes 69-85 ms. Recorded in ADR 0095.
- QA-fn89-3 (partly fixed, 2b527e6 and f04e248): a punctuation word after a determiner stays a word, and a spoken stop beside Whisper's own ends the sentence once. "…during the exam period." still pastes "…during the exam.", by design.

Tests: check_tests.rs a_lone_listed_name_stands_when_sure (pins the -0.344 sound-alike), bias::tests a_heard_word_spelling_the_terms_marks_adds_none_of_them (failed first), vocabulary.rs the_single_name_set_has_two_lone_takes_a_name_and_ten_sound_alikes. Focused crates whisper/parakeet/qa: 266 passed. `just lint` green.

Follow-ups (not done):
- More lone sound-alike renderings would show whether any lone level can admit the unsure names. On this set, none can.
- The silent warm-up (ADR 0084) leaves speech-decode pipelines uncompiled on a cold driver cache. This predates fn-89.
- From the first pass: file transcription, re-runs and meetings still prompt Whisper unchecked. No mapped feature route changed.

Tier: implementer (project routing block: claude-opus-5-5) (model: claude-opus-5-5)

stage: impl-review - ran [round 3: 3 reviewers (correctness, contracts, integration) gpt-6.1-sol via codex, SHIP with no findings at 0b6f9df, base c8546c7]

stage: plan-sync - skipped(config: planSync.enabled != true)
## Evidence
- Commits: e32311cae763, b562a13, 2b527e6, f04e248, 60efadb890e7ecfd28936d4c11161185396b87a4, 2618cc980656668df74cbe889e253be468971db8, 841270997d2ecd94a7ab9812d5e0c8b22438d55c, 2684ac6, ff79297, f7b3dd4, a5f5cf9, ce35eea, 4a25a62, 0b6f9df
- Tests: cargo test --no-fail-fast -p dettivo-engine-whisper -p dettivo-engine-parakeet -p dettivo-qa (266 passed, isolated env), just lint (green), dettivo-qa bench vocabulary --engines target/release at ce35eea: whisper on 10/15, 0 false insertions, guard WER 0.000; parakeet on 13/15, 0 false insertions; whisper single names 5/10, 0/10 sound-alikes; second decodes 70-85 ms, lone level -0.45 (uncommitted build on ff79297): single names 7/10 but 1 false insertion (lone-clay-amy 'Clef,' at -0.344) - rejected, single-name set (20 clips) through isolated daemon (xvfb-session, persistent driver cache): names 5/10, sound-alikes 0/10, identical to bench, impl-review codex gpt-6.1-sol x3 SHIP at 0b6f9df (base c8546c7)
- PRs: