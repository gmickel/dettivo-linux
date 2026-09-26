# 0072. Every remote line takes the speaker who holds most of its sentence, and a line outside every turn takes the nearest one within ten seconds

Status: Accepted 2026-09-26, amends [0035](0035-sherpa-onnx-diarization-engine-and-the-speaker-pass.md) (the labelling rule and its configuration)

## What this gives you

A finished meeting names a speaker on nearly every remote line. On the ten retained meetings the share of remote lines left blank falls from 18.0% to 1.1% with the current engine and from 19.5% to 2.4% with Nemotron. The error that counts a blank as a miss falls by 6.6 points on AMI dev with the current engine and by 7.8 points with Nemotron. The cost is more wrong names. Of the lines that carry a name, 5.2 points more carry the wrong one on AMI dev with the current engine, and 5.6 points more with Nemotron.

## Situation

ADR 0035 labelled a segment only when at least 25% of its span lay inside diarized speech (`min_coverage`) and one speaker held at least 60% of that speech (`min_speaker_share`). Every other segment stayed blank. The fn-64 report measured 22.7% of remote lines blank on eight retained meetings, and the fn-67 bench measured 18.0% on the ten it now holds. None of the 13 open-source tools surveyed on 2026-09-26 leaves lines blank this way. omarchy-meeting-recorder keeps a sentence with one speaker and cuts at a pause of 250 ms only where the speaker changes. whisper-diarization gives a sentence its majority speaker. Vibe falls back to the nearest turn within 1.5 s. anarlog merges fragments under three words and 1.5 s into a neighbour. noScribe PR #351 measured per-word assignment and a cut at every diarized change as net-harmful.

Two facts about Dettivo's own data shaped the rule. The Whisper engine aligns no words, so no stored meeting and no AMI transcript on the bench carries word times. Parakeet does align words. The fn-67 bench, which scores the product's Rust rule through `crates/dettivo-meeting/examples/diar_assign.rs`, showed where the blank lines come from. On AMI dev, 2,319 of the 12,875 scored lines were blank for a low share, and in 2,127 of those the reference itself holds two speakers. Lines like these hold two people's words, so any single name is right for only about half of them. The most-overlap speaker was right on 52% of them with the current engine and 66% with Nemotron. Choosing by the start, middle, end or longest turn of the line did worse.

## Decision

- **Sentence units.** `crates/dettivo-meeting/src/sentences.rs` cuts every assigned segment after a token that ends a sentence (`.`, `?`, `!`, `…` and their CJK forms, closing quotes aside). Where the engine aligned one word per token, it also cuts at a gap of at least `pause_ms` (250) between words. A cut inside an unaligned segment takes its time from the character offset. Consecutive pieces of the remote stream, across segment boundaries, join into one unit unless a sentence ended between them or a pause of at least `pause_ms` separates two pieces whose own majority speakers differ.
- **The vote.** A unit takes the speaker with the most overlapping time, the earlier-appearing one on a tie, and `speaker_confidence` is that speaker's share of the unit's diarized speech. A unit no turn overlaps takes the nearest turn within `nearest_turn_ms` (10,000) with confidence 0, and otherwise stays blank. `min_speaker_share` now applies to the unit and defaults to 0, so a unit any turn overlaps gets a name. Setting it to 0.6 brings back blanks on mixed sentences for a user who prefers a blank to a wrong name.
- **Splits.** A segment whose pieces took different speakers becomes one stored segment per speaker run, with its own times, text slice and words. Only the first keeps `gap_before_ms`. The daemon polishes the parts again with the global `[polish]` transforms (`MeetingArchive::polish_missing`), and every segment is renumbered in order. Search, exports and the transcript read the parts as ordinary segments.
- **Fragments stay whole.** Folding a unit of fewer than three words and under 1.5 s into its nearer neighbour raised the AMI dev headline from 14.66% to 15.90% with Nemotron and from 22.44% to 23.16% with the current engine. A two-word, 500 ms bound still raised it. The rule has no fragment merge, and a one-word sentence votes like any other.
- **Configuration.** `[meetings.diarization]` gains `pause_ms` and `nearest_turn_ms`, and `min_speaker_share` keeps its name with the unit meaning and a default of 0. `min_coverage` is retired. A file that still sets it loads and the value is ignored, so an upgrade never drops a configuration to defaults over it. `config.keys`, the Settings route and `docs/config.md` no longer list it.
- **Re-runs.** `dettivo meetings diarize <id>` applies the rule to an existing meeting. A second run over its own output changes nothing, because each part is one sentence run with one speaker. The joined text and the words stay the same. Earlier splits stay split.

## Consequences

The fn-67 bench measured the rule against a saved baseline of ADR 0035's rule on the same cached inputs. The engine turns were identical (engine DER delta 0.00), and the AMI and meeting segments were the cached ones that baseline read. The report is [diarization-bench-2026-09-26-sentence-units](../reports/benchmarks/diarization-bench-2026-09-26-sentence-units.md). Its engine identity for the current engine names the binary installed during the run. The turns it scored are the cached outputs of the fn-67 engine build.

| Split, metric | Current engine, before → after | Nemotron, before → after |
|---|---:|---:|
| AMI dev, attribution error | 28.99% → 22.43% | 22.43% → 14.65% |
| AMI dev, labelled lines with the wrong speaker | 15.42% → 20.57% | 5.46% → 11.03% |
| AMI dev, short-turn error | 79.45% → 61.95% | 74.44% → 54.16% |
| AMI test (held out), attribution error | 22.98% → 16.05% | 21.92% → 14.70% |
| AMI test (held out), labelled lines with the wrong speaker | 6.78% → 12.01% | 8.43% → 13.72% |
| Retained meetings, remote lines blank | 18.01% → 1.14% | 19.52% → 2.40% |

- The fn-68 spec asked that the wrong-speaker share of labelled lines rise by no more than one point on AMI dev. The rule misses that bound by 4.2 points with the current engine and 4.6 with Nemotron, and the held-out split confirms the size of the miss. A share floor of 0.6 keeps the current engine within a point (16.08%) but still misses with Nemotron (7.91%), and it takes back most of the gain (attribution error 28.50% and 21.32%). The likely route to that bound while leaving almost no line blank is word times on the lines that hold two voices, so a cut can fall between them. The Whisper engine aligns none, and changing it is outside this decision.
- `nearest_turn_ms` has no measurable effect on AMI dev, where nearly every line overlaps a turn. Its default comes from the retained meetings. There 1.5 s left 4.8% and 6.9% of remote lines blank, 5 s left 2.4% and 4.1%, and 10 s is the first round value under 3% for both engines. Those meetings carry no speaker labels, so the bench cannot yet say how often a nearest-turn name is right. fn-72's labels can.
- `pause_ms` acts only on segments with aligned words, so neither split measures it. It keeps omarchy-meeting-recorder's 250 ms.
- The bench gained a line, "Labelled lines, wrong speaker" (`labelled_wrong` in `scripts/qa/diarbench/metrics.py`), which the spec's bound reads.
