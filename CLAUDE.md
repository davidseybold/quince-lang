# Quince

Quince is a statically typed, reference-counted, effect-tracked language for services and tools, compiled ahead of time through LLVM. This repo will hold its whole implementation: compiler, runtime, standard library, tests and spec.

## The guiding principle

**Quince is designed to be the best possible language for AI coding agents.** Judge every decision, in the language and in this codebase, by what helps an agent read, write and verify code: locality (everything visible in the signature or body), one way to do each thing, familiar Rust/TypeScript/Go surface, and a compiler that explains and fixes. That applies to agents building Quince too: small crates, one command for all checks, precise errors, no hidden setup.

## Where things are

| Path | What it is |
| --- | --- |
| `spec/quince-spec.md` | The language spec, Draft v0.4. **Source of truth for what Quince means.** |
| `spec/std-api.md` | Standard library API: prelude methods, effect operations, every `std` package. |
| `docs/repo-plan.md` | Repo layout, crate list, and why Rust. |
| `docs/milestones.md` | M0 to M14 with exit criteria. Work in this order. |

## Decisions already made (don't reopen without asking David)

- One monorepo. Compiler and runtime in Rust. No self-hosting before 1.0.
- LLVM is the only backend. LLVM is linked only by `codegen-llvm` and `bind`, so most crates build without it.
- Memory is compiler-inserted reference counting (Perceus style) plus a cycle collector for reference types only. No tracing GC.
- Effects: built-in effects including a `State` effect, `uses E` effect variables, `effect X uses ...` declarations.
- Cancellation unwinds, can't be caught, and never happens at a State operation.
- No central registry: git dependencies plus path dependencies, minimal version selection, lockfile pinned by content hash.
- Name: Quince. CLI `quince`, manifest `quince.toml`, source `.qnc`.

## How work is done

- **The core loop is spec rule → conformance test → code.** For each rule ID you implement, first write `.qnc` conformance tests from the spec text alone, then implement until they pass. If you can't write a test for a rule, the rule is ambiguous: stop and ask.
- **OpenSpec, kept light.** The language spec says what Quince means. `openspec/specs/` holds only implementation decisions the language spec doesn't cover (JSON diagnostics schema, CST design, pipeline, runtime internals, package formats), and never restates the language spec. Open one OpenSpec change per milestone chunk (for example "M1: lexer"), with a design and a task list kept current so the next session can resume from it. Bug fixes and small tasks don't need a change. Revisit after M1 whether OpenSpec is earning its keep.
- **Don't edit `spec/` on your own.** If the spec is ambiguous, contradictory or missing something, stop and ask David, and propose wording. A change to the language edits the spec in the same PR as the code and tests.
- Every spec rule gets a stable ID, and every conformance test cites one. Every error code has `explain` text and a test.
- Before finishing any change, run `cargo xtask ci` (once M0 creates it) and keep it green. Never skip or delete a test to get green.
- The repo root has no `quince.toml`, because the spec makes a nested manifest a compile error. `.qnc` fixtures go in `testdata/` folders, which are never compiled.
