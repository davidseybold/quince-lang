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
- Punctuation is the longest match, except that `>` is always a token of its own. The parser forms `>>` and `>=` from a `>` and the token right after it, when no trivia sits between them.
- The lexer doesn't decide which line breaks end a statement. `SyntaxKind::continues_line()` is true for the kinds after which a line always continues: a comma, an opening bracket, `=`, a compound assignment, `=>`, `->`, and every binary operator but `>` and `*`. The parser decides `>`, `*` and the rest.

**Literals not lexed yet.** Strings, chars and numbers get their own rules in later layers of the lexer stack. Until then:
- Everything from a `"` to the next `"` on its line is `StringStart`, `StringText` and `StringEnd`, with no escapes or interpolation. A string with no closing `"` on its line has no `StringEnd`, and the parser accepts it without one. A prefix such as `re` is a separate `Ident`.
- Everything from a `'` to the next `'` on its line is one `Char`.
- A digit and the letters, digits and underscores after it are one `Int`.

## Errors the lexer owns

The lexer reports each lexical problem once, and the parser must not report it again:
- An `Error` token is trivia. A fixture or a fix that adds a bad character puts it where trivia may go.
- A `;` is a `Semicolon` token, reported as E0002. The parser reads it as a line break that ends a statement, so `a; b` is two statements with only the E0002.
- A `BlockComment`, reported as E0003, is a comment to the parser.

| Code | Reported for | Fix |
| --- | --- | --- |
| E0001 | A run of characters that start no token, outside literals and comments | For a run of whitespace other than spaces and tabs, replace each with a space (`safe`) |
| E0002 | A `;` | At a line end, delete it (`safe`) |
| E0003 | A block comment | When it covers whole lines, rewrite it as `//` lines (`safe`) |
| E0013 | A CR not followed by LF, also inside a block comment | Replace it with LF (`safe`), unless it ends an unclosed literal |
| E0014 | A line with invalid UTF-8, from `decode_source` | None |

Messages are written for the habits agents bring from other languages: E0001 on `#` says comments start with `//`, after `r` it says raw text that holds quotes is `r"""`, and on a backtick it says strings use `"` and interpolate with `{name}`.
