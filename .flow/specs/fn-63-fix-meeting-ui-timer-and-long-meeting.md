# Fix meeting live fragments and stop progress feedback

Meeting dogfooding exposed transient missing text and a stop transition that looked unresponsive. Preserve the recorded session while addressing these UI behaviors in a later implementation.

## Evidence from 2026-09-16 dogfood

The daemon emitted multiple provisional fragments for a source. LiveSegmentsModel::apply removes the existing provisional row by source before appending each new fragment, leaving only the latest visible. In an early sample, all 74 mature overwritten fragments were found intact in settled live segments. This proves a display defect for those cases, not whole-meeting transcription accuracy.

Gordon pressed Stop, saw the confirmation with two choices, clicked Stop, perceived no effect, and then could no longer press it. Read-only inspection confirmed recording had already ended and final transcription was at 18/22 chunks. It subsequently completed 22/22 and the app moved to meetings.detail. Speaker identification then ran. The journal recorded one stop and successful finalization, with no live_skip or live_error events. Both source audio files remained present. The disabled Stop control while finishing is intentional; improve the clarity of the accepted stop and processing transition rather than enabling duplicate stops. The exact reason Gordon missed the existing progress feedback needs a visual reproduction.

The saved comparison counted 9,812 settled live words versus 10,163 final words, normalized by source. Recognition wording differs beyond the temporary fragment replacement. These counts are differences between recognizer outputs, not an accuracy score against audio. Confidential transcript text and audio stay in the user's private application data and protected runtime comparison directory, never this spec.

## Post-processing completion must be unmistakable

Gordon confirmed that speaker identification finished successfully, but its subtle running label was easy to miss. Users may see the saved transcript and conclude the meeting is fully processed before speaker identification has even started. The interface must distinguish transcript availability from completion of all enabled post-processing.

Show the actual stages from recording stopped through transcription, speaker identification and analysis, including pending, running, complete, failed and skipped states as applicable. Keep the transcript usable while later stages run. Display progress prominently, using an indeterminate indicator when totals are unavailable. Show overall completion only when all enabled stages have reached a terminal state, and distinguish successful completion from completion with failed stages. Never leave a visually finished gap between queued stages.

## Rename from the meeting title

Gordon wants to rename a meeting by clicking its title at the top of the meeting view, with a keyboard shortcut for the same action. Make the title an accessible inline rename control. Prepopulate the current title, save with Enter, and cancel with Escape without changing the saved name. Persist the new title and update it consistently in the header and meeting list; show a failed save without losing the edit.

Choose a shortcut consistent with existing app bindings and verify it does not conflict. Show it in the keyboard hints and keep ordinary typing in notes or other text fields unaffected. The exact key remains an implementation choice, not an agreed binding.

## Acceptance for future work

- Clicking the meeting title opens inline renaming; a discoverable, conflict-free keyboard shortcut invokes the same control.
- Verify Enter saves, Escape cancels, persistence survives reopening, and the meeting list reflects the saved title.
- Preserve every provisional fragment in the daemon's current tail per source and retire stale ones correctly.
- Show immediate acknowledgement of Stop and obvious finalization progress, with no ambiguity about whether recording continues.
- Navigate to the saved transcript when it is ready, while prominently showing pending or running speaker identification and analysis.
- Verify users can distinguish transcript ready from all post-processing complete, including the interval before speaker identification starts and any analysis failure.
- Verify stage progress and overall outcome remain clear without relying on the subtle speaker-pass caption.
- Add focused regression coverage for multiple provisional fragments and stop-state transitions.
- Compare representative live/final discrepancies against retained audio before making claims about missing speech.

## Post-meeting verification findings

Read-only checks after completion verified both retained mono 16 kHz source tracks are 3,154.771625 seconds long. The journal contains a normal stop and successful finalization, without live_skip, live_error or capture-gap events. Speaker assignment preserved all 948 final segment texts and timestamps exactly.

Speaker assignment produced You plus two remote speaker clusters. All 446 microphone segments were labeled You. Of 502 system-track segments, 302 were assigned Speaker 1, 46 Speaker 2, and 154 remained unlabeled (700 of 4,493 system-track words). The reported 43.8 percent coverage means speaker-turn coverage of the entire system track, including silence; it is not a diarization accuracy score. Actual remote participant count and speaker identity accuracy remain unconfirmed.

Four selected live/final discrepancies were re-decoded locally from retained audio with the same Whisper model and automatic language detection, without changing the saved meeting. Recognition varies with window context and language. A suspicious 20-word final-only span around 12:28-13:08 was not reproduced in its short-window decode, which contained only acknowledgements. This is a targeted listening candidate, not proof of hallucination or a ground-truth accuracy measurement. Keep speech-content evidence in protected local storage.

Automatic analysis failed with invalid_output. Both the first answer and its repair pass contained no JSON object usable by the parser. No summary or action items were produced. Investigate separately from the successful transcript and speaker jobs, and show that failure clearly.

The daemon also logged two PipeWire wrong-context stream-destruction warnings at stop. Capture and finalization still completed; preserve this as a teardown investigation lead rather than claiming audio loss.

## Boundary

Gordon authorized implementation through flow-next-flow after the original recording and post-processing finished. Deliver a locally running hotfix with no full release. Verify the main daemon is idle before replacement; preserve the original meeting. Timer tracking remains in fn-62, whose final presentation pass also corrects the clipped unassigned-count caption found during native QA.





## Implementation verification

The patched daemon analysed a private isolated copy of the retained meeting in about 19 seconds, with 8 calls and 6 parts under the existing 4096-token context and 1024-token output limit. This verifies pipeline completion and output structure, not the factual accuracy of every generated item. Original meeting content and both audio tracks remained unchanged. Native title click/T, Enter save, Escape cancel, and reopening persistence passed on the isolated copy. The complete just build test lint gate passed, including all Rust tests and 81 Qt checks. The standalone user UI bundle, daemon and reviewed Omarchy plugin files are installed locally. Analysis of the original saved meeting now reaches ready with 8 calls across 6 parts. Title, raw/final transcript, 948 segments, speaker assignments, notes, both audio tracks and config passed unchanged-content checks. The package remains 0.1.0-1.58; no release was built or published. Hosted CI workflows were manually disabled when checked; verification was local.


## Goal and context

Resolve the observed meeting dogfood failures in one locally running hotfix. The branch includes fn-61 independent Whisper meeting selection while Parakeet remains dictation, fn-63 live text/processing states/title renaming and bounded long-meeting analysis, and fn-62 elapsed-clock and presentation corrections. The final repository gate and native checks are recorded in task evidence.

## Open questions

- Do the two detected remote voice clusters match the actual participants? A labeled speaker reference has not been supplied.
- Does the suspicious quiet-span recognition reflect spoken audio? Targeted human listening is still needed; recognizer tuning was not changed.

## Other observation

The shell logged one ListRow width-binding warning during plugin reload. No corresponding visible failure was reproduced and this shared installed component was not changed. Keep it separate from the corrected timer and meeting detail layout.
