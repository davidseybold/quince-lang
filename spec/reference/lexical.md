# Lexical structure

r[lex.braces] Quince uses C-family syntax with braces, semicolon-free statements, and a fixed set of 38 keywords. r[lex.indentation] Indentation never carries meaning, so a misapplied diff can't silently change program structure.

r[lex.line-end] A line break ends a statement unless the line ends with a binary operator, a comma or an opening bracket, as in Go. A long expression therefore breaks after its operator, never before, and `quince` `fmt` always formats it that way: a line that starts with `- b` is a new statement, not a continuation.

r[lex.continue.more] A line also continues when it ends with `=`, a compound assignment such as `+=`, `=>` or `->`, and when the next line's first token is `.`, so a method chain can lead each line with its dot. No statement can start with `.`, so this adds no ambiguity. r[lex.else-same-line] `else` must sit on the line of the closing `}`; a line that starts with `else` is an error whose fix joins it. r[lex.continue.signature] Inside a `fn` signature, a line that starts with `->`, `uses`, `requires`, `ensures` or `{` continues the signature, so `quince` `fmt` breaks a long signature before those words, as in [Effects](../quince-spec.md#5-effects-and-capabilities) and [Contracts](../quince-spec.md#requires-and-ensures).

## Keywords

r[lex.keywords] `fn` `let` `var` `type` `struct` `enum` `interface` `impl` `match` `is` `if` `else` `for` `in` `while` `return` `break` `continue` `import` `pub` `uses` `requires` `ensures` `test` `forall` `with` `concurrent` `spawn` `background` `foreign` `true` `false` `self` `Self` `const` `effect` `select` `as`

r[lex.no-contextual] There are no contextual keywords: every reserved word means the same thing in every position, and no other word is special, apart from main, the tagged-literal prefixes ([Literals](#literals)), the named arguments of keyword forms (limit:, settle:, on\_error:), the case names that @wire(case: ...) takes (camel, snake, kebab, pascal, upper\_snake), and TaskScope in a background header. r[lex.keyword-not-name] A keyword therefore can't name a binding, parameter, field or method. A wire name that is a keyword is written with `@wire` ([Prelude and special interfaces](prelude.md)).

## Literals

| Kind | Examples | Notes |
| --- | --- | --- |
| Integer | r[lex.lit.int.forms] `42`, `1_000_000`, `0xFF`, `0b1010`, `0o17` | r[lex.lit.int.default] Default type `Int` (64-bit, overflow panics, never wraps). r[lex.lit.int.context] Takes another integer type from context, as in let b: UInt8 = 200, and a literal that doesn't fit that type is a compile error. r[lex.lit.int.neg] A minus sign directly before a literal in prefix position is part of it, so Int.MIN can be written -9\_223\_372\_036\_854\_775\_808 |
| Float | r[lex.lit.float.forms] `3.14`, `1e-9` | r[lex.lit.float.default] Default type `Float` (64-bit). r[lex.lit.float.context] Takes Float32 from context the same way |
| String | `"hello {name}"` | r[lex.lit.string.interp] UTF-8; interpolation with {} is the only formatting syntax. r[lex.lit.string.interp-path] Inside braces only a name or a field path is allowed ({name}, {user.name}), never another expression. A field path may read a newtype or tuple position, as in {id.0}. r[lex.lit.string.lone-brace] Only { opens interpolation, so a lone } is plain text and needs no escape. r[lex.lit.string.escapes] Escapes: \\n \\t \\\\ \\" \\{ \\r \\0 \\u{1F600}. r[lex.lit.string.one-line] A "..." string stays on one line. r[lex.lit.string.multiline] A multi-line string sits between """ lines: it interpolates, starts on the line after the opening """, and drops the closing line's indentation from every line, as in Swift and Kotlin. r[lex.lit.string.raw-multiline] r""" ... """ is its raw form, which may contain " and { freely |
| Raw string | `r"C:\path"` | r[lex.lit.raw-string] No escapes, no interpolation; it can't contain a quote, so text with quotes uses r""" |
| Char | `'a'`, `'\n'`, `'\u{1F600}'` | r[lex.lit.char] Type Char: one Unicode scalar value. r[lex.lit.char.escapes] Same escapes as strings, plus \\' |
| Bytes | `b"GET /"`, `b"\x00\xFF"` | r[lex.lit.bytes] Type Bytes: ASCII characters, \\x escapes and the escapes \\n \\r \\t \\0 \\\\ \\". No interpolation |
| Tagged literal | `url"https://..."`, `re"[a-z]+"`, `dur"30s"` | r[lex.lit.tagged] Validated at compile time; a fixed, language-defined set of tags (v1: url, re, dur, ts). r[lex.lit.tagged.raw] The body is raw, as in a raw string: no escapes and no interpolation, so re"\\d+\\.\\d+" and re"a{2}" mean what they say |
| Collections | `[1, 2, 3]`, `{"a": 1}` | r[lex.lit.collections] List and Map. r[lex.lit.empty-map] The empty map is {:}, so it can't be mistaken for an empty block. r[lex.lit.no-set] Sets have no literal: Set.from(\[1, 2, 3\]) |

## Naming

- r[lex.name.case] Types and enum variants: `PascalCase`. Functions, bindings, fields, packages, files: `snake_case`. Constants: `SCREAMING_SNAKE_CASE`.
- r[lex.name.enforced] Naming case is enforced by the compiler, not the linter. r[lex.name.foreign-exempt] Names declared inside a foreign block are exempt and keep their exact C spelling, so they match the library's documentation ([Interop with C](../quince-spec.md#11-interop)).

## Comments

- r[lex.comment.line] `//` line comments only. No block comments, which break easily under partial edits.
- r[lex.comment.doc] `///` doc comments attach to the next declaration and may contain executable examples (see [Testing](../quince-spec.md#executable-examples-in-doc-comments)).

## Tokens and identifiers

r[lex.grammar-rules] Three lexer rules apply: `>>` is never one token (the parser joins two adjacent `>` for a shift), `..=` is one token, and `|` inside a pattern is alternation, so `x is A | B` tests for either variant.

- r[lex.no-shift-token] The lexer never makes `>>` one token, so `List<List<Int>>` closes without a space.

r[lex.ident-classes] `name` is a `snake_case` identifier, `Name` a `PascalCase` one and `NAME` a `SCREAMING_SNAKE_CASE` one. r[lex.ident-not-keyword] None of them can be a keyword. r[lex.c-name] `c_name` is any C identifier, kept in its C spelling, and appears only for what a `foreign` block declares. `path` is `name { "." name }`, a package path such as `std.http`. `literal`, `string`, `int` and `doc_comment` are the tokens of [Lexical structure](lexical.md).
