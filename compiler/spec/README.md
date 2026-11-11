# quince_spec

Reads the language spec for `cargo xtask` and the conformance runner: its chapter manifest, its rule-ID tags and its grammar. This file defines the formats it checks. The spec itself says what Quince means, so nothing here restates it.

## Chapters

The spec is a folder of Markdown chapters. `spec/README.md` is its index, and `spec/chapters.txt` is the manifest that lists the chapters.

**The manifest.**
- A chapter line starts at column 0 and gives the chapter's rule-ID prefix, its file relative to `spec/`, and its title: `lex  reference/lexical.md  Lexical structure`.
- An indented line under a chapter gives one construct of that chapter and the subject it covers: `  if  If expressions`. Construct names are scoped by chapter, so two chapters may list the same name.
- Blank lines and `#` comments are ignored.
- The order of the chapter lines is the reading order, the coverage order and the order the grammar is assembled in.
- A prefix follows the ID segment format below. `grammar` is reserved, and no prefix or file is listed twice.

**Files.**
- A chapter file is a Markdown file directly in `spec/reference/` (the language) or `spec/tooling/` (the tools the language guarantees).
- Every `.md` file under those folders must be listed in the manifest, and every listed file must exist.
- **The transitional file.** While the spec is split into chapters, the one-file spec is listed as the chapter `legacy  quince-spec.md`, directly in `spec/`, with no constructs. While that file exists, a listed chapter file may be missing. `legacy` is reserved and never appears in a rule ID.

**The index.** `spec/README.md` holds the spec's version header, a line of the form `<date> · <author> · Draft vX.Y`, within its first 10 lines; the coverage report reads the version from it. It links every chapter file that exists, in manifest order.

`cargo xtask spec-rules` checks all of the above. Everything that reads the spec checks the manifest and its files first, so a broken manifest fails every spec step and the conformance run.

## Rule IDs

Every normative statement in the spec's chapter files carries an ID that conformance fixtures cite, so coverage can be counted per rule.

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
- No ID appears in two tags, in the same chapter file or in different ones. An ID is never renamed or reused.
- When a rule's meaning changes, its old ID goes in `spec/retired-rules.txt` and the new text gets a new ID. A retired ID can never be a tag again. The retired list may hold `grammar.*` IDs.
- A wording change that keeps the meaning keeps the ID.

**Grammar productions** have the derived ID `grammar.<production-name>`, with no written tag. A production whose derived ID is retired fails the grammar check.

**Spec changes.**
- A change to what the spec means bumps its minor draft version (v0.5 to v0.6) in `spec/README.md` in the same pull request, retires the IDs whose meaning changed, and tags the new text.
- A change that only adds tags, or rewords without changing meaning, doesn't bump the version.
- A change that adds normative text tags it in the same pull request. No check can tell a rule from prose, so review enforces this.

**Stripping.** `cargo xtask spec-rules --strip <file>` prints one chapter file with every tag, and the single space after it, removed, so a wording diff can ignore tags. The file is given relative to the repo root or as the manifest lists it. With no file, it prints every chapter file in manifest order, each after a line `<!-- spec/<path> -->`.

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

`cargo xtask spec-links` fails on any link that breaks these rules, naming the file and line, and suggests a near anchor when a fragment is close to one.
