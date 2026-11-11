# quince_spec

Reads the language spec for `cargo xtask` and the conformance runner: its chapter manifest, its rule-ID tags and its grammar. This file defines the formats it checks. The spec itself says what Quince means, so nothing here restates it.

## Chapters

The spec is a folder of Markdown chapters. `spec/README.md` is its index, and `spec/chapters.txt` is the manifest that lists the chapters.

**The manifest.**
- A chapter line starts at column 0 and gives the chapter's rule-ID prefix, its file relative to `spec/`, and its title: `lex  reference/lexical.md  Lexical structure`.
- An indented line under a chapter gives one construct of that chapter and the subject it covers: `  if  If expressions`. A construct is what the second segment of a longer rule ID names (below). Construct names are scoped by chapter, so two chapters may list the same name.
- Blank lines and `#` comments are ignored.
- The order of the chapter lines is the reading order, the coverage order and the order the grammar is assembled in.
- A prefix follows the ID segment format below. `grammar` is reserved, and no prefix or file is listed twice.

**Files.**
- A chapter file is a Markdown file directly in `spec/reference/` (the language) or `spec/tooling/` (the tools the language guarantees).
- Every `.md` file under those folders must be listed in the manifest, and every listed file must exist.

**The index.** `spec/README.md` holds the spec's version header, a line of the form `<date> · <author> · Draft vX.Y`, within its first 10 lines; the coverage report reads the version from it. It links every chapter file that exists, in manifest order.

`cargo xtask spec-rules` checks all of the above. Everything that reads the spec checks the manifest and its files first, so a broken manifest fails every spec step and the conformance run.

## Rule IDs

Every normative statement in the spec's chapter files carries an ID that conformance fixtures cite, so coverage can be counted per rule.

**Tags.** The format follows the Rust Reference:

```markdown
r[expr.op.plus-numeric]
`+` is numeric only.

r[effect.net]
- `Net`: Outbound network connections
```

- A tag is a line holding only `r[<id>]`, at column 0. A blank line, a heading or the start of the file comes before it, and the block it covers comes directly after it: a paragraph, or a top-level `-` or `*` list item.
- A tag covers exactly that block: the paragraph, or the item's own text and its lazy continuation lines. A tagged item has no nested block after its own text (an indented list, code block, table or paragraph, with or without a blank line before it), so a rule never extends into one. An untagged item directly after a tagged one, in the same list, is not covered by the tag.
- Every other block is untagged: examples, code, tables, rationale and lead-in text. No tag sits inside a paragraph, a table, a nested list item, a blockquote or a code block. A rule that needs several blocks is several rules, each with its own tag.
- A tag line can't be followed by an indented line, a `+` item, a numbered item, a table, a heading, a code block, a blockquote, an HTML block, a thematic break, a blank line or another tag line. A tagged paragraph has no setext underline (`---` or `===`), which would make it and its tag a heading.
- `r[` anywhere but a tag line, outside code blocks and inline code spans, is an error, so a misplaced tag can never silently drop out of coverage: `Text r[bind.var] more`, `(r[bind.var] y)`, `**r[bind.var]**` and an indented tag line all fail. A backtick run left open at the end of a line is literal text, as in CommonMark.
- GitHub shows a tag line joined to its paragraph ("r[x] Text…"), or on its own line above a bullet item.

**Format.**
- An ID is `chapter.construct.rule`, with an optional fourth segment for detail, such as `stmt.bind.shadow.same-scope`. It has 2 to 4 dot-separated segments, each matching `[a-z0-9]+(-[a-z0-9]+)*`.
- The first segment is the prefix of a chapter in `spec/chapters.txt`, so an ID says which chapter holds its rule. It must be the chapter of the file the tag is in.
- In an ID of 3 or 4 segments, the second segment is a construct listed under that chapter. Every listed construct must be named by at least one ID. Reuse a listed construct before adding one.
- An ID of 2 segments is about the chapter's own subject or the chapter as a whole, such as `lex.braces` or `type.float`.
- The prefix `grammar` is reserved for derived production IDs and can't appear in a written tag.

**Stability.**
- No ID appears in two tags, in the same chapter file or in different ones. An ID is never renamed or reused.
- **The one-time rename.** When the spec was split into chapters (2026-11-10), every ID merged before then was renamed once, by script, from `topic.rule` to `chapter.topic.rule`, or kept when its topic was its chapter's prefix. `docs/design/spec-restructure/id-map.tsv` maps each old ID to its new one. Any later change of ID is a move or a retirement.
- When a rule's meaning changes, its old ID goes in `spec/retired-rules.txt` and the new text gets a new ID. A retired ID can never be a tag again. The retired list may hold `grammar.*` IDs, though a changed production usually keeps its ID (see Grammar productions).
- **Moved rules.** A rule that moves to another chapter gets a new ID with that chapter's prefix, and `spec/moved-rules.txt` gets the line `<old> <new>`. A moved ID can never be a tag again. The new ID is a tag, or is retired if the rule's meaning later changed. When a rule moves again, its line is updated to the final ID, so no line points at another moved ID. Grammar IDs never move. `retired-rules.txt` keeps one meaning: the rule's meaning changed.
- A fixture that cites a retired ID fails. One that cites a moved ID fails and names the ID to cite.
- A wording change that keeps the meaning keeps the ID.

