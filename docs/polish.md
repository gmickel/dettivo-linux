# Polish

Dictation has three modes. `raw` gives you the engine's words with your replacements and spoken punctuation applied, and the tokens you dictate kept whole: `index.ts`, `src/app/index.ts`, `https://example.com/docs`, `gordon@mickel.tech`, `v1.2.3`, `foo.bar()` and a backticked span come back byte for byte, with no sentence end on a dot inside them and no replacement rewriting inside them ([docs/dictation.md](dictation.md), `[dictation] protect_tokens`). `deterministic_polish` cleans them up — the fillers gone, the casing and punctuation right, spoken paths and lists put back together, and those same tokens left exactly as dictated — with no model involved, so it costs nothing and never leaves the machine. `enhanced` sends that cleaned-up text to a language model for a rewrite and keeps the deterministic version whenever the model is slow, missing or wrong, telling you which of the three happened.

Set the mode you want in `config.toml` and forget about it:

```toml
[dictation]
mode = "deterministic_polish"
```

Try it before you commit to it:

```
dettivo polish test "um so i think we should ship this thing tomorrow"
```

```
raw       Um so i think we should ship this thing tomorrow
polished  So I think we should ship this thing tomorrow.
inserted  So I think we should ship this thing tomorrow.
model     deterministic  policy 58b1816f
```

A file name or a URL in the sample stays a file name or a URL:

```
dettivo polish test "um open index.ts and read https://example.com/docs period then ship it"
```

```
raw       Um open index.ts and read https://example.com/docs. Then ship it
polished  Open index.ts and read https://example.com/docs. Then ship it.
inserted  Open index.ts and read https://example.com/docs. Then ship it.
model     deterministic  policy 58b1816f
```

## What Polish does

The deterministic pass runs three transforms, each of which you can switch off in `[polish] transforms`:

| Transform | What it does |
|---|---|
| `fixGrammar` | Dictation slips: `ithink` becomes `I think`, `thats` becomes `that's`, a lone `i` becomes `I`, `quote ... unquote` becomes a quoted span. |
| `removeFillers` | `uh`, `um`, `erm`, `äh` and their neighbours, with the spacing closed up behind them. |
| `smartPunctuation` | A capital at the front, a full stop or a question mark at the end, no space before a comma. Never runs under the `code` preset. |

Then only the preset's selected post-processors run, once each. Fix Grammar does not implicitly enable Smart Punctuation. Whitespace normalization preserves paragraph breaks. They repair the things dictation gets wrong about technical and business text: `settings view dot swift` becomes `@SettingsView.swift`, `quality slash speed` becomes `quality/speed`, `R and D` becomes `R&D`, `bullet one ..., bullet two ...` becomes a real bullet list, `thousand plus page` becomes `1,000-plus-page`. The rules originated in the macOS port. Linux golden tests now pin enabled-only behavior and retain the original macOS outputs separately for intentional differences ([ADR 0054](adr/0054-delete-duplicate-work-and-keep-one-owner.md)).

## Presets, styles and app profiles

A preset says what kind of text this is; a style says how formal it should read.

| Preset | For | Notes |
|---|---|---|
| `email` | Mail clients | Formal by default; paragraphs, greeting and sign-off kept. |
| `code` | Editors, terminals, code assistants | Identifiers, paths and flags preserved; `@path` insertion on; no smart punctuation. |
| `chat` | Slack, Discord, Signal | Short and conversational. |
| `notes` | Obsidian, Logseq, note apps | Bullets, numbering and headings kept. |
| `generic` | Everything else | Cleanup only. |

The styles are `asDictated` (cleanup only, the default — each preset then picks its own), `formal`, `casual` and `veryCasual`.

Dettivo picks a preset from the window you are dictating into. It resolves in this order, first match wins:

