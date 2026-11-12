# quince_syntax

The lexer, the parser and its CST now, and the formatter later. This file holds the contracts the spec leaves to the implementation: what a token is, which errors the lexer owns, the shape of the CST and where trivia sits in it, and how the parser recovers from errors. The designs are in [`docs/design/m1-lexer.md`](../../docs/design/m1-lexer.md) and [`docs/design/m1-parser.md`](../../docs/design/m1-parser.md). The language rules are in the spec's [Lexical structure](../../spec/reference/lexical.md) chapter, and aren't restated here.

## Source text

- `decode_source(&[u8])` is the one place source bytes become text. Every caller that reads a file uses it before lexing.
- Each maximal run of invalid UTF-8 becomes one U+FFFD in `Decoded::text`, and `Decoded::replaced` records where. Each line that holds invalid bytes gets one E0014, at its first run.
- When `Decoded::is_lossy()` is true, the text isn't the file, so no tool may write it back. Every writer checks it: `fmt`, `fix` and the language server. For the same reason, `lex` gives no error in a lossy text a fix, and `parse` clears every fix in a lossy text, once, after all its stages, so the rule covers each of them.
- A file over 4 GiB, more than a `u32` offset can address, is an I/O error, not a diagnostic.

## Tokens

- `lex(&Decoded)` lexes decoded text into a `Lexed`, which holds its tokens and errors. `lex_str(&str)` lexes text that never came from bytes.
- A `Token` is a `SyntaxKind` and a byte length. A token starts where the one before it ends.
- Every byte belongs to exactly one token, so the token texts concatenate to the input. No token is empty.
- The lexer never fails and never stops early. An error produces a `SyntaxError` (byte offsets, a code, a message and fixes) and a token, and lexing goes on. `Lexed::diagnostics` turns the errors into `Diagnostic`s through a `LineIndex`.
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
- A char literal is one `Char` token, quotes included, which `literal::char_value` decodes. Its body runs to the next `'` that an escape doesn't take, so `'\''` is one char, and one with no closing `'` on its line ends before its line ending.
- Punctuation is the longest match, except that `>` is always a token of its own. The parser forms `>>` and `>=` from a `>` and the token right after it, when no trivia sits between them.
- The lexer doesn't decide which line breaks end a statement. `SyntaxKind::continues_line()` is true for the kinds after which a line always continues: a comma, an opening bracket, `=`, a compound assignment, `=>`, `->`, and every binary operator but `>` and `*`. The parser decides `>`, `*` and the rest.

## Errors the lexer owns

