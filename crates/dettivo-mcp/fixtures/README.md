# MCP fixtures

`tools/<name>.json` is each tool as `tools/list` reports it (name, title,
description, input schema), the macOS definitions ported verbatim; the
`fixtures` test fails on any drift and `FN18_WRITE_FIXTURES=1` rewrites them
from the catalog. `framing/initialize.<framing>.{request,response}` are the
exact bytes of the `initialize` exchange in each framing; the harness test
sends the request and compares the server's answer byte for byte.
