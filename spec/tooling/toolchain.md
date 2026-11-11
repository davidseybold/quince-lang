# Toolchain

r[tool.cli.single-binary]
The toolchain is a single binary, `quince`, whose every command can emit structured JSON. It is designed to be driven by an agent as much as by a person.

## Commands

r[tool.cli.check]
- `quince` `check`: Type-checks, lints, and verifies formatting; no code generation; warnings don't fail it unless --deny warnings is passed, which CI does by default

r[tool.cli.build]
- `quince` `build`: Compiles a binary or library

r[tool.cli.run]
- `quince` `run`: Builds and runs `main`

r[tool.cli.test]
- `quince` `test`: Runs unit tests, integration tests, doc examples, property tests, and saved regressions

r[tool.cli.fmt]
- `quince` `fmt`: Applies the canonical format

r[tool.cli.fix]
- `quince` `fix`: Applies every machine-applicable suggested fix

r[tool.cli.query]
- `quince` `query`: Answers questions about the codebase ([Query API](diagnostics.md#query-api))

r[tool.cli.verify]
- `quince` `verify`: Attempts static proof of `@verify` contracts

r[tool.cli.publish]
- `quince` `publish`: Checks semver against the previous version, then creates and pushes the version's git tag

r[tool.cli.migrate]
- `quince` `migrate`: Moves a package to a newer edition

r[tool.cli.bind]
- `quince` `bind`: Generates raw foreign declarations from a C header ([Interop with C](../reference/interop.md))

r[tool.cli.update]
- `quince` `update`: Raises dependency minimums and reports API changes ([Dependencies and publishing](dependencies.md))

r[tool.cli.explain]
- `quince` `explain`: Prints the full explanation of an error code

r[tool.cli.add]
- `quince` `add`: Adds a dependency at its newest version and lists every version it raised ([Dependencies and publishing](dependencies.md))

r[tool.perf.targets]
Performance targets: `quince` `check` on a single changed file in a 500,000-line project should finish in under 1 second, and an incremental build in under 5 seconds.

## Formatting rules (enforced by `quince` `fmt`)

r[tool.fmt.one-per-line]
- One declaration, statement, or match arm per line.

r[tool.fmt.trailing-comma]
- Trailing commas required in all multi-line lists, arguments, fields, and arms.

r[tool.fmt.width]
- 100-column limit; the formatter decides all line breaks.

r[tool.fmt.imports]
- Imports sorted and grouped (standard library, then packages, then local).

r[tool.fmt.check]
- Unformatted code compiles but fails `quince` `check`, which CI runs by default.

- A struct literal may write a field whose value is a binding of the same name once: `Dashboard(profile:, orders:)`.

r[tool.fmt.field-shorthand]
- `quince` `fmt` rewrites `profile: profile` to that form, so each literal has one canonical layout.

## Compilation

r[tool.build.native]
Quince compiles ahead of time to native code through LLVM, producing a single self-contained binary that includes the runtime (scheduler and cycle collector).

r[tool.build.static]
On Linux the binary is fully static, unless a native library is available only as a shared library, which quince build reports; on macOS and Windows it links only the operating system's own libraries.

r[tool.build.no-jit]
There is no interpreter or JIT.

## Backend decision

r[tool.build.llvm-only]
LLVM is the only backend, for debug and release builds alike. A second, faster debug backend (as Zig and Rust have pursued) is not planned: it is years of work for modest gains, since type checking, monomorphization, and linking take much of the build time too. Fast feedback comes instead from `quince` `check` (no code generation), per-definition caching, and debug builds that skip optimization. Quince IR is kept backend-independent so a Cranelift debug backend can be added later if real-world build times require it.

## Build profiles

r[tool.profile.debug]
- `debug` (default). LLVM optimization: O0. Overflow checks: On. Contracts: On. Debug info: Full

r[tool.profile.release]
- `release`. LLVM optimization: O2, ThinLTO. Overflow checks: On. Contracts: On, except `@unchecked`. Debug info: Line tables

r[tool.profile.overflow]
Overflow checks stay on in release builds because overflow is defined as a panic ([Errors and failure](../reference/errors.md)), not an optimization opportunity.

## Targets

r[tool.target.tier1]
- Tier 1: x86-64 and ARM64 on Linux, macOS, Windows. Built and fully tested on every release

r[tool.target.tier2]
- Tier 2: WebAssembly (`wasm32`). Builds; concurrency limited until WebAssembly stack switching is widely available

r[tool.target.cross]
Cross-compilation is built in: `quince` `build --target linux-arm64` works from any tier 1 host with no separate toolchain install, for packages with no foreign dependencies.

r[tool.target.cross-foreign]
Cross-compiling foreign code needs the target's C libraries, and macOS targets need Apple's SDK.

## Incremental compilation

r[tool.incr.cache]
Compiled output is cached per definition, keyed by a hash of the definition's own body plus the signature hashes of everything it calls.

r[tool.incr.body]
Changing a function's body recompiles only that function and what it was inlined into; changing a signature also recompiles its callers. The full content hash from [Content addressing](dependencies.md#content-addressing), which covers every dependency, identifies definitions in the lockfile.

## Debugging

r[tool.debug.dwarf]
Binaries emit DWARF (or PDB on Windows), so standard debuggers work.

r[tool.debug.locations]
Panic traces and the deterministic test scheduler ([Testing](../reference/testing.md)) report Quince source locations, never generated code.
