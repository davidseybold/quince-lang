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

The toolchain is pinned in `rust-toolchain.toml`, and rustup installs it the first time you run `cargo` in the repo.

## Map

| Path | What it is |
| --- | --- |
| `spec/` | The language spec and std API. The source of truth for what Quince means. Don't edit it without David's approval. |
| `openspec/` | Implementation specs (`specs/`) and units of work (`changes/`). |
| `compiler/<name>/` | One crate per compiler stage, named `quince_<name>`. Each crate's `lib.rs` says what it holds. |
| `compiler/cli/` | The single `quince` binary. |
| `runtime/` | The runtime, built as a static library linked into every Quince binary. |
| `std/` | The standard library in Quince (from M6). |
| `xtask/` | `cargo xtask`: repo checks. Add a check as a `Step` in `xtask/src/ci.rs`. |
| `docs/` | The repo plan and milestones. |

Only `compiler/codegen-llvm` and `compiler/bind` may depend on LLVM.
