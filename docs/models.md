# Models

Dettivo knows which speech, language and diarization models exist, which are on disk and verified, and which one is selected, from one source the CLI, the GUI and the bar all read. Every download is verified against a checksum pinned in the catalogue, resumes after an interruption, reports progress and can be cancelled; a file that fails verification is moved aside, never used. A machine provisioned with a `config.toml` and a populated model directory needs no download at all.

## Picking and fetching a model

```
dettivo speech providers                      # providers, models, readiness
dettivo speech download --model large-v3-turbo --wait
dettivo speech download --provider parakeet --model parakeet-v3 --wait
dettivo speech selection set --model large-v3-turbo
dettivo speech selection set --provider parakeet   # switches to parakeet_model_id
dettivo speech selection set --meeting-model small
dettivo speech status                          # readiness per model
dettivo speech delete --model tiny.en          # the selected model needs --force
dettivo speech download --provider diarize --model diarization-en --wait   # the speaker pass's model set (ADR 0035)
```

`selection set` validates against the catalogue, writes `[speech]` in `config.toml` with your comments intact, and preloads the new model when its file exists so the next session starts warm. It never downloads by itself: a model that is not on disk stays `missing` until `download` fetches it.

The language models behind Enhanced go the same way under `dettivo llm` (ADR 0026):

```
dettivo llm status                                   # the four models and their readiness
dettivo llm download --model qwen3-4b-instruct-2507 --wait
dettivo llm delete --model qwen3-1.7b                # the selected model needs --force
dettivo config set llm.model qwen3-1.7b              # the selection is [llm] model
dettivo llm engine                                   # the engine process, its backend and memory
```

| Model | For | File | Size |
|---|---|---|---|
| `qwen3-4b-instruct-2507` (default) | Enhanced and meeting analysis | `Qwen3-4B-Instruct-2507-Q4_K_M.gguf` | 2.5 GB |
| `qwen3-1.7b` | Enhanced, the fast option | `Qwen3-1.7B-Q4_K_M.gguf` | 1.1 GB |
| `qwen3-4b` | Enhanced, the quality option | `Qwen3-4B-Q4_K_M.gguf` | 2.5 GB |
| `qwen3-8b` | Meeting analysis | `Qwen3-8B-Q4_K_M.gguf` | 5.0 GB |

They are the macOS catalogue in GGUF at the 4-bit footprint (Q4_K_M from the unsloth conversions of the Qwen releases, Apache-2.0), and each entry names its `roles` (`polish`, `analysis`) so a client can offer them by purpose. `[llm] model` selects the Enhanced model and `[llm] analysis_model` the analysis one (empty means the same). Once the selected model is on disk the `local` provider is what `[llm] provider = "auto"` picks first ([docs/polish.md](polish.md)).

## The catalogue

The catalogue is a versioned manifest compiled into the daemon (`crates/dettivo-speech/catalogue/v1.toml`) with one entry per model: provider, id, display name, kind (`stt`, `vad`, `llm`, `diarization`), size, source URL, SHA-256, license and redistribution note. A model set lists further `files` beside the entry's own, each with its source, checksum, size and license, and a file that arrives as an archive names the member to `unpack` (`tar.bz2`) with the member's own checksum; the diarization set `diarize/diarization-en` is pyannote segmentation 3.0 (MIT, unpacked from the k2-fsa archive to `segmentation.onnx`) and English VoxCeleb ERes2Net (Apache-2.0, `embedding.onnx`), 33.4 MB in all, which `dettivo-engine-diarize` loads as a directory. Model ids are the macOS and Windows aliases (`tiny.en`, `small`, `large-v3-turbo`), so a selection is portable across the ports. Whisper models come from the whisper.cpp release set on Hugging Face (`large-v3-turbo` is the `q8_0` file), the VAD model from `ggml-org/whisper-vad`, and the Parakeet models (`parakeet-v2`, English; `parakeet-v3`, 25 European languages) are parakeet.cpp's `q8_0` GGUF conversions from `mudler/parakeet-cpp-gguf`, which keep word error rate parity with the NeMo release ([docs/engines.md](engines.md)). A test keeps every entry complete and every source on an allowlisted host.

