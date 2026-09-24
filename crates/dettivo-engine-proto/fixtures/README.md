# Engine protocol fixtures

One file per message: `request.<name>.json`, `response.<name>.json` and `event.<name>.json`, each the JSON header of one frame (`v`, `id`, `kind`, `name`, `payload`, optional `attachments`). The unit job proves every file round-trips byte-stable through `dettivo-engine-proto`'s typed payloads and the frame codec; an engine binary and the daemon's supervisor are checked against the same shapes.

A `recognize` or `diarize` request declares one `pcm16k` attachment (16 kHz mono signed 16-bit little-endian) whose bytes follow the header on the wire; the fixture carries only the declaration.
