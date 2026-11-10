# spec-grammar Specification

## Purpose
Keeps a machine-readable copy of the language grammar in `spec/grammar.ebnf` that the parser is tested against, and guarantees it never drifts from the grammar appendices in the spec text.

## Requirements

### Requirement: Grammar file mirrors the appendices
`spec/grammar.ebnf` SHALL consist of a header of `//` comment lines followed by two blocks, each copied verbatim: the code block under `## Appendix: Expression grammar`, then one blank line, then the code block under `### Declarations, types and patterns`.
- Each heading SHALL be followed by exactly one code block before the next heading.
- The header is the only hand-edited part of the file, and `--write` regenerates everything after it.

#### Scenario: Spec grammar edited without updating the file
- **WHEN** a production in the spec's appendix changes and `spec/grammar.ebnf` does not
- **THEN** `cargo xtask spec-grammar` fails, prints a unified diff with hunk headers, and names `cargo xtask spec-grammar --write` as the fix

#### Scenario: Regenerating the file
- **WHEN** `cargo xtask spec-grammar --write` runs
- **THEN** the grammar part of `spec/grammar.ebnf` is rewritten from the spec and the header is kept, or the default header is used when the file doesn't exist

#### Scenario: Appendix heading missing
- **WHEN** either appendix heading is not found in the spec
- **THEN** the check fails and names the heading it looked for

#### Scenario: Second code block under an appendix heading
- **WHEN** an appendix heading is followed by two code blocks before the next heading
- **THEN** the check fails and names the heading

### Requirement: The grammar is closed
Every identifier on the right-hand side of a production, outside string literals and comments, SHALL be defined by a production or listed as a terminal in the file's header. No production SHALL be defined twice. Every production except `file` SHALL be referenced by another production.

Errors SHALL name the line in `spec/quince-spec.md`, because the grammar is fixed in the spec and then regenerated, never edited in the generated file.

#### Scenario: Undefined nonterminal
- **WHEN** a production references `type_list` and nothing defines it or lists it as a terminal
- **THEN** the check fails, names `type_list` and the production that references it, and gives the spec line

#### Scenario: Duplicate production
- **WHEN** two productions are both named `arg`
- **THEN** the check fails and names both spec lines

#### Scenario: Unused production
- **WHEN** a production `old_arg` is defined and nothing references it
- **THEN** the check fails and names `old_arg`