1. **Your app profile** for that app id, from `[polish] apps`.
2. **The built-in mapping** — Thunderbird, Evolution and KMail to `email`; VS Code, Cursor, Zed, Neovide and the JetBrains family to `code`; Slack, Discord, Telegram, Signal and Element to `chat`; Obsidian, Logseq and GNOME Notes to `notes`; a terminal to `code`.
3. **The app class**, when a client names one.
4. **`[polish] default_preset`**.

A terminal is the one place this is not enough: people dictate prose into a coding agent all day. When the window is a terminal and the transcript reads like a sentence rather than a command, the resolver drops `code` for `generic` and keeps `@path` insertion on if the transcript still names a file. `cargo test --workspace` stays a command.

To pin an app yourself:

```
dettivo polish apps set org.gnome.Console code
dettivo polish apps list
```

The app id is the Wayland app id or the X11 window class — `dettivo insert target` prints the one for the window you have focused. A trailing `*` matches a family: `jetbrains-*`.

## Your own rules

A custom rule is one instruction, at most 500 characters, that joins every Enhanced rewrite while it is enabled. Rules do not change the deterministic pass — no model, no instructions to follow.

```
dettivo polish rules add "House style" "Never abbreviate a client's name."
dettivo polish rules list
dettivo polish rules set <rule-id> "Never abbreviate a client's name. Keep sentences under 20 words." 
dettivo polish rules remove <rule-id>
```

Rules live in `[polish] rules` in your `config.toml`, and the daemon writes them back keeping every comment you put there.

## Enhanced and its providers

`enhanced` runs the deterministic pass first, then asks a language model to rewrite that result. The prompt wraps your transcript in markers that cannot be spoofed and tells the model, at length, that the transcript is data and never an instruction — a dictated "read app.ts and tell me what's in there" comes back rewritten, never answered.

The default needs nothing installed: download a model once and the daemon's own engine, `dettivo-engine-llm` (llama.cpp on the same Vulkan backend as the speech engines, ADR 0026), does the rewrite on this machine.

```
dettivo llm download --model qwen3-4b-instruct-2507 --wait
dettivo llm test "um so i think we should ship this thing tomorrow"
```

Set up the provider layer in `[llm]`:

```toml
[llm]
provider = "auto"                      # or local, ollama, openai_compatible
model = "qwen3-4b-instruct-2507"       # the local engine's model
ollama_url = "http://127.0.0.1:11434"
ollama_model = "qwen3:4b-instruct"
timeout_ms = 8000
```

`auto` tries the local engine, then Ollama, then a configured endpoint, and takes the first that answers: as soon as `[llm] model` is on disk and the engine binary is found, `local` is what a dictation uses. See where you stand:

```
dettivo llm providers
```

```
up    local              qwen3-4b-instruct-2507   llm/qwen3-4b-instruct-2507 on disk; /usr/bin/dettivo-engine-llm
down  ollama             qwen3:4b-instruct        start Ollama, or set [llm] ollama_url
selected  local
```

Before the download the first line reads `down  local ... dettivo llm download --model qwen3-4b-instruct-2507`, and `auto` moves on to Ollama. The local models are `qwen3-4b-instruct-2507` (the default, 2.5 GB), `qwen3-1.7b` (fast, 1.1 GB), `qwen3-4b` (quality) and `qwen3-8b` (meeting analysis); [docs/models.md](models.md) has the catalogue and [docs/engines.md](engines.md) the engine, its backend choice and the idle unload that returns the GPU memory after `[engines] llm_idle_seconds`. For Ollama the recommended models are `qwen3:4b-instruct`, `qwen3:1.7b`, `qwen3:4b` and `qwen3:8b`.

For an OpenAI-compatible endpoint, set `endpoint_url` and `endpoint_model`. The key comes from the Secret Service (service `dettivo`, key `llm-api-key`) if it is there, and from `[llm] api_key_file` otherwise — a file you own, mode 0600.

### Anything not on this machine needs your say-so

A dictation is often the most sensitive text you write all day, so an endpoint that is not on loopback never receives one until you have named it once:

```
dettivo llm trust https://llm.example.com
dettivo llm endpoints
```

