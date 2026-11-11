# Quince implementation milestones

Based on spec Draft v0.4 (2026-11-10). The doc this was copied from is the editable original; this file is the copy the repo works from. Its references to spec sections now name the chapters the spec was split into on 2026-11-10 ([spec](../spec/README.md)).

## How the milestones are ordered

Fifteen milestones in five phases take Quince from an empty repo to 1.0. **The checker and agent tooling come before code generation**, because a Quince that type-checks but can't yet run already lets agents write code against the spec and get JSON diagnostics and fixes back. That is the core promise, and it gets tested earliest.

- **Each milestone ends in a demo, not a percentage.** Its exit criterion is a command that works on real code, plus conformance tests for the spec chapters it covers.
- **Each milestone is a set of design docs** (`docs/design/`) whose tasks cite the spec rule IDs they implement, so coverage of the spec can be measured as work proceeds.
- **No dates yet.** The order and the dependencies are firm; durations are worth estimating after M1.
- The repo layout and crate names come from `docs/repo-plan.md`.

## Dependencies

```
Checker track          Running-code track         Ecosystem track
M0 Repo and harness
M1 Syntax and fmt
M2 Projects and names
M3 Types and patterns
M4 Effects/contracts ──┬──> M6 IR and interpreter ─┬──> M11 Packages (from M4)
M5 Agent tooling       │    M7 LLVM and memory ────┼──> M10 Complete testing
   (first agent test   │    M8 Concurrency runtime │
    on real checker)   │    M9 Std and effects     │
                       │    M12 C interop ─────────┴──> M13 Targets and speed
                       └──────────> M14 Release hardening <── everything
```

The critical path is M0 to M9. The runtime crate (scheduler, counts, cycle collector) can start in Rust right after M0. Packages need the checker for publish's API comparison, but no code generation.

## Phase 1: Foundations

Phase 1 ends when `quince check` parses and resolves a multi-file project, and `quince fmt` formats it.

**M0. Repo and harness.** The spec's Introduction, and Diagnostics and queries (the diagnostics format).

- Cargo workspace, `AGENTS.md`, `cargo xtask ci`, and the M0 design (`docs/design/m0-repo-harness.md`).
- `spec/` with stable rule IDs added to v0.4, plus `spec/grammar.ebnf` assembled from the spec's grammar, with a CI check that the two match.
- The conformance runner: reads `.qnc` files in `testdata/` folders, matches inline expectations (`//~ E0412`, expected output), and reports coverage by rule ID.
- The diagnostics crate: error-code registry, JSON schema, and a CI rule that every code has `explain` text and a test.
- *Exit:* CI is green on an empty suite, and the coverage report lists every rule ID as uncovered.

