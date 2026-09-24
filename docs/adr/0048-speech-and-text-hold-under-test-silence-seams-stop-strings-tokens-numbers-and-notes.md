# 0048. Speech and text are never dropped by a shortcut: silence, seams, stop strings, protected tokens, numbers and notes hold under test

Status: Accepted 2026-09-06. Amends 0022 (the near-silence guard and the seam rule), 0023 (the Polish pass and the numeric guard), 0026 (streamed partials) and 0038 (the notes editor).

## What this gives you

What you said reaches the transcript and what you typed reaches the daemon. One short answer in a long silent import is transcribed instead of averaged away; a word you said twice with a pause between stays said twice; a streamed rewrite reads the same as the answer it ends in; a filler or an `i` inside a backticked command or a path is left alone by Polish; a rewrite that turns `42` into `420` is refused; a dictation shorter than the level meter's window is still heard; and a meeting note is yours until the daemon has confirmed it saved.

## Situation

A cross-model review of main `28b4b7d` (ten gpt-6-astra reviews, one per area) named seven places where a shortcut could remove or alter speech or text: engines F4, F5 and F12, dictation F12, F17 and F18, qt-hosts F2. Each was verified against the code with a test that failed before its fix. The shortcuts were sound in the common case and wrong at the edges: a five-minute average judged by a per-frame threshold, a word match with no notion of time, a stop string checked only after the piece was already streamed, prose rules run over tokens the previous layer had gone to lengths to protect, a digit string compared by substring, a peak read from display events, and a dirty flag cleared before the write went out.

## Decision

- **The near-silence guard judges every 200 ms frame** (`filters::is_near_silent`, `SILENCE_FRAME`). A chunk is silent only when every frame stays under `silence_rms_floor`; the floor is the macOS speech activation level and applies to a frame, never to the chunk's average. An all-silent chunk still never reaches the engine and still yields `notice = silent`. This amends ADR 0022's reading of the guard.
- **Chunks reconcile only inside the audio they share.** `merger::merge` appends a chunk that starts at or after the previous one's end untouched, and the seam run considers only words that start inside the overlap widened by `SEAM_TOLERANCE_MS` (1500 ms, the engines' timestamp precision plus the spread of evenly estimated words). The midpoint rule and the 250 ms duplicate window are unchanged. The live path's windows, which skip silent ticks and so arrive disjoint, are the case this protects. This amends ADR 0022's seam rule.
- **A stop string's prefix is never streamed.** `dettivo-engine-llm` runs its token loop through `stop::Streamed`: the text streamed is the accumulated text less the longest suffix that is a proper prefix of a stop string, released when a later piece rules the match out and dropped when the stop completes, so the `partial` events concatenate to `text`. The delay is bounded by the longest stop string. This amends ADR 0026's streaming description.
- **Polish hides what Raw protects.** `polish::rewrite` swaps the protected spans (`raw::hide`, the scanner of ADR 0023's amendment) for placeholders before any transform, filler, casing, whitespace or style rule runs and restores them byte for byte before the post-processors, so the macOS rules stay verbatim over prose and never touch a token. The goldens hold unchanged. This amends ADR 0023's Polish pass.
- **The numeric guard compares numbers, with multiplicity.** `enhanced::guard` matches each dictated number against one output number with its decimal point and colon intact; a trailing sentence mark and thousands commas are the only differences allowed. `42` in `420`, `12` in `1` and `2`, `1.5` in `15` and one `3` for two are rejected. Signs and locale decimal commas are outside the rule: the regex does not capture a sign and a comma is read as grouping. This amends ADR 0023's protected-token check.
- **A take's silence is read from its samples.** `dettivo-session`'s worker takes the peak from every PCM chunk on the meter's scale; `Event::Level` feeds the display only. `silence_peak_threshold` keeps its meaning and its value.
- **A notes draft is dirty until the daemon acknowledged it.** `NotesSaver` (shared by `MeetingLiveModel` and `MeetingDetailModel`) keeps the draft revision and the acknowledged revision apart, sends one save at a time per meeting after the one-second debounce, sends an edit made during a save after its answer, keeps a refused or unanswered draft for the next flush, parks a draft it cannot send when the meeting is left and sends it on the next connection or visit, matches a late answer by meeting and revision, and lets the detail take the row's notes only when nothing is dirty. `dettivo-app` flushes both editors and the socket (`DaemonClient::finish`) on `aboutToQuit`. This amends ADR 0038's notes editor.

## Consequences

- Sparse imports cost more inference: a chunk with one 200 ms frame over the floor goes to the engine. The hallucination filter and `notice` still apply to what comes back.
- Some engine repetitions across a seam that the time-blind rule hid now show; the shared-audio rule with its 1.5 s tolerance is the bound, pinned by the merger's tests and the three-chunk golden.
- Streaming a rewrite lags by at most the longest stop string; the end of generation, a cancel and a length stop all flush what was held.
- The Polish pass protects tokens whether or not `[dictation] protect_tokens` is on; the setting still governs the raw layer's sentence-end and replacement rules.
- A rewrite that reformats a number in any way other than thousands grouping is refused and the Polish text goes in; that is the intended trade for never inserting a changed figure.
- A parked notes draft lives in the app's memory only; an app that exits while disconnected loses it, and the state says `Not saved`.
- Verified by `cargo test -p dettivo-transcribe -p dettivo-engine-llm -p dettivo-language -p dettivo-session` and `dettivo-app-meetings-test`, each with the case that failed before its fix.
