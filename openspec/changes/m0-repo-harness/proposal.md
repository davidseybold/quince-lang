# M0: Repo and harness

## Why

Nothing can be built against the spec yet: there is no workspace, no single check command, no way to cite a spec rule, and no runner for conformance tests. M0 puts the harness in place, so every later milestone can follow the core loop (spec rule, then conformance test, then code) and show its spec coverage as a number.

## What Changes

- **Cargo workspace** at the repo root with every crate from `docs/repo-plan.md` as an empty `quince_*` crate under `compiler/`, plus `runtime/`. A pinned Rust toolchain. No crate links LLVM yet.
- **`cargo xtask ci`**: one command that runs every fast check: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, and the repo checks below. GitHub Actions runs the same command.
- **`AGENTS.md`**: how to build and test, and a map of the repo.
- **Stable rule IDs in the spec.** Every normative sentence in `spec/quince-spec.md` gets an ID such as `r[bind.shadow.same-scope]`. The tags are pure insertions: removing them gives back the v0.4 text byte for byte. Grammar productions get derived IDs (`grammar.fn_decl`) without tags. A check enforces format, placement, a fixed list of topics, and uniqueness, and that retired IDs are never reused.
- **`spec/grammar.ebnf`**, extracted from the two grammar appendices. A check fails when it no longer matches the spec text, or when a production references a name that is neither defined nor a declared token.
- **Diagnostics crate skeleton** (`quince_diagnostics`): an error-code registry, the JSON diagnostic types and a JSON Schema for them, and a check that every registered code has `explain` text and at least one conformance test.
- **Conformance runner**: finds `.qnc` files in `testdata/` folders under `tests/conformance/`, reads their rule citations and inline expectations (`//~ E0412`), runs them through a compiler interface (no implementation yet), and writes a coverage report by rule ID.
- *Exit:* `cargo xtask ci` is green on an empty suite, and the coverage report lists every rule ID as uncovered.

## Capabilities

### New Capabilities

- `spec-rule-ids`: how rules in the language spec are identified, how IDs are written in the spec text, how they stay stable, and what is checked about them.
- `spec-grammar`: the extracted grammar file and its agreement with the spec's grammar appendices.
- `conformance-tests`: the `.qnc` conformance fixture format (rule citations, inline expectations), how the runner finds and reports them, and the coverage report.
- `diagnostics`: the error-code registry, the parts of the JSON diagnostic format the language spec leaves open (position units, edit forms, code format), and the rule that every code has `explain` text and a test.
- `repo-checks`: what `cargo xtask ci` runs and guarantees.

### Modified Capabilities

None. This is the first change.

## Impact

- **New top-level folders:** `compiler/` (including `compiler/spec`, a crate the repo plan doesn't list; see design.md decision 1), `runtime/`, `std/` (placeholder), `tests/conformance/`, `xtask/`, `.github/workflows/`.
- **`spec/quince-spec.md`** gains rule-ID tags, which change no wording. They went past David first as a sample, then as the full pass.
- **Spec wording changes (approved by David):** section 8's edit form becomes `{ start, end, text }` with an optional `file`, and the header moves to Draft v0.5 (diagnostics layer).
- **New `spec/grammar.ebnf`.**
- **Toolchain:** Rust via rustup, pinned in `rust-toolchain.toml` (1.99.1).
- **Dependencies (approved by David, 2026-11-10):** `serde` and `serde_json` in `quince_diagnostics` and the runner. Nothing else. The JSON Schema is validated by a test against a hand-written schema file, not by a schema crate.
- **Delivery:** a `gh stack` of about six PRs, each under ~500 changed lines except the rule-ID pass on the spec (see design.md).