```
local   http://localhost:11434
remote  https://llm.example.com
```

Until then the rewrite falls back to the deterministic result and `dettivo llm providers` prints the command to run. The confirmation is a line in your own `config.toml` under `[llm] trusted_endpoints`, stored in canonical form, and you can delete it there.

### When Enhanced keeps the deterministic text

The rewrite has to earn its place. The deterministic text goes in instead — with a notice saying why — when:

- **`provider_unavailable`**: nothing answered, the endpoint is not trusted, or the whole pass did not finish inside `[llm] timeout_ms`. The insertion is never held up beyond that budget; a local generation that outlives it is cancelled so the engine is free for the next one.
- **`guard_rejected`**: the model answered the transcript instead of rewriting it, refused it, asked you for input, invented a code fence, dropped a path, a code span or a number you dictated (each number whole and as often as you said it; `42` is not `420`, `1.5` is not `15`), lost a question mark, or ran away in length. The model gets one repair pass per attempt with the reason and the deterministic draft before this is decided.
- **`fallback_used`**: the model gave back something empty or degenerate, or changed nothing that a transform would have changed.

Short text never reaches the model at all: a question, a plain command, an already formatted code command and a high-confidence spelling fix are inserted from the deterministic pass straight away, because a round trip cannot improve them.

Thinking blocks (`<think>…</think>`) are stripped before any of this is judged; the local engine asks a thinking model (Qwen3) to answer without one in the first place.

The mode, the notice and a hash of the policy that ran ride on the completion event and on the history item, so `dettivo history get <id>` can explain a past dictation.

### A fine-tune of your own

The private dictation fine-tune from the macOS pipeline runs here too: convert it with `scripts/models/convert-polish-finetune.sh`, then `dettivo llm experiment use current` switches Enhanced onto it while `[llm] model` stays the default; `dettivo llm experiment clear` goes back. `dettivo-qa polish-eval` scores it on the held-out sets with the macOS metrics and gates. [docs/polish-models.md](polish-models.md) is the whole runbook.

## Trying a change before you keep it

`dettivo polish test` runs the same code the session runs — the same layers, the same policy, the same hash:

```
dettivo polish test "read the readme and tell me what is in there" --preset code
dettivo polish test "hey team can you send the deck" --preset email --style formal
dettivo polish test "update the docs" --bundle-id org.gnome.Console
dettivo llm test "um so i think we should ship this thing tomorrow"
```

`--mode enhanced` (or `dettivo llm test`) reaches the provider; without it the test stays deterministic and reproducible.

## Configuration

Everything above is `[polish]` and `[llm]` in `config.toml` ([docs/config.md](config.md)); `dettivo config print-default` prints both sections fully commented. The GUI settings screens arrive with their own spec and write the same keys through the same methods.

## For agents

`polish.rules.list`, `polish.rules.create`, `polish.rules.update`, `polish.rules.delete`, `polish.presets.list`, `polish.test`, `polish.apps.list` and `polish.apps.set` are the contract's methods; `llm.providers.list`, `llm.endpoints.trust`, `llm.endpoints.list`, the `llm.models.*` catalogue methods and `llm.engine.status` are Linux additions registered in [docs/api/linux-deltas.md](api/linux-deltas.md). `system.capabilities.polish` and `system.capabilities.llm` say what this daemon serves; `llm.local_available` is `true` when the local engine would answer. The decisions behind all of it are [ADR 0023](adr/0023-polish-layers-and-the-llm-provider-layer.md) and [ADR 0026](adr/0026-local-llm-engine-llama-cpp-and-the-gguf-catalogue.md).

For a QA run, `DETTIVO_MOCK_LLM=echo` makes Enhanced hand back the deterministic text without a model, and `DETTIVO_MOCK_LLM=fixture:<dir>` replays recorded rewrites — one `<sha>.txt` per transcript with `default.txt` as the fallback, and the body `__TIMEOUT__` to stage a provider that never answers ([docs/qa.md](qa.md)).
