# Design

## Context

The repo holds only the spec (`spec/quince-spec.md`, Draft v0.4), the std API, the repo plan and the milestones. See proposal.md for why M0 comes first. Constraints that shape the design:

- `CLAUDE.md`: one command for all checks, small crates, and the core loop of spec rule, then conformance test, then code. Never edit `spec/` without David's approval.
- The repo root can't hold a `quince.toml`, and `.qnc` fixtures live in `testdata/` folders, which are never compiled.
- No compiler exists yet, so the conformance runner can't execute anything in M0. It needs a seam where M1's parser plugs in.
- Rust is not installed on the dev machine.

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

## Decisions

### 1. Workspace layout

A root `Cargo.toml` with `members = ["compiler/*", "runtime", "tests/conformance", "xtask"]`, edition 2024, and shared lint settings in `[workspace.lints]`. The crates follow the repo plan: `compiler/{syntax,types,effects,diagnostics,query,ir,codegen-llvm,pkg,bind,native,lsp,cli}` named `quince_<dir>` (hyphens become underscores), plus `runtime/` as `quince_runtime` (`crate-type = ["staticlib", "rlib"]`). `cli` builds the `quince` binary, which for now only prints its version. Each crate starts with a one-line doc comment saying what it will hold. No crate depends on LLVM in M0.

`std/` is created with a README only. A `std/quince.toml` would be a project manifest, and it's better added in M6, once something reads it.

*Alternative:* one `compiler/` workspace separate from the root. Rejected: `cargo xtask` has to run from the root, and two workspaces mean two lockfiles.

The toolchain is pinned in `rust-toolchain.toml` to the exact stable version installed at setup, with `rustfmt` and `clippy`. `.claude/worktrees/` and `target/` go in `.gitignore`.

### 2. `cargo xtask ci`

`xtask` is a binary crate with no dependencies outside std, aliased in `.cargo/config.toml`. `ci` runs these steps and **runs all of them even after a failure**, then prints a pass/fail table and exits non-zero if any failed. This mirrors the compiler's "report all independent errors" rule.

1. `cargo fmt --all --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test --workspace`
4. `spec-rules`: rule-ID format, uniqueness, retired-ID reuse (decision 3)
5. `spec-grammar`: `spec/grammar.ebnf` agrees with the spec (decision 4)
6. `diagnostics`: every registered code has explain text and a conformance test (decision 5)
7. `conformance`: run the suite and write the coverage report (decision 6)

Each step is also its own subcommand (`cargo xtask spec-grammar`), so an agent can rerun one. GitHub Actions runs `cargo xtask ci` on `ubuntu-latest` for every PR. macOS and Windows are added in M13.

### 3. Rule IDs (pending David's approval of the sample)

- **Syntax in the spec:** `r[<id>]` written inline, directly before the sentence or sentences it covers. A tag covers text up to the next tag or the end of its paragraph, list item or table cell. This follows the Rust Reference's `r[...]` rule markers. Tags never go in code blocks or headings, or before non-normative text (rationale, examples, the overview's prose).
- **ID format:** 2 to 4 dot-separated segments, each `[a-z0-9]+(-[a-z0-9]+)*`, for example `bind.shadow.same-scope`. The first segment names a topic (`bind`, `const`, `lit`, `op`, `match`, `effect`...), not a section number, so moving text between sections never changes an ID. The coverage report finds a rule's section from where its tag sits.
- **Granularity:** one ID per independently testable statement. A sentence that states two rules with "and" keeps one ID. The spec's wording is never changed to make tagging easier.
- **Grammar productions** get derived IDs `grammar.<production>` (for example `grammar.fn_decl`) with no tag. The production name is already a stable identifier.
- **Stability:** an ID is never renamed or reused. When a rule's meaning changes, it gets a new ID, and the old one moves to `spec/retired-rules.txt`. A wording change that keeps the meaning keeps the ID.
- **Insertions only:** `cargo xtask spec-rules --strip` prints the spec with every tag removed. For the tagging PR, its output must equal v0.4 byte for byte, and the PR description shows that diff as empty.
- **Checks in CI:** every tag has a valid format, no ID appears twice, no tag uses a retired ID, and every ID cited by a fixture exists.

