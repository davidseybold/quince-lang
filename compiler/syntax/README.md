# quince_syntax

The lexer now, and the parser, CST and formatter later. This file holds the contracts the spec leaves to the implementation: what a token is, and which errors the lexer owns. The design is in [`docs/design/m1-lexer.md`](../../docs/design/m1-lexer.md). The language rules are in the spec's [Lexical structure](../../spec/reference/lexical.md) chapter, and aren't restated here.

## Source text

- `decode_source(&[u8])` is the one place source bytes become text. Every caller that reads a file uses it before lexing.
- Each maximal run of invalid UTF-8 becomes one U+FFFD in `Decoded::text`, and `Decoded::replaced` records where. Each line that holds invalid bytes gets one E0014, at its first run.
- When `Decoded::is_lossy()` is true, the text isn't the file, so no tool may write it back. Every writer checks it: `fmt`, `fix` and the language server.
- A file over 4 GiB, more than a `u32` offset can address, is an I/O error, not a diagnostic.

## Tokens

- `lex(&Decoded)` lexes decoded text into a `Lexed`, which holds its tokens and errors. `lex_str(&str)` lexes text that never came from bytes.
- A `Token` is a `SyntaxKind` and a byte length. A token starts where the one before it ends.
- Every byte belongs to exactly one token, so the token texts concatenate to the input. No token is empty.
- The lexer never fails and never stops early. An error produces a `LexError` (byte offsets, a code, a message and fixes) and a token, and lexing goes on. `Lexed::diagnostics` turns the errors into `Diagnostic`s through a `LineIndex`.
- Trivia, which the parser skips:
  - `Whitespace`: a run of spaces and tabs.
  - `Comment`: `//` up to its line ending.
  - `BlockComment`: `/*` to the next `*/`, or to the end of the file when there is none.
  - `Bom`: a byte order mark at the very start.
  - `Error`: a run of characters that start no token, or one U+FFFD that decoding put in place of invalid bytes.
- `Newline` is one line ending: LF, CR LF, or a lone CR. A lone CR is an error, but it ends a line here as it does in `LineIndex`, so the parser's line ends agree with the line numbers in diagnostics. Each line ending is its own token.
- `DocComment` is `///` up to the line ending, but not `////`. Whether the parser treats it as trivia is the parser's call.
- An identifier is `Ident` whatever its case. Each keyword has its own kind (`FnKw`, `SelfTypeKw` for `Self`). Nothing else is special: `main`, `TaskScope` and the tag names are identifiers. `_` alone is `Underscore`.
- A number is an `Int` or a `Float`. A `.` is part of a float only with a digit after it, so `1..10` and `3.14.to_fixed(2)` lex as you'd expect. Right after a `.` token, with no trivia between, a number is a tuple position: a decimal integer with no point or exponent, so `pair.0.1` is `pair` `.` `0` `.` `1`. A minus sign is always its own token. Tokens don't hold values: `literal::int_value` and `literal::float_value` compute them from the text.
- A `"` string is `StringStart` (its `"`), then any number of `StringText` and interpolations, then `StringEnd` (its closing `"`). `StringText` is a run of text with its escapes, which `literal::string_value` decodes, and no escape spans two of them. An interpolation is `InterpStart` (`{`), its path as ordinary `Ident`, `SelfKw`, `Dot` and `Int` tokens, and `InterpEnd` (`}`), so names in strings are real tokens. A `}` alone is text. A string with no closing `"` on its line ends before its line ending, with no `StringEnd`, and the parser accepts it without one.
- Punctuation is the longest match, except that `>` is always a token of its own. The parser forms `>>` and `>=` from a `>` and the token right after it, when no trivia sits between them.
- The lexer doesn't decide which line breaks end a statement. `SyntaxKind::continues_line()` is true for the kinds after which a line always continues: a comma, an opening bracket, `=`, a compound assignment, `=>`, `->`, and every binary operator but `>` and `*`. The parser decides `>`, `*` and the rest.

