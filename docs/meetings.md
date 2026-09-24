# Meetings

A meeting records both sides of a call and survives what a call throws at it. `dettivo meetings start` opens two PipeWire streams on one clock, your microphone and the monitor of the default sink (what the other side says), and writes them as `microphone.wav` and `system.wav` under the meeting's directory with a journal of everything that happened to them. A headset that vanishes mid-call restarts the microphone on the new default as `microphone-2.wav` with a gap marker; a daemon that dies mid-call leaves a live checkpoint the next start turns into a partial meeting you recover or discard; stop is idempotent and visible as its own state (ADR 0027). Words arrive as `meeting.segment` events while the meeting runs, and the transcript on the row is built from every take once it stops, both sides on one clock (ADR 0031). A few seconds after that the meeting knows who spoke: the other side's segments carry speakers, yours carry `You`, and a speaker can be named across the whole transcript (ADR 0035). Once it stops the meeting is a record: your notes in Markdown, the model's summary, decisions and action items, one search over all of it, five export formats and a delete that removes exactly what you name (ADR 0036).

```
dettivo meetings disclosure [--acknowledge]     # the recording notice, once
dettivo meetings start [--title T] [--language L] [--no-system-audio] [--acknowledge-disclosure]
dettivo meetings status <id>                    # recording | stopping | stopped | partial, the takes, the recoverables
dettivo meetings stop <id>                      # answers stopping, then the meeting is stopped
dettivo meetings cancel <id>                    # drops the takes
dettivo meetings list | get <id> | segments <id> [--follow] | search <words>
dettivo meetings notes get <id> | notes set <id> [text|--file <path>|--stdin] [--source live]
dettivo meetings analyze <id> [--force] | analysis <id>
dettivo meetings export <id> --format txt|md|srt|vtt|json --out <path> [--raw]
dettivo meetings delete <id> [--policy transcript_only|transcript_and_audio|all]
dettivo meetings disclosure --copy              # the notice on the clipboard
dettivo meetings recover <id>                  # retry validated retained audio after partial/failed/cancelled/stopped
dettivo meetings discard <id>                  # remove a partial meeting
dettivo meetings diarize <id> [--speakers 2]    # the speaker pass again, with the count when known
dettivo meetings speakers <id>                  # the speakers with their talk time
dettivo meetings speakers rename <id> speaker_00 "Ada"   # a name across the transcript
dettivo speech download --provider diarize --model diarization-en --wait   # the configured model set, once
dettivo audio devices                           # the default source and the default sink
dettivo events --follow --topic meeting.state --topic meeting.segment --topic audio.level
```

## Before the first meeting

`meetings.start` refuses, in this order, with a `CONFLICT` whose `details.kind` names the gate:

| Gate | `kind` | What clears it |
|---|---|---|
| A dictation or a meeting is active | `sessionActive` | Stop or cancel it; one session runs at a time. |
| The recording disclosure has not been acknowledged | `meetingDisclosureRequired` | `dettivo meetings disclosure --acknowledge`, or `acknowledge_meeting_disclosure = true` on the start (and on a meeting import). The acknowledgement is recorded once in `state.toml` (`acknowledgements.meeting_disclosure`, `meeting_disclosure_at`) and never asked again; `meetings.disclosure.get` returns the text shown, the macOS notice. |
| The selected provider is not meeting-capable | `engineWithoutTimestamps` | Pick a meeting-capable provider under Settings > Models (`dettivo app open settings.models`, `dettivo speech selection set --provider whisper`); `details.provider` names the one refused and `details.settings` the route. Parakeet stays dictation-only (ADR 0018). |

`capture.microphone` must be true: a meeting always records the room. `capture.system_audio = false` records the microphone alone.

## While it records

```
idle ──start──▶ recording ──stop──▶ stopping ──▶ stopped ──▶ transcribing ──▶ completed
                   │                                              │
                   │                                              └─cancel ──▶ stopped     └─engine error ──▶ failed
                   └─cancel ──▶ cancelled        (daemon killed) ──▶ partial ──recover──▶ transcribing ──▶ completed
                   └─take unwritable ──▶ failed                                 └──discard──▶ gone
```