The lexer reports each lexical problem once, and the parser must not report it again:
- An `Error` token is trivia. A fixture or a fix that adds a bad character puts it where trivia may go.
- A `;` is a `Semicolon` token, reported as E0002. The parser reads it as a line break that ends a statement, so `a; b` is two statements with only the E0002. It ends the statement outright: `a;` followed by a line that starts with `.` is two statements, not a method chain. Where a construct can't end, the E0002 is the only error: the parser reports nothing at a break that holds a `;`, so `fn f() ->; Int {}` is the E0002 alone. That includes the end of the file after a `;`, as in `fn f() ->;`.
- A `BlockComment`, reported as E0003, is a comment to the parser.
- A malformed number, reported as E0008, keeps its `Int` or `Float` kind, glued letters included (`1u8` is one `Int`), so the parser accepts it as a literal.
- An invalid escape, reported as E0006, stays part of its `StringText`, or its `Char`.
- A malformed char literal, reported as E0009, keeps its `Char` kind, so the parser accepts it as a literal.
- An interpolation that isn't a path, reported as E0007, is one `Error` token: up to and including the next `}`, or up to the next `"` or line ending, so the string goes on after it, and `"{"` still ends at its second `"`. Escapes don't count inside the braces. The lexer rejects a path that starts with `Self`, but it can't tell that a name such as `Config` is a type, so name resolution reports a path that starts with one.
- A string, raw string, bytes or tagged literal with no closing `"`, reported as E0004, has no `StringEnd`.
- An unknown tag, reported as E0010, is lexed as a tagged literal, with a raw body, and is that literal's one error: with no closing `"` it gets no E0004. So in `"say "hi" now"`, an unescaped quote, `hi" now"` is one literal with an unknown tag.
- `b"""` or a tag before `"""`, reported as E0010 at the prefix, is lexed as an `r"""` string, so its closing line closes it.
- A non-ASCII character in bytes, reported as E0011, stays part of its `StringText`.
- A control or bidirectional-format character in a literal or comment, reported as E0015, stays part of its token: a `StringText`, a `Char`, a comment, or the `Error` token of a broken interpolation.
- A multi-line string with no closing line, reported as E0005, runs to the end of the file with no `StringEnd`. Each line after the opening one is one `StringText`, with no escapes, interpolation or control characters checked, since it is more likely code than text.
- A string ends at its first `"""` that an escape doesn't take, as in `\"""` (escapes don't count in a raw string). When that `"""` follows text on a line, it is still the string's `StringEnd`, reported as E0012, so the rest of the file is lexed as code. The lines up to it are lexed as in a closed string, dropping the indentation they share.
- Text after an opening `"""`, reported as E0012 at the text without its trailing spaces, or at a second `"""` on that line when there is no text, as in `""""""`, is one `StringText`, with no escapes or control characters checked. A `"""` after it on that line is the string's `StringEnd`, so `"""text"""` can't swallow the file. A line that doesn't start with the closing line's indentation, also E0012, keeps the part of its indentation that matches as its `StringIndent`, and the rest is text.