*Alternatives:* section-number IDs (`2.3.1`) break whenever sections move. Opaque random IDs (`fls_a8x2`, as Ferrocene uses) are stable, but neither an agent nor a person can tell what one means without looking it up. HTML-comment tags are invisible when rendered, so a reader can't see the ID they need to cite.

### 4. `spec/grammar.ebnf`

The file holds a header of `//` comment lines, then the expression grammar block, a blank line, and the declarations block, both copied verbatim. The header lists the terminals the prose declares: `name`, `Name`, `NAME`, `c_name`, `path` (with its prose definition `name { "." name }`), `literal`, `string`, `int`, `doc_comment` and `NEWLINE`.

`xtask spec-grammar` checks:
1. **Agreement.** It finds the first code block after `## Appendix: Expression grammar` and after `### Declarations, types and patterns` in the spec. Their concatenation must equal the file minus its header. On a mismatch, it prints a unified diff and the command that regenerates the file (`cargo xtask spec-grammar --write`).
2. **Closure.** Every identifier on a production's right-hand side, outside string literals and `//` comments, is either defined by a production or listed as a terminal in the header. Today that holds: 80 productions, none defined twice, and none unused apart from `file`.

The grammar keeps the spec's own notation and gets no conversion to ISO EBNF, so the agreement check stays a plain text comparison.

### 5. Diagnostics crate (`quince_diagnostics`)

- **Registry:** a Rust table with one entry per code: code, default severity, a short title, and its explain text, loaded with `include_str!` from `compiler/diagnostics/explain/<CODE>.md`. Adding a code means adding one table row and one Markdown file. It is empty in M0.
- **Code format (approved by David, 2026-11-10):** `E` plus four digits for errors and `W` plus four digits for warnings. A code is never reused, and a retired code keeps its row and explain text, marked retired. `--deny warnings` changes a warning's reported severity, not its code. `E0412` is reserved for a non-exhaustive match, so the spec's own example stays true.
- **JSON types:** serde structs that serialize exactly to the shape in spec section 8: `code`, `severity`, `message`, `span { file, start, end }`, `fixes [{ description, applicability, edits }]` and `docs`. The language spec leaves these open, so the decisions are made here:
  - Positions are `[line, column]`, both 1-based. `column` counts Unicode scalar values, which is what an agent editing text counts. `end` is exclusive.
  - `file` is relative to the project root, with `/` separators.
  - `severity` is `"error"` or `"warning"`.
  - `docs` is always `"quince explain <code>"`.
  - **Edits (approved by David, 2026-11-10):** every edit is `{ "start": [l, c], "end": [l, c], "text": "..." }` and replaces the text from `start` up to `end`. When `start` equals `end` it inserts, and an empty `text` deletes. This is the same model as LSP `TextEdit`. The spec's section 8 example uses `{ "at", "insert" }`, so the diagnostics layer also edits the spec: the example's `edits` line becomes `[{ "start": [46, 5], "end": [46, 5], "text": "FetchError.Timeout(after:) => todo(\"handle timeout\"),\n" }]`, and one sentence is added after the `applicability` bullet: "An edit replaces the text from `start` up to `end` with `text`: when `start` equals `end` it inserts, and an empty `text` deletes."
  - Positions (1-based, Unicode scalar columns, exclusive end) were approved by David on 2026-11-10.
- **Spec layout:** the spec stays one file through M0, so the strip check stays a byte comparison. Splitting it into one file per section, as the repo plan says, is a separate later change (agreed with David, 2026-11-10).
- **Schema:** `compiler/diagnostics/schema/diagnostic.schema.json` (JSON Schema 2020-12), written by hand. A test serializes sample diagnostics and checks them against the schema with a small checker for the subset of JSON Schema that the schema uses. That avoids a schema-validation dependency.
- **Coverage rule (`xtask diagnostics`):** every registry code has a non-empty explain file and appears in at least one fixture's `//~` expectation. It also reports explain files with no registry row.

