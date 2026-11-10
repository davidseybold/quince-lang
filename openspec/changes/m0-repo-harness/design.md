# Design

## Context

The repo holds only the spec (`spec/quince-spec.md`, Draft v0.4), the std API, the repo plan and the milestones. See proposal.md for why M0 comes first. Constraints that shape the design:

- `CLAUDE.md`: one command for all checks, small crates, and the core loop of spec rule, then conformance test, then code. Never edit `spec/` without David's approval.
- The repo root can't hold a `quince.toml`, and `.qnc` fixtures live in `testdata/` folders, which are never compiled.
- No compiler exists yet, so the conformance runner can't execute anything in M0. It needs a seam where M1's parser plugs in.
- Rust was not installed on the dev machine. M0 installs it with rustup (Rust 1.99.1).

## Goals / Non-Goals

**Goals:**
- Every check that later milestones depend on (rule IDs, grammar agreement, error-code coverage, conformance coverage) exists and runs in `cargo xtask ci` from day one, against an empty suite.
- The fixture format and rule-ID format are settled now, because every later test depends on them.

**Non-Goals:**
- Any lexing, parsing or checking of Quince. M1 starts that.
- Fixtures that are whole projects with a `quince.toml` and several files. Those arrive with M2 (see Open Questions).
- `run` fixtures with expected output actually executing. The format is defined now, but no backend exists until M6.
- Rule IDs in `spec/std-api.md` (see Open Questions).
- The `quince` CLI's JSON output stream (array vs. JSON Lines). That comes with M5.
- Conformance coverage of spec sections 1 and 8. The milestones list them under M0 because M0 builds the harness they describe. Their fixtures arrive with the features they test: `excl.*` from M1 on, `diag.*` with M5's agent tooling.

## Decisions

### 1. Workspace layout

A root `Cargo.toml` with `members = ["compiler/*", "runtime", "tests/conformance", "xtask"]`, edition 2024, and shared lint settings in `[workspace.lints]`. Shared dependency versions live in `[workspace.dependencies]`.

The crates follow the repo plan: `compiler/{syntax,types,effects,diagnostics,query,ir,codegen-llvm,pkg,bind,native,lsp,cli}` named `quince_<dir>` (hyphens become underscores), plus `runtime/` as `quince_runtime` (`crate-type = ["staticlib", "rlib"]`). `cli` builds the `quince` binary, which for now only prints its version. Each crate starts with a one-line doc comment saying what it will hold. No crate depends on LLVM in M0.

M0 adds one crate the repo plan doesn't list: `compiler/spec` (`quince_spec`), which reads the spec text. It parses rule tags, extracts the grammar, and owns the paths of the spec files. Both xtask and the conformance runner need rule IDs, and a shared crate keeps one parser. `docs/repo-plan.md` gets the row when the plan is next revised.

`std/` is created with a README only. A `std/quince.toml` would be a project manifest, and it's better added in M6, once something reads it.

*Alternative:* one `compiler/` workspace separate from the root. Rejected: `cargo xtask` has to run from the root, and two workspaces mean two lockfiles.

The toolchain is pinned in `rust-toolchain.toml` to the exact stable version installed at setup, with `rustfmt` and `clippy`.

These files also go in at the root:
- `.gitignore`, covering `target/`, `.claude/worktrees/` and `.claude/agent-memory-local/`;
- `.gitattributes`, set to `* -text`, so git never rewrites line endings. That keeps the byte comparisons stable on every platform and lets M1 commit CRLF fixtures.

### 2. `cargo xtask ci`

`xtask` is a binary crate aliased in `.cargo/config.toml`. Its only dependencies are the workspace crates it checks with (`quince_spec`, `quince_diagnostics`, `quince_conformance`) and `serde_json`. It SHALL never depend, directly or through the runner, on a crate that links LLVM, so `cargo xtask spec-rules` stays fast. Open Questions covers how backends are wired.

`ci` runs these steps and **runs all of them even after a failure**, then prints a pass/fail table and exits non-zero if any failed. This mirrors the compiler's "report all independent errors" rule.

