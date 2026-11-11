# AGENTS.md

How to build and check this repo. `CLAUDE.md` holds the working rules and the decisions already made. This file covers commands and layout.

## Commands

| Command | Does |
| --- | --- |
| `cargo xtask ci` | Runs every check below, even after one fails, then prints a pass/fail table. Keep it green before finishing any change. |
| `cargo build --workspace` | Builds every crate. Nothing links LLVM yet. |
| `cargo test --workspace` | Runs every Rust test. |
| `cargo fmt --all` | Formats Rust code. `ci` runs it with `--check`. |
| `cargo clippy --workspace --all-targets -- -D warnings` | Lints Rust code with warnings treated as errors, as `ci` does. |
| `cargo run -p quince_cli -- --version` | Runs the `quince` binary. Only `--version` works so far. |
| `cargo xtask spec-rules` | Checks the chapter manifest `spec/chapters.txt`, the chapter list in `spec/README.md`, and the rule-ID tags in every chapter: format, placement, topics, uniqueness, retired IDs. |
| `cargo xtask spec-rules --strip [<file>]` | Prints one spec chapter, or every chapter in order, with every tag removed, so you can diff the wording without the tags. |
| `cargo xtask spec-grammar` | Checks that `spec/grammar.ebnf` matches the spec's `ebnf` code blocks, that every name it uses is defined and every production is used, and that no production reuses a retired name. Errors give the chapter file and line to fix. |
| `cargo xtask spec-grammar --write` | Regenerates `spec/grammar.ebnf` from the spec. Run it after the spec's grammar changes. Only the file's header (its terminal list) is edited by hand. |
| `cargo xtask spec-links` | Checks that every relative link in `spec/`, and every link from `std/API.md` into `spec/`, names a file that exists, and that each `#fragment` names a heading there (GitHub's anchor rules). |
| `cargo xtask spec-compare` | Temporary, while the spec is split into chapters: checks that every rule keeps its text and no other text is lost or added, against the pinned baseline in `docs/design/spec-restructure/baseline/` (decision 8 of `docs/design/spec-restructure.md`). |
| `cargo xtask diagnostics` | Checks that every registered diagnostic code has explain text, and every code that isn't retired has a conformance test. To add a code, see `compiler/diagnostics/src/registry.rs`. |
| `cargo xtask conformance` | Runs the `.qnc` fixtures in `tests/conformance/testdata/` and writes spec coverage by rule ID to `target/conformance/coverage.json`. The fixture format is in that folder's README. |

The toolchain is pinned in `rust-toolchain.toml`, and rustup installs it the first time you run `cargo` in the repo.

`ci` runs `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `spec-rules`, `spec-grammar`, `spec-links`, `spec-compare`, `diagnostics` and `conformance`, and exits non-zero if any step failed.

GitHub Actions runs `cargo xtask ci` with a read-only token on every pull request, whatever its base branch (so each layer of a stack is checked), and on every push to `main`. `.gitattributes` turns off line-ending conversion, so the byte comparisons in the spec checks agree on every platform and fixtures can hold CRLF on purpose.

## Rule IDs

The full rules for chapters, tags, IDs and the grammar file are in `compiler/spec/README.md`. In short: the spec is the chapters listed in `spec/chapters.txt`, indexed by `spec/README.md`. Every normative sentence in it carries a tag such as `r[bind.shadow.same-scope]`, written directly before the sentences it covers. Conformance tests cite these IDs. The first segment names a topic, not a section, and must be listed in `spec/rule-topics.txt`: reuse a listed topic before adding one. An ID is never renamed or reused: when a rule's meaning changes, its old ID goes in `spec/retired-rules.txt` and the rule gets a new one. Grammar productions have implicit IDs, `grammar.<production>`, and the `grammar` topic is reserved for them.

A spec change that adds normative text tags it in the same PR. No check can tell a rule from prose, so review enforces this. A change to what the spec means also bumps its minor version (Draft v0.5 to v0.6, in `spec/README.md`) in the same PR. Tags and wording-only edits don't.

## Map

| Path | What it is |
| --- | --- |
| `spec/` | The language spec. The source of truth for what Quince means. Don't edit it without David's approval. |
| `compiler/<name>/` | One crate per compiler stage, named `quince_<name>`. Each crate's `lib.rs` says what it holds, and its `README.md`, when it has one, holds the formats and rules it enforces. |
| `compiler/cli/` | The single `quince` binary. |
| `compiler/spec/` | `quince_spec`: reads the spec (chapter manifest, rule tags, `ebnf` grammar blocks) for xtask and the conformance runner. |
| `runtime/` | The runtime, built as a static library linked into every Quince binary. |
| `std/` | The standard library in Quince (from M6), and `std/API.md`, its API doc. For a std signature, read `std/API.md`; once `quince query` works (M5), use it instead. |
| `tests/conformance/` | The conformance runner (`quince_conformance`); fixtures live in its `testdata/`. |
| `xtask/` | `cargo xtask`: repo checks. Add a check as a `Step` in `xtask/src/ci.rs`. |
| `docs/` | The repo plan and milestones. `docs/design/` holds one design doc per milestone chunk, with its task list. |

Only `compiler/codegen-llvm` and `compiler/bind` may depend on LLVM.

## Commits

Every commit subject and PR title follows [Conventional Commits](https://www.conventionalcommits.org/): `type(scope): imperative summary`, such as `feat(spec): read rule tags from chapter files`. Pull requests are squash-merged, and the squash commit takes the PR title, so the title matters most.
