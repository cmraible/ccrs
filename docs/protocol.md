# JSON service protocol

[Back to the README](../README.md) · [Report fields and counting rules](metrics.md)

Run one long-lived `ccrs serve` process. Send one request per line on stdin; each
response is flushed immediately to stdout. Requests are processed sequentially,
and responses correspond in input order. JSON strings must escape embedded newlines.

```jsonl
{"op":"open","path":"example.ts","source":"const f = () => 1;"}
{"op":"edit","path":"example.ts","version":0,"start_byte":16,"end_byte":17,"text":"x ? 1 : 0"}
{"op":"close","path":"example.ts"}
```

`open` returns `{ "path": "…", "version": 0, "report": { … } }`.
It accepts an optional per-document metric selection:

```jsonl
{"op":"open","path":"example.ts","source":"const f = () => 1;","metrics":["cyclomatic","cognitive"]}
```

Omitting `metrics` (or sending `null`) enables all four; `[]` requests only scope
locations and parser status. Unknown names are rejected. The selection persists
across all edits to that document and is independent of other open documents.
Close and reopen the document to change its selection. `metrics` is accepted on
`open` only; subsequent `edit` responses include the selected fields.

An edit supplies the **current** version and returns the next version and report.
Every successful edit advances the version, including an empty edit. Syntax errors
in an edited buffer still produce a successful report with `has_errors: true`.
`close` releases the document and returns `{ "path": "…", "closed": true }`.

Malformed requests, unsupported extensions, duplicate opens, unknown documents,
stale versions, and invalid ranges return `{ "error": "…" }` without terminating
the service. Failed edits preserve the current version and document. To replace
an entire buffer, edit its full byte range. To reopen a path, close it first.
Paths are exact document keys; the service does not read or write those files.

Editor clients must convert UTF-16 positions to UTF-8 byte offsets. Keep one
request in flight per document and associate replies in order. This is a local
stdio protocol, not LSP or MCP. Editor adapters and filesystem watching are not
included in this initial version.
