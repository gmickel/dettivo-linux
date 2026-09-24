---
name: Dettivo for Linux
last_updated: 2026-09-03
generator: flow-next-strategy
---

# Dettivo for Linux Strategy

## Target problem

Linux developers and privacy-sensitive knowledge workers still cannot rely on speech as a fast, private, universal input layer. The only local option stops at push-to-talk text; anything that also covers meetings, recall and agents means uploading audio to someone's cloud, so speech stays a side utility instead of desktop infrastructure.

## Our approach

We win by making speech local infrastructure rather than a cloud app: a resident daemon with on-device engines that compositor keys, a native Qt app, the Omarchy bar, a CLI and MCP all drive through the same contract Dettivo already ships on macOS and Windows. Cloud transcription, meeting bots and web views are off the table, so privacy, latency and agent access are guaranteed by construction rather than promised.

## Who it's for

**Primary:** Omarchy and Hyprland developers who dictate into editors, terminals, PR bodies and coding agents and want meeting notes without a bot — they're hiring Dettivo to be a reliable, private keyboard and memory across the apps and agents they already use.

**Secondary:** AI-native operators who need transcripts and meetings reachable from Claude Code, Codex and Cursor; privacy-required professionals on GNOME or KDE who need local meeting capture and recall with no third-party processor.

## Key metrics

- **Time to first insert** — hotkey release to text visible in the focused app, p50 per engine tier; measured by the QA rig on Thor and on a CPU-only VM.
- **Local success rate** — share of dictations and meetings completed without failed insertion, device loss or manual recovery; measured from local event logs and soak runs.
- **Long-audio throughput** — meeting transcription plus diarization speed versus audio duration, per hardware tier; measured by engine benchmark fixtures.
- **Agent surface coverage** — share of contract methods exposed through CLI and MCP with passing conformance fixtures; measured in CI.

## Tracks

### Contract parity and agent surfaces

Implement the Dettivo IPC v1, CLI, MCP and REST contracts on Linux with honest capability flags, a documented delta list and a fixture suite that could run against any port.

_Why it serves the approach:_ One contract across three platforms is what makes agents and skills work everywhere; parity is the reason the port is worth doing.

### Omarchy-native, beautiful by default

Compositor hotkeys, in-process Wayland insertion, PipeWire capture, the bar widget and panel, and a design system that resolves every colour, size and spacing from the active Omarchy theme, held to approved baselines.

_Why it serves the approach:_ Infrastructure has to feel native to the desktop it lives in; on Omarchy that means Qt, theme tokens and the bar, and the same tokens keep GNOME and KDE working.

### Local engines and performance

One ggml family (whisper.cpp, parakeet.cpp, llama.cpp) in isolated engine processes with Vulkan by default, model catalogue with checksums, and benchmarks per hardware class.

_Why it serves the approach:_ Local only wins if it is faster and more reliable than cloud tools on the GPUs people actually have, including AMD.

### Complete speech workflows, proven by drives

Dictation modes, meetings with diarization and notes, history and exports as one loop, each with an automated evidence route: contract fixtures, the virtual audio rig and cua-driver desktop drives.

_Why it serves the approach:_ Users should not choose between good dictation and good meetings, and a workflow without a drive that proves it is not done.

## Not working on

- Cloud speech-to-text or a cloud language model as a default path; any remote endpoint stays explicit opt-in.
- Flatpak or Snap packaging in v1; the sandbox fights the virtual keyboard, uinput and the Unix socket contract.
- The macOS extras in v1: automation providers, email delivery, knowledge Q&A, meeting templates and retention policies stay reserved in the contract.
