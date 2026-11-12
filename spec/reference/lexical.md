# Lexical structure

r[lex.braces]
Quince uses C-family syntax with braces, semicolon-free statements, and a fixed set of 38 keywords.

r[lex.indentation]
Indentation never carries meaning, so a misapplied diff can't silently change program structure.

r[lex.line-end]
A line break ends a statement unless the line ends with a binary operator, a comma or an opening bracket, as in Go. A long expression therefore breaks after its operator, never before, and `quince` `fmt` always formats it that way: a line that starts with `- b` is a new statement, not a continuation.

r[lex.continue.more]
A line also continues when it ends with `=`, a compound assignment such as `+=`, `=>` or `->`, and when the next line's first token is `.`, so a method chain can lead each line with its dot. No statement can start with `.`, so this adds no ambiguity.

r[lex.else-same-line]
`else` must sit on the line of the closing `}`; a line that starts with `else` is an error whose fix joins it.

r[lex.continue.fn-header]
Inside any `fn` header, whether a declaration's signature, a closure's or a function type, a line that starts with `->`, `uses`, `requires`, `ensures` or `{` continues the header. So `quince` `fmt` breaks a long signature before those words, as in [Effects](effects.md) and [Contracts](contracts.md).

r[lex.continue.lists]
Inside parentheses, square brackets, angle brackets and the braces of a comma-separated list, such as fields, variants, map entries and match arms, a line break before a `,` or the closing bracket is only spacing. Elsewhere in them the rules above apply, so a line that starts with `-` or `(` starts a new item, which needs a `,` before it. A block inside them is a line context again. `else` still goes on the line of the closing `}`.

## Source text

r[lex.source.utf8]
A source file is UTF-8 text. A file that isn't valid UTF-8 is a compile error.

r[lex.source.bom]
A byte order mark at the start of a file is ignored by the compiler.

r[lex.source.line-end]
A line ends at a line feed, or at a carriage return followed by a line feed. A carriage return anywhere else is a compile error.

r[lex.source.canonical]
`quince` `fmt` writes every line ending as a line feed and removes a byte order mark. So, like any unformatted code, a file with CR LF line endings or a byte order mark compiles but fails `quince` `check` ([Toolchain](../tooling/toolchain.md)).

r[lex.source.whitespace]
Within a line, spaces and tabs are the only whitespace.

r[lex.source.ascii]
Outside literals and comments, source text is ASCII, so identifiers are ASCII letters, digits and underscores.

r[lex.source.control]
Apart from tabs and line endings, a literal or comment can't contain a control character (U+0000 to U+001F, and U+007F) or a bidirectional-format character (U+202A to U+202E, U+2066 to U+2069). A `"` or `"""` string writes one as an escape. Raw and tagged literals and comments can't hold them.

## Keywords

r[lex.keywords]
`fn` `let` `var` `type` `struct` `enum` `interface` `impl` `match` `is` `if` `else` `for` `in` `while` `return` `break` `continue` `import` `pub` `uses` `requires` `ensures` `test` `forall` `with` `concurrent` `spawn` `background` `foreign` `true` `false` `self` `Self` `const` `effect` `select` `as`

