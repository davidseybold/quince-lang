# Spec Delta

## Purpose

Fixes the parts of the machine-readable diagnostic format that the language spec leaves open, and keeps every error code documented and tested.

## ADDED Requirements

### Requirement: Error code format
An error code SHALL be `E` followed by four digits. A warning code SHALL be `W` followed by four digits. A code SHALL never be reused for a different diagnostic, and a retired code SHALL keep its registry entry and explain text.

#### Scenario: Promoted warning keeps its code
- **WHEN** a warning W0003 is reported under `--deny warnings`
- **THEN** its `code` is still `W0003` and its `severity` is `"error"`

### Requirement: Positions
A position SHALL be `[line, column]` with both values 1-based. `column` SHALL count Unicode scalar values from the start of the line. A span's `end` SHALL be exclusive. `span.file` SHALL be relative to the project root and use `/` separators.

#### Scenario: Span of a non-ASCII identifier
- **WHEN** a diagnostic covers `café` starting at the first column of line 3
- **THEN** its span is `"start": [3, 1], "end": [3, 5]`

### Requirement: Edits
Every edit in a fix SHALL be an object with exactly `start`, `end` and `text`, and SHALL mean: replace the text from `start` up to `end` with `text`. An insertion SHALL be written with `start` equal to `end`, and a deletion with empty `text`. No other edit form SHALL exist.

#### Scenario: Insertion
- **WHEN** a fix adds a match arm at line 46, column 5
- **THEN** its edit is `{ "start": [46, 5], "end": [46, 5], "text": "..." }`

#### Scenario: Deletion
- **WHEN** a fix shortens `email: email` to `email:` on line 9, where the second `email` covers columns 15 to 20
- **THEN** its edit is `{ "start": [9, 14], "end": [9, 20], "text": "" }`

### Requirement: JSON shape
A diagnostic SHALL serialize with exactly the fields `code`, `severity`, `message`, `span`, `fixes` and `docs`, where `severity` is `"error"` or `"warning"`, each fix has `description`, `applicability` (`"safe"`, `"needs-review"` or `"placeholder"`) and `edits`, and `docs` is `"quince explain <code>"`. Output SHALL validate against `compiler/diagnostics/schema/diagnostic.schema.json`.

#### Scenario: Spec example validates
- **WHEN** the example diagnostic from spec section 8 is checked against the schema
- **THEN** it validates

#### Scenario: Unknown field rejected
- **WHEN** a diagnostic object carries an extra field `"notes"`
- **THEN** it fails validation against the schema

### Requirement: Every code is documented and tested
Every code in the registry SHALL have non-empty explain text and SHALL appear in at least one conformance fixture's inline expectation. Every explain file SHALL belong to a registry entry.

#### Scenario: Code without a test
- **WHEN** E0412 is registered and no fixture expects `//~ E0412`
- **THEN** `cargo xtask diagnostics` fails and names E0412

#### Scenario: Orphan explain file
- **WHEN** `explain/E0999.md` exists and no registry entry has code E0999
- **THEN** `cargo xtask diagnostics` fails and names the file