**Grammar productions** have the derived ID `grammar.<production-name>`, with no written tag. A production whose derived ID is retired fails the grammar check.
- A grammar ID is retired only when its production's name is to be reused for a different construct. A production that is widened or narrowed keeps its ID, since a fixture that cites it passes or fails on what the parser accepts now (decided by David, 2026-11-11).

**Spec changes.**
- A change to what the spec means bumps its minor draft version (v0.5 to v0.6) in `spec/README.md` in the same pull request, retires the IDs whose meaning changed, and tags the new text.
- A change that only adds tags, or rewords without changing meaning, doesn't bump the version.
- A change that adds normative text tags it in the same pull request. No check can tell a rule from prose, so review enforces this.

**Stripping.** `cargo xtask spec-rules --strip <file>` prints one chapter file with every tag line removed, so a wording diff can ignore tags. The file is given relative to the repo root or as the manifest lists it. With no file, it prints every chapter file in manifest order, each after a line `<!-- spec/<path> -->`.

`cargo xtask spec-rules` checks all of the above that a tool can check, and names the file and line of every problem.

## Grammar

`spec/grammar.ebnf` is a machine-readable copy of the spec's grammar that the parser is tested against.

- **Grammar blocks:** grammar lives in fenced code blocks whose info string is `ebnf`, each next to the prose it formalizes. A chapter may hold any number of them. Any other code block is an example.
- **Contents:** a header of `//` comment lines, then every `ebnf` block in manifest order (document order within a chapter), copied verbatim and separated by one blank line.
- **Editing:** the header (its terminal list) is the only hand-edited part. `cargo xtask spec-grammar --write` regenerates everything after it, keeping the existing header, or writing the default header when the file doesn't exist.

`cargo xtask spec-grammar` fails when:
- the file doesn't match the spec, printing a unified diff and naming `--write` as the fix;
- no chapter has an `ebnf` block, or one is never closed;
- an identifier on the right-hand side of a production, outside string literals and comments, is neither defined by a production nor listed as a terminal in the header;
- a production is defined twice;
- a production other than `file` is never referenced;
- a production's derived ID is retired.

Errors give the chapter file and line, because the grammar is fixed in the spec and then regenerated, never edited in the generated file.

## Links

Spec files point at each other with relative Markdown links, never by position, so text can move without breaking them.

- **Which links.** Every relative link in a Markdown file under `spec/` is checked, and so is every link from `std/API.md` whose target is under `spec/`. Inline links, images and link reference definitions count. Links inside code blocks and inline code spans don't. External links (with a URL scheme such as `https:`, or starting with `//`) aren't fetched.
- **Targets.** A link resolves against the folder of the file that holds it; a leading `/` means the repo root. It must name a file or folder that exists, and may not climb above the repo root.
- **Fragments.** A `#fragment` on a link to a Markdown file, or on a same-file `#fragment` link, must be the anchor of one of that file's headings. Fragments on links to other files aren't checked.
- **Anchors** follow GitHub's rules, as github-slugger implements them. Take the heading's text with each link replaced by its text, lowercase it, remove every character that isn't a letter, a digit, a space, `-` or `_`, and turn each space into `-`. ``### The `?` operator`` becomes `the--operator`. When a file has already used that anchor, append `-1`, then `-2`, and so on, skipping any anchor already taken. Only ATX headings (`#` to `######`) outside code blocks count. Emphasis written with `_` and HTML inside a heading aren't supported, because GitHub would drop the markup and this check keeps it.

- **No positional references.** Prose in a Markdown file under `spec/` never locates other text by position. `section` or `sections` followed by a number, `§` followed by a number, `appendix`, `(see ` followed by a capital letter, and a parenthesized `below` or `above` are errors, in any letter case. Write a relative link to the text instead: `(section 5)` becomes `([Effects](reference/effects.md))`. Headings are checked like prose. Code blocks, inline code and link destinations aren't prose. A bare "above" or "below" that points at the sentence next to it is allowed, because it moves with that sentence.

`cargo xtask spec-links` fails on any link that breaks these rules, and on any positional reference, naming the file and line. It suggests a near anchor when a fragment is close to one.