### 6. Conformance runner (`quince_conformance`, in `tests/conformance/`)

- **Discovery:** every `*.qnc` file under `tests/conformance/testdata/`, in sorted path order.
- **Fixture header:** the leading `//` comment lines, before any code.
  - `// rule: <id>`: one or more required. Each cites a spec rule or `grammar.*` ID.
  - `// mode: check` (the default) or `// mode: run`.
  - In `run` mode, `// exit: <n>` (default 0). Expected standard output goes in a sibling `<name>.stdout` file.
- **Inline expectations:**
  - `//~ CODE` expects a diagnostic with that code whose span starts on this line.
  - `//~^ CODE` expects one on the line above. Each extra `^` moves one more line up, for lines where a trailing comment would be inside a multi-line string.
  - A fixture passes when every expectation matches a diagnostic and every error or warning is expected. Unexpected diagnostics fail the fixture.
- **Compiler seam:** a `Backend` trait with `check(path, source) -> Vec<Diagnostic>` and `run(...) -> RunOutput`. M0 ships no backend, so any fixture fails with "no backend: quince_syntax is not wired up yet". Tests are written before the code and land in the same PR that makes them pass, so `main` stays green.
- **Coverage report:** the runner reads every rule ID from the spec (tags plus grammar productions). A rule is covered when at least one passing fixture cites it. The runner writes `target/conformance/coverage.json` and prints a summary per spec section (total, covered, and uncovered IDs). A fixture that cites an unknown or retired ID is an error.
- The runner's own unit tests use a fake `Backend` and fixtures under `tests/conformance/src/selftest/`, so the matching logic is tested before any real compiler exists.

### 7. Delivery as a stack

Each layer builds and passes `cargo xtask ci` on its own:

1. `m0/openspec`: `openspec init`, this change, `.gitignore`
2. `m0/workspace`: Cargo workspace, empty crates, toolchain pin, `xtask ci` (steps 1 to 3), `AGENTS.md`, GitHub Actions
3. `m0/rule-ids`: tags in the spec, `retired-rules.txt`, `xtask spec-rules`
4. `m0/grammar`: `spec/grammar.ebnf` and `xtask spec-grammar`
5. `m0/diagnostics`: `quince_diagnostics` skeleton, schema, `xtask diagnostics`
6. `m0/conformance`: runner, coverage report, `xtask conformance`. This is the exit demo.

Layer 3 is mostly tag insertions in the spec, probably 600 to 900 tags, and may go past 1000 changed lines. It can't be split by meaning: half the spec tagged is a coverage report that lies. Its PR says so and shows the empty strip diff.

## Risks / Trade-offs

- [Tagging judgment varies: some rules end up too coarse, others too fine] → The sample is approved before the full pass. Splitting an ID later is allowed: the old ID is retired and two new ones are added.
- [The grammar check depends on heading text] → The check fails loudly and names the heading it couldn't find. Renaming a heading is a spec edit, so it gets reviewed anyway.
- [An empty registry and suite mean the checks run on nothing in CI] → Every xtask check has its own unit tests with small fixtures, including failing ones.
- [Visible `r[...]` tags make the spec noisier to read] → They are short, sit at sentence starts, and are what an agent needs in order to cite a rule.

## Migration Plan

None. This is greenfield. Rollback means reverting the stack's PRs.

## Open Questions

- Project-shaped fixtures (a folder with `quince.toml` and several files, needed for section 7) get a format in M2's change.
- Whether `spec/std-api.md` gets rule IDs, or uses signature paths such as `std.List.map` as IDs, is decided when M6 starts testing std.
- Whether `quince check --format json` prints one JSON array or JSON Lines is decided in M5.