1. `cargo fmt --all --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test --workspace`
4. `spec-rules`: rule-ID format, placement, topics, uniqueness, retired-ID reuse (decision 3)
5. `spec-grammar`: `spec/grammar.ebnf` agrees with the spec, and the grammar is closed (decision 4)
6. `diagnostics`: every registered code has explain text, and every live code has a conformance test (decision 5)
7. `conformance`: run the suite and write the coverage report (decision 6)

Each step is also its own subcommand (`cargo xtask spec-grammar`), so an agent can rerun one. Each step reports every problem it finds, not just the first.

GitHub Actions runs `cargo xtask ci` on `ubuntu-latest`:
- on every pull request, whatever its base, so each layer of a stack is checked;
- on every push to `main`;
- with a read-only token.

macOS and Windows are added in M13.

### 3. Rule IDs (sample approved by David, 2026-11-10)

- **Syntax in the spec:** `r[<id>]` written inline, directly before the sentence or sentences it covers. This follows the Rust Reference's `r[...]` rule markers.
  - A tag covers text up to the next tag or the end of its paragraph, list item or table cell.
  - Tags never go in code blocks or headings, or before non-normative text (rationale, examples, the overview's prose).
  - A tag starts a line or follows whitespace, and is followed by a space. Tag-shaped text anywhere else outside inline code is an error, so a tag glued to punctuation (`(r[x.y]`, `**r[x.y]**`) can't silently drop out of coverage.
  - Inline code spans are skipped. A backtick run left open at the end of a line is literal, as in CommonMark.
- **ID format:** 2 to 4 dot-separated segments, each `[a-z0-9]+(-[a-z0-9]+)*`, for example `bind.shadow.same-scope`. The first segment names a topic (`bind`, `const`, `lit`, `op`, `match`, `effect`...), not a section number, so moving text between sections never changes an ID. The coverage report finds a rule's section from where its tag sits.
- **Topics (approved by David, 2026-11-10):**
  - `spec/rule-topics.txt` lists every topic with the subject it covers, so a new tag reuses an existing topic instead of inventing a synonym. `spec-rules` fails on a tag whose topic isn't listed, and on a listed topic no tag uses.
  - Before the IDs freeze, eight opaque topics are renamed: `targ`→`type-arg` (it sat next to `target`), `ty`→`type`, `bg`→`background`, `res`→`cresource`, `opuser`→`user-op`, `gen`→`generic`, `cb`→`callback` and `fnval`→`fn-value`. `dbg` stays, because it names the `dbg()` helper it covers. The C-side family (`cmem`, `ctype`, `cbuf`, `cstring`, `cbind`) keeps its shared `c` prefix.
- **Granularity:** one ID per independently testable statement. A sentence that states two rules with "and" keeps one ID. The spec's wording is never changed to make tagging easier.
- **Grammar productions:**
  - They get derived IDs `grammar.<production>` (for example `grammar.fn_decl`) with no tag. The production name is already a stable identifier.
  - The `grammar` topic is reserved, so no written tag can use it.
  - Renaming a production retires its old ID like any other rule. `spec/retired-rules.txt` may list `grammar.*` IDs, and `spec-grammar` fails if a production's derived ID is retired.
- **Stability:** an ID is never renamed or reused. When a rule's meaning changes, it gets a new ID, and the old one moves to `spec/retired-rules.txt`. A wording change that keeps the meaning keeps the ID.
- **New text is tagged:** a spec change that adds normative text tags it in the same PR. Review enforces this, and `AGENTS.md` says so.
- **Stripping:** `cargo xtask spec-rules --strip` prints the spec with every tag removed, so wording diffs can ignore tags. The tagging PR is gated once on its strip output equaling v0.4 byte for byte, and its description shows that diff as empty. Later approved spec edits change the wording on purpose.
- **Checks:** `spec-rules` checks format, placement, topics, uniqueness and retired IDs. The conformance runner checks that every ID a fixture cites exists and isn't retired, because only the runner reads fixtures.

*Alternatives:* section-number IDs (`2.3.1`) break whenever sections move. Opaque random IDs (`fls_a8x2`, as Ferrocene uses) are stable, but neither an agent nor a person can tell what one means without looking it up. HTML-comment tags are invisible when rendered, so a reader can't see the ID they need to cite.

### 4. `spec/grammar.ebnf`

The file holds a header of `//` comment lines, then the expression grammar block, a blank line, and the declarations block, both copied verbatim.
- The header lists the terminals the prose declares: `name`, `Name`, `NAME`, `c_name`, `path` (with its prose definition `name { "." name }`), `literal`, `string`, `int`, `doc_comment` and `NEWLINE`.
- The header is the one hand-edited part. `--write` keeps it, and uses a default header when the file doesn't exist.

`xtask spec-grammar` checks:
1. **Agreement.** It takes the code block after `## Appendix: Expression grammar` and the one after `### Declarations, types and patterns` in the spec. Each heading must have exactly one code block before the next heading, so an added block can't be silently skipped. Their concatenation must equal the file minus its header. On a mismatch, it prints a unified diff with hunk headers and the command that regenerates the file (`cargo xtask spec-grammar --write`).
2. **Closure.** It covers three things:
   - Every identifier on a production's right-hand side, outside string literals and `//` comments, is either defined by a production or listed as a terminal in the header.
   - No production is defined twice.
   - Every production except `file` is referenced by another.

   Today that holds for all 80 productions.
3. **Retired names.** No production's derived ID is in `spec/retired-rules.txt`.

Errors give the line in `spec/quince-spec.md`, not in the generated file, and say to fix the spec, then run `--write`.

The grammar keeps the spec's own notation and gets no conversion to ISO EBNF, so the agreement check stays a plain text comparison.

### 5. Diagnostics crate (`quince_diagnostics`)

- **Registry:** a Rust table with one entry per code: code, default severity, a short title, and its explain text, loaded with `include_str!` from `compiler/diagnostics/explain/<CODE>.md`. Adding a code means adding one table row and one Markdown file. It is empty in M0. A test checks that each row's severity matches its code's letter.
- **Code format (approved by David, 2026-11-10):** `E` plus four digits for errors and `W` plus four digits for warnings. A code is never reused, and a retired code keeps its row and explain text, marked retired. `--deny warnings` changes a warning's reported severity, not its code. `E0412` is reserved for a non-exhaustive match, so the spec's own example stays true.
- **JSON types:** serde structs that serialize exactly to the shape `r[diag.json]` defines. The field set is closed: unknown fields are rejected, and a new field (such as secondary spans or notes) needs a spec change first. The language spec leaves the following open, so they are decided here:
  - **Positions (approved by David, 2026-11-10):** `[line, column]`, both 1-based. `column` counts Unicode scalar values, which is what an agent editing text counts, and a tab is one column. `end` is exclusive.
  - **Lines:** a line ends at LF. A CR right before an LF is part of the line ending, and any other CR is an ordinary character. A leading UTF-8 BOM isn't counted. The end of input after a trailing newline is `[last + 1, 1]`. Which line endings the lexer accepts is section 2's business and M1's. This only fixes how positions count.
  - `file`, in a span or an edit, is relative to the project root, with `/` separators.
  - `severity` is `"error"` or `"warning"`. `fixes` is always present, as `[]` when empty.
- **Edits (approved by David, 2026-11-10):** every edit is `{ "start": [l, c], "end": [l, c], "text": "..." }` with an optional `"file"`, and replaces the text from `start` up to `end`. When `start` equals `end` it inserts, and an empty `text` deletes. Without `file`, an edit applies to the diagnostic's span file. An insertion at `[1, 1]` into a file that doesn't exist creates it. That covers the fixes the spec requires across files, such as `r[pkg.main-fix]` moving `main` into `cmd/<package>/` and `r[contract.pub-visibility]` adding an accessor where the struct is declared.

  As in LSP's `TextEdit` and `WorkspaceEdit`, all positions in a fix refer to the text before any edit is applied, edits to one file never overlap, and two insertions never share a position. A fix is applied as if all at once. `Fix::check` rejects a fix that breaks these rules.

  The diagnostics layer edits spec section 8 to match:
  - the example's `edits` line becomes `[{ "start": [46, 5], "end": [46, 5], "text": "FetchError.Timeout(after:) => todo(\"handle timeout\"),\n" }]`;
  - `r[diag.edits]` is added after the `applicability` bullet: "An edit replaces the text from `start` up to `end` with `text`: when `start` equals `end` it inserts, and an empty `text` deletes.";
  - `r[diag.edit-file]` follows it: "An edit may name a `file`, a path relative to the project root; without one it applies to the diagnostic's span file. An edit at `[1, 1]` to a file that doesn't exist creates it."
- **Spec version (approved by David, 2026-11-10):** these edits change what the spec means, so the same layer bumps the header to Draft v0.5. From now on, any change to the spec's meaning bumps the minor version in the same PR. A tag-only or wording-only change doesn't.
- **Spec layout:** the spec stays one file through M0, so the strip check stays a byte comparison. Splitting it into one file per section, as the repo plan says, is a separate later change (agreed with David, 2026-11-10).
- **Schema:** `compiler/diagnostics/schema/diagnostic.schema.json` (JSON Schema 2020-12), written by hand.
  - A test serializes sample diagnostics and checks them against the schema with a small checker for the subset of JSON Schema that the schema uses. That avoids a schema-validation dependency.
  - The checker fails on any keyword it doesn't implement, so the schema can't hold a constraint nothing enforces.
  - The code `pattern` is checked by `Code::parse`, and a test asserts the schema's pattern is the same regex.
- **Coverage rule (`xtask diagnostics`):** every registry code has a non-empty explain file, and every code that isn't retired appears in at least one fixture expectation. It also reports explain files with no registry row. M0 reads `//~` in `.qnc` fixtures. M2's project fixtures add an expectation form for files like `quince.toml`, and those count too.

### 6. Conformance runner (`quince_conformance`, in `tests/conformance/`)

- **Discovery:** every `*.qnc` file under `tests/conformance/testdata/`, in sorted path order, each paired with a sibling `<name>.stdout` if one exists.
  - A `.stdout` file with no `.qnc` file is an error, because it's usually left behind by a rename.
  - Any failure to read a `.stdout` file other than "not found" is an error, never "no output expected".
- **Fixture header:** the leading run of `//` lines and blank lines. It ends at the first other line.
  - A line `// <word>:`, where the word is ASCII letters and hyphens, is a directive.
  - The directives are `rule`, `mode` and `exit`. Any other word, in any case (`// Mode:`, `// rules:`), is an error.
  - Other comments are free text.
  - A `// rule:`, `// mode:` or `// exit:` line after the header is an error that says to move it up.
  - `mode` and `exit` appear at most once, and a rule is cited at most once.
- **Directives:**
  - `// rule: <id>`: one or more required. Each cites a spec rule or `grammar.*` ID.
  - `// mode: check` (the default), `parse` or `run`. Modes are an open set: an unknown one is an error, and later changes add more (`fmt` in M1). `parse` runs only the parser even after later stages exist, so a syntax fixture doesn't start failing when name resolution or lints add diagnostics.
  - In `run` mode, `// exit: <n>` (default 0). Standard output must equal the sibling `<name>.stdout`, and a missing file means the program prints nothing. The program must build without errors. Warnings aren't judged in run mode. Standard error isn't compared until a later change adds `<name>.stderr`.
- **Inline expectations** (in `check` and `parse` fixtures):
  - `//~ CODE` expects a diagnostic with that code whose span starts on this line.
  - `//~^ CODE` expects one on the line above. Each extra `^` moves one more line up, for lines where a trailing comment would be inside a multi-line string.
  - One comment may list several codes, and each code expects one diagnostic, so `//~ E0001 E0001` expects two. Each diagnostic matches at most one expectation.
  - A diagnostic starting at the end of input after a trailing newline matches expectations on the last line, where a `//~` can be written.
  - A fixture passes when every expectation matches a diagnostic and every error or warning is expected. Unexpected diagnostics fail the fixture.
- **Compiler seam:** a `Backend` trait with `parse` and `check` (each returning diagnostics) and `run` (returning exit code and output). Each returns `None` when that backend isn't wired up yet. M0 ships no backend, so any fixture fails with "no backend: … not wired up yet". Tests are written before the code and land in the same PR that makes them pass, so `main` stays green.
- **Coverage report:**
  - The runner reads every rule ID from the spec (tags plus grammar productions). A rule is covered when at least one passing fixture cites it.
  - The runner writes `target/conformance/coverage.json`, whose shape is fixed in the delta spec, and prints the total and covered counts per spec section. Grammar IDs are grouped last under `Grammar productions`.
  - Listing roughly 970 uncovered IDs in the terminal would bury the summary, so the JSON report is where agents find them.
  - A fixture that cites an unknown ID is an error, and so is one that cites a retired ID, with a message saying it's retired.
- **Self-tests:** the runner's unit tests use a fake `Backend`. A small fixture tree under `tests/conformance/src/selftest/` (a nested fixture, a `.stdout` pair, an orphan `.stdout`) tests discovery. A test runs the rule loader on the real spec. So the matching logic is tested before any real compiler exists.

### 7. Delivery as a stack

Each layer builds and passes `cargo xtask ci` on its own:

1. `m0/openspec`: `openspec init`, this change, `.gitignore`, `.gitattributes`
2. `m0/workspace`: Cargo workspace, empty crates, toolchain pin, `xtask ci` (steps 1 to 3), `AGENTS.md`, GitHub Actions
3. `m0/rule-ids`: `quince_spec`, tags in the spec, `rule-topics.txt`, `retired-rules.txt`, `xtask spec-rules`
4. `m0/grammar`: `spec/grammar.ebnf` and `xtask spec-grammar`
5. `m0/diagnostics`: `quince_diagnostics` skeleton, schema, `xtask diagnostics`, the section 8 spec edits and the bump to v0.5
6. `m0/conformance`: runner, coverage report, `xtask conformance`. This is the exit demo.

Layer 3 is mostly tag insertions in the spec, probably 600 to 900 tags, and may go past 1000 changed lines. It can't be split by meaning: half the spec tagged is a coverage report that lies. Its PR says so and shows the empty strip diff.

## Risks / Trade-offs

- [Tagging judgment varies: some rules end up too coarse, others too fine] → The sample is approved before the full pass. Splitting an ID later is allowed: the old ID is retired and two new ones are added.
- [The grammar check depends on heading text] → The check fails loudly and names the heading it couldn't find. Renaming a heading is a spec edit, so it gets reviewed anyway.
- [An empty registry and suite mean the checks run on nothing in CI] → Every xtask check has its own unit tests with small fixtures, including failing and malformed-but-plausible ones.
- [Visible `r[...]` tags make the spec noisier to read] → They are short, sit at sentence starts, and are what an agent needs in order to cite a rule.
- [Coverage can fall silently when a fixture is deleted] → Accepted for M0, while coverage is zero. See Open Questions.

## Migration Plan

None. This is greenfield. Rollback means reverting the stack's PRs.

## Open Questions

- **Project-shaped fixtures** (a folder with `quince.toml` and several files, needed for section 7) get a format in M2's change. That change also decides:
  - which project a single-file fixture belongs to;
  - whether expectation matching must check `span.file`;
  - how expectations are written for files that can't hold `//~`.
- **Backend wiring (M1).** Should the runner call the compiler in-process through `Backend`, or drive `quince check --format json` as a subprocess? A subprocess would also test the CLI's JSON path, and keeps xtask from compiling the compiler (and, from M7, LLVM). Either way, xtask must not link LLVM (decision 2).
- **Rules that `.qnc` fixtures can't cover** (about 60 today: `cli.*`, `query.*`, `perf.*`, `target.*`, `publish.*`, `semver.*`). Snapshot tests, perf runs and git fixtures need a way to report the rules they cite into the coverage report, or the 1.0 exit of "no uncovered rule IDs" can't be met honestly. Each milestone that adds such a test kind decides how.
- **Coverage ratchet.** Should a committed baseline fail CI when a rule loses coverage? Decide in M1, once there is coverage to lose.
- **LSP positions (M5).** LSP 3.17 makes UTF-16 code units the default position encoding. The language server converts unless the client negotiates `utf-32`.
- Whether `spec/std-api.md` gets rule IDs, or uses signature paths such as `std.List.map` as IDs, is decided when M6 starts testing std.
- Whether `quince check --format json` prints one JSON array or JSON Lines is decided in M5.
