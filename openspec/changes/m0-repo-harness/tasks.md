# Tasks

Each group is one layer of the M0 stack (design.md, decision 7) and ends with `cargo xtask ci` green, once group 2 has created it.

## 1. OpenSpec and change docs (`m0/openspec`)

- [x] 1.1 Run `openspec init --tools claude` and verify `openspec/config.yaml` and `.claude/skills/openspec-*` exist
- [x] 1.2 Write proposal, design, specs and tasks for `m0-repo-harness` and verify `openspec validate m0-repo-harness --strict` passes
- [x] 1.3 Add `.gitignore` (`target/`, `.claude/worktrees/`) and verify `git status` in the main checkout no longer lists worktrees
- [x] 1.4 Get David's approval of the proposal and of the pending decisions in design.md (rule-ID scheme, code format, edit forms, dependencies, Rust install)
- [x] 1.5 Address the design review: the edit `file` field, edit and position semantics, fixture header, modes and expectation rules, topic list and renames, grammar ID stability, `.gitattributes`. Verify `openspec validate m0-repo-harness --strict` passes

## 2. Workspace and `xtask ci` (`m0/workspace`)

- [x] 2.1 Install Rust with rustup (after approval), pin it in `rust-toolchain.toml` with `rustfmt` and `clippy`, and verify `cargo --version` matches the pin
- [x] 2.2 Create the root `Cargo.toml` workspace and every crate under `compiler/` plus `runtime/`, each with a one-line crate doc, and verify `cargo build --workspace` succeeds without LLVM
- [x] 2.3 Make `quince_cli` build a `quince` binary that prints its version, and verify `cargo run -p quince_cli -- --version`
- [x] 2.4 Create `xtask` with the `.cargo/config.toml` alias and the `ci` runner for fmt, clippy and test. Run every step, print a summary table, and exit non-zero on any failure. Verify with a unit test of the summary logic and by running `cargo xtask ci` on the clean tree
- [x] 2.5 Add `.github/workflows/ci.yml` running `cargo xtask ci` on pull requests, and verify it passes on the layer's PR
- [x] 2.6 Write `AGENTS.md` (build, test, repo map) and verify every command in it runs as written
- [x] 2.7 Review fixes: give the workflow a read-only token, and verify `cargo xtask ci` passes on the layer's PR

## 3. Rule IDs in the spec (`m0/rule-ids`)

- [x] 3.1 Implement `xtask spec-rules`: parse tags, check format, uniqueness, code-block and heading placement, and retired IDs. Verify with unit tests covering each failure in the spec-rule-ids scenarios
- [x] 3.2 Implement `--strip`, and verify that stripping a tagged sample gives back its untagged source
- [x] 3.3 Show David a tagged sample of one section and get approval (done before 3.4)
- [x] 3.4 Tag every normative sentence in `spec/quince-spec.md`, verify `cargo xtask spec-rules --strip` equals spec v0.4 byte for byte, and add `spec/retired-rules.txt` (empty)
- [x] 3.5 Wire `spec-rules` into `xtask ci` and verify `cargo xtask ci` passes
- [x] 3.6 Review fixes: tags glued to punctuation are errors, an unclosed backtick is literal, `spec/rule-topics.txt` is checked, `grammar` is reserved, eight topics renamed, problems in all three files reported together, and indexing in code blocks (`counter[user.id]`) isn't mistaken for a tag. Verify with unit tests and an unchanged strip output

## 4. Grammar extraction (`m0/grammar`)

- [x] 4.1 Implement `xtask spec-grammar` agreement check and `--write`, with unit tests for mismatch, regenerate and missing heading
- [x] 4.2 Implement the closure check (undefined names, duplicate productions), with unit tests for each
- [x] 4.3 Generate `spec/grammar.ebnf` with its terminal header, wire the check into `xtask ci`, and verify it passes on the real spec
- [x] 4.4 Review fixes: errors give spec lines, a second code block under a heading fails, unused productions and retired production names fail, the diff has hunk headers, and `--write` is a tested pure function that only treats a missing file as "no header". Verify with unit tests and `--write` leaving the file unchanged

## 5. Diagnostics skeleton (`m0/diagnostics`)

- [x] 5.1 Add `serde` and `serde_json` (after approval) and the diagnostic types in `quince_diagnostics`, edit the spec section 8 example to the approved edit form, and verify the example round-trips byte-equivalent through them
- [x] 5.2 Write `diagnostic.schema.json` and the schema-subset checker, and verify the spec example validates and an extra field is rejected
- [x] 5.3 Add the empty code registry and the `explain/` folder convention, and document how to add a code in `AGENTS.md`
- [x] 5.4 Implement `xtask diagnostics` (explain text present, a fixture expects each code, no orphan explain files), with unit tests for each failure, and wire it into `xtask ci`
- [x] 5.5 Review fixes: add the approved `r[diag.edit-file]` rule and the optional edit `file`, bump the spec to Draft v0.5, count positions over CRLF, lone CR and a BOM as specified, reject overlapping edits and insertions at another edit's edge with `Fix::check`, add the promoted-warning, deletion and schema-pattern tests, and share serde versions through `[workspace.dependencies]`

## 6. Conformance runner and coverage (`m0/conformance`)

- [x] 6.1 Implement fixture discovery and header parsing (`rule`, `mode`, `exit`), with self-tests for the missing-citation and unknown-rule failures
- [x] 6.2 Implement `//~` and `//~^` expectation matching against a `Backend`, with self-tests using a fake backend for matched, unexpected and caret cases
- [x] 6.3 Make a missing backend fail loudly, and verify with a self-test
- [x] 6.4 Implement the rule-ID loader (spec tags plus `grammar.*`) and the coverage report (JSON plus per-section summary), and verify a failing fixture doesn't count as coverage
- [x] 6.5 Wire `conformance` into `xtask ci` and verify the M0 exit: CI is green on the empty suite, and `target/conformance/coverage.json` lists every rule ID as uncovered
- [x] 6.6 Review fixes: blank lines stay in the header, and directives after code, misspelled directives and repeats are errors. Add `parse` mode, several codes per `//~`, the end-of-input rule, a build-failure result for run fixtures, "no `.stdout`" meaning no output, read errors and orphan `.stdout` files as errors, and a retired-rule message. Add self-test fixtures for discovery and a test that loads the real spec

## Workflow follow-up

- Submit the stack with `gh stack submit --auto` (drafts). Run the design-reviewer and pr-reviewer loops before asking David for a review.
- Archive the change after the stack merges, which creates the first `openspec/specs/` entries.