**Literals not lexed yet.** Multi-line, raw, bytes, tagged and char literals get their own rules in a later layer of the lexer stack. Until then:
- A `"` right after an `Ident`, as in `r"C:\path"`, `re"a{2}"` and `b"\x00"`, and a `"` that starts `"""`, lex from the `"` to the next `"` on the line as `StringStart`, `StringText` and `StringEnd`, with no escapes or interpolation. So the prefix is a separate `Ident`, and `"""` is an empty string and then another, as is `r"""` after its `r`. The `"` rules would report false errors in these forms. A keyword is never a prefix: `test"adds"` is `test` and a checked string. A side effect: in `"say "hi" now"`, an unescaped quote, `" now"` follows the `Ident` `hi`, so it isn't checked either. The parser reports the stray `hi`.
- Everything from a `'` to the next `'` on its line is one `Char`.

## Errors the lexer owns

The lexer reports each lexical problem once, and the parser must not report it again:
- An `Error` token is trivia. A fixture or a fix that adds a bad character puts it where trivia may go.
- A `;` is a `Semicolon` token, reported as E0002. The parser reads it as a line break that ends a statement, so `a; b` is two statements with only the E0002. It ends the statement outright: `a;` followed by a line that starts with `.` is two statements, not a method chain.
- A `BlockComment`, reported as E0003, is a comment to the parser.
- A malformed number, reported as E0008, keeps its `Int` or `Float` kind, glued letters included (`1u8` is one `Int`), so the parser accepts it as a literal.
- An invalid escape, reported as E0006, stays part of its `StringText`.
- An interpolation that isn't a path, reported as E0007, is one `Error` token: up to and including the next `}`, or up to the next `"` or line ending, so the string goes on after it, and `"{"` still ends at its second `"`. Escapes don't count inside the braces. The lexer rejects a path that starts with `Self`, but it can't tell that a name such as `Config` is a type, so name resolution reports a path that starts with one.
- A string with no closing `"`, reported as E0004, has no `StringEnd`.

| Code | Reported for | Fix |
| --- | --- | --- |
| E0001 | A run of characters that start no token, outside literals and comments | For a run of whitespace other than spaces and tabs, replace each with a space (`safe`) |
| E0002 | A `;` | At a line end, delete it: `safe`, or `needs-review` when the next line starts with `.`, which the deletion joins to the statement |
| E0003 | A block comment | When it covers whole lines, rewrite it as `//` lines (`safe`) |
| E0004 | A `"` string with no closing `"` on its line, at its opening `"`. Not when a lone CR ends it, which only E0013 reports | Insert `"` at the line end (`needs-review`), unless the line ends with a `\`, which would escape it |
| E0006 | An invalid escape in a `"` string: unknown (`\q`), `\'`, `\}`, `\x`, a bad `\u{...}` or `\u` with no braces, or a `\` at a line end | For `\'` and `\}`, delete the backslash (`safe`) |
| E0007 | An interpolation that isn't a path: empty, an expression, a format specifier, a number, spaces, a path that starts with `Self`, or a `{` never closed | When the braces hold a path with spaces that each touch a brace or a `.`, delete them (`safe`) |
| E0008 | A malformed number: a misplaced underscore, an uppercase prefix or `E`, a prefix or exponent with no digits, a digit outside its base, a leading zero, or a suffix. Each problem is its own E0008, at most one of each kind per digit group | Lowercase a prefix or `E` (`safe`); remove a suffix (`needs-review`), unless a `.` and a digit follow it, as in `1x.5`, which would become a float |
| E0013 | A CR not followed by LF, also inside a block comment | Replace it with LF (`safe`), unless it ends an unclosed literal |
| E0014 | A line with invalid UTF-8, from `decode_source` | None |

Messages are written for the habits agents bring from other languages: E0001 on `#` says comments start with `//`, on `r#` before a quote, or on `#`s after a closing quote, it says raw text that holds quotes is `r"""`, and on a backtick it says strings use `"` and interpolate with `{name}`. E0006 says what to write where the lexer can tell: `\u{41}` for `\x41` or `\u0041`, `\u{7}` for C's `\a`, `{name}` for Swift's `\(name)`, and `\\` for a backslash. E0007 on `{}` says the braces need a name and that a literal brace is `\{`. On an expression it gives the `let` to write first (`let value = a + b`, then `{value}`), on `{total:.2}` the `to_fixed` call, and on `{0}` that strings have no positional placeholders. `"Total: ${amount}"` gets no warning: the `$` is text.
