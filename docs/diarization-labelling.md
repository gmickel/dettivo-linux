# Labelling meetings for the diarization bench

`just diar-label DE-2` opens a page on your own machine where you say who spoke each transcript line of a retained meeting, mostly by pressing Enter. When every line is decided, the kit writes a labels file that [the diarization bench](diarization-bench.md) scores on its next run, so `just diar-bench` shows a real word-level speaker error (WDER) and headline number for Dettivo's own English and German meetings instead of the local/remote proxy. The page is built for about 20 minutes of work per 15-minute excerpt, and that figure is a target nobody has timed yet. The goal is a set of 15 to 30 excerpts, 2 to 4 hours in total, across both languages.

## Start a meeting

1. Run `just diar-label` with no alias to list the bench's meetings (`EN-1`, `DE-2`, and so on) with their length and whether they are labelled or in progress.
2. Run `just diar-label <alias>`. The kit picks the 15-minute window with the most speaker changes, so each minute of labelling buys the most signal, and opens the page in your browser. `--minutes 10` to `--minutes 20` changes the length, and `--whole` labels the entire meeting.
3. Label, then stop the server with Ctrl-C. The page saves after every key, so you can stop at any point, and the same command resumes where you left off.

A meeting the kit has started keeps its excerpt and its draft. To start that meeting over, delete `<eval>/labelling/<alias>.json`.

## The keys

| Key | What it does |
|---|---|
| Enter | Confirm the line's speaker and move on |
| 1 to 9 | Give the line that speaker (the legend shows the numbers) and move on |
| 0 or x | Mark the line unknown and move on |
| n | Give the line a new speaker and move on |
| / | Type a speaker's letter or name, which also creates a named speaker |
| r | Rename the line's speaker, for example A to Anna |
| Space | Play the line again |
| j and k, or the arrows | Move without deciding |
| Tab | Jump to the next line nobody has decided |
| Backspace | Reopen a decided line |
| a, f, o | Autoplay on or off, 1x or 1.5x speed, disagreements-first or time order |

Autoplay plays each line as you reach it, from its own track with a quarter second either side. The strip at the bottom shows the two lines before and after the current one in time order, which helps on short lines.

## The drafts

The kit prefills speakers from every engine's cached output in the bench cache, the current engine and Nemotron today. Correcting one system's output biases a reference towards that system, so a draft never comes from one engine alone. For a two-track meeting the drafts come from the engines' runs on the summed tracks, which puts both sides of the call in one speaker space.

Each line shows one of three states.

- **Both engines agree.** The line carries their speaker as a dashed draft, and Enter confirms it.
- **The engines disagree.** The line shows both proposals as "A or B", without saying which engine proposed which, and has no draft until you choose.
- **Decided.** The speaker is solid, or the line says unknown.

The page lists every disagreement first, then the lines both engines agree on. Listen to each disagreement before you decide, and glance at each agreement before you confirm it, since two engines can make the same mistake.

When fewer than two engines have cached outputs for the meeting, the kit refuses a draft and names `just diar-bench --full`, which computes them. `--draft blank` starts from no draft at all.

The page header, the progress file and the labels file all record the draft, as `blend:current+nemotron` or `blank`.

## Who said a line

Label the person whose words make up most of the line. The bench counts each labelled line's words against its speaker, so a line split between two people costs the minority speaker's words either way.

- **Overlap.** When two people talk at once, choose the one whose words the transcript line holds. If the transcript merged both into one line, choose whoever says more of it.
- **Backchannels.** A "yeah" or "mhm" that has its own line belongs to whoever said it. A backchannel buried inside someone else's line does not change that line's speaker.
- **Unknown.** Press 0 when you cannot tell who spoke after listening, or when the line is noise, music or a recording. The bench leaves unknown lines out of every figure.
- **The microphone side.** Microphone lines are usually you, but label anyone else in the room as their own speaker.
- **Names or letters.** Either works. The bench matches speakers by the best one-to-one map, so only consistency within a meeting matters. Two letters renamed to the same name become one speaker, which fixes a person the engines split in two.
- **Missed speech.** The kit labels the transcript's lines only. Speech with no line stays out of the headline, since no speaker choice fixes it.

## Where the labels go

Everything stays on the machine. The page is served on 127.0.0.1 behind a random path that changes every run, rejects requests addressed to any other host name, and loads nothing from the network. The progress file sits at `<eval>/labelling/<alias>.json` and the finished labels at `<eval>/labels/<alias>.json`, both readable by you only, inside the protected eval directory (`~/.local/share/dettivo-eval/bench`, or `$DETTIVO_EVAL_DIR`). None of it enters git.

The labels file is the bench's format ([Labels](diarization-bench.md#labels)), with `window_ms` set to the excerpt. The kit writes it only once every line in the excerpt is decided, so a half-labelled meeting never reaches the bench. After a change to a decided line it rewrites the file. The next `just diar-bench` reports the meeting under `labelled-en` or `labelled-de`.
