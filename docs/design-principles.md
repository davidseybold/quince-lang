# Design principles

What Quince is for, the principles behind its design, and the questions its design left open. They guide decisions about the language but aren't rules a program follows, so they aren't part of the [spec](../spec/README.md). They moved here from the spec when it was split into chapters ([design](design/spec-restructure.md)).

## Target domain

Quince is built for services and tools: web servers and APIs, command-line tools, data pipelines, and automation. It relies on compiler-inserted reference counting and a green-thread runtime, and accepts their small runtime costs in exchange for never making anyone manage memory by hand.

- **Non-goals:** operating systems, drivers, and embedded devices, which can't host a runtime; and latency-critical work such as game engines and audio, where every allocation and free must be under the programmer's control.
- **Future option:** arenas or regions (memory allocated and freed as a group) for hot paths that can't afford per-object reference counting. The runtime and Quince IR should not rule this out, but it is not part of version 1.
- **Ruled out:** an ownership system like Rust's. It would reach every domain but reintroduces the complexity this language exists to avoid.

## Core principles

1. **Locality.** Everything that affects a function's behavior is visible in its signature, its body, and the declarations of the types and effects it names: its types, its effects, its failure modes, and its contracts.
2. **One way to do it.** Where two constructs would overlap, the language keeps one, and where it keeps two (such as is and match) the spec says when to use each. Fewer choices means fewer wrong choices.
3. **The compiler is a collaborator.** Diagnostics are precise, machine-readable, and come with suggested fixes. The compiler can be queried, not just run.
4. **Correctness is stated, then checked.** Types, exhaustive matching, contracts, and executable examples turn intent into something the toolchain verifies.
5. **Nothing changes meaning silently.** Published code never changes behavior under a dependent. Changes to the shape of a public API (types, signatures, effects and contracts) are detected mechanically.
6. **Familiar surface.** Syntax stays close to TypeScript, Rust, and Go so knowledge transfers. Novelty goes into semantics and tooling, not punctuation.

## Open questions

Only the name remains open. Every design question raised in the v0.1 and v0.2 reviews has been decided in this draft, and the reviews record each decision.

- [x] **Name: Quince.** Decided 2026-11-10, before the first public release. "Lucid" was the working title, dropped because it collides with an active Princeton research language for network data planes (with a verification-focused successor, VeriLucid) and an old 1970s dataflow language. A web search found no language or developer tool named Quince. The command is quince, the manifest is quince.toml, and source files use .qnc.
