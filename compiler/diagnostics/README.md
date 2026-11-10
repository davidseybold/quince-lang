# quince_diagnostics

The error-code registry, the JSON diagnostic types and their schema, and fix checking. The language spec defines what a diagnostic holds and what its edits mean (`r[diag.json]`, `r[diag.applicability]`, `r[diag.edits]`, `r[diag.edit-file]`). This file fixes the parts the spec leaves open, and doesn't restate the rest.

## Codes

- An error code is `E` plus four digits, and a warning code is `W` plus four digits. A registry entry's default severity matches its code's letter.
- A code is never reused for a different diagnostic. A retired code keeps its registry row and explain text, marked `retired: true`.
- `--deny warnings` changes a warning's reported `severity` to `"error"`, not its code.
- `E0412` is reserved for a non-exhaustive match, so the spec's example diagnostic stays true.
- To add a code, see `src/registry.rs`.

## Positions

- A position is `[line, column]`, both 1-based. A span's `end` is exclusive.
- `column` counts Unicode scalar values from the start of the line, and a tab is one column. So `café` at the start of line 3 spans `[3, 1]` to `[3, 5]`.
- A line ends at LF, at CR LF (one line ending), or at a CR not followed by LF, as in LSP. Which line endings the lexer accepts is the spec's business. This only fixes how positions count.
- A UTF-8 byte order mark at the start of a file isn't counted, so the character after it is `[1, 1]`.
- The end of input is the position after the last character. After a trailing newline, that's `[last line + 1, 1]`.

## Edits and fixes

- An edit carries only `start`, `end` and `text`, plus an optional `file`. It replaces the text from `start` up to `end`. When `start` equals `end` it inserts, and an empty `text` deletes.
- An edit to a file that doesn't exist yet is an insertion at `[1, 1]` with a `file`, such as `{ "file": "cmd/app/main.qnc", "start": [1, 1], "end": [1, 1], "text": "..." }`.
- Within one fix, as in LSP's `WorkspaceEdit`:
  - every position refers to the text before any of the fix's edits are applied;
  - no two edits to one file overlap;
  - an insertion never sits at another insertion's position, or at either end of another edit's range (the replacement can carry the inserted text instead);
  - adjacent replacements, such as `[3, 1]`–`[3, 5]` and `[3, 5]`–`[3, 9]`, are fine.

  So the order of texts is never ambiguous, and a tool applies a fix as if all at once.
- An edit's `start` is never after its `end`. Both positions exist in the original text, except `[1, 1]` in a file the edit creates.
- `Fix::check` rejects a fix that breaks these rules. The conformance runner runs it on every fix a backend reports, and a malformed fix fails the fixture. Checking that each position exists needs the source text, so that part arrives with M1's backend.

## JSON shape

- A diagnostic carries exactly the fields `r[diag.json]` lists, and any other field is rejected. The field set stays closed until a spec change adds a field.
- `severity` is `"error"` or `"warning"`. `fixes` is always present, as `[]` when empty.
- Every `file`, in a span or an edit, is relative to the project root and uses `/` separators.
- Output validates against `schema/diagnostic.schema.json` (JSON Schema 2020-12, written by hand). A test checks that the spec's example diagnostic validates.
- The test-only schema checker fails on any schema keyword it doesn't implement, so the schema can't hold a constraint that nothing enforces.

## Every code is documented and tested

`cargo xtask diagnostics` fails unless:
- every registry code has non-empty explain text in `explain/<CODE>.md`;
- every code that isn't retired appears in at least one conformance fixture's expectation (a retired code is never emitted, so it needs no fixture);
- every explain file belongs to a registry row.

Expectations are the `//~` comments in `.qnc` fixtures. M2's project fixtures add a form for files that can't hold `//~`, such as `quince.toml`, and those expectations will count too.
