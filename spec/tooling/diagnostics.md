# Diagnostics and queries

## Structured diagnostics

r[diag.json]
With `--format json`, every diagnostic has a stable code, an exact span, a plain-language message, and zero or more fixes expressed as text edits:

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

r[diag.applicability]
- `applicability` is `safe` (apply automatically), `needs-review`, or `placeholder` (inserts a `todo`).

r[diag.edits]
- An edit replaces the text from `start` up to `end` with `text`: when `start` equals `end` it inserts, and an empty `text` deletes.

r[diag.edit-file]
- An edit may name a `file`, a path relative to the project root; without one it applies to the diagnostic's span file. An edit at `[1, 1]` to a file that doesn't exist creates it.

r[diag.code-stable]
- Error codes are stable across editions.

r[diag.explain]
- `quince` `explain <code>` prints a full explanation with examples.

r[diag.all-errors]
- The compiler reports all independent errors in one run, ordered so fixing the first rarely invalidates the rest.

## Query API

`quince` `query` exposes what the compiler knows, so an agent can look things up instead of guessing:

r[diag.query.members]
- `members <Type>`: Fields, methods, and implemented interfaces, with signatures

r[diag.query.signature]
- `signature <path>`: Full signature, effects, contracts, and doc comment

r[diag.query.callers]
- `callers <path>` / `callees <path>`: The call graph around a function

r[diag.query.impls]
- `impls <Interface>`: Every type implementing an interface

r[diag.query.type-at]
- `type-at <file:line:col>`: The inferred type of an expression

r[diag.query.effects]
- `effects <path>`: Each of a function's effects, with the call chain that brings it in and every package effect expanded to the built-in effects beneath it

r[diag.query.search]
- `search <type pattern>`: Functions matching a type shape, e.g., `String -> Result<Int, _>`

r[diag.query.dependents]
- `dependents <path>`: Everything that would break if a definition changed

r[diag.query.wire]
- `wire <Type>`: The JSON shape a type encodes to and decodes from, with wire names and which fields are optional

r[diag.query.source]
- source \<file> \[--without-tests\]: A file's source, optionally with test blocks folded away ([Testing](../reference/testing.md))

r[diag.query.handlers]
- `handlers <fn>`: Which handler each of a function's effects resolves to, from each program's main ([Effects](../reference/effects.md))

r[diag.query.lsp]
The same queries are served over a long-running language server for editors and agents that keep a session open.

r[diag.query.without-tests]
Unit tests make files longer, so `quince` `query source <file> --without-tests` returns the file with its test blocks folded away. An agent reading code to change one function doesn't pay for every test in its context.

r[diag.query.signature-location]
- Since an import doesn't name a file, `quince` `query signature <name>` and the language server return the file and line where any name is defined.
