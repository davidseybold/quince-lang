# quince_syntax

The lexer now, and the parser, CST and formatter later. This file holds the contracts the spec leaves to the implementation, starting with what a token is. The design is in [`docs/design/m1-lexer.md`](../../docs/design/m1-lexer.md). The language rules are in the spec's [Lexical structure](../../spec/reference/lexical.md) chapter, and aren't restated here.

## Tokens

- `lex_str(&str)` lexes a text into a `Lexed`, which holds its tokens.
- A `Token` is a `SyntaxKind` and a byte length. A token starts where the one before it ends.
- Every byte belongs to exactly one token, so the token texts concatenate to the input. No token is empty.
- The lexer never fails and never stops early: text it can't use becomes a token too.
- Trivia, which the parser skips:
  - `Whitespace`: a run of spaces and tabs.
  - `Comment`: `//` up to its line ending.
  - `BlockComment`: `/*` to the next `*/`, or to the end of the file when there is none.
  - `Bom`: a byte order mark at the very start.
  - `Error`: a run of characters that start no token.
- `Newline` is one line ending: LF, CR LF, or a lone CR. A lone CR ends a line here as it does in `LineIndex`, so the parser's line ends agree with the line numbers in diagnostics. Each line ending is its own token.
- `DocComment` is `///` up to the line ending, but not `////`. Whether the parser treats it as trivia is the parser's call.
- An identifier is `Ident` whatever its case. Each keyword has its own kind (`FnKw`, `SelfTypeKw` for `Self`). Nothing else is special: `main`, `TaskScope` and the tag names are identifiers. `_` alone is `Underscore`.
- A `;` is a `Semicolon` token. The parser reads it as a line break that ends a statement.
- Punctuation is the longest match, except that `>` is always a token of its own. The parser forms `>>` and `>=` from a `>` and the token right after it, when no trivia sits between them.
- The lexer doesn't decide which line breaks end a statement. `SyntaxKind::continues_line()` is true for the kinds after which a line always continues: a comma, an opening bracket, `=`, a compound assignment, `=>`, `->`, and every binary operator but `>` and `*`. The parser decides `>`, `*` and the rest.

**Literals not lexed yet.** Strings, chars and numbers get their own rules in later layers of the lexer stack. Until then:
- Everything from a `"` to the next `"` on its line is `StringStart`, `StringText` and `StringEnd`, with no escapes or interpolation. A string with no closing `"` on its line has no `StringEnd`, and the parser accepts it without one. A prefix such as `re` is a separate `Ident`.
- Everything from a `'` to the next `'` on its line is one `Char`.
- A digit and the letters, digits and underscores after it are one `Int`.
