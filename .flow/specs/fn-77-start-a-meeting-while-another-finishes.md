# Start a meeting while another finishes, and go back to the GPU once it has room

## Goal & Context

On 2026-09-30 a meeting started at 12:59, when the GPU was out of memory: Whisper's first allocation failed with `vk::Device::allocateMemory: ErrorOutOfDeviceMemory` (read from the core dump), the engine aborted, and the supervisor moved it to the CPU as ADR 0080 intends. Live transcription kept working. Three things then went wrong.

1. **The engine never went back to the GPU.** The CPU fallback lasts until the engine is unloaded after 5 idle minutes (`supervisor.rs`: `cpu_fallback` is cleared only on unload or a changed load). Back-to-back meetings kept Whisper busy, so it ran on the CPU for more than two hours with the GPU mostly free (3.7 of 24.5 GB in use at 14:50). Each finalisation took about as long as the meeting itself (a 38-minute meeting took 30 minutes; the next one was still at 80 of 165 chunks well after it stopped), where the GPU takes a few minutes.
2. **Start meeting did nothing while a meeting finalised.** The daemon frees the recording slot before finalisation (`worker_end.rs`), so a new meeting may start. The app's `MeetingLiveModel` keeps the finishing meeting's id until the meeting completes, `active` stays true, and `NewMeetingRail.start()` then only reopens the live view of the finishing meeting. For half an hour the user could not start the next meeting.
3. **The crash warning hid its reason.** The daemon log showed the two `ggml_vulkan:` lines redacted and dropped the C++ runtime's `terminate called after throwing an instance of …` / `what(): …` lines, which carry only library text such as `vk::Device::allocateMemory: ErrorOutOfDeviceMemory`. Telling a full GPU from any other Vulkan failure took a core dump.

This spec lets the next meeting start at any time, returns an engine to the GPU once the GPU has room again, and keeps the error name in the crash warning.

## Acceptance

- **R1:** Start meeting starts a new meeting whenever the daemon has no meeting recording or stopping, including while another meeting is being transcribed, analysed or assigned speakers. It reopens the live view only for a meeting that is recording or stopping.
- **R2:** The finishing meeting keeps its progress on its own row and in its meeting view while the new one records; its completion or failure never resets or disturbs the new live meeting in `MeetingLiveModel`.
- **R3:** An engine running on the CPU because of a GPU crash or an `OutOfDeviceMemory` error is tried on the GPU again: at its next request once a hold has passed since the fallback (start at 2 minutes). If the GPU attempt fails again the same way, it falls back to the CPU for that request as today and the hold doubles, up to 30 minutes. A successful GPU load clears the hold. An unload still clears everything.
- **R4:** The GPU retry happens only between requests, never inside one, so a live window, a chunk or an analysis never pays for a failed retry beyond the one CPU re-run ADR 0080 already allows.
- **R5:** The crash warning keeps the C++ runtime's `terminate called after throwing an instance of '<type>'` line and its `what():` line when the text is a library error (a `vk::` or ggml message), matched by format like the other diagnostic lines; transcript text never reaches the log.
- **R6:** Tests:
  - C++ against the fake link: Start during another meeting's `transcribing` state calls `meetings.start`, and the finishing meeting's `completed` event leaves the new live meeting intact.
  - Supervisor with the fake engine's `gpu-oom-on-request` and `gpu-oom-error-on-request` modes: the engine returns to the GPU after the hold, the hold doubles after a second failure, and a request that hits a failed retry is still answered on the CPU.
  - `process_diagnostics`: the terminate and `what()` lines of a Vulkan exception survive, and an ordinary stderr line does not.
- **R7:** Live check on the desktop, with the VRAM tool (`scratchpad/vram_hog`) filling the GPU and then freeing it: a recording meeting moves to the CPU, and returns to the GPU (log `engine loaded … backend=Vulkan`) within one hold after the memory is freed. Its finalisation runs on the GPU. Warn Gordon before filling the GPU.
- **R8:** ADR 0080 records the retry rule; `CHANGELOG.md` gets a 0.4.3 entry.

## Boundaries

- No GPU memory probing or VRAM budget API: the retry is a plain attempt with a backoff.
- No new config key; the hold and its cap are constants.
- No change to the daemon's one-recording-at-a-time rule.
- No app UI for "running on the CPU" in this spec.

## Decision Context

Reported by Gordon on 2026-09-30 after he couldn't start a meeting at 14:07 and the next meeting was still transcribing slowly with the GPU free. What filled the GPU at 12:59 is unknown: no other Dettivo engine was loaded, and nothing records VRAM history. A Whisper engine started from a terminal also crashed at 12:57, with no Vulkan error in its dump, and no agent session ran then.
