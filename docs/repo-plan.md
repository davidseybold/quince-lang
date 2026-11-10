# Quince repo plan

Updated 2026-11-10 against spec Draft v0.4. Nothing has been created yet.

## Decisions

| Question | Answer | Changed since v0.1? |
| --- | --- | --- |
| Repo shape | One monorepo, `quince` | Simpler: with no central registry there is no registry service, so nothing lives outside it |
| Compiler language | Rust | Kept, and v0.4 strengthens the case (below) |
| Runtime language | Rust, built as a static library linked into every binary | Kept |
| Self-hosting | Not before 1.0, maybe never | Kept |
| Language spec | Stays prose with rule IDs, in `spec/` once it moves out of the doc | Kept |
| Implementation specs and change process | OpenSpec, in `openspec/` | Added after v0.1 |
| First build step | The parser, in `compiler/syntax` | New |

## Why Rust still fits v0.4

Every requirement v0.4 added has an established Rust library behind it:

- `quince bind` reads C headers. libclang through `clang-sys`, the same route bindgen takes.
- `[native] sources` are built with "the toolchain's C compiler", and cross-compiling needs no separate install. Rust can link LLVM's clang and lld libraries, so one binary carries the C compiler, the linker and codegen. The `cc` crate's target handling is a useful reference.
- Git dependencies over HTTPS with no registry. `gix` (gitoxide) is pure Rust, so `quince` doesn't need a `git` binary installed to fetch, list tags, or push publish tags.
- `quince.toml` is strict TOML that `quince fmt` formats, and that `update` and `add` rewrite. `toml_edit` preserves comments and layout.
- Content hashes for the lockfile and build cache. `blake3`.
- Quince IR is meant to allow a Cranelift debug backend later. Cranelift is itself a Rust library.
- Enums, `Result`, traits and exhaustive `match` are close to Quince's own model, and agents write Rust well.

What I looked at and rejected:
- **Zig** has a built-in C toolchain, but it's pre-1.0 and changes under you, and agents write it less reliably.
- **Go** has no solid LLVM bindings.
- **OCaml** is a classic compiler language, but there's far less agent training data for it.

The cost is that LLVM makes Rust builds slow and fussy. To contain that, pin one LLVM version, use a prebuilt LLVM in CI, and keep LLVM confined to `codegen-llvm` and `bind`. Then the parser, checker, `query`, `fix` and the language server all build without it, and that's where most agent work happens.

## Layout

```
quince/
  AGENTS.md              build, test, bless; map of the repo
  spec/                  language spec (Markdown, one file per section, rule IDs)
    grammar.ebnf         the grammar appendices, extracted; the parser is tested against it
  openspec/
    specs/               cli, diagnostics, query, compiler-pipeline, runtime, pkg, bind, lsp
    changes/             every unit of work; changes cite spec rule IDs
  compiler/              cargo workspace, crates named quince_*
    syntax/              lexer, parser, lossless CST, formatter        <- start here
    types/ effects/      name resolution, type and effect checking, exhaustiveness
    diagnostics/         error codes, JSON schema, fixes, `explain` text
    query/               incremental database behind check, query, fix and the LSP
    ir/                  Quince IR: monomorphization, contracts, RC insertion
    codegen-llvm/        the only crate besides bind that links LLVM
    pkg/                 quince.toml, MVS, git fetch, quince.lock, hashing, publish semver
    bind/                C headers to foreign blocks (libclang)
    native/              build [native] sources, link, cross-compile
    lsp/
    cli/                 the single `quince` binary
  runtime/               Rust staticlib: scheduler, RC, cycle collector, FFI pool, deterministic test scheduler
  std/                   standard library in Quince, built from std-api v0.4
  tests/
    conformance/         .qnc files with inline expectations, tagged with rule IDs (in testdata/ folders)
    diagnostics/ fix/ query/   snapshot tests of the JSON outputs
    pkg/                 local git fixture repos for resolution, tags, lockfile
    perf/                the 1s check and 5s incremental build targets
  xtask/                 `cargo xtask ci` runs every fast check
```

v0.4 rules the layout has to respect:
- A `quince.toml` below a project root is a compile error. So the repo root has no `quince.toml`. `std/` is its own project, and every test that needs a manifest gets its own folder.
- `testdata/` folders are never compiled. Conformance fixtures go there, so tooling that walks the repo can't mistake them for code.

## First step: the parser

The parser lives in `compiler/syntax` (`quince_syntax`). It has no dependencies on the rest of the compiler and no LLVM, so it builds in seconds.

1. Extract the two grammar appendices into `spec/grammar.ebnf`, and add a CI check that the file matches the spec text.
2. Write a hand-written recursive-descent parser that produces a lossless CST, in the style of `rowan`. Agent tools need comments and spacing preserved: `fmt`, `fix` and the LSP all rewrite source, and error recovery depends on it too.
3. Tests:
   - Every code block in the spec and the std API parses.
   - One conformance test per grammar production, tagged with its rule ID.
   - Snapshot tests of the CST and of parse errors.
   - A fuzzer that checks the parser never panics, and that printing the CST gives back the exact input.
4. Next, `quince fmt`, since the formatter only needs the CST. After that, name resolution.