The Parakeet provider offers three models, all CC-BY-4.0 and all 0.6B TDT models that return a time for every word. `parakeet-v2` is NVIDIA's English model and `parakeet-v3` NVIDIA's 25-language model, the default. `parakeet-ultra` is [Moondream's post-train of v3](https://huggingface.co/moondream/parakeet-ultra), with the same languages and size and a lower word error rate on Moondream's own benchmarks, converted for parakeet.cpp by [trevest/parakeet-ultra-GGUF](https://huggingface.co/trevest/parakeet-ultra-GGUF). `dettivo speech download --provider parakeet --model parakeet-ultra` fetches it, and `dettivo speech selection set --provider parakeet --model parakeet-ultra` makes it the dictation model ([ADR 0068](adr/0068-parakeet-ultra-is-a-catalogue-option.md)).

A private polish fine-tune is never a catalogue entry: it loads from a sideload manifest under `<models>/polish-experiments/` when `[llm] polish_experiment` names one, shows up in `llm.models.status` with `source = "sideload"`, and is never offered for download ([docs/polish-models.md](polish-models.md)).

`[models] catalogue_file` points the daemon at another catalogue TOML, for mirrors and tests; loopback HTTP is allowed there, everything else must be HTTPS on an allowlisted host.

## On disk

```
$XDG_DATA_HOME/dettivo/models/
  whisper/large-v3-turbo/ggml-large-v3-turbo.bin
  whisper/large-v3-turbo/manifest.json
  parakeet/parakeet-v3/tdt-0.6b-v3-q8_0.gguf
  parakeet/parakeet-v3/manifest.json
  llm/qwen3-4b-instruct-2507/Qwen3-4B-Instruct-2507-Q4_K_M.gguf
  llm/qwen3-4b-instruct-2507/manifest.json
  diarize/diarization-en/segmentation.onnx        # a model set: every file in one directory
  diarize/diarization-en/embedding.onnx
  diarize/diarization-en/manifest.json
  whisper/tiny.en/ggml-tiny.en.bin.part        # a paused or interrupted download
  whisper/tiny.en/ggml-tiny.en.bin.part.json   # what it was fetching
  quarantine/whisper/base/1756930000/           # a file that failed verification
```

`manifest.json` records the checksum the file was verified against, its size, when it landed, the catalogue version and `verified`. Readiness is derived from these files: `ready` (present and verified), `unverified` (present, not hashed yet), `downloading`, `partial` (a `.part` waits for a resume), `missing`, `quarantined`. A model set is `ready` when every file is present and verified and `partial` with the bytes on disk while some are; one corrupt file quarantines the set.

A verified manifest names every final file of the set with the checksum it hashed to and the size and time it had then; a catalogue that changes a checksum, a file replaced in place, or a manifest written before the identities were recorded makes the model `unverified` again. No engine opens a catalogue model that is not `ready`: with `[models] verify_on_start` (the default) the daemon re-hashes every unverified model when it starts, off the request path, and the start-up preload waits for it; without it, or for a model that turns unverified later, the first load hashes the file first (a session start, a re-run, an import, the speaker pass and the local language model all pass the same gate) and a mismatch is quarantined and refused by name instead of loaded. A quarantined model downloads afresh on the next `download`. A private sideload (`[llm] polish_experiment`) is not a catalogue model and passes as it is.

## Downloads

A download runs on its own thread, one per model, bounded by `[models] max_concurrent_downloads`; further requests queue. It writes `<file>.part`, asks the server for `Range: bytes=<n>-` when a partial file for the same URL exists, falls back to the start when the server ignores ranges, and reports progress at most four times a second. When the last byte lands the file is hashed: a match renames it into place and writes the manifest; a mismatch moves it to `quarantine/` and reports `quarantined`. Cancelling keeps the partial file for a later resume. A model set downloads its files one after the other under one progress (`bytes_total` is the whole set), skips a file already on disk and verified, unpacks an archive's member and verifies the member too, and writes the manifest once every file landed.

`dettivo speech download --wait` prints progress lines until the download ends and exits 1 if it did not land `ready`. `dettivo speech status --follow` prints every model's readiness once a second while a download runs. The `model.download` event (provider, model, state, bytes done, bytes total, error) is published on the event stream four times a second while a download runs and once when it ends, which is what the first-run Models step and the app's engines rail follow; the daemon also logs each download that ended.

## Methods

`speech.providers.list`, `speech.selection.get` and `speech.selection.set` answer with the shapes the Windows port adopted. The Linux additions `speech.models.status`, `speech.models.download`, `speech.models.cancel` and `speech.models.delete` are recorded in [docs/api/linux-deltas.md](api/linux-deltas.md) with fixtures under `crates/dettivo-proto/fixtures/speech/`. Every row of `speech.models.status` carries the catalogue's `languages` and, on the three models first run offers (`parakeet-v3`, `large-v3-turbo`, `small.en`), `recommended_for`, the one-line recommendation the Models step shows beside the size ([docs/app.md](app.md)); a model without one is not offered there. `speech.models.delete` refuses the selected model with `CONFLICT` unless `force` is set; an unknown model is `INVALID_PARAMS` naming it. The speech methods know the speech providers and the diarization model set (`provider = "diarize"`, which `speech.providers.list` and the selection methods never offer as a dictation choice); the language models answer under `llm.models.status`, `llm.models.download`, `llm.models.cancel` and `llm.models.delete` (the same shapes over the `llm` provider, `is_selected` marking `[llm] model`, an unknown id `NOT_FOUND` naming the ids the catalogue lists), and `llm.engine.status` reports the engine process; fixtures under `crates/dettivo-proto/fixtures/llm/`. `dettivo doctor` prints a models row (how many are ready, the directory, the quarantined count and the selected speech model's readiness) and an llm row (the provider Enhanced would use, whether the local engine would answer and why not, the selected language model's readiness and the engine process).

The CUDA diarization drop-in reuses the same `diarize/diarization-en` segmentation and embedding files. It needs no model conversion or second download. When CUDA prerequisite checks fail, `auto` keeps those models on CPU and reports the fallback reason ([docs/engines.md](engines.md), [installation](install.md#optional-nvidia-diarization)).

## Configuration

| Key | Default | Meaning |
|---|---|---|
| `[speech] provider` | `"whisper"` | The speech provider. |
| `[speech] model` | `"large-v3-turbo"` | The dictation model id. |
| `[speech] meeting_model` | `""` | The meeting model; empty means the dictation model. |
| `[speech] parakeet_model_id` | `"parakeet-v3"` | The Parakeet model a switch to `provider = "parakeet"` selects. |
| `[meetings.diarization] model` | `"diarization-en"` | The model set the speaker pass loads ([docs/meetings.md](meetings.md)). |
| `[engines.diarize] backend` | `"auto"` | CUDA when its prerequisite checks pass, else CPU with the reason; `cpu` pins CPU and `cuda` requires CUDA. Settings / Models offers the same choice. |
| `[llm] model` | `"qwen3-4b-instruct-2507"` | The language model the local engine loads for Enhanced. |
| `[llm] analysis_model` | `""` | The language model meeting analysis loads; empty means `model`. |
| `[models] max_concurrent_downloads` | `1` | Downloads that may run at once. |
| `[models] verify_on_start` | `true` | Re-hash unverified models at start, before the preload; off, each load hashes what it needs first. |
| `[models] catalogue_file` | `""` | A catalogue TOML instead of the built-in one. |
| `[paths] models_dir` | `""` | The models directory; empty means `<data_dir>/models`. |