r[lex.no-contextual]
There are no contextual keywords: every reserved word means the same thing in every position, and no other word is special, apart from main, the tagged-literal prefixes ([Literals](#literals)), the named arguments of keyword forms (limit:, settle:, on\_error:), the case names that @wire(case: ...) takes (camel, snake, kebab, pascal, upper\_snake), and TaskScope in a background header.

r[lex.keyword-not-name]
A keyword therefore can't name a binding, parameter, field or method. A wire name that is a keyword is written with `@wire` ([Prelude and special interfaces](prelude.md)).

## Literals

r[lex.lit.int.forms]
- Integer: `42`, `1_000_000`, `0xFF`, `0b1010`, `0o17`

r[lex.lit.int.default]
- An integer literal's default type is `Int` (64-bit, overflow panics, never wraps).

r[lex.lit.int.context]
- An integer literal takes another integer type from context, as in let b: UInt8 = 200, and a literal that doesn't fit that type is a compile error.

r[lex.lit.int.neg]
- A minus sign directly before an integer literal in prefix position is part of it, so Int.MIN can be written -9\_223\_372\_036\_854\_775\_808

r[lex.lit.neg-adjacent]
- Directly before means with nothing between them, not even a space, and the literal isn't followed by a postfix operator. So `- 1` is the prefix minus applied to `1`, and `-2.pow(2)` is `-(2.pow(2))`, as `-x.abs()` is.

r[lex.lit.float.forms]
- Float: `3.14`, `1e-9`

r[lex.lit.float.default]
- A float literal's default type is `Float` (64-bit).

r[lex.lit.float.context]
- A float literal takes Float32 from context the same way

r[lex.lit.underscore]
- In any numeric literal, including a float's fraction and exponent, an underscore may appear only between two digits. So `1_000` and `1_000.5` are valid, and `_1`, `1_`, `1__0`, `0x_FF` and `1_.5` are not.

r[lex.lit.int.prefix]
- The base prefixes are lowercase (`0x`, `0b`, `0o`). Hex digits may be either case.

r[lex.lit.int.leading-zero]
- A decimal literal, or a float's integer part, doesn't start with `0` unless it is `0`. So `007` and `00.5` are errors, rather than octal as in C, and `0.5` is fine.

r[lex.lit.float.shape]
- A float has digits on both sides of its point (`1.0`, not `1.` or `.5`), and its exponent is a lowercase `e`, an optional sign and digits. `1E9` is an error whose fix lowercases the `E`.

r[lex.lit.no-suffix]
- A numeric literal has no type suffix: its type comes from context.

r[lex.lit.string.interp]
- String: `"hello {name}"`. UTF-8; interpolation with {} is the only formatting syntax.

r[lex.lit.string.interp-root]
- Inside braces only a path is allowed, never another expression: a name, `self` or a constant name, followed by any number of field names or tuple positions, with no spaces: `{name}`, `{user.name}`, `{self.email}`, `{id.0}`, `{MAX_RETRIES}`.

r[lex.lit.string.lone-brace]
- Only { opens interpolation, so a lone } is plain text and needs no escape.

r[lex.lit.string.escapes]
- String escapes: \\n \\t \\\\ \\" \\{ \\r \\0 \\u{1F600}.

r[lex.lit.string.unicode-escape]
- `\u{...}` holds one to six hex digits naming a Unicode scalar value, so `\u{}`, surrogates (`\u{D800}`) and values above `\u{10FFFF}` are errors.

r[lex.lit.string.one-line]
- A "..." string stays on one line.

r[lex.lit.string.multiline]
- A multi-line string sits between """ lines: it interpolates, starts on the line after the opening """, and drops the closing line's indentation from every line, as in Swift and Kotlin.

r[lex.lit.string.raw-multiline]
- r""" ... """ is its raw form, which may contain " and { freely

r[lex.lit.multiline-open]
- After the opening `"""`, only spaces and tabs may come before the line break.

r[lex.lit.multiline-quotes]
- The closing `"""` is the first thing on its line after its indentation, and code may follow it on that line (`""")`). Elsewhere in the string, `"` and `""` are text, but `"""` is a compile error: a `"""` string writes it `\"""`, and an `r"""` string can't hold it.

r[lex.lit.multiline-indent]
- Every line between must start with exactly the closing line's indentation, the same spaces and tabs, except a line that holds only whitespace, which becomes empty. Otherwise it is a compile error.

r[lex.lit.multiline-value]
- The value doesn't include the line break before the closing line, and every line break in it is a line feed, whatever the file uses.

r[lex.lit.multiline-forms]
- Only `"""` and `r"""` are multi-line. A `"""` string takes the same escapes as a `"` string. `b"""` and tagged multi-line literals don't exist.

r[lex.lit.raw-string]
- Raw string: `r"C:\path"`. No escapes, no interpolation; it can't contain a quote, so text with quotes uses r"""

r[lex.lit.char]
- Char: `'a'`, `'\n'`, `'\u{1F600}'`. Type Char: one Unicode scalar value.

r[lex.lit.char.escapes]
- A char literal takes the same escapes as strings, plus \\'

r[lex.lit.bool]
- `true` and `false` are the literals of `Bool`.

r[lex.lit.bytes]
- Bytes: `b"GET /"`, `b"\x00\xFF"`. Type Bytes: ASCII characters, \\x escapes and the escapes \\n \\r \\t \\0 \\\\ \\". No interpolation

r[lex.lit.bytes.hex-escape]
- `\x` takes exactly two hex digits, and appears only in bytes literals.

r[lex.lit.tagged]
- Tagged literal: `url"https://..."`, `re"[a-z]+"`, `dur"30s"`. Validated at compile time; a fixed, language-defined set of tags (v1: url, re, dur, ts).

r[lex.lit.tagged.raw]
- A tagged literal's body is raw, as in a raw string: no escapes and no interpolation, so re"\\d+\\.\\d+" and re"a{2}" mean what they say

r[lex.lit.collections]
- Collections: `[1, 2, 3]`, `{"a": 1}`. List and Map.

r[lex.lit.empty-map]
- The empty map is {:}, so it can't be mistaken for an empty block.

r[lex.lit.no-set]
- Sets have no literal: Set.from(\[1, 2, 3\])

## Naming

r[lex.name.case]
- Types and enum variants: `PascalCase`. Functions, bindings, fields, packages, files: `snake_case`. Constants: `SCREAMING_SNAKE_CASE`.

r[lex.name.enforced]
- Naming case is enforced by the compiler, not the linter.

r[lex.name.foreign-exempt]
- Names declared inside a foreign block are exempt and keep their exact C spelling, so they match the library's documentation ([Interop with C](interop.md)).

## Comments

r[lex.comment.line]
- `//` line comments only. No block comments, which break easily under partial edits.

r[lex.comment.doc]
- `///` doc comments attach to the next declaration and may contain executable examples (see [Testing](testing.md)).

r[lex.comment.doc-exact]
- A doc comment starts with exactly three slashes: `////` starts an ordinary comment.

## Tokens and identifiers

r[lex.grammar-rules]
Three lexer rules apply: `>>` is never one token (the parser joins two adjacent `>` for a shift), `..=` is one token, and `|` inside a pattern is alternation, so `x is A | B` tests for either variant.

r[lex.no-shift-token]
- The lexer never makes `>>` one token, so `List<List<Int>>` closes without a space.

r[lex.ident-classes]
`name` is a `snake_case` identifier, `Name` a `PascalCase` one and `NAME` a `SCREAMING_SNAKE_CASE` one.

r[lex.ident-not-keyword]
None of them can be a keyword.

r[lex.c-name]
`c_name` is any C identifier, kept in its C spelling, and appears only for what a `foreign` block declares. `path` is `name { "." name }`, a package path such as `std.http`. `literal`, `string`, `int` and `doc_comment` are the tokens of [Lexical structure](lexical.md).
