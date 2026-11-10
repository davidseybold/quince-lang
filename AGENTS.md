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
| `cargo xtask spec-rules` | Checks the rule-ID tags in `spec/quince-spec.md`: format, placement, topics, uniqueness, retired IDs. |
| `cargo xtask spec-rules --strip` | Prints the spec with every tag removed, so you can diff the wording without the tags. |
| `cargo xtask spec-grammar` | Checks that `spec/grammar.ebnf` matches the spec's grammar appendices, that every name it uses is defined and every production is used, and that no production reuses a retired name. Errors give the spec line to fix. |
| `cargo xtask spec-grammar --write` | Regenerates `spec/grammar.ebnf` from the spec. Run it after the spec's grammar changes. Only the file's header (its terminal list) is edited by hand. |
| `cargo xtask diagnostics` | Checks that every registered diagnostic code has explain text and a conformance test. To add a code, see `compiler/diagnostics/src/registry.rs`. |

The toolchain is pinned in `rust-toolchain.toml`, and rustup installs it the first time you run `cargo` in the repo.

## Rule IDs

Every normative sentence in the spec carries a tag such as `r[bind.shadow.same-scope]`, written directly before the sentences it covers. Conformance tests cite these IDs. The first segment names a topic, not a section, and must be listed in `spec/rule-topics.txt`: reuse a listed topic before adding one. An ID is never renamed or reused: when a rule's meaning changes, its old ID goes in `spec/retired-rules.txt` and the rule gets a new one. Grammar productions have implicit IDs, `grammar.<production>`, and the `grammar` topic is reserved for them.

A spec change that adds normative text tags it in the same PR. No check can tell a rule from prose, so review enforces this.

## Map

| Path | What it is |
| --- | --- |
| `spec/` | The language spec and std API. The source of truth for what Quince means. Don't edit it without David's approval. |
| `openspec/` | Implementation specs (`specs/`) and units of work (`changes/`). |
| `compiler/<name>/` | One crate per compiler stage, named `quince_<name>`. Each crate's `lib.rs` says what it holds. |
| `compiler/cli/` | The single `quince` binary. |
| `compiler/spec/` | `quince_spec`: reads the spec text (rule tags, grammar appendices) for xtask and the conformance runner. |
| `runtime/` | The runtime, built as a static library linked into every Quince binary. |
| `std/` | The standard library in Quince (from M6). |
| `xtask/` | `cargo xtask`: repo checks. Add a check as a `Step` in `xtask/src/ci.rs`. |
| `docs/` | The repo plan and milestones. |

Only `compiler/codegen-llvm` and `compiler/bind` may depend on LLVM.