| Code | Reported for | Fix |
| --- | --- | --- |
| E0001 | A run of characters that start no token, outside literals and comments | For a run of whitespace other than spaces and tabs, replace each with a space (`safe`) |
| E0002 | A `;` | At a line end, delete it: `safe`, or `needs-review` when the next line starts with `.`, which the deletion joins to the statement |
| E0003 | A block comment | When it covers whole lines, rewrite it as `//` lines (`safe`) |
| E0004 | A `"` string, raw string, bytes or tagged literal with no closing `"` on its line, at its `StringStart`. Not when a lone CR ends it, which only E0013 reports, or after an unknown tag | Insert `"` at the line end (`needs-review`), unless a `"` string or bytes literal ends with a `\`, which would escape it |
| E0005 | A multi-line string with no `"""` after its opening quotes, at those quotes | None |
| E0006 | An invalid escape in a `"` or `"""` string: unknown (`\q`), `\'`, `\}`, `\x`, a bad `\u{...}` or `\u` with no braces, or a `\` at a line end. In bytes: unknown, `\'`, `\{`, `\}`, `\x` without two hex digits, any `\u`, or a `\` at a line end. In a char literal: as in a string, but `\'` is valid | For `\'` and `\}`, and `\{` in bytes, delete the backslash (`safe`), unless the escape follows a `\u` with no closing `}`, which the deletion could join it to |
| E0007 | An interpolation that isn't a path: empty, spaces, any other body that isn't a path, or a `{` never closed | When the braces hold a path with spaces that each touch a brace or a `.`, delete them (`safe`) |
| E0008 | A malformed number: a misplaced underscore, an uppercase prefix or `E`, a prefix or exponent with no digits, a digit outside its base, a leading zero, or a suffix. Each problem is its own E0008, at most one of each kind per digit group | Lowercase a prefix or `E` (`safe`); remove a suffix (`needs-review`), unless a `.` and a digit follow it, as in `1x.5`, which would become a float |
| E0009 | A char literal that is empty or holds more than one Unicode scalar value, over the literal, or that has no closing `'` on its line, at its `'`. Not when a lone CR ends it, which only E0013 reports. An escape counts as one, valid or not | For more than one, make it a `"` string with the same value (`needs-review`), unless the body has an invalid escape or a control character, whose own fix comes first, or a name character or a `"` comes right before it, which the new `"` would join |
| E0010 | An identifier glued to a `"` that isn't `r`, `b` or a tag, at the identifier; or `b"""` or a tag before `"""`, at the prefix | None |
| E0011 | A run of non-ASCII characters in a bytes literal, other than replacements from decoding, which E0014 reports. A `\` before one is an E0006 instead | Replace it with the `\x` escapes of its UTF-8 bytes (`safe`) |
| E0012 | A multi-line string layout error: text after the opening `"""`, at the text, or at a second `"""` on that line when there is none; a line that doesn't start with the closing line's indentation, at its indentation, or at the line and its line ending when it has none; or, when no line starts with it, the closing line's indentation, reported once; or `"""` after text on a content line, at the quotes | For quotes after text, move them to a line of their own, indented as the lines share (`needs-review`, since they may have been meant as text). Otherwise none |
| E0013 | A CR not followed by LF, also inside a block comment or a multi-line string | Replace it with LF (`safe`), unless it ends an unclosed literal, or follows another CR, with which an LF would make one CR LF |
| E0014 | A line with invalid UTF-8, from `decode_source` | None |
| E0015 | A run of control characters other than tabs and line endings, or bidirectional-format characters (`r[lex.source.control]`), in a literal or comment. Not inside an invalid escape, which is an E0006, nor a bidirectional character in bytes, which is an E0011, nor in the unchecked text of a multi-line string | In a `"` or `"""` string, a test's name or a char, replace each with its `\u{...}` escape; in bytes, with its `\x` escape (`safe`). None in raw and tagged literals, comments, or an interpolation's braces |

Messages describe Quince, the same way for every input. An error says what Quince expects, such as the escapes a string takes or what a path is, and the lexer never looks for a construct from another language to call it out (decided by David, 2026-11-11). An E0006 in bytes gives the `\x` escapes for a `\u{...}` escape, and E0011 the `\x` escapes of the character, since both are Quince escapes. No message holds a control or bidirectional-format character: it names one by its code point, such as U+0007.

## The CST

The tree is a `rowan` tree, in the `cst` module. `parse(&Decoded)`, or `parse_str` for text that never came from bytes, gives a `Parse`: the root, a `File` node, and every error the lexer and the parser found, in source order. `Parse::diagnostics` turns them into `Diagnostic`s, as `Lexed::diagnostics` does.

- `SyntaxKind` holds the node kinds after the token kinds, and `QuinceLanguage` maps it to `rowan`. A node kind is named after the grammar production it holds, where there is one. `File` is the root, and `Skipped` holds the tokens the parser skipped while recovering from an error.
- The tree is lossless: its text is the input, and its tokens are the lexer's, in order, with their kinds. The exceptions are `GtGt` and `GtEq`, CST-only tokens that each hold a `>` and the token right after it, which the parser joins. No token is empty, and no node is, except the root of an empty file.
- The parser never builds the tree. It emits events: start a node, finish it, take the next significant token, join the next two, and report an error. The tree builder turns them and the lexer's tokens into the tree, and places every token the parser didn't read. Those are the trivia: the kinds for which `SyntaxKind::is_trivia()` is true, and doc comments that document nothing. `is_trivia()` covers the trivia under Tokens, plus `Newline` and `Semicolon`, which the parser reads only as the breaks between the tokens it sees.
- Where trivia goes:
  - Trivia before a node's first token goes outside the node, before it.
  - Trivia after a node's last token goes outside it too. So trivia between two tokens goes in the innermost node that holds both, unless the rule below takes it: a comment at the end of a block's last line goes in the block, before its `}`, and the blank lines between two declarations go in the node around both. Trivia before the first token and after the last goes in the root.
  - The exception: comment lines directly above a node whose kind `attaches_comments()` (a declaration, field, enum variant, member or match arm) go inside the node, with their line endings and the indentation of the node's own line. A comment line starts a line, holds only comments and whitespace, and ends with a line ending. The run stops at a blank line, or at a line with anything else on it, such as code or a `;`. The indentation before the first comment stays outside.
  - For that rule, a comment is a `Comment`, a `BlockComment`, an `Error` token or a doc comment the parser left as trivia. So a comment at the end of a line of code never goes in the node below it. The byte order mark never goes inside a node: it belongs to the file.
  - Comments go in the outermost node that starts at the token below them, and only when that node's kind takes them. The builder writes them as soon as it opens that node, so a node opened inside it at the same token finds none left.

### Node shapes

A node holds its tokens and child nodes in source order. A part the code leaves out, or that an error stopped the parser before, is simply absent: there are no placeholder nodes. The kinds the parser builds so far:

| Kind | Holds |
| --- | --- |
| `File` | The declarations. |
| `FnDecl` | `DocComments` and `Attribute`s if present, as for every declaration of the file, then `pub` if present, `fn`, a `Name`, `Generics` if present, a `ParamList`, a `RetType` and `Effects` if present, and a `Block`. A `uses` after a bare function type in the `RetType` is the declaration's `Effects` (`r[effect.uses-return]`). |
| `ParamList` | `(`, `Param`s and their commas, and `)`. |
| `Param` | `var` if present, then `self`, or a `Name` or `_` followed by `:`, a type, and `=` and an expression for a default. |
| `RetType` | `->` and a type. |
| `TestDecl` | `test`, its name as a `Literal`, and a `Block`. |
| `ConstDecl` | `pub` if present, `const`, a `Name`, `:`, a type, `=` and an expression. |
| `ImplDecl` | `impl`, a type, `{`, a `FnDecl` per method, and `}`. |
| `Import` | `import`, the package path's names and the `.` tokens between them, then `.` and `ImportNames`, or `as` and a `Name`, if present. |
| `ImportNames` | `{`, a `NameRef` per name and their commas, and `}`. |
| `StructDecl` | `DocComments` and `Attribute`s if present, `pub` if present, `struct`, a `Name`, `Generics`, then a `FieldList` and `requires` and an expression if present. |
| `FieldList` | `{`, `FieldDecl`s and their commas, and `}`. |
| `FieldDecl` | `DocComments` and `Attribute`s if present, `pub` if present, a `Name`, `:` and a type. |
| `NewtypeDecl` | Its declaration's start, as for `StructDecl`, then `(`, `pub` if present, a type, and `)`. |
| `EnumDecl` | Its declaration's start, as for `StructDecl`, with `enum`, then a `VariantList`. |
| `VariantList` | `{`, `Variant`s and their commas, and `}`. |
| `Variant` | `DocComments` and `Attribute`s if present, a `Name`, and `VariantFields` if present. |
| `VariantFields` | `(`, `FieldDecl`s and their commas, and `)`. |
| `AliasDecl` | Its declaration's start, as for `StructDecl`, with `type`, then `=` and a type. |
| `Attribute` | `@`, a `NameRef`, and an `ArgList` if present. |
| `DocComments` | The `DocComment` tokens before a declaration, a field or a variant, and the trivia between them. |
| `TypePath` | Its names and the `.` tokens between them, whatever their case, then its `TypeArgs` if present. An effect in a `uses` list and a bound are `TypePath`s too. |
| `TypeArgs` | `<`, types and their commas, and `>`. The last item may be `Effects`, and in a bound an item may be a `NamedTypeArg`. |
| `NamedTypeArg` | A `NameRef`, `=` and a type: `Item = Int` in a bound. |
| `TupleType` | `(`, two or more types and their commas, and `)`, or just `(` and `)` for the unit type. |
| `ParenType` | `(`, one type and `)`. The type is a `FnType` unless the node holds an error. |
| `FnType` | `fn`, `(`, the parameter types and their commas, `)`, then a `RetType` and `Effects` if present. |
| `Effects` | `uses`, then `*` or `TypePath`s and their commas. |
| `Generics` | `<`, `GenericParam`s and their commas, and `>`. |
| `GenericParam` | A `Name`, then `:` and bounds joined by `+` if present, or `uses` and a `Name`. |
| `Block` | `{`, its statements, and `}`. An expression statement is the expression's own node. |
| `LetStmt` | `let`, a `Name`, `:` and a type if present, `=` and an expression. |
| `Name` | The `Ident` of a name being declared. |
| `NameRef` | The `Ident`, `self` or `Self` of a name being used. |
| `Literal` | One `Int`, `Float`, `Char`, `true` or `false`, or a whole string: its `StringStart`, text, `Interp`s and `StringEnd`, which an unclosed string lacks. |
| `Interp` | An interpolation's `InterpStart`, the tokens of its path, and its `InterpEnd`. |
| `ListLit` | `[`, expressions and their commas, and `]`. |
| `Range` | An expression, `..` or `..=`, and an expression. |
| `Binary` | An expression, an operator other than a range's, and an expression. `>>` and `>=` are one token each. |
| `Prefix` | `-`, `!` or `~`, and an expression. |
| `Paren` | `(`, an expression, and `)`. |
| `Call` | The callee, any expression, and an `ArgList`. A method call is a `Call` whose callee is a `Field`, and a struct literal or a variant is a `Call` too, since only name resolution can tell them apart. |
| `ArgList` | `(`, expressions and their commas, and `)`. |
| `Field` | An expression, `.`, and a `NameRef` or a tuple position's `Int`. |
| `Skipped` | The tokens the parser skipped after an error, a bracket and what it holds included. |

Postfix forms nest to the left: `a.b(c)` is a `Call` of a `Field` of a `NameRef`. The parser opens such a node after its left side is parsed. The parser's own steps say which node a later one wraps, as rust-analyzer's `forward_parent` does, and one pass turns them into the builder's events, so the builder never sees a node opened out of order.

## Parsing

The parser reads the significant tokens: every token but the trivia and the doc comments. Each records the break before it: none, a line break, or a hard break, when a `;` comes between it and the token before. Nothing comes before the first token, so it has no break.

- **Doc comments** are taken by the declaration, field or variant whose first token they come before, into its `DocComments` node, and so become part of the tree's significant tokens. Any other doc comment stays trivia: one in a block, after an attribute, before an `import`, at the end of the file, or before an `impl` member, which takes its doc comments in layer 7.
- **Attributes** come after the doc comments and before `pub`. Each goes on a line of its own, and the next line that isn't blank continues what they apply to. Something after an attribute on its line is an E0016. When it can start what the attribute applies to, the fix moves it to the next line. A keyword after `@` is an E0016 at the keyword, and the declaration after it is still parsed.

- **Where a break ends a construct.** In a line context (the file, a block, an `impl` body), a break ends the statement or declaration unless the token before it continues the line (`SyntaxKind::continues_line()`), the next token is `.`, or the parser is in a `fn` header and the next token is `->`, `uses`, `requires`, `ensures` or `{`. A hard break always ends it. Inside brackets, a break before a `,` or a closing bracket is spacing, but an expression in brackets still ends at a break, as a statement does.
- **Errors.** Every syntax error is reported once, and then the parser skips to a sync point, reporting nothing more until it next takes a token as part of a construct. The skipped tokens go in a `Skipped` node, and a bracket the skip meets is skipped up to its closing bracket. In a list, a `<` counts as a bracket, closed by `>`, so a skip there stays out of type arguments.
  - A list syncs at its next `,` and at its closing bracket. Type arguments and generics are lists closed by `>`, so their errors stay inside them, and a closing bracket further out closes them with an E0017 at the `<`. A line context syncs at its next break, which ends the member that went wrong. An `impl` header syncs at the `{` on its line, so its body is still parsed as its body.
  - A token at the start of a line that can only start a member of some open container closes every construct inside the innermost such container, with an E0017 at each opening bracket it closes. For the file those tokens are `fn` and a name, `struct`, `enum`, `type`, `interface`, `impl`, `effect`, `foreign`, `import`, `const` and `test`, each also after `pub`. For an `impl` body they are `fn` and a name, `type` and `const`. For struct braces, enum braces and a variant's fields they are `pub` and a name. For a block they are `let`, `var` and `while`. The check looks past attributes to the token after them, so a member start after an attribute line closes what it closes, with no error at the `@`.
  - A closing bracket that matches a bracket further out closes everything inside it, with an E0017 at each opener. One that matches nothing open closes the innermost list in its place, with one E0016 that names the bracket expected, and in a line context it is skipped as an E0016. The end of the file closes everything still open, with an E0017 at each opener.
  - A bracket closed while the parser is skipping after an error gets no E0017, since that error covers the slip.
  - After a bracket closes without its closing bracket, what the constructs around it lack is part of that slip, until the parser next takes a token. A line break there ends them, whatever token comes before it.
  - An E0016 after a list item, at an item that starts the next line, has a fix that inserts the `,` (`needs-review`).
- **Operators.** The parser accepts any mix of binary operators and builds the tree of the usual reading, then reports what the spec forbids. The usual reading puts the operators with no level where the spec's examples do, so the order, tightest first, is postfix, prefix, `*` `/` `%`, `+` `-`, `<<` `>>`, `&`, `^`, `|`, `..` `..=`, comparisons, `&&`, `||`. Every level groups to the left, comparisons and ranges included, and their chains are E0020s.
  - A `>` and a `>` or `=` right after it, with no trivia between, are one operator, `>>` or `>=`, which the tree holds as one `GtGt` or `GtEq` token. A closing `>` of type arguments is never joined.
  - After a binary operator a line break is spacing, `*` and `>` included, which `continues_line()` leaves to the parser. After `-` it is spacing too, since `-` can be binary, but after `!` or `~` it ends the line.
  - E0019 is reported at each operand that needs parentheses, and E0020 at the second operator of a chain, once per chain. Neither is reported for an operator any of whose operands holds a syntax error, a missing operand included, since that error is the slip.
- **Errors the parser leaves to the lexer.** Beyond the lexer's own list (above):
  - The parser drops an error it would report right after an `Error` token, with only trivia between, so `let x = $` is the E0001 alone.
  - An unclosed one-line string or char closes, at its line's end, the parentheses and square brackets opened on that line, with no E0017, and the parser reports nothing on that line. A block's `{` stays open.
  - When a multi-line string or a block comment runs unclosed to the end of the file, the parser reports nothing at the end of the file and gives no E0017 for what the end closes.
- **Loop safety.** Every parser loop takes a token, closes a construct or exits. Debug and fuzz builds panic when the parser looks at the tokens 256 times without taking one, closing a construct or completing a node.

| Code | Reported for | Fix |
| --- | --- | --- |
| E0016 | A token that can't go here: "expected X, found Y", where Y can be a line break or the end of the file. A keyword where a name goes is one, which the message names | Insert the missing `,` between list items on separate lines (`needs-review`). Remove the parentheses around one type that isn't a function type (`safe`). Move what follows an attribute on its line to the next line (`safe`). Otherwise none |
| E0017 | An opening bracket with no closing one, at the opener | None |
| E0019 | An operand that needs parentheses: `&&` mixed with `||`, a bit operator or a shift as an operand of another operator or with an arithmetic operand, `&`, `|` or `^` mixed, a shift of a shift, or a range as an operand | Add the parentheses of the usual reading (`needs-review`) |
| E0020 | A chained comparison or range, at its second operator | For comparisons, repeat each middle operand: `a < b && b < c` (`needs-review`), in parentheses when the chain is an operand of `\|\|`. None when a middle operand holds a comment or a line break or needs parentheses itself, and none for ranges |
| E0024 | A struct with no fields written with braces, at the braces | Remove them (`safe`), unless a `requires` follows or a comment lies before or between them |
