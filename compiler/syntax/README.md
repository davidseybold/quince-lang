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
- A `"` string is `StringStart` (its `"`), then any number of `StringText` and interpolations, then `StringEnd` (its closing `"`). `StringText` is a run of text with its escapes, which `literal::string_value` decodes, and no escape spans two of them. An interpolation is `InterpStart` (`{`), its path as ordinary `Ident`, `SelfKw`, `Dot` and `Int` tokens, and `InterpEnd` (`}`), so names in strings are real tokens. A `}` alone is text. A test's name, the string after `test` and any spaces, doesn't interpolate (`r[test.name]`), so its `{` is text too, but its escapes are checked (`r[test.name-escapes]`). A string with no closing `"` on its line ends before its line ending, with no `StringEnd`, and the parser accepts it without one.
- A multi-line string, `"""` or `r"""`, has the same shape, and its `StringStart` and `StringEnd` are `"""`, or `r"""` for the raw form's start. Any other prefix before `"""` is an error (E0010, below), and its string is lexed as an `r"""` one, so its `StringStart` is that prefix and `"""`. Each line ending in it is a `StringNewline`. A `StringIndent` is spaces and tabs that aren't part of the value: those after the opening `"""`, the closing line's indentation at the start of each line and before the closing `"""`, and all of a line that holds only whitespace. A `"""` line takes a `"` string's escapes and interpolation, and a test's name doesn't interpolate in this form either. A raw line is one `StringText`. The value is the tokens between the string's first `StringNewline` and its last: each `StringText` decoded, unless the string is raw, each `StringNewline` between them a line feed, and each `StringIndent` nothing. The formatter re-indents a multi-line string by rewriting its `StringIndent`s, which leaves the value as it is.
- An identifier glued to a `"` is a literal's prefix, and the literal's `StringStart` holds both: `r"`, `b"`, or a tag such as `re"`. A keyword is never a prefix, so `test"adds"` is `test` and a string, and `_` isn't one either. A raw string or a tagged literal is `StringStart`, at most one `StringText` holding its raw body up to the next `"` on the line, and `StringEnd`. A bytes literal has the same shape, and its `StringText` holds its escapes, which `literal::bytes_value` decodes. None of them interpolate or span lines, and one with no closing `"` ends before its line ending, as a `"` string does. `literal::TAGS` lists the tags.
- Punctuation is the longest match, except that `>` is always a token of its own. The parser forms `>>` and `>=` from a `>` and the token right after it, when no trivia sits between them.
- The lexer doesn't decide which line breaks end a statement. `SyntaxKind::continues_line()` is true for the kinds after which a line always continues: a comma, an opening bracket, `=`, a compound assignment, `=>`, `->`, and every binary operator but `>` and `*`. The parser decides `>`, `*` and the rest.

**Literals not lexed yet.** Char literals get their own rules in the next layer of the lexer stack. Until then, everything from a `'` to the next `'` on its line is one `Char`.

## Errors the lexer owns

The lexer reports each lexical problem once, and the parser must not report it again:
- An `Error` token is trivia. A fixture or a fix that adds a bad character puts it where trivia may go.
- A `;` is a `Semicolon` token, reported as E0002. The parser reads it as a line break that ends a statement, so `a; b` is two statements with only the E0002. It ends the statement outright: `a;` followed by a line that starts with `.` is two statements, not a method chain.
- A `BlockComment`, reported as E0003, is a comment to the parser.
- A malformed number, reported as E0008, keeps its `Int` or `Float` kind, glued letters included (`1u8` is one `Int`), so the parser accepts it as a literal.
- An invalid escape, reported as E0006, stays part of its `StringText`.
- An interpolation that isn't a path, reported as E0007, is one `Error` token: up to and including the next `}`, or up to the next `"` or line ending, so the string goes on after it, and `"{"` still ends at its second `"`. Escapes don't count inside the braces. The lexer rejects a path that starts with `Self`, but it can't tell that a name such as `Config` is a type, so name resolution reports a path that starts with one.
- A string, raw string, bytes or tagged literal with no closing `"`, reported as E0004, has no `StringEnd`.
- An unknown tag, reported as E0010, is lexed as a tagged literal, with a raw body, and is that literal's one error: with no closing `"` it gets no E0004. So in `"say "hi" now"`, an unescaped quote, `hi" now"` is one literal with an unknown tag.
- `b"""` or a tag before `"""`, reported as E0010 at the prefix, is lexed as an `r"""` string, so its closing line closes it.
- A non-ASCII character in bytes, reported as E0011, stays part of its `StringText`.
- A multi-line string with no closing line, reported as E0005, runs to the end of the file with no `StringEnd`. Each line after the opening one is one `StringText`, with no escapes or interpolation checked, since it is more likely code than text.
- A string ends at its first `"""` that an escape doesn't take, as in `\"""` (escapes don't count in a raw string). When that `"""` follows text on a line, it is still the string's `StringEnd`, reported as E0012, so the rest of the file is lexed as code. The lines up to it are lexed as in a closed string, dropping the indentation they share.
- Text after an opening `"""`, reported as E0012 at the text without its trailing spaces, or at a second `"""` on that line when there is no text, as in `""""""`, is one `StringText`, with no escapes checked. A `"""` after it on that line is the string's `StringEnd`, so `"""text"""` can't swallow the file. A line that doesn't start with the closing line's indentation, also E0012, keeps the part of its indentation that matches as its `StringIndent`, and the rest is text.

