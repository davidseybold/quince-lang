# Introduction

Quince is a statically typed, automatically memory-managed, general-purpose language designed so that both AI agents and humans can write, read, and verify code with as little hidden context as possible. Every rule below serves one goal: a reader should be able to understand a function correctly from the function itself.

## Grammar notation

`{ x }` means zero or more, `[ x ]` optional.

## Deliberately excluded

| Feature | Why it's out |
| --- | --- |
| r[intro.excl.exceptions] Exceptions | Failure must appear in the return type |
| r[intro.excl.null] `null` / `nil` | Replaced by `Option<T>` |
| r[intro.excl.macros] Macros and compile-time metaprogramming | Code must mean what it appears to mean |
| r[intro.excl.user-operators] User-defined operators and impure overloading | Each operator calls one fixed, pure prelude interface ([Prelude and special interfaces](../quince-spec.md#operators-on-user-types)), so a + b can never do I/O |
| r[intro.excl.overloading] Function name overloading | Every name resolves to exactly one definition; an interface method resolves through the impl the argument types select |
| r[intro.excl.implicit-conversions] Implicit conversions | All conversions are explicit calls |
| r[intro.excl.global-state] Global mutable state | Shared state is passed explicitly as a `Ref<T>` parameter, and reading or writing it requires `uses State` |
| r[intro.excl.reopen-types] Monkeypatching / reopening types | A type is fully defined in one place |
| r[intro.excl.wildcard-imports] Wildcard imports | Every imported name is listed |
| r[intro.excl.inheritance] Inheritance | Composition and interfaces only |
| r[intro.excl.fmt-config] Formatter configuration | One canonical layout |
