# Spec Delta

## Purpose

Gives every normative statement in the Quince language spec a stable, citable ID, so conformance tests can name the rule they check and coverage can be measured per rule.

## ADDED Requirements

### Requirement: Rule tag syntax
A rule ID SHALL be written in the spec as `r[<id>]` directly before the sentence or sentences it covers. A tag covers text up to the next tag or the end of its paragraph, list item or table cell. Tags SHALL NOT appear in code blocks or headings.

#### Scenario: Tag inside a code block
- **WHEN** `r[bind.let]` appears inside a fenced code block in the spec
- **THEN** the rule-ID check fails and names the file and line

### Requirement: Rule ID format
An ID SHALL have 2 to 4 dot-separated segments, each matching `[a-z0-9]+(-[a-z0-9]+)*`. The first segment SHALL name a topic, not a section number.

#### Scenario: Valid ID
- **WHEN** the spec contains `r[bind.shadow.same-scope]`
- **THEN** the rule-ID check accepts it

#### Scenario: Invalid ID
- **WHEN** the spec contains `r[Bind_Shadow]` or `r[bind]`
- **THEN** the rule-ID check fails and names the tag and line

### Requirement: Rule IDs are unique and never reused
No ID SHALL appear in more than one tag. An ID listed in `spec/retired-rules.txt` SHALL NOT appear as a tag.

#### Scenario: Duplicate ID
- **WHEN** two tags in the spec carry the same ID
- **THEN** the rule-ID check fails and names both lines

#### Scenario: Retired ID reused
- **WHEN** a tag carries an ID listed in `spec/retired-rules.txt`
- **THEN** the rule-ID check fails and says the ID is retired

### Requirement: Grammar productions have derived IDs
Each production in the spec's grammar appendices SHALL have the rule ID `grammar.<production-name>` without a written tag.

#### Scenario: Production ID in coverage
- **WHEN** the coverage report is generated
- **THEN** it lists `grammar.fn_decl` among the rule IDs

### Requirement: Tags are pure insertions
Removing every tag (and the single space after it) from the spec SHALL yield text that differs from the untagged source only where the spec was deliberately edited. `cargo xtask spec-rules --strip` SHALL print the spec with tags removed.

#### Scenario: Strip after tagging
- **WHEN** the tagging change is applied to spec v0.4 and the spec is stripped
- **THEN** the output is byte-identical to spec v0.4