- **recording** opens the microphone (a pinned `[audio] input_device` or the default source) and the system track (`[audio] system_source`: the default sink's monitor, followed when the default sink changes, or a pinned sink's monitor), both at 16 kHz mono through PipeWire, and writes each as a take under `$XDG_DATA_HOME/dettivo/meetings/<id>/`. Both take writers share the meeting's clock, so `takes.json` and `system-takes.json` carry offsets on one time base (`start_offset_ms`, `start_offset_ns`); on the virtual rig the two first takes start within a few milliseconds of each other. `audio.level` carries `source = microphone` and `source = system` so a meter can show both. Without a default sink or its monitor the meeting starts microphone-only and the journal says `system_unavailable`; it is room audio.
- **a microphone that vanishes** (a headset unplugged, the default source moved, a pinned device gone) closes the current take, reopens the microphone on the current default (a pinned device that is gone falls back to the default rather than losing the room), starts `microphone-2.wav` with `gap_before = true` and its offset, and records `device_lost` (or `device_switch`) then `gap` in the journal naming the take. A loss that finds no source at all records the gap with the reason and the system track carries on alone; the next default change tries again. The system stream follows the default sink on its own; a sink change is a `system_switch` line, not a gap.
- **the live checkpoint** `live-checkpoint.json` (schema version 1) is written atomically every `[meetings] checkpoint_interval_seconds` (15): the meeting id and start, every take of both tracks with its offset and the samples flushed so far, the retained segment tail (`segments`, the final live segments so far, and `next_sequence`), `chunks_completed`, `chunks_total`, `is_finalizing` and `last_error`. The takes are flushed to disk at the same moment. A checkpoint that cannot be written is logged and the next interval retries; the capture never stops for it.
- **stop** answers at once with the `stopping` state and the contract's `transcribing` job; the worker closes both takes, marks the row `stopped`, and the finalisation follows (the next section): `transcribing` with `is_finalizing = true` and the chunk counts on `meetings.status`, then `completed` with the transcript, the checkpoint removed and `metadata.json` written. A second stop while it runs answers the same; a stop on a settled meeting answers its state (`succeeded`, `completed`), never an error.
- **cancel** stops both streams, removes the meeting directory and marks the row `cancelled`.
- **a take that cannot be written** (a full disk, a directory that cannot be created) ends the meeting `failed` with the reason on the row and on `meeting.state`; the takes written so far stay and the journal records the error.

`system.health` reports `recording_state = meeting` while one runs; `meetings.status` reports the state with `live_segment_count`, `live_last_end_ms`, `is_finalizing` and a Linux `capture` block (`duration_ms`, `microphone_takes`, `system_audio`, `is_partial`, `chunks_completed`, `chunks_total`, `reason`).

## Transcription

A meeting is transcribed twice by the same merger (ADR 0031): live, in windows, while it records, and in full from every take when it stops. Both give segments on the meeting clock with a side, `you` (the microphone) or `remote` (the system track).

**Live.** Each track is cut into windows at the macOS tuning: every `live_tick_ms` (900) of new audio cuts a window that reaches back `live_overlap_ms` (450) into the previous one and never exceeds `live_window_ms` (3000). A window whose RMS stays under `speech_floor_rms` (0.0065; 70 % of it right after speech) is silence and never reaches the engine, so nothing is invented for a quiet room. Each window goes through the selected engine, the other side first when both are due, and its result is folded into that side's segments: whatever ends more than `boundary_merge_gap_ms` (1200) before the newest audio is final and numbered (`you-12`), the rest is the provisional tail, one fragment per sentence (`you-p1`, `you-p2`, ...) that the next window announces again from `p1` (ADR 0061). Every fragment is a `meeting.segment` event, and `meeting.state` carries `live_segment_count` and `live_last_end_ms`. An engine that falls more than two ticks behind has the oldest audio skipped (`live_skip` in the journal, a gap on the next segment); the finalisation covers the span anyway. A device switch hardens the old take's tail and puts the gap on the first segment of the new take. The engine is frozen at start: a model that is not on disk is `NOT_FOUND` naming `dettivo speech download`, and a provider that answers a window without timestamps ends the live path (`live_unavailable`) while the capture goes on. `live = false` records without the live path and runs the finalisation alone.

