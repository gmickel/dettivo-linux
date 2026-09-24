# 0068. Parakeet Ultra is a catalogue option beside v2 and v3

Status: Accepted 2026-09-24, extends 0018 (the Parakeet catalogue)

## What this gives you

`dettivo speech download --provider parakeet --model parakeet-ultra` fetches a Parakeet model that has the same size and the same 25 languages as v3, with a lower word error rate on its publisher's benchmarks. It runs on the Parakeet engine you already have, with word timestamps, and can be picked in first run or in Settings / Models.

## Situation

On 2026-09-22 Moondream (M87 Labs) published `moondream/parakeet-ultra`, a post-train of NVIDIA's Parakeet TDT 0.6B v3 under CC-BY-4.0. Its model card reports a lower word error rate than v3 on every set it lists:

| Set | v3 | Ultra |
|---|---|---|
| Open ASR English, average of 7 sets | 6.26 | 5.80 |
| AMI | 10.86 | 9.77 |
| FLEURS, 25 languages | 11.62 | 9.55 |
| MUSAN noise | 6.72 | 5.82 |

These are Moondream's own measurements in its Photon runtime, and nobody outside Moondream has reproduced them. Moondream ships HF-transformers safetensors, not the `.nemo` checkpoint parakeet.cpp's converter reads. `trevest/parakeet-ultra-GGUF` (CC-BY-4.0, 2026-09-23) converted it for parakeet.cpp by inverting the transformers tensor mapping.

The q8_0 file is 940,663,680 bytes, the same as v3's, with the same tensor count. Our engine loads it at the pinned parakeet.cpp 0.5.0 and reads the jfk fixture word for word with word error 0.000, on the CPU and on Vulkan (RTX 4090, the installed 0.1.0 engine, about 0.6 s including the load, as fast as v3). Its word timestamps match v3's within one 80 ms frame.

## Decision

The catalogue gains `parakeet-ultra`: the trevest q8_0 GGUF at its Hugging Face URL, pinned by SHA-256 `665168ed…a379`, licence CC-BY-4.0, with a redistribution note crediting Moondream, NVIDIA and the converter. `parakeet-v3` stays the default. The engine accepts v3's 25 languages for a model whose GGUF `general.name` is `moondream/parakeet-ultra`, and the WER fixture gains a CPU and a Vulkan row for it.

Moondream's companion `parakeet-redux` stays out. Its only advantage is compressed weights, which need Moondream's own runtime and would be expanded back to full size for parakeet.cpp, and its reported English and noise error rates are worse than v3's.

## Consequences

The file comes from one community uploader, and the SHA-256 pin guarantees only that every download matches the file we tested. If that source disappears or changes, the entry has to move to a conversion we reproduce and host ourselves. Ultra's long-form gains in Moondream's numbers partly come from Photon's pause segmentation, which our fixed windows do not use, so they may not carry over. The meeting lane is unchanged, and Ultra is a dictation option like v2 and v3 (ADR 0018).
