# quince_spec

Reads the language spec for `cargo xtask` and the conformance runner: its rule-ID tags and its grammar. This file defines the formats it checks. The spec itself says what Quince means, so nothing here restates it.

## Rule IDs

Every normative statement in `spec/quince-spec.md` carries an ID that conformance fixtures cite, so coverage can be counted per rule.

**Tags.**
- A rule ID is written `r[<id>]` directly before the sentence or sentences it covers. A tag covers text up to the next tag, or to the end of its paragraph, list item or table cell.
- Tags never go in code blocks or headings, or before non-normative text such as rationale or examples.
- A tag starts a line or follows whitespace (including a list marker or a table bar), and is followed by a space. Tag-shaped text anywhere else outside inline code is an error, so a misplaced tag can never silently drop out of coverage: `(r[bind.var] y)` and `**r[bind.var]**` both fail.
- Inline code spans are ignored. A backtick run left open at the end of a line is literal text, as in CommonMark.

**Format.**
- An ID has 2 to 4 dot-separated segments, each matching `[a-z0-9]+(-[a-z0-9]+)*`, such as `bind.shadow.same-scope`.
- The first segment is a topic listed in `spec/rule-topics.txt`, never a section number. Every listed topic must be used by at least one tag. Reuse a listed topic before adding one.
- The topic `grammar` is reserved for derived production IDs and can't appear in a written tag.

**Stability.**
- No ID appears in two tags. An ID is never renamed or reused.
- When a rule's meaning changes, its old ID goes in `spec/retired-rules.txt` and the new text gets a new ID. A retired ID can never be a tag again. The retired list may hold `grammar.*` IDs.
- A wording change that keeps the meaning keeps the ID.

**Grammar productions** have the derived ID `grammar.<production-name>`, with no written tag. A production whose derived ID is retired fails the grammar check.

**Spec changes.**
- A change to what the spec means bumps its minor draft version (v0.5 to v0.6) in the same pull request, retires the IDs whose meaning changed, and tags the new text.
- A change that only adds tags, or rewords without changing meaning, doesn't bump the version.
- A change that adds normative text tags it in the same pull request. No check can tell a rule from prose, so review enforces this.

**Stripping.** `cargo xtask spec-rules --strip` prints the spec with every tag, and the single space after it, removed, so a wording diff can ignore tags.

`cargo xtask spec-rules` checks all of the above that a tool can check, and names the file and line of every problem.

## Grammar

`spec/grammar.ebnf` is a machine-readable copy of the spec's grammar that the parser is tested against.

- **Contents:** a header of `//` comment lines, then the code block under `## Appendix: Expression grammar`, one blank line, and the code block under `### Declarations, types and patterns`, each copied verbatim.
- **Code blocks:** each of those headings must be followed by exactly one code block before the next heading.
- **Editing:** the header (its terminal list) is the only hand-edited part. `cargo xtask spec-grammar --write` regenerates everything after it, keeping the existing header, or writing the default header when the file doesn't exist.

`cargo xtask spec-grammar` fails when:
- the file doesn't match the spec, printing a unified diff and naming `--write` as the fix;
- either heading is missing, or is followed by more than one code block;
- an identifier on the right-hand side of a production, outside string literals and comments, is neither defined by a production nor listed as a terminal in the header;
- a production is defined twice;
- a production other than `file` is never referenced;
- a production's derived ID is retired.

Errors give the line in `spec/quince-spec.md`, because the grammar is fixed in the spec and then regenerated, never edited in the generated file.
