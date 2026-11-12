# Quince Language Spec

Nov 12, 2026 · @David · Draft v0.15

The spec says what Quince programs mean, and what the tools guarantee about them. On 2026-11-10 it was split from one file into chapters, and its rule IDs were renamed once to name their chapter ([design](../docs/design/spec-restructure.md)); [`id-map.tsv`](../docs/design/spec-restructure/id-map.tsv) maps each old ID to its new one. No meaning changed.

## Chapters

Part I, the language:

- [Introduction](reference/introduction.md): what Quince is for, the grammar notation, and the features it leaves out.
- [Lexical structure](reference/lexical.md): line breaks, keywords, literals, naming, comments and tokens.
- [Packages and modules](reference/packages.md): imports, the layout of a project's library, programs and tests, and editions.
- [Items](reference/items.md): constants, structs, enums, newtypes, methods, interfaces, generics, arguments and attributes.
- [Types](reference/types.md): the built-in types, type arguments, mutation and reference types, and inference.
- [Prelude and special interfaces](reference/prelude.md): the names every file can use, and the interfaces the language calls for operators, iteration, equality and serialization.
- [Expressions](reference/expressions.md): blocks, operators, ranges, indexing, `match`, `is`, jumps and closures.
- [Statements](reference/statements.md): bindings, assignment and loops.
- [Patterns](reference/patterns.md): what a pattern matches and binds.
- [Errors and failure](reference/errors.md): `Result` and `Option` for expected failures, the `?` operator, and panics.
- [Effects](reference/effects.md): the built-in effects, declaring effects, effect variables, handlers and `with`, and the built-in effects' operations.
- [Concurrency](reference/concurrency.md): `concurrent` blocks, fan-out, background scopes, cancellation and timeouts, shared state, channels and `select`.
- [Contracts](reference/contracts.md): `requires`, `ensures`, struct invariants and static verification.
- [Testing](reference/testing.md): doc examples, unit and integration tests, effect fakes, property tests and deterministic scheduling.
- [Memory](reference/memory.md): reference counting, sharing across tasks, and the cycle collector.
- [Program execution](reference/execution.md): the capabilities behind effects, and how `main` starts and ends a program.
- [Interop with C](reference/interop.md): foreign blocks and generated bindings, C types, trust, linking, C memory, pools, callbacks and foreign calls.

Part II, the tools the language guarantees:

- [Dependencies and publishing](tooling/dependencies.md): the `quince.toml` manifest, the lockfile, minimal version selection, content addressing, and the semantic versioning `publish` enforces.
- [Toolchain](tooling/toolchain.md): the `quince` commands, performance targets, formatting rules, compilation and its backend, build profiles, targets, incremental builds and debugging.
- [Diagnostics and queries](tooling/diagnostics.md): structured diagnostics and their fixes, and the query API.

Read Part I in the order listed, and Part II as needed. [`chapters.txt`](chapters.txt) lists them with their rule-ID prefixes.

The standard library's API isn't part of the spec. It is in [`std/API.md`](../std/API.md), which isn't normative and needs no approval to change.
