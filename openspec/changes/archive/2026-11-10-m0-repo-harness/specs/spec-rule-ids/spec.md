# Spec Delta

## Purpose

Gives every normative statement in the Quince language spec a stable, citable ID, so conformance tests can name the rule they check and coverage can be measured per rule.

## ADDED Requirements

### Requirement: Rule tag syntax
A rule ID SHALL be written in the spec as `r[<id>]` directly before the sentence or sentences it covers. A tag covers text up to the next tag or the end of its paragraph, list item or table cell. Tags SHALL NOT appear in code blocks or headings.

#### Scenario: Tag inside a code block
- **WHEN** `r[bind.let]` appears inside a fenced code block in the spec
- **THEN** the rule-ID check fails and names the file and line

### Requirement: Tag placement
- A tag SHALL start a line or follow whitespace (including a list marker or a table bar), and SHALL be followed by a space.
- Text shaped like a tag anywhere else outside inline code SHALL be an error, so a misplaced tag can never silently drop out of coverage.
- Inline code spans are ignored. A backtick run left open at the end of a line is literal text, as in CommonMark.

#### Scenario: Tag glued to punctuation
- **WHEN** the spec contains `(r[bind.var] y)` or `**r[bind.var]**`
- **THEN** the rule-ID check fails and says a tag must start a line or follow a space

### Requirement: Rule ID format
An ID SHALL have 2 to 4 dot-separated segments, each matching `[a-z0-9]+(-[a-z0-9]+)*`. The first segment SHALL name a topic listed in `spec/rule-topics.txt`, never a section number. Every listed topic SHALL be used by at least one tag. The topic `grammar` is reserved for derived production IDs and SHALL NOT appear in a written tag.

#### Scenario: Valid ID
- **WHEN** the spec contains `r[bind.shadow.same-scope]` and `bind` is a listed topic
- **THEN** the rule-ID check accepts it

#### Scenario: Invalid ID
- **WHEN** the spec contains `r[Bind_Shadow]` or `r[bind]`
- **THEN** the rule-ID check fails and names the tag and line

#### Scenario: Unlisted topic
- **WHEN** the spec contains `r[types.nominal]` and `types` isn't in `spec/rule-topics.txt`
- **THEN** the rule-ID check fails, names the topic, and points to `spec/rule-topics.txt`

#### Scenario: Reserved topic
- **WHEN** the spec contains `r[grammar.block]`
- **THEN** the rule-ID check fails and says `grammar` is reserved

### Requirement: Rule IDs are unique and never reused
No ID SHALL appear in more than one tag. An ID listed in `spec/retired-rules.txt` SHALL NOT appear as a tag. The retired list MAY hold `grammar.*` IDs, and a production whose derived ID is retired SHALL fail the grammar check.

#### Scenario: Duplicate ID
- **WHEN** two tags in the spec carry the same ID
- **THEN** the rule-ID check fails and names both lines

#### Scenario: Retired ID reused
- **WHEN** a tag carries an ID listed in `spec/retired-rules.txt`
- **THEN** the rule-ID check fails and says the ID is retired

### Requirement: Grammar productions have derived IDs
Each production in the spec's grammar appendices SHALL have the rule ID `grammar.<production-name>` without a written tag.

#### Scenario: Retired production name comes back
- **WHEN** `grammar.old_arg` is retired and a production named `old_arg` is added to the spec
- **THEN** `cargo xtask spec-grammar` fails and says the ID is retired

### Requirement: Spec versions
A change to what the spec means SHALL bump the spec's minor draft version (for example v0.5 to v0.6) in the same pull request. A change that only adds tags or rewords without changing meaning SHALL NOT bump it.

#### Scenario: A rule's meaning changes
- **WHEN** a pull request changes what a rule requires
- **THEN** the same pull request retires the old ID, tags the new text, and bumps the draft version

### Requirement: New spec text is tagged
A change that adds normative text to the spec SHALL tag it in the same pull request. Review enforces this, since no check can tell normative text from prose.

#### Scenario: Spec change adds a rule
- **WHEN** a pull request adds a normative sentence to the spec
- **THEN** the same pull request tags it with a new rule ID listed under an existing or newly listed topic

### Requirement: Stripping tags
`cargo xtask spec-rules --strip` SHALL print the spec with every tag, and the single space after it, removed, so a wording diff can ignore tags. The tagging change itself SHALL change no wording.

#### Scenario: Strip after tagging
- **WHEN** the tagging change is applied to spec v0.4 and the spec is stripped
- **THEN** the output is byte-identical to spec v0.4
