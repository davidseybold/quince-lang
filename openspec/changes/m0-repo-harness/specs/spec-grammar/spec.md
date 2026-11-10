# Spec Delta

## Purpose

Keeps a machine-readable copy of the language grammar in `spec/grammar.ebnf` that the parser is tested against, and guarantees it never drifts from the grammar appendices in the spec text.

## ADDED Requirements

### Requirement: Grammar file mirrors the appendices
`spec/grammar.ebnf` SHALL consist of a header of `//` comment lines followed by the code block under `## Appendix: Expression grammar`, one blank line, and the code block under `### Declarations, types and patterns`, each copied verbatim.

#### Scenario: Spec grammar edited without updating the file
- **WHEN** a production in the spec's appendix changes and `spec/grammar.ebnf` does not
- **THEN** `cargo xtask spec-grammar` fails, prints a unified diff, and names `cargo xtask spec-grammar --write` as the fix

#### Scenario: Regenerating the file
- **WHEN** `cargo xtask spec-grammar --write` runs
- **THEN** the grammar part of `spec/grammar.ebnf` is rewritten from the spec and the header is kept

#### Scenario: Appendix heading missing
- **WHEN** either appendix heading is not found in the spec
- **THEN** the check fails and names the heading it looked for

### Requirement: Every referenced name is defined
Every identifier on the right-hand side of a production, outside string literals and comments, SHALL be defined by a production or listed as a terminal in the file's header. No production SHALL be defined twice.

#### Scenario: Undefined nonterminal
- **WHEN** a production references `type_list` and nothing defines it or lists it as a terminal
- **THEN** the check fails and names `type_list` and the production that references it

#### Scenario: Duplicate production
- **WHEN** two productions are both named `arg`
- **THEN** the check fails and names both lines
