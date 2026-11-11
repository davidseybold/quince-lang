# Diagnostics and queries

## Structured diagnostics

r[diag.json] With `--format json`, every diagnostic has a stable code, an exact span, a plain-language message, and zero or more fixes expressed as text edits:

```
{
  "code": "E0412",
  "severity": "error",
  "message": "match on `FetchError` is missing variant `Timeout`",
  "span": { "file": "src/fetch.qnc", "start": [42, 3], "end": [47, 4] },
  "fixes": [
    {
      "description": "add an arm for `Timeout`",
      "applicability": "needs-review",
      "edits": [{ "start": [46, 5], "end": [46, 5], "text": "FetchError.Timeout(after:) => todo(\"handle timeout\"),\n" }]
    }
  ],
  "docs": "quince explain E0412"
}
```

- r[diag.applicability] `applicability` is `safe` (apply automatically), `needs-review`, or `placeholder` (inserts a `todo`).
- r[diag.edits] An edit replaces the text from `start` up to `end` with `text`: when `start` equals `end` it inserts, and an empty `text` deletes.
- r[diag.edit-file] An edit may name a `file`, a path relative to the project root; without one it applies to the diagnostic's span file. An edit at `[1, 1]` to a file that doesn't exist creates it.
- r[diag.code-stable] Error codes are stable across editions; r[diag.explain] `quince` `explain <code>` prints a full explanation with examples.
- r[diag.all-errors] The compiler reports all independent errors in one run, ordered so fixing the first rarely invalidates the rest.

## Query API

`quince` `query` exposes what the compiler knows, so an agent can look things up instead of guessing:

| Query | Returns |
| --- | --- |
| `members <Type>` | r[diag.query.members] Fields, methods, and implemented interfaces, with signatures |
| `signature <path>` | r[diag.query.signature] Full signature, effects, contracts, and doc comment |
| `callers <path>` / `callees <path>` | r[diag.query.callers] The call graph around a function |
| `impls <Interface>` | r[diag.query.impls] Every type implementing an interface |
| `type-at <file:line:col>` | r[diag.query.type-at] The inferred type of an expression |
| `effects <path>` | r[diag.query.effects] Each of a function's effects, with the call chain that brings it in and every package effect expanded to the built-in effects beneath it |
| `search <type pattern>` | r[diag.query.search] Functions matching a type shape, e.g., `String -> Result<Int, _>` |
| `dependents <path>` | r[diag.query.dependents] Everything that would break if a definition changed |
| `wire <Type>` | r[diag.query.wire] The JSON shape a type encodes to and decodes from, with wire names and which fields are optional |
| source \<file> \[--without-tests\] | r[diag.query.source] A file's source, optionally with test blocks folded away ([Testing](../reference/testing.md)) |
| `handlers <fn>` | r[diag.query.handlers] Which handler each of a function's effects resolves to, from each program's main ([Effects](../reference/effects.md)) |

r[diag.query.lsp] The same queries are served over a long-running language server for editors and agents that keep a session open.

r[diag.query.without-tests] Unit tests make files longer, so `quince` `query source <file> --without-tests` returns the file with its test blocks folded away. An agent reading code to change one function doesn't pay for every test in its context.

- r[diag.query.signature-location] Since an import doesn't name a file, `quince` `query signature <name>` and the language server return the file and line where any name is defined.
