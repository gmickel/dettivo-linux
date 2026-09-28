# 0079. A release builds each ggml engine in its own job

Status: Accepted 2026-09-29, amends [0070](0070-a-release-is-local-qa-and-a-tag.md)

## What this gives you

A tagged release builds the Whisper, Parakeet, language-model and Nemotron engines in four CI jobs side by side, runs the test-and-lint gate in a fifth, and assembles the package from the four engines. The release waits for its slowest job instead of the sum of them, and an engine that lost a shader fails its own build instead of the engine at start-up.

## Situation

Each of the four engines carries its own copy of ggml, and each copy compiles about two thousand Vulkan shaders into a generated source file. The 0.4.0 release built all four copies in one `cargo build` on one four-core runner, after `just toolchain test lint` in the same job, which took about 80 minutes. That build lost the subgroup cooperative-matrix `f16acc` matrix-multiply shaders of one ggml copy every time: Nemotron's in two release runs, Parakeet's in a diagnostic run. Every `glslc` compile succeeded and wrote its SPIR-V; the shaders went missing between the compiled files and the generated source. The same library built alone in the same container, and every build on the 32-thread development machine, had all of them.

ggml's generator skips a shader it cannot embed and exits successfully. The static engines then fail to link, but Nemotron's shared library linked with the symbols undefined, and the engine failed only when the release install test started it.

## Decision

- `scripts/package.sh --engine <name>` (`just package-engine <name>`) builds one ggml engine on Vulkan into `dist/engines/`, with the libraries it loads, and starts it once with `--help`.
- `DETTIVO_PREBUILT_ENGINES=<dir> just package-bin` builds the rest of the workspace without the four ggml engines and takes them from `<dir>`. Without the variable, `just package` builds everything as before, so a local package needs nothing new.
- The rig's `engines` job runs `just package-engine` once per engine as a matrix, each with its own build cache. `package` needs those four and downloads them. The `gate` job runs `just toolchain test lint` beside them, and `publish` still needs the whole rig.
- NeMo-Speech.cpp's shared libraries link with `-Wl,--no-undefined`, so a missing shader fails the build.

## Consequences

- A release uses six runners at once instead of two. Public repositories pay nothing for standard runners.
- The root cause inside ggml's build is not pinned down. One ggml copy per build is the configuration that has never lost a shader, and `--no-undefined` plus the static engines' link turn a recurrence into a build failure.
- The advisory `whisper-vulkan`, `parakeet-vulkan` and `llm-vulkan` jobs of the manual rig run stay as they are.
