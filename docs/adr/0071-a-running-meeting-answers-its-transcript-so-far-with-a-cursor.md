# 0071. A running meeting answers its transcript so far, with a cursor

Status: Accepted 2026-09-26

## What this gives you

An agent that attaches to a meeting late reads everything said so far in one call, and then reads only what is new every minute or two. `meetings.segments { meeting_id, since? }` answers the live finals in order, the provisional tail flagged, and a cursor. The same cursor keeps working through the stop and the finalisation, and one `reset` marks the moment the stored transcript replaces the live one.

## Situation

Before this record, every request-style read returned the stored row. The row's segments stay empty until the stop, because the worker copies the live finals onto it only at `stopped` (`crates/dettivo-meeting/src/worker_end.rs`). The live tail (`live::Tail`) lived only in the capture worker. The only live source was the `meeting.segment` event stream, which starts at subscription and has no replay (`crates/dettivod/src/events.rs`). MCP and REST serve no events. The checkpoint holds finals only and is written every 15 seconds.

A live-meeting coaching skill polls every one to two minutes and may attach after the meeting started. It had to tell Linux users that history before attaching is not available until the meeting stops. On macOS the same skill reads `live-checkpoint.json`, which the app rewrites about a second after each change with finals and provisional text. That file's format and interval stay as they are here.

## Decision

- **The session holds the tail readable.** `Session::start` registers the meeting's `Arc<Mutex<Tail>>` in a map by meeting id before the worker runs. The live thread folds its segments into that tail as before. The map entry stays through `stopping`, `stopped` and `transcribing`, and the session removes it only after the finalisation has stored the settled row (completed, failed, or stopped after a cancelled finalisation). A capture that ends cancelled or failed removes it once that row is stored.
- **A read copies and never writes.** `Session::read_segments` holds the tail's lock only to copy the finals after the cursor and the provisional tail. It touches no file, no row and no channel of the capture. Without a registered tail it answers the stored row's segments, the ones `meetings.get` carries.
- **The cursor is `<transcript>:<finals>`.** `live:<n>` counts the live finals in the order they became final, which is append-only. `stored:<n>` counts the stored segments. `since` returns the finals from position `n` on, plus the current provisional tail whole. A cursor from the other transcript, or one past the end, answers the whole transcript with `reset = true`. A client replaces what it holds on a reset.
- **Every segment names its side twice.** Each segment carries `source` (`you`, `remote`, as the `meeting.segment` event spells it) and the contract's `source_type` (`microphone`, `system`, `merged`). Live finals keep the event's `segment_id` (`you-12`), provisional segments keep theirs (`you-p1`), and stored segments are `stored-<index>`.
- **`meetings.get` keeps its contract meaning.** The method is a Linux addition, listed in `system.capabilities.meetings.methods`.
- **`--follow` subscribes before it reads.** `dettivo meetings segments --follow` opens the event subscription, reads the backlog, then drops every streamed final whose `segment_id` the backlog printed and every provisional event identical to a backlog provisional.

## Consequences

- The retained tail costs memory until the finalisation ends. A two-hour meeting with a final every three seconds on each side holds about 4,800 segments. The unit test in `crates/dettivo-meeting/src/transcript.rs` reads such a tail whole in well under the 100 ms bound.
- The live and the stored transcripts are different segmentations of the same audio, so a poller sees exactly one reset per meeting that finalises. A daemon restart drops the tail, and the next read resets to the stored row.
- `status` reads the session's own state while the meeting is active, so a read can answer `stopping` before the row says so.
- `crates/dettivo-meeting/tests/live_read.rs` polls from recording to the completed row and checks that no live final is skipped or repeated. It also runs thousands of reads during a recording and checks that the takes, the checkpoint and the journal stay unchanged. `crates/dettivod/tests/meetings_segments.rs` does the same against a live daemon with a late `--follow --json` follower.
- MCP gains `get_meeting_segments`, hidden with the other meeting tools while no meeting export exists. REST reaches the method through the generic mapping as `/v1/meetings/segments`.