**M1. Syntax.** Lexical structure, the syntax in Items, Expressions and Statements, the grammar (every chapter's `ebnf` blocks), and the formatting rules in Toolchain.

- Lexer and hand-written recursive-descent parser producing a lossless CST, with error recovery.
- Parse diagnostics with codes and spans; operator precedence and the rules against mixing `&&` with `||` or chaining comparisons.
- `quince fmt` for `.qnc` files, including attribute and comment placement.
- *Exit:* every `parse` and `fmt` fixture in `testdata` passes, and `fmt` is idempotent on every fixture; a fuzzer finds no panics and printing the CST returns the input byte for byte. No test lexes, parses or formats the spec's code.

**M2. Projects, modules and names.** Packages and modules, and the manifest in Dependencies and publishing (path dependencies only).

- The project model: library, `cmd/<name>/` programs, test packages under `tests/`, `testdata/` exclusion, the nested-manifest error.
- Strict `quince.toml` parsing with errors that name the line, and `fmt` for `quince.toml`.
- Imports, visibility, the one-import-path rule, circular-dependency errors, unused imports.
- *Exit:* a fixture project with a library, two programs and tests resolves every name; `quince query signature` returns a definition's file and line.

## Phase 2: The checker and agent tooling

Phase 2 ends when `quince check` fully checks any Quince program, an agent can drive it through JSON diagnostics, `fix` and `query`, and it meets the 1-second target. **This is the first point where agents can write real Quince**, so rerun the agent test against the real checker here.

**M3. Types and patterns.** Items, Types, Prelude and special interfaces, Errors and failure, and Patterns.

- Built-in types, structs, enums, newtypes, constants, generics, interfaces and methods.
- Mutation rules (`var`, `var self`), operators on user types, automatic `Eq` and `Hash`, `Encode` and `Decode` derivation checks.
- Inference, `Result` and `Option`, `?`, patterns, `is` tests, exhaustive `match`, the Never type.
- *Exit:* conformance tests for every rule in those chapters pass, including `check` fixtures of whole programs that should type-check.

**M4. Effects and contracts.** Effects, Program execution, Contracts and Testing (static parts), and the static rules of Concurrency.

- Built-in effects, `uses` inference and checking, effect variables, `effect X uses ...` declarations, `uses *` only on `main`.
- Static capability checks against `[cmd.*]` limits where they can be decided at compile time.
- Contract checking: `requires` and `ensures` purity, the visibility rule on `pub` contracts, struct invariants.
- Task-boundary rules: a `Ref` can't cross into `spawn`, one shared error type per `concurrent` block, one `spawn` per fan-out body.
- *Exit:* Effects and Program execution conformance passes; `quince query effects` explains each effect as a call chain.

**M5. Agent tooling.** Diagnostics and queries, and the commands and performance targets in Toolchain.

- Every diagnostic carries fixes with `safe`, `needs-review` or `placeholder` applicability; `quince fix` applies them; `quince explain` prints each code.
- All `query` commands: members, signature, callers and callees, impls, type-at, effects, search, dependents, source `--without-tests`, raw operations.
- The incremental query database underneath `check`, `query` and `fix`, and the language server on top of it.
- *Exit:* `quince check` on one changed file in a generated 500,000-line project finishes in under 1 second; snapshot tests cover the JSON output of every command.

## Phase 3: Running code

Phase 3 ends when `quince build` produces a native binary that runs concurrent Quince correctly, and `quince test` runs unit tests and doc examples under the seeded scheduler.

**M6. Quince IR and the reference interpreter.** Step 3 of the pipeline (`docs/repo-plan.md`) and Testing (tests and examples).

- Lowering to Quince IR: monomorphization, contracts as explicit checks, overflow checks, panics.
- An internal interpreter over Quince IR. It never ships (the spec rules out an interpreter), but it runs the conformance suite before codegen exists and later serves as an oracle for differential testing.
- First `quince test`: `test` blocks and doc examples, single-threaded, with test fakes for `Net`, `Clock` and `Random`.
- Start `std/` in Quince: Option, Result, List, Map, Set, String, Iter, numbers.
- *Exit:* the core std modules pass their own doc examples in the interpreter.

**M7. Native backend and memory.** Memory, and the compilation sections of Toolchain.

- Reference-count insertion in Quince IR: borrowing, move at last use, reuse in place, the hidden-copy report in `check` and `query`.
- LLVM codegen, debug and release profiles, DWARF, linking a self-contained binary; `quince build` and `quince run`.
- Runtime core in Rust: allocator, counts, panics with Quince source locations, large frees handed to a background task.
- *Exit:* every conformance test gives the same result interpreted and native, in debug and in release; no leaks under a leak checker on the std test suite.

**M8. Concurrency runtime.** Concurrency.

- Green-thread scheduler; `concurrent` blocks, fan-out with `limit`, `background` and `TaskScope` failure policies.
- Cancellation that unwinds at effect calls (never at State), Resource cleanup on the way, `with_timeout`.
- Channels, waiting on several channels, `Atomic`, `ConcurrentMap`, `Pool`; marking values shared and switching them to atomic counts.
- The incremental cycle collector over reference types, per task and for cross-task types.
- The deterministic test scheduler: seeded orderings, several per test, `--seed` replay.
- *Exit:* Concurrency conformance passes under many seeds; a stress test of cancellation storms shows no leaks and no stuck tasks.

## Phase 4: Real programs

Phase 4 ends when four programs (an HTTP service with a database, a CLI tool, a fan-out pipeline, and a C library wrapper) build from git dependencies and pass their tests.

**M9. Standard library and runtime effects.** Effects and Program execution (runtime) and `std/API.md`.

- Real handlers for every built-in effect (`Net`, `Listen`, `Fs`, `Clock`, `Random`, `Env`, `Process`, `Console`); handlers in tests and production; `[cmd.*]` capability limits enforced at runtime, with path normalization.
- The rest of `std/` from the std API: time, URLs, regex, JSON and serialization, HTTP, sync.
- As each package lands, its doc comments take over its section of `std/API.md`, and that section is deleted. Docs generated from the std source replace `std/API.md`.
- *Exit:* an HTTP service and a CLI tool written only from the spec and `quince query` build, run and pass their tests; every std function declares its cost in a form the compiler reads, which `r[contract.cost-warn]` needs; and `quince query signature` and `quince query members` cover every prelude and std item.

**M10. Complete testing.** Testing.

- Integration test packages under `tests/`, `[test-dependencies]`, `--unit` and `--integration`.
- Property tests: derived generators, shrinking, saved regressions in `.quince/regressions`.
- *Exit:* every test kind in Testing has conformance coverage, and a seeded failure replays exactly.

**M11. Packages.** Dependencies and publishing (git dependencies onward).

- Git fetch and tag listing with gitoxide, `dir` subprojects and prefixed tags, minimal version selection, the renaming rules.
- `quince.lock` with content hashes; `quince add` and `quince update` with API-change reports.
- `quince publish`: the API comparison, the version bump it allows, then creating and pushing the tag.
- *Exit:* a fixture graph of local git repos resolves identically on every run; `publish` refuses each breaking-change case the spec lists.

**M12. C interop.** Interop with C.

- `foreign` blocks, C types, `Resource`, `CBuf`, `CString`, callbacks, `@borrowed` and `@owned`, the foreign-call thread pool, `@pinned`.
- `[native]` linking and `sources` built with the bundled C compiler, `allow_native`, the test opt-in rules.
- `quince bind` over libclang.
- *Exit:* a sqlite wrapper, kept as a fixture project in `testdata`, works end to end, from `quince bind` on the header to passing tests.

## Phase 5: Toward 1.0

Phase 5 ends with a 1.0 release: every rule ID in the spec has a passing conformance test, on every tier 1 target.

**M13. Targets and build speed.** Toolchain (targets, incremental compilation).

- Bundled lld and C toolchain; cross-compiling between all tier 1 targets with `--target`; PDB on Windows.
- Per-definition codegen cache keyed by body and signature hashes.
- WebAssembly as tier 2, with concurrency limited as the spec says.
- *Exit:* an incremental build of the 500,000-line project finishes in under 5 seconds; CI builds and tests on all six tier 1 targets.

**M14. Release hardening.** Contracts, Testing, Packages and modules, Dependencies and publishing, and the open questions in `docs/design-principles.md`.

- `quince migrate` scaffolding (a no-op with one edition, but the mechanism and its tests exist).
- `quince verify` with an SMT solver for `@verify` contracts. It never blocks a build, so it can slip past 1.0 without breaking anything.
- Full conformance coverage, a performance pass against the targets, fuzzing of every input format, and docs generated from `explain` texts and std doc comments.
- *Exit:* the coverage report shows no uncovered rule IDs; a fresh agent given only the docs completes the agent test with no meaning-changing guesses.
