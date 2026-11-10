# Spec Delta

## Purpose

Fixes the parts of the machine-readable diagnostic format that the language spec leaves open, and keeps every error code documented and tested. The language spec defines what a diagnostic holds and what its edits mean (`r[diag.json]`, `r[diag.applicability]`, `r[diag.edits]`, `r[diag.edit-file]`). This delta doesn't restate those rules.

## ADDED Requirements

### Requirement: Error code format
An error code SHALL be `E` followed by four digits. A warning code SHALL be `W` followed by four digits. A code SHALL never be reused for a different diagnostic, and a retired code SHALL keep its registry entry and explain text. A registry entry's default severity SHALL match its code's letter.

#### Scenario: Promoted warning keeps its code
- **WHEN** a warning W0003 is reported under `--deny warnings`
- **THEN** its `code` is still `W0003`, its `severity` is `"error"`, and it validates against the schema

### Requirement: Positions
A position SHALL be `[line, column]` with both values 1-based, and a span's `end` SHALL be exclusive.
- `column` SHALL count Unicode scalar values from the start of the line. A tab is one column.

#### Scenario: Span of a non-ASCII identifier
- **WHEN** a diagnostic covers `café` starting at the first column of line 3
- **THEN** its span is `"start": [3, 1], "end": [3, 5]`

### Requirement: Lines in positions
- A line SHALL end at LF (U+000A), at CR LF, or at a CR not followed by LF, as in LSP. A CR LF is one line ending. Which line endings the lexer accepts is for section 2 and M1 to decide.
- A UTF-8 byte order mark at the start of a file SHALL NOT be counted, so the character after it is `[1, 1]`.
- The position after the last character SHALL be the end of input. After a trailing newline, it is `[last line + 1, 1]`.

#### Scenario: CRLF line endings
- **WHEN** a file is `a\r\nbc` and a diagnostic covers `bc`
- **THEN** its span is `"start": [2, 1], "end": [2, 3]`

#### Scenario: Lone CR
- **WHEN** a file is `a\rbc` and a diagnostic covers `bc`
- **THEN** its span is `"start": [2, 1], "end": [2, 3]`

### Requirement: Edits
An edit SHALL carry only the fields the language spec defines for it: `start`, `end` and `text`, plus the optional `file`. Within one fix:
- every position SHALL refer to the text as it was before any of the fix's edits were applied;
- no two edits to the same file SHALL overlap, and an insertion SHALL NOT sit at the same position as another insertion or at either end of another edit's range, so the order of the texts is never ambiguous.

A tool applies a fix's edits as if all at once.

#### Scenario: Insertion
- **WHEN** a fix adds a match arm at line 46, column 5
- **THEN** its edit is `{ "start": [46, 5], "end": [46, 5], "text": "..." }`

#### Scenario: Deletion
- **WHEN** a fix shortens `email: email` to `email:` on line 9, where the space and the second `email` cover columns 14 to 19
- **THEN** its edit is `{ "start": [9, 14], "end": [9, 20], "text": "" }`

#### Scenario: Edit in another file
- **WHEN** a fix creates `cmd/app/main.qnc`
- **THEN** its edit is `{ "file": "cmd/app/main.qnc", "start": [1, 1], "end": [1, 1], "text": "..." }`

#### Scenario: Overlapping edits
- **WHEN** a fix has edits covering `[3, 1]`–`[3, 9]` and `[3, 5]`–`[3, 12]` in the same file
- **THEN** the fix is rejected as malformed

#### Scenario: Insertion at the edge of a replacement
- **WHEN** a fix replaces `[3, 1]`–`[3, 5]` and inserts at `[3, 5]` in the same file
- **THEN** the fix is rejected as malformed, because the replacement can include the inserted text instead

#### Scenario: Adjacent replacements
- **WHEN** a fix replaces `[3, 1]`–`[3, 5]` and `[3, 5]`–`[3, 9]` in the same file
- **THEN** the fix is accepted

### Requirement: Reported fixes are checked
The conformance runner SHALL check every fix of every diagnostic a backend reports against the Edits requirement, and a malformed fix SHALL fail the fixture.

#### Scenario: Backend reports an overlapping fix
- **WHEN** a backend reports a diagnostic whose fix has overlapping edits
- **THEN** the fixture fails and names the diagnostic and the overlap

### Requirement: JSON shape
A diagnostic SHALL carry exactly the fields `r[diag.json]` lists, and any other field SHALL be rejected. The field set stays closed until a spec change adds a field. In addition:
- `severity` SHALL be `"error"` or `"warning"`;
- `fixes` SHALL always be present, as `[]` when there are none;
- every `file`, in a span or an edit, SHALL be relative to the project root and use `/` separators.

Output SHALL validate against `compiler/diagnostics/schema/diagnostic.schema.json`.

#### Scenario: Spec example validates
- **WHEN** the example diagnostic from spec section 8 is checked against the schema
- **THEN** it validates

#### Scenario: Unknown field rejected
- **WHEN** a diagnostic object carries an extra field `"notes"`
- **THEN** it fails validation against the schema

### Requirement: The schema checker never ignores a keyword
The test-only schema checker SHALL fail on any schema keyword it doesn't implement, so the schema can't contain a constraint that nothing enforces.

#### Scenario: Unsupported keyword
- **WHEN** the schema uses `oneOf`, which the checker doesn't implement
- **THEN** checking any value against it fails and names `oneOf`

### Requirement: Every code is documented and tested
Every code in the registry SHALL have non-empty explain text. Every code that isn't retired SHALL appear in at least one conformance fixture's expectation. A retired code is never emitted, so it needs no fixture. Every explain file SHALL belong to a registry entry.

In M0, expectations are the `//~` comments in `.qnc` fixtures. M2's project-fixture format adds an expectation form for files that can't hold `//~`, such as `quince.toml`, and those expectations count too.

#### Scenario: Code without a test
- **WHEN** E0412 is registered and no fixture expects `//~ E0412`
- **THEN** `cargo xtask diagnostics` fails and names E0412

#### Scenario: Retired code
- **WHEN** a retired code has explain text and no fixture expects it
- **THEN** `cargo xtask diagnostics` passes

#### Scenario: Orphan explain file
- **WHEN** `explain/E0999.md` exists and no registry entry has code E0999
- **THEN** `cargo xtask diagnostics` fails and names the file