**Finalisation.** The stop closes the takes; then every preserved take of both tracks goes through the chunked pipeline the imports use ([docs/history.md](history.md), ADR 0022), each take's segments are shifted onto the meeting clock by the take's offset, a take that began after a gap carries `gap_before_ms` on its first segment, the journal's other gaps become gap markers, the known filler hallucinations are dropped and near-silent chunks never reach the engine, and the two sides interleave by start. `meetings.stop` answers the contract's `transcribing` job at once and `job.progress` counts chunks over every take under the meeting's job id; the segments and the text land on the row and the state settles at `completed`. A cancel while it runs leaves the meeting `stopped` with its audio; a failed finalisation leaves it `failed` with the reason on the row and the audio kept. `meetings.recover` finalises a partial meeting the same way.

**Polished segments.** When the finalisation lands, every segment goes through the deterministic Polish pass (the global `[polish]` transforms, no app rules, no model; [docs/polish.md](polish.md)) into `polished_text`, so `meetings.get.transcript` and every export read clean sentences while `text` and `text_raw` keep the engine's words; `transcripts.export` with `raw = true` (`dettivo meetings export --raw`) renders the engine's words instead.

**Both sides at once.** Segments sort by start on the meeting clock, the other side first on a tie. Your microphone hears the speakers too: a microphone segment that is a filler (`yeah`, `okay`, `thank you`, or four characters or less) and overlaps, within `cross_source_padding_ms` (800), a remote segment at least twice as long and eighteen characters or more is that echo and is dropped. Everything else stays: two people talking over each other are both in the transcript, one after the other by start.

```
dettivo meetings segments <id>              # the transcript: side, span on the meeting clock, text
dettivo meetings segments <id> --follow     # a running meeting's live segments (~ provisional), then the transcript
dettivo events --follow --topic meeting.segment
```

`segments` prints one line per segment, `remote 00:04.200-00:07.350  ask not what your country can do for you`, with `[gap 1500 ms]` on a segment that followed missing capture; `--json` prints the row's segments. Under `--follow` on a running meeting the live events stream first, provisional lines marked `~`, until the meeting settles.

## Speakers