| Code | Reported for | Fix |
| --- | --- | --- |
| E0001 | A run of characters that start no token, outside literals and comments | For a run of whitespace other than spaces and tabs, replace each with a space (`safe`) |
| E0002 | A `;` | At a line end, delete it: `safe`, or `needs-review` when the next line starts with `.`, which the deletion joins to the statement |
| E0003 | A block comment | When it covers whole lines, rewrite it as `//` lines (`safe`) |
| E0004 | A `"` string, raw string, bytes or tagged literal with no closing `"` on its line, at its `StringStart`. Not when a lone CR ends it, which only E0013 reports, or after an unknown tag | Insert `"` at the line end (`needs-review`), unless a `"` string or bytes literal ends with a `\`, which would escape it |
| E0005 | A multi-line string with no `"""` after its opening quotes, at those quotes | None |
| E0006 | An invalid escape in a `"` or `"""` string: unknown (`\q`), `\'`, `\}`, `\x`, a bad `\u{...}` or `\u` with no braces, or a `\` at a line end. In bytes: unknown, `\'`, `\{`, `\}`, `\x` without two hex digits, any `\u`, or a `\` at a line end | For `\'` and `\}`, and `\{` in bytes, delete the backslash (`safe`), unless the escape follows a `\u` with no closing `}`, which the deletion could join it to |
| E0007 | An interpolation that isn't a path: empty, spaces, any other body that isn't a path, or a `{` never closed | When the braces hold a path with spaces that each touch a brace or a `.`, delete them (`safe`) |
| E0008 | A malformed number: a misplaced underscore, an uppercase prefix or `E`, a prefix or exponent with no digits, a digit outside its base, a leading zero, or a suffix. Each problem is its own E0008, at most one of each kind per digit group | Lowercase a prefix or `E` (`safe`); remove a suffix (`needs-review`), unless a `.` and a digit follow it, as in `1x.5`, which would become a float |
| E0010 | An identifier glued to a `"` that isn't `r`, `b` or a tag, at the identifier; or `b"""` or a tag before `"""`, at the prefix | None |
| E0011 | A run of non-ASCII characters in a bytes literal, other than replacements from decoding, which E0014 reports. A `\` before one is an E0006 instead | Replace it with the `\x` escapes of its UTF-8 bytes (`safe`) |
| E0012 | A multi-line string layout error: text after the opening `"""`, at the text, or at a second `"""` on that line when there is none; a line that doesn't start with the closing line's indentation, at its indentation, or at the line and its line ending when it has none; or, when no line starts with it, the closing line's indentation, reported once; or `"""` after text on a content line, at the quotes | For quotes after text, move them to a line of their own, indented as the lines share (`needs-review`, since they may have been meant as text). Otherwise none |
| E0013 | A CR not followed by LF, also inside a block comment or a multi-line string | Replace it with LF (`safe`), unless it ends an unclosed literal, or follows another CR, with which an LF would make one CR LF |
| E0014 | A line with invalid UTF-8, from `decode_source` | None |

Messages describe Quince, the same way for every input. An error says what Quince expects, such as the escapes a string takes or what a path is, and the lexer never looks for a construct from another language to call it out (decided by David, 2026-11-11). An E0006 in bytes gives the `\x` escapes for a `\u{...}` escape, and E0011 the `\x` escapes of the character, since both are Quince escapes.
