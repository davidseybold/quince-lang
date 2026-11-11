# Spec restructure

Status: approved by David (PR #11, merged 2026-11-10), being built as a stack. The Tasks list tracks progress.

## Why

`spec/quince-spec.md` (Draft v0.5, 2,071 lines) is organized by theme, so rules about one construct are spread across sections:
- `if` is defined in "Blocks, statements and expressions" under section 2.
- `match` arm syntax is there too, but `match` exhaustiveness is in section 4.
- Loops are under "Keywords".

Toolchain, package-manager and rationale material sits between the language chapters, and about 530 lines are standard-library API. An agent that wants to know what `match` means has to read the whole file.

The Rust Reference and the Go spec both organize by construct and keep library API out. The Rust Reference also puts the grammar next to the prose and uses rule IDs that say where a rule lives (`expr.if.syntax`). The Go spec shows the same structure can stay terse.

Doing this before M1 means the first conformance fixtures cite final rule IDs. Coverage is still 0 of 974, so renaming IDs breaks nothing.

## What changes

- **The spec becomes a directory of chapters.**
  - `spec/README.md` is the index.
  - Language chapters go in `spec/reference/`.
  - Tooling chapters (package manager, toolchain, diagnostics) go in `spec/tooling/`.
  - `spec/quince-spec.md` is removed.
- **A chapter manifest**, `spec/chapters.txt`, replaces `spec/rule-topics.txt`. It lists each chapter's ID prefix, file and title, and the constructs under it.
- **Grammar inline per construct**, in fenced `ebnf` blocks. `spec/grammar.ebnf` is assembled from them, and the grammar appendices go away.
- **Hierarchical rule IDs**, `chapter.construct.rule[.detail]`, checked against the chapter that holds them.
  - 644 IDs are renamed once, by script. This is a one-time exception to "never renamed".
  - From then on, a rule that moves chapter is recorded in `spec/moved-rules.txt`.
- **No standard-library API in the spec.**
  - `spec/std-api.md` moves to `std/API.md`, which has no approval gate and is the plan for the M9 std build.
  - The Prelude appendix splits:
    - the language part becomes the chapter "Prelude and special interfaces";
    - the library part, with 34 tags, moves to `std/API.md` with its tags removed.
- **Positional references become links.** "section N", "§N", "appendix" and "(…, below)" become relative links, and a check stops them from coming back.
- **Rationale moves to `docs/`.**
- **No change in meaning.** A temporary `spec-compare` CI step proves it against a pinned baseline. Every removal and rewording is listed in a table.

## Context

The spec today, measured on `origin/main` at 81aa5f0: 894 rule tags, 80 grammar productions, twelve numbered sections and three appendices. M0 made it machine-checked:
- `quince_spec` parses tags and extracts the grammar;
- `xtask spec-rules` and `xtask spec-grammar` check them;
- the conformance runner reports coverage per `## ` heading.

All of that assumes one file. The formats are documented in `compiler/spec/README.md` and `tests/conformance/testdata/README.md`.

**Constructs are scattered.** "Blocks, statements and expressions" (§2) holds tags from ten topics, including `if`, `match`, loops, closures and assignment. Loop and `return` rules sit under "Keywords".

**Positional references are everywhere.** None survive a reorder unchanged:
- 147 "section N" in the spec: 114 on tagged lines, 5 inside code blocks;
- about 11 "appendix" references, one inside `r[prelude.effvars]`;
- about 13 heading-relative references, such as "(Packages, below)";
- 70 "§N" in `spec/std-api.md`.

**Non-language material** sits between the language chapters:
- §8 Toolchain;
- most of §7: manifest, dependencies, lockfile, versioning, publishing;
- §10's compilation model, profiles and targets;
- untagged rationale: §1, §10 Pipeline, §12.

**Standard-library API is inside the spec.** Most of the Prelude appendix (about 530 of its 690 lines) is method lists, std types and effect operations. It was put there so "the spec alone is enough to write code" (the v0.3 agent test).

**IDs are 2 or 3 segments** (831 and 63). No fixture cites any of them.

**Constraints:**
- **Approval.** The project `CLAUDE.md` says never edit `spec/` without David's approval. This doc, once approved, is that approval for the moves, reference rewrites, removals and rewordings it lists. No rule may change meaning.
- **ID stability.** `compiler/spec/README.md` says an ID is never renamed or reused. David agreed a one-time exception for this change (2026-11-10).
- **No new dependencies.** In particular, no mdBook.

## Goals and non-goals

**Goals:**
- `spec/` is the language spec and nothing else, like the Rust Reference and the Go spec: the language in Part I, the tools the language guarantees in Part II, and no library API.
- An agent finds everything about a construct in one chapter, and can tell from a rule ID which chapter holds it.
- Every M0 check keeps working across many files, and `cargo xtask ci` stays green on every layer.
- The restructure is provably meaning-preserving. Each surviving rule's text is unchanged, apart from its ID, its positional references and the listed rewordings, and every removal is listed.
- M1's lexer fixtures cite final IDs.

**Non-goals:**
- Any change to what Quince means, including rewording for terseness. Existing sentences move as they are. Go's terseness is the target for new text.
- Deduplicating the Prelude's signatures against `std-api.md`'s entries. Once both are in `std/API.md`, that's doc editing for M9.
- Rule IDs for the standard library. It moves to `std/`, where std's own tests and doc examples check it (M9).
- Rendering the spec as a website.

## Decisions

### 1. Layout

```
spec/
  README.md            index: title, version header, chapter list, reading order
  chapters.txt         manifest: chapter prefixes, files, titles, constructs (decision 3)
  reference/           Part I, the language: one file per chapter
  tooling/             Part II, the tools the language guarantees: one file per chapter
  grammar.ebnf         still generated, now from every chapter (decision 5)
  retired-rules.txt    unchanged
  moved-rules.txt      new: rules that moved chapter (decision 4)
std/
  API.md               was spec/std-api.md, plus the Prelude's library material (decision 2)
docs/design/spec-restructure/
  baseline/            the spec and std-api.md at 81aa5f0 (decision 8)
  id-map.tsv  refs.tsv  headings.tsv  removed.tsv  rewordings.tsv
```

- **Format.** Plain Markdown, readable on GitHub and with `cat`. Chapters link to each other with relative links.
- **Chapters.** Each chapter starts with `# <Title>`, then `## ` sections per construct. There are no section numbers anywhere.
- **`README.md`** holds:
  - the header line that `quince-spec.md` has today (date, author, `Draft v0.5`), within its first 10 lines, where `spec_version` looks (a test pins this);
  - a one-line summary of each chapter;
  - the reading order: Part I in order, Part II as needed;
  - a pointer to `std/API.md` as the non-normative std reference.
- **Deleted:** `spec/rule-topics.txt`, `spec/quince-spec.md` and `spec/std-api.md`.
- **The tables** live under `docs/design/spec-restructure/` and stay there after the change as its record. `id-map.tsv` is how anyone maps an old ID to its new one. The baseline is deleted in the cleanup layer.

This supersedes the repo plan's "one file per section" (`docs/repo-plan.md`). Chapters are per construct.

*Alternative: keep one file, reordered.* Rejected. An agent still loads 2,000 lines to read one construct.
*Alternative: mdBook.* Rejected. It's a new dependency, and nothing needs rendered HTML.

### 2. Chapters (approved by David, 2026-11-10)

Part I, `spec/reference/`:

| # | File | Prefix | Title | Takes these current topics (and sections) |
| --- | --- | --- | --- | --- |
| 1 | `introduction.md` | `intro` | Introduction | §1's opening paragraph; the grammar notation sentence; `excl` |
| 2 | `lexical.md` | `lex` | Lexical structure | `lex`, `name`, `lit`, `comment` |
| 3 | `packages.md` | `mod` | Packages and modules | `mod`, `import`, `pkg` (what a package is, project layout, program folders), `edition` |
| 4 | `items.md` | `item` | Items | `const`, `struct`, `enum`, `newtype`, `alias`, `iface`, `method`, `generic`, `arg`, `attr` |
| 5 | `types.md` | `type` | Types | `type`, `num`, `tuple`, `never`, `map`, `ref`, `mut`, `infer`, `type-arg` |
| 6 | `prelude.md` | `prelude` | Prelude and special interfaces | the language part of `prelude` (below), `user-op`, `iter`, `auto`, `dbg`, `wire` |
| 7 | `expressions.md` | `expr` | Expressions | `block`, `op`, `range`, `index`, `if`, `match`, `is`, `jump`, `ret`, `closure`, `fn-value` |
| 8 | `statements.md` | `stmt` | Statements | `bind`, `assign`, `loop` |
| 9 | `patterns.md` | `pat` | Patterns | `pat` |
| 10 | `errors.md` | `err` | Errors and failure | `err`, `try`, `panic` |
| 11 | `effects.md` | `effect` | Effects | `effect`, `effvar`, `handler`, `with` |
| 12 | `concurrency.md` | `conc` | Concurrency | `conc`, `fanout`, `background`, `cancel`, `timeout`, `share`, `chan`, `select` |
| 13 | `contracts.md` | `contract` | Contracts | `contract`, `verify` |
| 14 | `testing.md` | `test` | Testing | `test`, `doc`, `itest`, `prop`, `dtest` |
| 15 | `memory.md` | `mem` | Memory | `mem` |
| 16 | `execution.md` | `exec` | Program execution | `main`, `cap` |
| 17 | `interop.md` | `ffi` | Interop with C | `ffi`, `cbind`, `ctype`, `cbuf`, `cstring`, `cresource`, `trust`, `link`, `cmem`, `pool`, `callback`, `ffitest` |

Part II, `spec/tooling/`:

| # | File | Prefix | Title | Takes |
| --- | --- | --- | --- | --- |
| 18 | `dependencies.md` | `dep` | Dependencies and publishing | `manifest`, `dep`, `lock`, `mvs`, `update`, `hash`, `semver`, `publish` |
| 19 | `toolchain.md` | `tool` | Toolchain | `cli`, `perf`, `fmt`, `build`, `profile`, `target`, `incr`, `debug` |
| 20 | `diagnostics.md` | `diag` | Diagnostics and queries | `diag`, `query` |

All 105 current topics are placed exactly once: 104 in the tables above, and `std` out to `std/API.md`.

**Chapter order.**
- It follows the Rust Reference where Quince has the same thing: lexical structure, items, types, expressions, statements, patterns.
- Quince's own semantics come next: errors, effects, concurrency, contracts. The runtime-facing chapters come last.
- Editions sit in Part I, because they change what code means.

**Tooling stays normative in `spec/tooling/`.** Its 124 tags are rules a user relies on:
- `r[pkg.main-outside]` is a compile error;
- `r[diag.json]` is the agent contract;
- `r[semver.*]` is what `publish` refuses.

**The Prelude splits (approved by David, 2026-11-10).**
- The spec keeps what the language's syntax, semantics or compiler depend on, following Go's predeclared identifiers and the Rust Reference's "Special types and traits". The rest is library API.
- A declaration stays when a kept rule needs it to make sense.
- The five `prelude.*` tags in §7 (`only`, `contents`, `not-ref-types`, `fixed`, `criterion`) are about what the prelude is. They move to chapter 6 unchanged, like any rule whose topic is its chapter's prefix.

| Stays in the spec | Where |
| --- | --- |
| The table of prelude names and the sentence "A name is in the prelude only if…"; `r[prelude.hash-eq]`; `r[prelude.ord-eq]`, whose paragraph also holds the note that a body-less `fn` is a signature listing (the Introduction links to it) | Prelude and special interfaces |
| The Interfaces block (`Eq`, `Ord`, `Ordering`, `Hash`, `Hasher`, `Display`, `From`, `Encode`, `Decode`, `Iterator`, `DecodeError`) and `r[prelude.encode-data]`, `display-impls`, `eq-ord-impls`, `unit-url-eq`, `collection-eq`, `no-other-ord`. The language calls these interfaces for `==`, `<`, interpolation, `for` and `@wire`. | Prelude and special interfaces |
| The Functions block (`assert`, `todo`, `unreachable`, `dbg`) and `r[prelude.assert]` | Prelude and special interfaces |
| `r[prelude.tagged-values]`: tagged literals build their values at compile time (reworded, decision 6) | Prelude and special interfaces |
| `r[prelude.list-index]`: `xs[i]` is `xs.get(i)`, and it panics when out of range. This is a deliberate dependency of normative text on a method documented only in `std/API.md`. | Expressions (`expr.index`) |
| `r[prelude.map-order]`: `Map` and `Set` iterate in insertion order, which `for` exposes | Types (`type.map`) |
| `r[prelude.effect-ops]`, `r[prelude.effect-ops-no-import]`, `r[prelude.state-no-ops]`: how built-in effect operations are named and handled | Effects |
| **(approved by David, 2026-11-10)** The built-in effect declarations (`effect Console { … }`, `effect Fs { … }`, and the rest: about 75 lines). `r[handler.receiver]`, `r[effect.interface]` and `r[prelude.effect-ops]` say a handler implements each operation of an effect, so the fixed built-in effects' operation sets are part of the language. | Effects |
| **(approved by David, 2026-11-10)** The declarations of `FailurePolicy` and `ScopeFailed` (needed by `r[background.policy]`, `r[background.result]` and `r[background.scope-failed]`), `TimedOut` (`r[timeout.result]`) and `ChannelClosed` (`r[chan.send]`) | Concurrency |
| **(approved by David, 2026-11-10)** The declarations of `FsError` and `NetError`, the error enums that the `Fs` and `Net` operations return. `r[cap.violation]` returns `FsError.PermissionDenied` and `NetError.PermissionDenied`, and `r[test.default-fakes]` relies on `NetError.Refused`. The other types the effect declarations name (`Metadata`, `Outbound`, `Inbound`, `Listener`, `ProcessError`, `ProcessOutput`) are named only, so they move, a deliberate dependency like `list-index`'s. | Effects |
| The untagged notes after the effect declarations: a manifest limit returns `FsError.PermissionDenied`, and which operations are cancellation points | Effects |
| `r[prelude.broken-pipe]`: a write to closed standard output ends the program with code 0 | Program execution |

**Moves to `std/API.md`,** with 33 Prelude tags removed and the text kept:
- every method listing: numbers, `Bool`, `Char`, ranges, `String`, `Bytes`, `List`, `Iter`, `Map`, `Set`, `Option`, `Result`, `Duration`, `Timestamp`, `Url`, `Regex`;
- the rest of `std.sync`, `with_timeout` and `with_resource`, and the other types the effect operations use (`Metadata`, `Outbound`, `Inbound`, `Listener`, `ProcessError`, `ProcessOutput`);
- the Standard library index;
- the rules `r[prelude.effvars]`, `std-imports`, `expect`, `timeout-resource-pkg`, `int-conversions`, `int-consts`, `float-consts`, `float32`, `bool`, `range-step`, `range-step-methods`, `string-no-len`, `string-immutable`, `list-no-inplace`, `no-for-each`, `iter-type`, `iter-lazy`, `iter-effects`, `map-from`, `try-from`, `map-update`, `time-display`, `regex-imports`, `parse-error`, `sync-imports`, `ref-self`, `no-unbounded`, `pool-open`, `pool-checkout-err`, `taskscope-no-methods`, `effect-types-import`, `conn-ref` and `inbound-listen`;
- `r[std.no-unwrap]`, from §4, as the 34th tag.

**Borderline rules (decided by David, 2026-11-10): all move.**
- `string-no-len` commits the compiler to particular suggested fixes for `s.len()`. David chose not to pin that in the spec.
- The rest repeat or follow from rules that stay:
  - `ref-self` follows from `r[mut.var-self.call]` plus the signatures;
  - `conn-ref` repeats the §3 row `r[ref.net]`;
  - `taskscope-no-methods`'s `Resource` restriction is already `r[ffi.package-only]`;
  - `try-from` and `inbound-listen` are library conventions.

**What this supersedes.** The v0.3 decision that the spec alone is enough to write code.
- An agent now writes code from the spec plus `std/API.md`, and from M5 on, the spec plus `quince query` (`r[query.members]`, `r[query.signature]`). `quince query` reads the implementation, so it can't drift.
- The agent test changes to match (decision 12).
- Coverage drops by 34 rules, to 940 IDs.

**Out to `docs/`.**
- §1 Target domain and Core principles, and §12 Open questions, go to a new `docs/design-principles.md`.
- The §10 Pipeline list goes to `docs/repo-plan.md` as a holding place. The project `CLAUDE.md` puts pipeline decisions in the README of the crate that enforces them, which takes this text over once that crate exists.
- All of it is untagged. Tagged text that stays part of the language never leaves `spec/`: `r[build.llvm-only]` and its rationale go to Toolchain as they are.

### 3. The chapter manifest, `spec/chapters.txt`

```
# chapter  file                           title
intro      reference/introduction.md      Introduction
  excl       Features deliberately excluded
lex        reference/lexical.md           Lexical structure
  continue   Line continuation
  name       Naming conventions
  ...
```

- **Lines.**
  - A chapter line gives the prefix, the file (relative to `spec/`) and the title.
  - An indented line lists a construct and its subject, as `rule-topics.txt` does today.
  - Construct names are scoped by chapter, so two chapters may list the same name.
- **Order.** The order of the chapter lines is the reading order, the coverage order and the grammar assembly order. A check fails if `README.md` doesn't link every chapter file in that order.
- **Files.** Every chapter `.md` under `spec/reference/` and `spec/tooling/` must be listed. A listed file must exist, except while the transitional file exists.
- **The transitional file.** During the transition, `quince-spec.md` is listed as `legacy  quince-spec.md  Quince Language Spec (being split)`, with no constructs. `legacy` is reserved and never appears in an ID.
- **API.** `quince_spec` exposes the manifest. `CHAPTERS_PATH` and `README_PATH` replace `SPEC_PATH` and `TOPICS_PATH`.

### 4. Rule IDs

**Format.** 2 to 4 segments, as today.
- The **first segment is a chapter prefix**. In a chapter file, it must be that file's chapter.
- In an ID with 3 or 4 segments, the second segment is a construct listed under that chapter.
- A 2-segment ID is about the chapter's own construct, or about the chapter as a whole (`lex.braces`, `type.never`).

**Check.** `spec-rules` fails, naming the file and line, on:
- a tag whose prefix isn't its file's chapter (in the `legacy` file: isn't any chapter's);
- a construct that isn't listed under the chapter;
- a listed construct that no tag uses.

**The rename is mechanical.**
- An old ID `T.rest` whose topic `T` maps to chapter `C` becomes `C.T.rest`, and `T` becomes a construct of `C`. For example, `if.type` becomes `expr.if.type`, and `bind.shadow.same-scope` becomes `stmt.bind.shadow.same-scope`.
- When `T` already is `C`'s prefix (`lex`, `mod`, `type`, `prelude`, `pat`, `err`, `effect`, `conc`, `contract`, `test`, `mem`, `ffi`, `dep`, `diag`), the ID stays as it is.
- Every renamed ID gains exactly one segment, and none of today's has 4, so none goes past 4. Renamed 3-segment IDs reach the cap, so they can't get sub-rules. Raising the cap later is backward compatible.

**Hand exceptions.** A rule placed outside its topic's chapter takes the chapter where it sits.
- These are the six kept Prelude rules placed in other chapters, and rules kept next to a neighbour (decision 6).
- Each is marked `hand` in the table.
- More than 40 means the chapter list is wrong: stop and ask David.

**Numbers.** Of 894 IDs, 34 are removed, 216 stay and 644 change. A script generates `id-map.tsv`, with columns: old ID, new ID or `removed`, and `mechanical` or `hand`.

**The one-time exception.**
- Renamed and removed IDs don't go in `retired-rules.txt` or `moved-rules.txt`.
- `compiler/spec/README.md` records that IDs merged before this change were renamed once by it, and points at `id-map.tsv`.
- This also supersedes M0's reason for topic IDs, that "moving text between sections never changes an ID" (`docs/design/m0-repo-harness.md`, decision 3). IDs now say where a rule lives, and `moved-rules.txt` keeps later moves safe.

**Later moves (approved by David, 2026-11-10).** A rule that moves chapter after this change gets a new ID with the new prefix, and `spec/moved-rules.txt` gets the line `old new`.
- A moved ID is never a tag again.
- The new ID is a live tag, or is listed in `retired-rules.txt` if its meaning later changed.
- When a rule moves twice, the earlier line is updated to point at the final ID.
- The conformance runner fails a fixture that cites a moved ID with "rule `<old>` moved: cite `<new>`", or "moved to `<new>`, which is retired".
- `retired-rules.txt` keeps one meaning: the rule's meaning changed.

**Grammar IDs.** `grammar.<production>` IDs are unchanged, and `grammar` stays reserved.

*Alternative: hierarchical IDs without the prefix check, as in the Rust Reference.* Rejected. An unchecked prefix stops telling the truth without anyone noticing.
*Alternative: always 3 or 4 segments.* Rejected. It needs about 200 hand-picked construct names for no gain.

### 5. Grammar inline

- **Blocks.** Grammar lives in fenced blocks whose info string is `ebnf`. All 54 blocks today have no info string. Any block that isn't `ebnf` is an example. A chapter may hold any number of `ebnf` blocks, each next to the prose it formalizes.
- **The assembled file.** `spec/grammar.ebnf` is the hand-edited header, then every `ebnf` block in manifest order (document order within a chapter), separated by one blank line. `--write` regenerates it, and the agreement check prints a diff, as today.
- **Checks.** The closure, duplicate, unused-production and retired-name checks are unchanged. Errors name the file and line in the chapter. The header's and `DEFAULT_HEADER`'s mentions of the appendices are updated.
- **A production unit** is its first line (`name =`), every following line that starts with whitespace, and the comments on those lines. A comment line standing alone between productions is its own unit, and moves with the production after it.
- **Where productions go.** Each production moves to the chapter of the construct it defines: `if_expr` to Expressions, `struct_decl` to Items. `file` and `import` go to Packages and modules. Chapters 1 and 2 hold no productions, so the assembled file starts with `file`.
- **Appendix prose.**
  - The lexer rules and identifier classes go to Lexical structure: `r[lex.grammar-rules]`, `r[lex.ident-classes]`, `r[lex.ident-not-keyword]`, `r[lex.c-name]`, and the sentences defining `path` and the token names.
  - The notation sentence goes to the Introduction.
  - Three sentences that only describe the appendices are removed (`removed.tsv`):
    - "The grammar of expressions and statements, as section 2 describes them."
    - "Generic arguments … are in section 2, Type arguments and construction; patterns are in section 4."
    - "The rest of the grammar, as sections 2 to 11 describe it."

### 6. Moving text

- **The unit of a move is a tagged rule**, not a section. A paragraph holding rules from two chapters is split between them.
- **Neighbours stay together when splitting would change the reading.** A rule that starts with "So", "Likewise", "This", "They" or "Such", or that names something only the previous sentence introduces, stays next to that sentence and takes the chapter where it sits.
- **Flagged units.** The move script flags every unit that:
  - starts with one of those words;
  - contains "these", "those", "above", "below" or "previous";
  - has an antecedent that moves elsewhere.

  Each move PR lists the flagged units and where they landed, and a reviewer checks them. `spec-compare` can't see context.
- **Untagged text** (examples, code blocks, rationale) moves with the rule it illustrates.
- **Code blocks.** A code block may split at a declaration boundary when its declarations belong to different chapters or files: the Prelude's std.sync block, for example. A declaration unit is a line at column 0 and every following line up to the next line at column 0, except that a line which only closes (`}`, `)` or `]`) belongs to the unit before it. Blank lines separate units and belong to none. A column-0 `//` comment line is its own unit and moves with the declaration after it. Each part keeps the fence.
- **Tables** split by row when their rows belong to different chapters. Each part repeats the header row. Rows bound for a chapter that a later layer creates wait in `quince-spec.md`.
- **Allowed additions:**
  - headings (through `headings.tsv`);
  - each chapter's `# Title` line;
  - `README.md`;
  - the Introduction's link to the signature-listing note.
- **No connective prose.** None is written to smooth a split. If a split reads badly, the rule stays where it reads well.

**Text the split makes false.** Each of these is either removed (`removed.tsv`) or reworded (`rewordings.tsv`). The two rewordings are the only tagged sentences whose wording changes beyond their positional references, and David approved them (2026-11-10):

| Where | Text | Action |
| --- | --- | --- |
| Prelude intro (untagged) | "This appendix is the single source for each prelude signature; the standard library API doc cites these lines and adds cost, failure and example notes." | Removed |
| Prelude, before `r[prelude.std-imports]` (untagged) | "This appendix also gives the signatures of the std types and functions that the spec's own sections and the prelude's signatures use, so the spec alone is enough to write code." | Removed: it states the superseded decision |
| `r[prelude.only]` (§7) | "…Appendix: Prelude lists every one with its signature" | Reworded: "…the [table of prelude names](#prelude-names) lists every one" (the signatures of most types are now in `std/API.md`) |
| `r[prelude.tagged-values]` | "The tagged literals … build these values at compile time" (its antecedent, the Duration/Url block, moves) | Reworded: "build `Duration`, `Timestamp`, `Url` and `Regex` values at compile time" |
| `r[prelude.state-no-ops]` | "the permission for the reference-type methods above" (those methods move) | Positional reference: "above" becomes a link to the std.sync section of `std/API.md` (`refs.tsv`) |
| Effects, line 549 (untagged) | "…are listed in Appendix: Prelude" | Positional reference: becomes a link to the declarations in Effects |

### 7. Positional references

- **Definition.** A positional reference is text that locates other text by position or name: "section N", "§N", "Appendix: …", "this appendix", "(Heading, below)", "see Heading above", "(see Heading)", or a bare "above" or "below" that points at another rule.
- **Finding them.** A scan of `spec/` and `std-api.md` for these patterns builds the full list. Every hit is reviewed. A bare "above" that still points at its neighbour stays, but every parenthesized "(below)" or "(above)" is rewritten as a link, even a local one, so the leftover check has no exceptions. The scan includes `\(see [A-Z]`, which finds the five name-only references ("(see Ranges)", "(see Inference rules)", "(see Cancellation)" twice, "(see Iter)"). Each rewrite is a row in `refs.tsv`: file, rule ID or block, old phrase, new phrase, and **final link target**.
- **Rewrites.**
  - **Outside code blocks,** the reference becomes a relative Markdown link, and the surrounding words are unchanged: `section 5` → `[Effects](effects.md)`.
  - **Inside code blocks,** it becomes the plain target name: `(section 11)` → `(Interop with C)`.
  - **Self-references** become plain text: "this appendix" → "this chapter", or "this file" in `std/API.md`.
- **Ambiguous targets.** The row names the rule the reference means. When the text doesn't say, it links to the chapter holding most of the old section, marked `review`.
- **One rewrite, then mechanical retargeting.**
  - A single layer rewrites every reference to its final words, with links pointing at the matching heading in `quince-spec.md` for now.
  - Move layers change only link targets, by script, through `headings.tsv`.
  - In the last move layer, a check asserts that every link equals its recorded final target in `refs.tsv`. That catches a link whose words name one chapter while its target followed an old heading into another.
- **New check `spec-links`,** with its own `cargo xtask spec-links` subcommand and a `ci` step:
  - Every relative link in a Markdown file under `spec/`, and every link from `std/API.md` into `spec/`, names an existing file.
  - A `#fragment` must name a heading in that file, under GitHub's anchor rules as github-slugger implements them, with unit tests on the spec's awkward headings.
  - External links aren't fetched.
- **Leftover check.** From the reference layer on, `spec-links` also fails on prose in the Markdown files under `spec/` that matches `section [0-9]`, `§ ?[0-9]`, `[Aa]ppendix`, `\(see [A-Z]`, or a parenthesized "below" or "above". Headings, link destinations and `grammar.ebnf` aren't checked, so the `legacy` file's "Appendix" headings and the interim links to them pass until layer 10 deletes the file. From layer 10 on, headings are checked too.
- **Interim headings.** Layer 7 adds a `### Prelude names` heading above the names table in `quince-spec.md` (a `headings.tsv` row), so `r[prelude.only]`'s link to `#prelude-names` resolves from the start. The same layer updates the `grammar.ebnf` header and `DEFAULT_HEADER` wording to stop calling the grammar "the two grammar appendices".

### 8. Proving nothing changed: `cargo xtask spec-compare`

**Baseline.** The `compare` layer commits `spec/quince-spec.md` and `spec/std-api.md` as they are at 81aa5f0 to `docs/design/spec-restructure/baseline/`. Every comparison runs against the baseline, so merged layers and rebases can't move it.

**Enforcement.** `spec-compare` is a `ci` step from the `compare` layer until the cleanup layer deletes it with the baseline.

**Before and after.**
- "Before" is the spec baseline.
- "After" is:
  - every manifest file, including the leftover `quince-spec.md`;
  - `docs/design-principles.md`;
  - the Pipeline section of `docs/repo-plan.md`;
  - `std/API.md`, as a place where spec units may land.
- `std/API.md` contributes to "after" only its `to-std` units and its units carrying a `removed` ID, matched by content. Its other text is ignored, so it can't fail the check or inflate the multiset.
- `std-api.md`'s own text isn't compared, because `std/API.md` has no approval gate. Its references are still rewritten, and `spec-links` checks its links into `spec/`. Edits to its own text, such as fixing the sentences that say the spec holds the prelude signatures, are listed in the PR that makes them.

**Units.**
- one per tagged rule: the tag and the text it covers;
- untagged paragraphs, list items, table rows and code blocks;
- production units and declaration units;
- headings.

The segmenter that extracts the text a tag covers is new code (`rules::parse` only records lines). It gets unit tests with planted losses, edits, duplicates and swaps.

**Normalization, each with its reason:**
- **References.** `refs.tsv` is applied. Link targets are ignored, because `spec-links` and the final-target check cover them.
- **Rewordings.** `rewordings.tsv` is applied.
- **IDs.** They map through `id-map.tsv`, and tags marked `removed` are stripped. Layer 5 adds the 34 `removed` rows, and layer 6 adds the renames.
- **Whitespace.** Each unit is trimmed at both ends, and the space before a tag at a split point is dropped. Inner whitespace is compared exactly.
- **List markers.** `-` and `*` count as the same. The spec alternates them to keep neighbouring lists apart, and a split can make two lists neighbours.
- **Info strings.** The `ebnf` info string is ignored.

**It fails unless:**
- every surviving rule has the same text before and after, under its mapped ID, and sits in a manifest file;
- every unit whose ID is `removed` sits only in `std/API.md`, with its tag stripped;
- an untagged unit sits in `std/API.md` only if `removed.tsv` lists it as `to-std`;
- the tag count is 894 before layer 5 and 860 from layer 5 on, and the production count is 80;
- every old heading appears in `headings.tsv`, and every new heading is either a mapped target or listed as new;
- the untagged units are the same multiset before and after, apart from:
  - headings;
  - repeated table header rows and code fences;
  - `README.md`;
  - the Introduction's link;
  - the units in `removed.tsv` (removed outright, or `to-std`).

**Output.**
- each unmatched unit, with its location;
- the tag count before and after: 894, then 860 after layer 5;
- the production count: 80 both;
- the counts of units moved, removed, reworded and added.

`spec-rules`, `spec-grammar` and `spec-links` run on every layer as part of `ci`.

**Stripping.** `cargo xtask spec-rules --strip` takes an optional chapter file. With one, it prints that file with its tags removed; without one, it prints every manifest file in order, each after a line `<!-- spec/<path> -->`. `compiler/spec/README.md` says so from layer 2.

### 9. Coverage report

- **Sections** are chapters in manifest order, named by chapter title. The `legacy` file counts as one section while it exists, and `Grammar productions` stays last.
- **JSON shape.** Unchanged.
- **`spec_version`** comes from `spec/README.md`.
- **Example diagnostic.** The `quince_diagnostics` test that reads it from the spec reads `spec/tooling/diagnostics.md`, starting with the layer that moves it there.

### 10. Spec version (approved by David, 2026-11-10)

The spec stays at **Draft v0.5**.
- No meaning changes.
- Taking library API out changes what is specified, not what any code means.
- The README notes the date the layout and IDs changed, and links `id-map.tsv`.

### 11. Delivery as a stack

On a new worktree off `main`, once the OpenSpec removal (#10) is merged. Each layer passes `cargo xtask ci` on its own, and each layer updates the docs whose contract it changes: `compiler/spec/README.md`, `tests/conformance/testdata/README.md`, `compiler/diagnostics/README.md`, `AGENTS.md`.

1. **`design`:** this doc.
2. **`manifest`:**
   - `chapters.txt` with the `legacy` entry;
   - `README.md` with the header;
   - `ebnf` info strings on the two appendix blocks, and grammar assembly from `ebnf` blocks;
   - rule parsing across files;
   - coverage by chapter;
   - the pointers to `spec/quince-spec.md` (project `CLAUDE.md`, `AGENTS.md`, the testdata README, `docs/kickoff-prompt.md`, the xtask help, the crate docs) now point at `spec/README.md`.

   - `AGENTS.md` gets a "Commits" section: Conventional Commits for every commit subject and PR title, `type(scope): imperative summary` (David, 2026-11-10). Squash merges use the PR title.

   The two `spec/` edits (the header move and the info strings) come before `spec-compare` exists, so a manual diff in the PR covers them.
3. **`links`:** `spec-links` and its subcommand.
4. **`compare`:** `spec-compare`, the segmenter, the baseline and the `ci` step.
5. **`move-library`:**
   - `spec/std-api.md` moves to `std/API.md`;
   - the Prelude's library material and `r[std.no-unwrap]` are appended to it, with their tags removed;
   - `id-map.tsv` starts, with its 34 `removed` rows;
   - `std` is deleted from `rule-topics.txt`;
   - `std/README.md`, the project `CLAUDE.md` table, `docs/milestones.md` and `docs/repo-plan.md` (their `spec/std-api.md` paths) and `AGENTS.md` are updated. `AGENTS.md` says: for std signatures, read `std/API.md`; once `quince query` works, use it.
6. **`rename-ids`:**
   - the renames are added to `id-map.tsv`;
   - every chapter and construct goes in `chapters.txt`;
   - `rule-topics.txt` is deleted;
   - an empty `moved-rules.txt` is added, and the runner learns moved IDs;
   - the chapter checks are turned on;
   - `compiler/spec/README.md` describes the new ID format, chapters and `moved-rules.txt`, and `AGENTS.md`'s ID example is updated. (`diag.*` IDs keep their names, since `diag` is its own chapter's prefix.)

   From here on, IDs are final, and M1 may cite them.
7. **`references`:** every positional reference is rewritten (decision 7), with the reworded and removed text from decision 6 that isn't tied to a move. `refs.tsv` and `rewordings.tsv` are added, and the leftover check is turned on.
8. **`move-core`:** chapters 1 to 9, about 600 spec lines.
9. **`move-semantics`:** chapters 10 to 17, about 600 lines.
10. **`move-tooling`:**
    - chapters 18 to 20;
    - `docs/design-principles.md`;
    - the Pipeline goes into `docs/repo-plan.md`;
    - `quince-spec.md` and the `legacy` entry are deleted;
    - the final-target link check runs.
11. **`cleanup`:**
    - `docs/repo-plan.md`: layout and pipeline;
    - `docs/milestones.md`: chapter names instead of section numbers, the stale v0.4 mentions, and M9's notes from decision 12;
    - `spec-compare` and the baseline are deleted;
    - this doc's status becomes "done".

**Layer sizes.**
- Layers 2 to 4 are code with tests, each a few hundred lines.
- A move counts twice in a diff, as a deletion and an addition, so layers 5, 8 and 9 come to 1,100 to 1,300 changed lines each. A chapter is the smallest move that leaves both files readable.
- Each PR says this. Its review is the green `spec-compare`, the table rows it adds, its flagged units, and a read of the new chapters.
- Layer 6 touches about 640 tag lines.

**`spec/` is frozen on `main` while the stack is open.** Any other spec edit waits, or goes into the stack in the layer that owns that text.

### 12. Agents and milestones after the split

- **The v0.3 agent test** gives a fresh agent the spec plus `std/API.md`, and from M5 on, the spec plus `quince query`.
- **M9's exit** gains two requirements, because docs generated from the std source replace `std/API.md`:
  - every std function declares its cost in a form the compiler reads, which `r[contract.cost-warn]` needs;
  - `quince query signature` and `quince query members` cover every prelude and std item.
- **As each M9 package lands,** its doc comments take over its section of `std/API.md`, and that section is deleted.

## Risks

- **`spec-compare` hides a lost or altered sentence.** The segmenter is tested with planted failures, the tag and production counts are an independent check, and `spec-rules` fails when a lost rule leaves a construct unused.
- **The baseline drifts.** It's pinned under `docs/design/`, and `spec/` is frozen on `main`.
- **A positional reference is missed, or points to the wrong place.** The scan builds the list, the leftover check stops any from coming back, `spec-links` checks targets, and the final-target check catches mismatched links.
- **A split reads worse, or a rule loses its context.** Decision 6 keeps dependent sentences together, the move script flags context-dependent units, and reviewers read each chapter whole.
- **Kept text becomes false.** Decision 6 lists the known cases, and the flagged-unit review catches others.
- **A borderline Prelude rule lands on the wrong side.** David checks the lists. Moving one back later is an ordinary spec change.
- **A later cross-chapter move breaks fixtures.** `moved-rules.txt` says which ID to cite.
- **Agents read stale pointers mid-stack.** Layer 2 points them all at `spec/README.md`.
- **The move PRs are large.** See decision 11.

## Rollback

Nothing outside the repo consumes the spec, and no fixture cites an ID.
- **Partial merge.** Every layer leaves `main` consistent and checked. If the stack stalls after a move layer, the spec is split between `quince-spec.md` and the chapters, every check covers both, and that state can last. IDs are final once layer 6 lands.
- **Revert.** Before M1 cites an ID, the author of the stack reverts merged layers in reverse order, on David's request. Once M1 fixtures cite the new IDs, layer 6 and everything below it are fixed forward, not reverted.

## Open questions

None. David settled the chapter list, the tooling chapters, the ID scheme, moved rules, the Prelude split and its borderline rules, the rewordings, and the version (2026-11-10).

## Tasks

One `gh stack`, a PR per layer (decision 11). Tick each box as it lands on its branch. Run the pr-reviewer loop on the whole stack before handing it to David.

- [x] 1. `design`: this doc reviewed and approved
- [ ] 2. `manifest`, as two PRs to stay under the size limit: `manifest`, then `grammar-blocks`
  - [x] `chapters.txt` with `legacy`; manifest parsing in `quince_spec`
  - [x] `spec/README.md` with the header (test: within 10 lines); manual diff in the PR
  - [ ] `ebnf` info strings (with `grammar-blocks`)
  - [ ] grammar assembled from `ebnf` blocks; errors with file and line (second PR, `grammar-blocks`)
  - [x] rule parsing across files; `--strip` across files
  - [x] coverage by chapter; `spec_version` from the README
  - [x] pointers to `spec/quince-spec.md` moved to `spec/README.md`; `compiler/spec/README.md` and the testdata README updated
  - [x] `AGENTS.md`: Conventional Commits section
- [ ] 3. `links`: `spec-links` with github-slugger anchors, its subcommand and `ci` step, tests on awkward headings
- [ ] 4. `compare`: baseline committed; segmenter with planted-failure tests; production and declaration units; placement and count checks; `ci` step
- [ ] 5. `move-library`
  - [ ] `std/API.md` from `spec/std-api.md`, plus the Prelude's library material, tags stripped
  - [ ] `id-map.tsv` with 34 `removed` rows; `removed.tsv` `to-std` rows; `std` out of `rule-topics.txt`
  - [ ] `std/README.md`, `CLAUDE.md`, `AGENTS.md`, `docs/milestones.md`, `docs/repo-plan.md` paths
- [ ] 6. `rename-ids`
  - [ ] `id-map.tsv` renames, with `hand` rows (40 or fewer)
  - [ ] `chapters.txt` constructs; `rule-topics.txt` deleted; chapter checks on
  - [ ] `moved-rules.txt` and the runner's moved-ID error
  - [ ] `compiler/spec/README.md`, `tests/conformance/testdata/README.md`, `AGENTS.md`
- [ ] 7. `references`: scan; `refs.tsv` with final targets; `rewordings.tsv`; removals not tied to a move; `### Prelude names` interim heading; `grammar.ebnf` header and `DEFAULT_HEADER` wording; leftover check on (prose only)
- [ ] 8. `move-core`: chapters 1 to 9; `headings.tsv` rows; flagged units listed in the PR
- [ ] 9. `move-semantics`: chapters 10 to 17; flagged units listed
- [ ] 10. `move-tooling`: chapters 18 to 20; `docs/design-principles.md`; Pipeline to `docs/repo-plan.md`; `quince-spec.md` and `legacy` deleted; final-target check; diagnostics example test path; leftover check extended to headings
- [ ] 11. `cleanup`: `docs/repo-plan.md`, `docs/milestones.md` (chapters, v0.4, M9 notes); `spec-compare` and baseline deleted; status "done"