Once the transcript is on the row, the speaker pass (ADR 0035) runs by itself when `[meetings.diarization] auto` is on (the start's `diarize` overrides it for one meeting), and `dettivo meetings diarize <id>` runs it again at any time. The finalisation writes the plan on the row before it announces `completed`: a pass that will run is `queued` there, so a client that reads the meeting on that transition sees it coming, and a plan that cannot launch (or one a daemon restart interrupted) settles to `failed` with the reason (ADR 0061). The system track goes through `dettivo-engine-diarize` in one request (every system take on the meeting clock, the gaps as silence); a room-audio meeting, one recorded without a system track or imported from a file, sends its whole microphone track instead. The engine returns speaker turns, `job.progress` counts the engine's chunks under `stage = diarizing` with the meeting's `job_diarize_<n>` id, and `meeting.state` reports `diarization_status` (`queued` on the `completed` transition, `running`, then `ready`, `failed` or `unavailable`) while the meeting stays `completed`.

**The rule.** For a remote segment, coverage is the share of its span inside any speaker turn and the winner is the speaker with the most overlap. The segment gets the winner only when coverage is at least `min_coverage` (0.25) and the winner holds at least `min_speaker_share` (0.60) of the segment's diarized speech; otherwise it stays unlabelled, so a segment that straddles two speakers evenly or sits mostly in silence carries no name rather than a wrong one. Microphone segments are `You` (`speaker_id = you`); in a room-audio meeting every segment is assigned from the turns. The engine's labels are renumbered in order of first appearance, so `speaker_00` (`Speaker 1`) is whoever spoke first and a re-run lands on the same ids.

**On the row.** `meetings.get` and `transcripts.get` carry `speakers` (`speaker_id`, `name`, `color_index` in order of first appearance with `you` first, `talk_ms` summed over the speaker's segments), `diarization` (`status`, `coverage` of the track, `engine`, `model`, `ran_at`, `error`, `expected_speakers`, `auto`) and, on every segment, `speaker` (the name), `speaker_id` and `speaker_confidence` (the winner's share); `meetings.list` items carry `speaker_count`. `dettivo meetings segments <id>` prints the name where one is assigned and the side where none is.

**Speaker count.** The clustering decides how many speakers there are unless the meeting says: `expected_speakers` on `meetings.start` or the meeting import, `--speakers` on `dettivo meetings diarize`, or `[meetings.diarization] max_speakers`. `clustering_threshold` (0.6) is the initial average-linkage cosine cutoff. Low-support clusters are reassigned afterward, so final speaker counts need not change monotonically with the cutoff.

**Re-run.** `meetings.diarize { meeting_id, speakers? }` on a completed meeting replaces the assignment with fresh progress notices, keeps the names by id when the diarized speaker count is unchanged, and rebuilds the speakers otherwise (the names stay as suggestions). It answers `CONFLICT` with `kind = diarizationRunning` while a pass runs, `meetingNotCompleted` before the finalisation, `diarizationDisabled` under `enabled = false`, and `NOT_FOUND` with `reason = modelMissing` naming `dettivo speech download --provider diarize --model diarization-en` when the model set is not on disk; the automatic run records that case as `diarization.status = unavailable` with the command on the row and leaves the transcript as it is. An engine that crashes mid-pass fails the job with its last redacted log line on the row, the daemon and its socket carry on, and three crashes in a row mark the engine degraded in `speech.engines` and `dettivo doctor`. `meetings.cancel` on a completed meeting stops a running pass and keeps the transcript.

**Names.** `meetings.speakers.rename { meeting_id, speaker_id, name }` names a speaker across the meeting: the speaker row, every segment it holds (`txt` and `md` write the name before each line, `srt` and `vtt` before each cue, `json` carries the speakers), and the search index, so `meetings.search` finds a meeting by who spoke in it. An empty name restores the label (`You`, `Speaker 2`); a name over 64 characters is `INVALID_PARAMS` naming the limit; a rename while the pass runs is `CONFLICT`. Every name given is remembered, and `meetings.speakers.suggest { prefix?, limit? }` offers them for the next meeting, most recently used first. `meetings.speakers.list { meeting_id }` reads the speakers and the diarization block.

`dettivo-engine-diarize` runs sherpa-onnx's offline speaker diarization (pyannote segmentation 3.0 for the turns, English VoxCeleb ERes2Net for the embeddings) on CPU or optional CUDA ([docs/engines.md](engines.md)); `[engines.diarize] threads` sets the count. The model set is one 33.4 MB download ([docs/models.md](models.md)).

## The meeting directory

```
$XDG_DATA_HOME/dettivo/meetings/<id>/
  microphone.wav          the first microphone take (16 kHz mono)
  microphone-2.wav        the take after a device switch, and so on
  system.wav              the system track
  takes.json              the microphone takes: file, start_offset_ms, start_offset_ns, gap_before, samples
  system-takes.json       the same for the system track
  journal.jsonl           one line per event (below)
  live-checkpoint.json    while recording, and after a kill until recover or discard
  metadata.json           meeting identity, both take lists and checkpoint schema; no duplicate row text
  notes.md                your notes, written whenever you save them
  analysis.json           the summary, decisions and action items, with the model and the time
```

The journal is append-only JSON lines, each with `at` (UTC), `offset_ms` on the meeting clock, `event`, and where it applies `track`, `take` and `detail`: `start`, `system_unavailable`, `checkpoint`, `device_switch`, `device_lost`, `gap`, `system_switch`, `system_ended`, `fixture_finished`, `live_skip` (the engine fell behind; the span skipped), `live_error` (one window the engine failed), `live_unavailable` (the live path ended; why), `error`, `stop`, `cancel`, `finalize` (the takes and gaps it read), `finalized` (segments and chunks), `finalize_cancelled`, `recovered`. The finalisation reads its gaps from it, and so can you.

`[meetings]` in `config.toml` decides what stays ([docs/config.md](config.md)): `keep_audio = true` keeps both tracks (off removes the takes after automatic speaker assignment settles), `checkpoint_interval_seconds = 15`, and `artifacts = "keep"` writes `metadata.json` beside the takes (`audio_only` skips it, `none` removes the directory once the meeting stopped). What a delete removes is the contract's `artifact_policy` ([Delete](#delete)).

## Notes

Your notes are yours: Markdown you write during or after the meeting, stored on the row and as `notes.md` in the meeting directory, and never touched by a model. `meetings.notes.set { meeting_id, markdown, source? }` writes both (the file through a sibling temporary file and a rename, so a reader never sees half of it); `meetings.notes.get` answers the text, who wrote it last (`user`, or `live` for the app's editor while the meeting recorded) and when. Notes are allowed on a meeting that is still recording, partial or failed. A body over 1 MiB is `INVALID_PARAMS` naming the limit. `dettivo meetings notes set <id> "text"`, `--file <path>` or `--stdin` writes them; `notes get` prints them. `meetings.get` and `transcripts.get` carry `notes` and `notes_source`, and `meetings.list` says `has_notes`.

## Analysis

Once the transcript is final the meeting gets a summary, the decisions and the action items (each with its owner and due text when the transcript said) from the language model provider layer the `enhanced` mode uses ([docs/polish.md](polish.md), ADR 0023): `[meetings.analysis] provider` names it (`local`, `ollama`, `openai_compatible`, `auto`; empty follows `[llm] provider`), `[llm] analysis_model` names the local model (empty means `[llm] model`, and then one engine process serves both), a remote endpoint needs `dettivo llm trust` first, and `DETTIVO_MOCK_LLM` stands in under QA. The transcript goes to the model one line per segment with the speaker (or the side) in brackets; a transcript longer than `chunk_chars` (12000) is cut on segment lines and each part analysed on its own. A part whose answer the model's output budget cut off (the local engine reports `length`, or the JSON object never closes), or whose prompt did not fit the context, is halved on its lines and each half analysed, up to four levels deep, before the run fails naming the split count (ADR 0062). The parts' decisions and action items are joined in order with exact repeats removed, and the model combines only their summaries, in calls of at most `chunk_chars` of summaries, until one remains; a combining call that is cut off or does not fit is retried on smaller groups of summaries, and only a pair that still does not fit fails the run. Every answer has its thinking block stripped and is parsed strictly; an answer that finished with no usable summary is asked for once more with the reason, and a second one fails the run naming both. The whole run must finish within `timeout_ms` (60000).

It runs on its own after every finalisation (and after an import into a meeting) while `[meetings.analysis] auto = true`; `meetings.start` and `transcripts.import` take `analyze` to decide per meeting. The finalisation marks the row `queued` before `completed` goes out and starts the job once the speaker pass is done; a start the daemon refuses settles the row `failed` with the refusal, so `queued` never outlives the plan (ADR 0061). `meetings.analyze { meeting_id, force? }` runs it by hand and answers the job: `force` regenerates a meeting that already has an analysis (the old one stays until the new one parses; a failed regenerate keeps it and reports the error), a meeting without one runs at once, and one that has one answers `succeeded`. `CONFLICT` names `analysisRunning`, `meetingNotCompleted` (no final transcript yet) or `endpointNotTrusted`; when no provider answers the row is `failed` with `provider_unavailable` and the answer carries the `notice`, so the same call succeeds once one does. An empty transcript fails with `emptyTranscript` and asks no model. `meetings.analysis.get` answers `analysis_status` (`none`, `queued`, `running`, `ready`, `failed`), the analysis, the error, the model and the time; `meetings.get` and `transcripts.get` carry `analysis` and `analysis_status`, `meetings.list` carries `summary` and `analysis_status`, and `meeting.state` reports `analysis_status` as the job moves. The result lands on the row; `analysis.json` is written only when the artifact policy is `keep`. Automatic speaker assignment runs before analysis, and the completion owner applies temporary-audio retention afterward. `dettivo meetings analyze <id> [--force]` and `dettivo meetings analysis <id>` are the verbs.

## Search

`meetings.search` and the meeting half of `transcripts.search` run one FTS5 index over the title, the transcript, the speaker names, the notes and the analysis, prefix-matched by word start with diacritics folded ([docs/history.md](history.md)). A notes or analysis write refreshes the index in the same transaction. Every hit carries the snippet from the column it matched and names it in `matched_field`: `title`, `transcript`, `notes`, `analysis` or `speakers`, in that order when several match. `dettivo meetings search <words>` prints the hits.

## Export

`transcripts.export` over a meeting reference renders `txt`, `md`, `srt`, `vtt` or `json` into a download transfer; `dettivo meetings export <id> --format <f> --out <path>` pulls it into a file. Every format labels a segment with its speaker, or with `you` and `remote` while the speakers are unnamed; `txt` and `md` mark a gap in the capture before a segment (`[gap 2.3 s]`); `srt` and `vtt` carry one cue per segment with the speaker prefix. `md` is the document: the title, the facts, a Notes section, an Analysis section (summary, decisions, action items with owner and due) when one is ready, and the transcript. `json` carries the macOS keys: `id`, `title`, `started_at`, `ended_at`, `duration_ms`, `status`, `language`, `stt_provider_id`, `stt_model_id`, `transcript`, `segments` and `analysis`, plus `notes` and `speakers` when present.

Export chooses an explicitly supplied `notes_override` before checking its content. An empty or whitespace-only live draft suppresses saved notes; only an absent override falls back to them. The app passes its current draft, including a cleared editor. `raw = true` (`--raw`) renders the engine's words instead of polished segments. Speaker names reject control characters, and SRT/VTT flatten cue whitespace and escape markup while stored transcript text stays unchanged. The seeded roadmap meeting still matches `crates/dettivo-storage/tests/goldens/meeting.{txt,md,srt,vtt,json}`; an unknown format is `INVALID_PARAMS` listing the five (ADR 0055).

## Delete

`meetings.delete { meeting_id, artifact_policy }` removes what the policy names and nothing more:

| Policy | Goes | Stays |
|---|---|---|
| `transcript_only` | The texts, the segments (and speaker labels), summary, analysis, `analysis.json`, search entry, checkpoint segment text and any legacy metadata row copy | The row, notes and `notes.md`, take manifests, checkpoint timing, journal and capture-only `metadata.json` |
| `transcript_and_audio` | All of the above, every `.wav`, `takes.json` and `system-takes.json`; the row forgets `audio_dir`, `system_audio` and `microphone_takes` | The row's other facts (title, times, engine, language), the notes, the journal, `metadata.json` |
| `all` | The row and the directory | Nothing |

The active meeting is `CONFLICT` `sessionActive`; a meeting a job is finalising is `CONFLICT` `jobRunning`; a running analysis is cancelled first and its result dropped. A meeting whose audio is already gone deletes under every policy. The daemon applies `[meetings] delete_artifact_policy` (`all`) when the request names none, so `dettivo meetings delete <id> [--policy …]` sends no policy unless one is given; the app names the one its settings show. `transcripts.delete` on a meeting reference is `all`.

## The disclosure

`meetings.disclosure.get` answers the macOS notice, and `dettivo meetings disclosure --copy` puts it on the clipboard through `insert.perform` in clipboard-only mode, the same backend that copies a transcript (the QA mock sink under `DETTIVO_MOCK_INSERT`), so you can paste it into the call before you start. Under QA mode `DETTIVO_E2E_DISCLOSURE=acknowledged` seeds the acknowledgement in the profile's `state.toml` at start and `pending` clears it, so a drive shows the dialog on a fresh profile or skips it ([docs/qa.md](qa.md)).

## Recovery

The daemon's next start settles what a killed one left. Rows still recording or stopping become partial. Recovery reconciles take identities and offsets from readable sidecars, the checkpoint and the journal, repairs WAV headers, and measures through the latest take end including gaps. A missing checkpoint leaves chunk counts at zero and records its reason. Files with unknown timing stay on disk and finalization refuses them instead of overlapping every take at zero. Interrupted transcription becomes failed with retained input available for retry (ADR 0055).

`meetings.status` lists partial rows for recovery and failed, cancelled or stopped rows whose retained audio validates under `recoverable` (`ref`, `title`, `started_at`, `duration_ms`, chunk counts and reason). A restart during final transcription leaves a failed row that can use the same retry operation (ADR 0055):

- `meetings.recover { meeting_id }` validates the retained capture takes or imported audio, keeps the ID and notes, and starts finalization again. Its answer is `transcribing`; the row settles at `completed` only after the result is durable. Missing or damaged input is `CONFLICT` with `kind = audioNotRetained` and remains on disk.
- `meetings.discard { meeting_id }` removes the row and the directory.

`meetings.recover` refuses other states with `CONFLICT`, `kind = meetingNotPartial`; `meetings.discard` remains partial-only. The GUI exposes Recover from the server's recoverable list and Cancel transcription on a running row. `meetings.cancel` cancels captured finalization and imported jobs, including retries; notes edited during a retry remain on the row. The CLI exposes retry through `dettivo meetings recover <id>`. The recovery drives exercise restart, cancellation, retry and discard, and daemon tests cover both layouts and damaged input.

## In the history

Meetings live in the `meetings` table of the history store (migration `0004-meetings`, the speakers and the remembered names in `0006-speakers`, the notes and the analysis in `0007-notes-analysis`, the `queued` analysis status in `0008-analysis-queued`, [docs/history.md](history.md)) with the macOS `MeetingSession` fields, and `transcripts.list` with `kinds = ["dictation", "meeting"]` walks one timeline over both, newest first. `transcripts.get` on a meeting reference returns its transcript, segments (each with `source_type`, `start_ms`, `end_ms`, `text`, `polished_text`, `speaker`, `speaker_id` and `speaker_confidence` once the pass ran, `words` where the engine aligned them, and `gap_before_ms` where capture was missing), speakers and diarization block, and a `meeting` block with the notes, the analysis and the facts; `transcripts.search` and `meetings.search` find meetings by title, transcript, summary, speaker names, notes and analysis ([Search](#search)), `transcripts.latest { kind: "any" }` answers the newer of the two kinds, and `transcripts.export` renders a meeting as `txt`, `md`, `json`, `srt` or `vtt` ([Export](#export)); every format names the speaker (the side until one is assigned) of every segment, `txt` and `md` mark the gaps. `transcripts.import { target_kind: "meeting" }` runs an audio file through the chunked pipeline into a meeting row (source `audioImport`, its segments on the microphone source) after the same disclosure gate, and answers with `is_partial = false` and the running job as the contract states. A meeting re-run stays `NOT_IMPLEMENTED`.

## The event stream

| Topic | Payload |
|---|---|
| `meeting.state` | `kind = meeting`, `meeting_id`, `state` (`recording`, `stopping`, `stopped`, `transcribing`, `completed`, `cancelled`, `failed`, `partial`), `live_segment_count`, `live_last_end_ms`, `is_finalizing`, and the Linux fields `previous_state`, `reason`, `duration_ms`, `microphone_takes`, `system_audio`, `diarization_status` (`queued`, `running`, `ready`, `failed`, `unavailable`) on the `completed` transition and the transitions the speaker pass makes, and `analysis_status` (`queued`, `running`, `ready`, `failed`) on the `completed` transition and the transitions the analysis job makes |
| `meeting.segment` | `meeting_id`, `source` (`you`, `remote`), `segment_id` (`you-12` final, `you-p1` provisional), `provisional`, `start_ms`, `end_ms`, `text`, `words`, `gap_before_ms` when capture was missing before it; `<source>-p1` opens a fresh provisional tail for its source and drops the earlier one, a later `-pN` extends it, a final one is appended for good and retires the provisional fragments of its source that start before its end (fixture `crates/dettivo-proto/fixtures/events/meeting.segment.event.json`) |
| `job.progress` | `job_id` (the meeting's), `stage` (`transcribing`, `merging`, `done`), `chunks_done`, `chunks_total` while the finalisation runs; `job_diarize_<n>` with `stage = diarizing` and the engine's chunk counts while the speaker pass runs |
| `audio.level` | `rms`, `peak`, `source` (`microphone` or `system`) |

The pill shows Listening with the meeting's elapsed time while it records ([docs/osd.md](osd.md)).

## Testing without a microphone

`DETTIVO_MOCK_MIC=<file.wav>` feeds the microphone track and `DETTIVO_MOCK_SYSTEM_AUDIO=<file.wav>` the system track from fixtures at real time, through the same take writers, journal and checkpoint ([docs/qa.md](qa.md)); with only the first set the meeting is room audio, as on a machine without a sink. `dettivo-qa drive meeting_live` plays the jfk clip as the other side and its last phrase, after four seconds of silence, as the microphone, so the two overlap; it judges the live events (provisional then final per source, in time order), the `transcribing` stop, the progress per chunk and the finalised transcript against the two-source golden at `crates/dettivo-qa/fixtures/meeting-live/golden.txt` by word error rate per source ([docs/qa.md](qa.md)); CI runs it with tiny.en. On a machine with PipeWire, `dettivo-qa drive meeting_rig` loads two null sinks, pins the daemon to them, plays one fixture into each, unloads the microphone sink mid-meeting and judges the takes: each track correlates with its own fixture and not the other's, the first takes start within 200 ms of each other, the second microphone take carries the gap marker, the system track carried on, and the finalisation completes over every take. `dettivo-qa pack meetings` runs those and the rest of the lane in one order with the throughput report ([docs/qa.md](qa.md#the-meetings-pack)); its `meeting_token_coverage` scenario plays a two-voice fixture, Alice on the microphone and Ben on the system track, and requires every word of `crates/dettivo-qa/fixtures/meetings/alpha.tokens.json` on its own source after the finalisation and the speaker pass.

## In the app

`dettivo app open meetings` is the list by week beside the new-meeting rail, `meetings.live` the meeting that records with both meters and the live transcript, and `meetings.detail --id <uuid>` the transcript, the notes and the analysis with the processing strip while a pass is still to come, the talk-time bar, the title that renames the meeting, the rename popover, the export sheet and the delete; the import dialog maps onto `transcripts.import` and the disclosure dialog asks once ([docs/app.md](app.md#meetings), ADR 0038).

## Through the contract and the agents

The `meetings.*` methods are the contract's (`start`, `stop`, `cancel`, `status`, `list`, `get`, `search`, `delete`) plus the Linux additions `meetings.recover`, `meetings.discard`, `meetings.disclosure.get`, `meetings.disclosure.acknowledge`, `meetings.diarize`, `meetings.speakers.list`, `meetings.speakers.rename`, `meetings.speakers.suggest`, `meetings.notes.get`, `meetings.notes.set`, `meetings.analyze`, `meetings.analysis.get` and `meetings.rename` (the title, trimmed, at most 200 characters; ADR 0061), listed in `system.capabilities.meetings.methods` with `checkpoint_schema = 1` and registered in [docs/api/linux-deltas.md](api/linux-deltas.md) with fixtures under `crates/dettivo-proto/fixtures/meetings/`. The MCP tools `start_meeting`, `list_meetings`, `get_meeting` and `search_meetings` answer through them ([docs/mcp.md](mcp.md)), so `get_meeting` and the `meeting://{id}` resource carry the speakers, the notes and the analysis and `search_meetings` names the matched column; `stop_session` and `cancel_session` with a `meeting_id` reach the meeting. With `DETTIVO_E2E_SEED=1` the store holds four meetings across two weeks: the contract's sample (its `You` speaker from a finished pass), a completed roadmap review with two named speakers, notes and a ready analysis (the export goldens), a partial design call and a vendor intro with notes and no analysis.
