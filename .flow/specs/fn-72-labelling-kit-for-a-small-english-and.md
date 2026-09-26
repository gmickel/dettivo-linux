# Labelling kit for a small English and German meeting set

## Goal & Context

Every number for Dettivo's own meetings is still a proxy: the local/remote track split and a model judge. No public German meeting benchmark exists. The only German diarization numbers are two-speaker phone calls. To know where we really stand, and to keep the fn-67 bench honest for German, we need a small human-labelled set: about 15–30 short multi-speaker meetings or excerpts, 2–4 hours in total, English and German. A published benchmark study warns that differences of a few DER points are usually noise on small sets, which is why the bench reports intervals.

Labelling must be fast enough that Gordon actually does it. Line-level labels are what the headline number needs: which speaker said each transcript line. Frame-precise turns are not needed.

## Approach

- A local labelling page served on localhost. Everything stays on the machine and nothing is uploaded. It shows the meeting's transcript lines with audio playback per line and keyboard shortcuts to set or confirm the speaker. It supports speaker names or anonymous letters.
- Drafts are prefilled from a blend of both engines, or blank, and the page shows where the systems disagree first. The research warns that correcting one system's output biases the reference towards it, so a draft never comes from a single system, and the page records which draft seeded each file.
- Excerpt picking chooses 10–20 minute windows with the most speaker changes, so labelling time buys the most signal.
- Labels are stored in the protected eval directory in the bench's format, and fn-67 picks them up automatically.
- A short protocol page covers what counts as a line's speaker, overlap, backchannels and unknown speakers.

## Quick commands

- `just build test lint`
- `just diar-label`

## Acceptance

- **R1:** `just diar-label` opens the local page on a chosen meeting or excerpt, and labelling a 15-minute excerpt takes about 20 minutes or less with the keyboard.
- **R2:** Labels are saved in the bench's format in the protected eval directory, and `just diar-bench` reports the labelled set's word-level speaker error (WDER) and headline number per language.
- **R3:** Drafts never come from a single system, and the page and the saved file record which draft was used.
- **R4:** Nothing leaves the machine: the page binds to localhost only, loads no external assets, and nothing is committed. A test asserts that the server binds to localhost only.
- **R5:** The protocol is written down in the docs next to the bench.

## Boundaries

- The labels themselves are Gordon's work. The spec delivers the kit, not the labelled set.
- No cloud labelling service.

## Decision Context

Gordon approved recommendation 5 on 2026-09-26.
