# Quince Language Spec

Nov 10, 2026 · @David · Draft v0.5

The spec says what Quince programs mean, and what the tools guarantee about them. It is being split from one file into chapters ([design](../docs/design/spec-restructure.md)). Until the split is done, the chapters below and the file being split hold the whole spec between them.

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

Still being split:

- [Quince Language Spec (being split)](quince-spec.md): every rule not yet moved to a chapter.

Read the chapters in the order listed. [`chapters.txt`](chapters.txt) lists them with their rule-ID prefixes.

The standard library's API isn't part of the spec. It is in [`std/API.md`](../std/API.md), which isn't normative and needs no approval to change.
