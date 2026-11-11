# M1: Lexer

Status: approved. Passed design review (three passes). David decided the spec questions on 2026-11-11: strict on question 1, the rest as proposed. The first design doc of M1 (Syntax). A parser doc and a formatter doc follow it.

## Why

M1 ends with every code block in the spec parsing, `quince fmt` idempotent, and a lossless CST that prints back the input byte for byte ([milestones](../milestones.md)). Every one of those needs a token stream first, and so does every later stage that reads source. The lexer is also where most of the spec's [Lexical structure](../../spec/reference/lexical.md) chapter is enforced, and where the first diagnostic codes and the first conformance backend arrive.

Getting the token contract right now matters more than the code, which is small. The parser, the formatter, `fix`, `query` and the language server all read tokens. Changing what a token is later means changing all of them, and every fixture and CST test that depends on token boundaries.

## Impact and approvals

- **Code:** `compiler/syntax` (`quince_syntax`) gets its first modules and a `README.md` holding the token contract. `quince_diagnostics` gets a line index. `quince_conformance` gets its first backend.
- **Crate dependencies:** `quince_syntax` gains `quince_diagnostics` (and through it the approved `serde` and `serde_json`), and `quince_spec` as a dev-dependency. `quince_conformance`, and so xtask, gain `quince_syntax`. M0 listed xtask's dependencies as `quince_spec`, `quince_diagnostics`, `quince_conformance` and `serde_json` ([M0 decision 2](m0-repo-harness.md#2-cargo-xtask-ci)). Decision 10 says why the change is fine.
- **Spec (approved by David, 2026-11-11):** the chapter leaves several things open that a lexer must decide. Spec questions, below, gives the approved wording for each. Each rule lands in the layer whose code enforces it, and each such layer bumps the spec's minor version.
- **Diagnostic codes:** up to fifteen, the first in the registry (decision 9). A code whose rule waits on a spec question is registered only after approval.
- **Dependencies:** none new. No `logos`, no `rowan`. Whether the CST uses `rowan` is the parser doc's question.
- **Delivery:** a stack of seven PRs (decision 12).

## Context

- `quince_syntax` is an empty crate with a one-line doc comment. It depends on nothing and must stay free of LLVM.
- The Lexical structure chapter has 42 rules. 24 of them describe lexing (Tasks lists them), and the lexer's fixtures can test 17 of those on their own (decision 11). The rest belong to the parser (line continuation, `else` placement, negative literals, collection literals) or to type checking (literal types).
- `spec/grammar.ebnf` quotes 42 non-word terminals (punctuation and `_`), 36 of the 38 keywords (not `true` or `false`, which it leaves to `literal`), and `"TaskScope"`, which isn't a keyword. Its prose terminals are `name`, `Name`, `NAME`, `c_name`, `path`, `literal`, `string`, `int`, `doc_comment` and `NEWLINE`.
- `quince_diagnostics` fixes how positions count (`compiler/diagnostics/README.md`): `[line, column]`, 1-based, columns in Unicode scalar values, lines ending at LF, CR LF or a lone CR, a leading BOM not counted. It says which line endings the lexer accepts is the spec's business. The spec doesn't say.
- The conformance runner has a `Backend` trait with `parse`, `check` and `run`, taking `&str`, and ships `NoBackend`. Its fixture reader splits lines with `str::lines()`, which doesn't split at a lone CR, and finds header comments with `trim_start()`, which doesn't skip a BOM. The README requires `check` fixtures to be valid programs apart from their expectations, and says nothing of the kind for `parse` fixtures (`tests/conformance/testdata/README.md`).
- `Fix::check` can't yet check that a fix's positions exist in the source. M0 left that for M1's backend.
- The spec and `std/API.md` hold 66 code blocks with no info string. Most are Quince. A few are TOML, JSON or REPL transcripts (`>>> 3.14159.to_fixed(2)`). Between them they hold two interpolations, one tagged literal and no multi-line strings, so they say little about literals.

## Goals and non-goals

**Goals:**
- A lexer that turns any source into a token stream whose texts concatenate to the input exactly, never panics, and reports every lexical error with an exact span and, where one is clear, a fix.
- A token contract the parser and formatter can build on without re-scanning text, documented in `compiler/syntax/README.md`, including what the parser does with the lexer's error tokens.
- The first diagnostic codes, each with explain text and a conformance fixture, and the first wired conformance backend.

**Non-goals:**
- Deciding which line breaks end a statement. The parser decides that (decision 4).
- Naming case (`lex.name.*`): what counts as `snake_case`, `PascalCase` or `SCREAMING_SNAKE_CASE`, and the checks. The parser doc takes it, with the questions in "For the parser doc".
- Keywords used as names, and C names. They need parse context.
- Checking a tagged literal's body (`url"..."` is a valid URL, `dur"30s"` a valid duration). That needs the literal's type, so type checking does it.
- Literal types and range checks (`let b: UInt8 = 300`). M3.
- Incremental relexing for the language server. Lexing a whole file is fast enough until M5 shows otherwise.
- A `quince` command that prints tokens. The parser doc may add a CST dump, which covers it.

## Decisions

### 1. Where it lives

The lexer is the `lexer` module of `quince_syntax`, with its literal handling in submodules. Literal values (decoded strings, integer and float values) are computed on demand by a `literal` module from token text, not stored in tokens.

*Alternative:* a separate `quince_lexer` crate. Rejected: nothing but the parser and formatter use tokens directly, they live in the same crate, and the repo plan puts the lexer there. Splitting later is cheap if a second consumer appears.

### 2. Lossless tokens

`lex(&Decoded) -> Lexed`, or `lex_str(&str)` for text that never came from bytes (decision 9), where `Lexed` holds a `Vec<Token>` and a `Vec<LexError>`.
- A `Token` is a `SyntaxKind` and a byte length (`u32`). Start offsets are the running sum, so a token is 8 bytes.
- Every byte of the input belongs to exactly one token, so concatenating the token texts gives back the input. Comments, spaces and line breaks are tokens too (trivia). No token is empty.
- The lexer never fails and never stops early. An error produces a diagnostic and a token, and lexing goes on.
- `SyntaxKind` is one `u16` enum in `quince_syntax::kind`. The parser adds its node kinds to the same enum, as rowan-style CSTs need, but nothing here requires rowan.

**The error contract.** The lexer reports each lexical problem once, and the parser must not report it again. So the README gives the parser these rules:
- An `Error` token (decision 9: characters that start no token, a broken interpolation) is trivia to the parser.
- A `;` is a `Semicolon` token. The parser treats it as a statement-ending line break, so `a; b` parses as two statements with only the lexer's E0002.
- A block comment is a `BlockComment` token, which the parser treats as a comment (trivia).
- A malformed literal keeps its literal kind (decisions 7 and 8). The parser accepts it as that literal.
- A string literal with no closing delimiter has no `StringEnd` token. The parser accepts a string that ends without one, since the lexer already reported it.

*Alternative:* tokens without trivia, plus a side table of comments. Rejected: the formatter, `fix` and error recovery all need trivia in place, and the repo plan already chose a lossless CST.

### 3. Trivia kinds

- `Whitespace`: a run of spaces and tabs.
- `Newline`: one line ending: LF, CR LF, or a lone CR. A lone CR is also reported as E0013 (spec question 1), but it ends a line here as it does in `LineIndex`, so the parser's line ends always agree with reported line numbers. Each line ending is its own token, so the formatter can count blank lines and the parser can find line ends without scanning text. Line breaks inside a multi-line string are never `Newline` (decision 8).
- `Comment`: `//` up to, not including, the line ending.
- `DocComment`: `///` up to the line ending. It's a separate kind because the grammar names `doc_comment` as a terminal. Whether the parser treats it as trivia is the parser's call.
- `BlockComment`: `/*` to the next `*/`, or to the end of the file when there is none, with E0003.
- `Bom`: a byte order mark at the very start (spec question 1).

### 4. Line breaks are tokens, and the parser decides what they mean

The lexer doesn't insert statement terminators, as Go's lexer does. It emits every line ending as a `Newline` token, and the parser decides whether one ends a statement.

The rules can't be applied from tokens alone:
- `r[lex.line-end]` continues a line that ends with a binary operator. A trailing `>` is greater-than in `a >` but closes type arguments in `fn get() -> Option<Int>` in an interface, where the line must end. Only the parser knows which.
- `r[lex.continue.signature]` applies only inside a `fn` signature.
- `r[lex.else-same-line]` needs to know the `else` follows a block, to give its fix.

So the lexer offers only a helper for the parser: `SyntaxKind::continues_line()`, true for exactly the kinds that continue a line whatever their role:
- `,`, `(`, `[`, `{` (`r[lex.line-end]`);
- `=`, `+=`, `-=`, `*=`, `/=`, `%=`, `=>`, `->` (`r[lex.continue.more]`);
- the binary operators other than `>` and `*`: `+`, `-`, `/`, `%`, `&`, `|`, `^`, `<<`, `&&`, `||`, `==`, `!=`, `<`, `<=`, `..`, `..=` and `is`. `-`, `|` and `<` also have other roles (prefix minus, pattern alternation, opening type arguments), but each of those continues the line too.

`>` and `*` are the two kinds that depend on context: `>` closes type arguments, and `*` is the effect set in `uses *`. The parser decides those, including the second `>` of a `>>` shift. A unit test checks the list against the binary operators in `spec/grammar.ebnf`. Comments are trivia here too: a line ends with its last token before any comment.

*Alternative:* Go-style terminator insertion in the lexer. Rejected for the reasons above: it would need a lexer that tracks generics and signatures, which is a parser.

### 5. Punctuation, and `>` always alone

The lexer takes the longest match among the grammar's punctuation terminals, with one exception: **`>` is always a token of its own.** The parser forms `>>` (shift) and `>=` from a `>` token and the next token when no trivia sits between them, as rust-analyzer joins tokens. So `a > = b` is an error, just as `a = = b` is.
- `r[lex.no-shift-token]` already requires this for `>>`, so `List<List<Int>>` closes without a space.
- Doing the same for `>=` means `let x: Option<Int>= y` doesn't lex as `Option<Int` followed by `>=`. One mechanism covers both.
- `..=` is one token (`r[lex.grammar-rules]`), and so are `->`, `=>`, `==`, `!=`, `<=`, `<<`, `&&`, `||` and the compound assignments.
- `_` alone is `Underscore`. `_` followed by a letter, digit or `_` is an identifier.

*Alternative:* lex `>=` and `>>` greedily and have the parser split them where it expects `>`, as rustc does. Rejected: the spec forbids a `>>` token outright, and splitting is a second mechanism for the same problem.

### 6. Identifiers and keywords

- An identifier is an ASCII letter or `_`, then ASCII letters, digits and `_` (spec question 1). It's one `Ident` kind whatever its case. Which case class an identifier belongs to is the parser doc's question.
- The 38 keywords of `r[lex.keywords]` each have their own kind (`FnKw`, `SelfTypeKw` for `Self`, and so on). There are no contextual keywords (`r[lex.no-contextual]`), so the lexer has no context to track: `main`, `limit`, `TaskScope` and the tag names lex as identifiers everywhere.
- `c_name` lexes as an identifier too. Whether a C name may be a keyword is a question for the parser doc.

A test reads the keyword list from `spec/reference/lexical.md` through `quince_spec` and fails if it differs from the lexer's table. A second test lexes every quoted terminal in `spec/grammar.ebnf` and checks each is one token, except `>>` and `>=`, which must be two `>`-led tokens, and `TaskScope`, which must be an `Ident`. So the spec and the lexer can't drift apart silently.

### 7. Numbers

- `Int` covers `42`, `1_000_000`, `0xFF`, `0b1010` and `0o17`. `Float` covers `3.14`, `1e-9` and `2.5e10`. The exact forms are spec question 4.
- A `.` belongs to a float only when a digit follows it. So `1..10` is `1`, `..`, `10`, and `3.14159.to_fixed(2)` is a float then `.to_fixed`. An `e` after digits starts an exponent.
- **After a `.` token with no trivia between, only digits are lexed.** So `pair.0.1` is `pair` `.` `0` `.` `1`, not `pair` `.` `0.1`, and the grammar's `"." int` works for nested tuples. Apart from E0001's raw-string hint, which reads the token before a `#`, and two string rules in decision 8, this is the only place the lexer looks back, and only one token. Those string rules are a `"` glued to an identifier, one token back, and a test's name, which looks back past spaces to `test` (decided by David, 2026-11-11). Layer 4 reads "only digits" as a decimal integer with no point, exponent or base prefix: letters glued to it are still a suffix (`t.0u8` is one malformed `Int`), and a leading zero is still an error (`t.01`).
- A minus sign is always its own token. `r[lex.lit.int.neg]` joins it to the literal in prefix position, which the parser knows and the lexer doesn't.
- A malformed number (`0b102`, `1__0`, `1_`, `0x`, `1e`, `007`) keeps its `Int` or `Float` kind and gets an E0008, so the parser sees a literal and reports nothing more.
- Letters glued to a number (`1u8`, `10px`, `1.5f32`) stay part of it, as a malformed literal of the kind its digits give. The message says literal types come from context (`let b: UInt8 = 1`), and the fix removes the suffix (`needs-review`).
- The lexer doesn't compute values. `literal::int_value` returns a `u128`, or `None` when it doesn't fit, and `literal::float_value` an `f64`, for the type checker.

### 8. String and char literals

Every `"`-delimited literal has one shape:

```
StringStart  ( StringText | StringIndent | StringNewline | Interp... )*  [ StringEnd ]
```

- `StringStart` is the opening delimiter with its prefix: `"`, `"""`, `r"`, `r"""`, `b"`, or a tag such as `re"`. The parser reads the form from it.
- `StringText` is literal text, escapes included. Escapes are validated here and decoded later by `literal::string_value`.
- In the multi-line forms (`"""` and `r"""`), each line break inside the string is a `StringNewline` token. That keeps the parser's line-end logic out of strings, while the formatter still sees the string's lines.
- Each line's stripped indentation is a `StringIndent` token: the closing line's indentation on each content line (`r[lex.lit.string.multiline]`), the closing line's own indentation before `"""`, and all of any line that holds only whitespace, whatever its length, since such a line becomes empty. The formatter re-indents a multi-line string by rewriting those tokens, which leaves its value unchanged.
- **Interpolation is tokenized**, in the forms that interpolate (`"` and `"""`): `InterpStart` (`{`), then the path as ordinary `Ident`, `SelfKw`, `Dot` and `Int` tokens, then `InterpEnd` (`}`). Since only a name, `self` or a constant followed by field steps is allowed (`r[lex.lit.string.interp-root]`, which replaced `interp-path` after spec question 2), the lexer never has to lex an expression or a nested string, so this is one extra lexer state, not recursion. Names in strings are then real tokens in the CST, and rename, find-references and `query` treat them like any other name.
- **A test's name doesn't interpolate** (`r[test.name]`), but it takes a string's escapes (`r[test.name-escapes]`, decided by David, 2026-11-11). So a `"` string whose previous token, skipping spaces, is `test` has no interpolation tokens, and its `{` is text.
- `}` alone is text (`r[lex.lit.string.lone-brace]`).
- Raw, bytes and tagged forms produce only `StringText` between start and end, with no escapes (raw and tagged) or a reduced set (bytes, `r[lex.lit.bytes]`).
- A known tag is one of `url`, `re`, `dur`, `ts`. An identifier glued to `"` that isn't a tag (`foo"x"`, `br"x"`, `f"x"`) is an E0010 listing the tags. A keyword is never a tag, so `test"adds"` is `test` and a string.
- `Char` is one token, `'a'`, with the string escapes plus `\'` (`r[lex.lit.char.escapes]`).

*Alternative:* each string as one token, with interpolation found later by re-scanning its text. Simpler for the lexer, but every consumer of names (resolution, rename, the language server) would need a special case for names inside strings. Rejected.

**Recovery.**
- A one-line string with no closing quote ends before its line ending (`r[lex.lit.string.one-line]`), with no `StringEnd`, so one missing quote can't swallow the file.
- A broken interpolation (`{a + b}`, `{}`, `{ name }`, a `{` never closed) becomes an `Error` token up to and including the next `}`, or up to but excluding the next `"` or line ending, and the string goes on. So `"{"` reports one E0007, and its closing quote still ends the string.
- A char literal with no closing `'` ends before its line ending.
- A lone CR is a line ending here too (decision 3), so it ends a one-line string or char literal. Only E0013 is reported then, not E0004 or E0009, since the missing quote is the CR's fault.
- A multi-line string with no closing `"""` line runs to the end of the file. That one does swallow the file, and Risks covers it.

### 9. Diagnostics

The lexer reports `LexError { range, code, message, fixes }` in byte offsets. `quince_syntax` converts them to `Diagnostic` values through a `LineIndex`, which goes in `quince_diagnostics`, because that crate owns how positions count. The conformance runner uses the same `LineIndex` to number fixture lines and to run `Fix::check`.

Codes are numbered in the order they are registered, from E0001, skipping the reserved E0412, with no ranges per stage. Ranges encode which stage reports a problem, which can change, and they run out. Codes are permanent (`r[diag.code-stable]`), so a code whose rule waits on a spec question is registered only once the question is approved, and the numbers below are provisional until then.

| Code | Problem | Fix | Waits on |
| --- | --- | --- | --- |
| E0001 | A character that starts no token (`$`, `#`, a backtick, non-ASCII outside a literal or comment) | For a Unicode space such as U+00A0, replace it with a space (`safe`) | Q1 for non-ASCII |
| E0002 | A `;` (statements end at line breaks) | At a line end, delete it (`safe`) | |
| E0003 | A block comment `/* */` (`r[lex.comment.line]`) | When it covers whole lines, rewrite it as `//` lines (`safe`) | |
| E0004 | A one-line string, raw, bytes or tagged literal with no closing `"` | Insert `"` at the line end (`needs-review`) | |
| E0005 | A multi-line string with no closing `"""` line | None | |
| E0006 | An invalid escape: unknown (`\q`), `\'` in a string, `\}`, a bad `\u{...}`, a bad `\x` or one outside bytes | For `\'` and `\}`, delete the backslash (`safe`) | Q7 for `\u` and `\x` details |
| E0007 | An interpolation that isn't a name or field path | For spaces inside the braces, delete them (`safe`) | Q2 |
| E0008 | A malformed number (decision 7) | Remove a suffix (`needs-review`); lowercase an exponent `E` or a base prefix (`safe`) | Q4 |
| E0009 | An invalid char literal: empty, more than one scalar value, or unclosed | For more than one character, use `"` (`needs-review`) | |
| E0010 | An unknown literal tag | None. The message lists the tags | |
| E0011 | A non-ASCII character in a bytes literal | Replace it with the `\x` escapes of its UTF-8 bytes (`safe`) | |
| E0012 | A multi-line string layout error | None | Q3 |
| E0013 | A carriage return not followed by a line feed | Outside a literal, replace it with a line feed (`safe`). None inside a one-line literal, which the CR ends (decision 8) | Q1 |
| E0014 | A file that isn't valid UTF-8 | None | Q1 |
| E0015 | A control or bidirectional-format character in a literal or comment | In a `"` or `"""` string, replace it with its `\u{...}` escape (`safe`). None in raw and tagged bodies or comments | Q8 |

Messages are written for the habits agents bring from other languages, since that costs nothing now:
- E0001 on `#` suggests `//`, and on a backtick says Quince strings use `"` and interpolate with `{name}`.
- E0001 on `r#"` says raw text that holds quotes uses `r"""`.
- E0007 on `{}` says the braces need a name, and that a literal brace is written `\{`. On an expression, it suggests binding it with `let` first.
- E0010 on `f"` says every `"` string interpolates already.

`"Total: ${amount}"` gets no warning. It is valid text followed by an interpolation, and prices are a real use of it.

Consecutive invalid characters make one `Error` token and one E0001, so a pasted line of emoji is one diagnostic, not thirty. A malformed literal keeps its literal kind (decision 2), so nothing reports it again (`r[diag.all-errors]`).

**E0014 and file size.** `quince_syntax::decode_source(&[u8]) -> Decoded` is the one place source bytes become text, and every caller that reads a file uses it before lexing.
- It reports one E0014 for each line that holds invalid bytes, spanning that line's first invalid run, so a binary file passed by mistake gives one diagnostic per line, not one per byte.
- It returns a `Decoded`: text in which each maximal run of invalid bytes is one U+FFFD, and the ranges of those replacements. Lexing and parsing go on over that text, so the rest of the file is still checked.
- The lexer takes the replaced ranges (`lex(&Decoded)`, or `lex_str(&str)` for text that was never bytes). A replacement character in one of those ranges becomes an `Error` token with no diagnostic, since E0014 already reported it. A U+FFFD that was really in the file still gets E0001.
- Each replacement counts as one column. That is a position rule, so layer 3 adds it to `compiler/diagnostics/README.md`.
- Since that text no longer matches the file, no tool may write it back. `Decoded::is_lossy()` says so, and every writer checks it: `fmt` (the formatter doc), `fix` (M5) and the language server (M5).
- It refuses a file over 4 GiB, the most a `u32` offset can address, as an I/O error, not a diagnostic.

### 10. Wiring the conformance runner

`SyntaxBackend` lives in `quince_conformance` and calls `quince_syntax` in-process. This settles the M0 open question for `parse`:
- `quince_syntax` has no LLVM and builds in seconds, so xtask can depend on it without slowing `cargo xtask spec-rules`, which was M0's concern.
- A subprocess would also test the CLI's JSON output, but that output's format is M5's decision.
- This doesn't decide `check`. Wiring it in-process would compile the type and effect checkers into xtask. M2, which first wires `check`, decides between that and a subprocess, and may split the runner out of xtask's spec steps. `run` waits for M6.

Until the parser lands, `SyntaxBackend::parse` returns the lexer's diagnostics only. `check` and `run` stay unwired, so nobody writes a `check` fixture that passes only because nothing checks it.

Lexer fixtures go in `tests/conformance/testdata/lex/` in `parse` mode. They must be valid syntax apart from their expected diagnostics, so they keep passing when the parser arrives instead of failing on syntax the lexer couldn't see. The fixture README doesn't say this for `parse` fixtures yet, so layer 1 adds it. The error contract (decision 2) is what makes it possible for fixtures that contain an `Error` token or a `;`. An `Error` token is only trivia, so a fixture puts a bad character where trivia may go (`let x = 1 $`), not where the parser needs a token (`let x = $`, which would leave "expected expression"). Whether the parser should suppress such an error after an `Error` token is the parser doc's call.

The runner changes:
- It reads fixtures as bytes, and every `Backend` method takes `&[u8]`, so E0014 can have a fixture.
- It reads headers and expectations from decoded text, strips a leading BOM before reading the header, and numbers lines with `LineIndex` instead of `str::lines()`. So the BOM and lone-CR fixtures agree with the diagnostics.
- Layer 1 decodes with `String::from_utf8_lossy`, because `decode_source` doesn't exist until layer 3, which switches the runner to it. The two place U+FFFD differently (per maximal subpart, not per run). That only moves columns on lines with invalid bytes, where E0014 has no fix to check, so the switch changes no fixture result.
- It runs the position half of `Fix::check`, with the source text, on every fix a backend reports.

### 11. Testing

- **Unit tests** in `quince_syntax` compare token dumps (`kind "text"` per line) with hand-written expected text, and check decoded values through `literal`. No snapshot crate.
- **Spec sync tests** (decisions 4 and 6): keywords against the spec, punctuation and `continues_line()` against the grammar.
- **Corpus:** every code block labelled `quince` in the spec chapters and `std/API.md` lexes with no errors (spec question 5).
- **Invariants on every input:** no panic; the token texts concatenate to the input; no token is empty; every error range lies inside the input on character boundaries. These run over seeded mutations (inserted, deleted and swapped characters, from a small dependency-free PRNG) of the corpus, the unit-test inputs and the `lex/` fixtures. The last two exercise the string and interpolation states, which the corpus barely reaches. They are bounded so `cargo test` stays under a second for this crate. A `cargo-fuzz` target needs nightly and a dependency, so it waits for the parser doc, which owns the fuzzing exit criterion.
- **Conformance fixtures:** one per diagnostic code, as `cargo xtask diagnostics` requires, and one per rule in Tasks.

**Coverage honesty.** A fixture cites a rule only when a lexer that broke the rule would fail it. With no parser yet, a `parse` fixture fails only on an unexpected or missing diagnostic. So:
- Rules with a lexical error (E0002 for `r[lex.braces]`, E0003 for `r[lex.comment.line]`) are cited by their negative fixtures.
- Accepted forms (`0b1010`, a multi-line string) are cited by fixtures a lexer that rejected the form would fail. They check acceptance, not values. Values are unit-tested until `run` fixtures exist in M6, which then cite the same rules.
- Rules a broken lexer would still pass, because it would report nothing, are tested by unit tests now and cited by the parser's fixtures. They are `lex.keywords`, `lex.no-contextual`, `lex.no-shift-token`, `lex.grammar-rules`, `lex.ident-classes`, `lex.indentation`, `lex.comment.doc` and `lex.comment.doc-exact`. A lexer that made `>>` one token would fail a parser fixture, not a lexer one.
- Rules about other tools or about values are cited by those tools' fixtures: `lex.source.canonical` by `fmt` and `check` fixtures (the formatter doc), and `lex.lit.multiline-value` by `run` fixtures from M6.

### 12. Delivery as a stack

Each layer builds and passes `cargo xtask ci` on its own:

1. `m1/lexer-harness`: `LineIndex`, `Backend` on bytes, the runner's BOM and line numbering, fix-position checks, and the `parse`-fixture rule in the fixture README. All of it is tested with the existing fake backend, so this layer adds no codes and cites no rules.
2. `m1/lexer-tokens`: spec question 6, and a minor version bump; `SyntaxKind`, `Token`, `lex_str`, trivia, identifiers, keywords, punctuation and comments; the token contract in `compiler/syntax/README.md`; `AGENTS.md`'s map. Problems already become tokens, such as `Error` and `Semicolon`, but nothing reports them yet, so this layer adds no codes.
3. `m1/lexer-errors`: spec question 1, and a bump; `LexError`, `decode_source` and `lex`; `SyntaxBackend`; E0001 to E0003, E0013 and E0014; the error contract in the README.
4. `m1/lexer-numbers`: spec question 4, and a bump; numbers, the tuple-index rule and `literal`'s number values; E0008.
5. `m1/lexer-strings`: spec questions 2 and 7, and a bump; one-line strings, escapes and interpolation; E0004, E0006 and E0007.
6. `m1/lexer-literals`: spec questions 3 and 8, and a bump; multi-line, raw, bytes, tagged and char literals; E0005, E0009 to E0012 and E0015.
7. `m1/lexer-corpus`: spec question 5 (labels, no bump); the corpus and mutation tests.

Each layer should come in at around 500 to 900 changed lines. Layer 2 was first planned to hold layers 2 to 4, but it would have run to about 1,600 lines, so it is split into the token stream, its errors, and numbers. The codes keep the numbers the table gives, so E0008 is registered after E0013 and E0014.

**Literals before their layer.** Layers 2 to 5 must stand alone, so they lex the literal forms they don't handle yet by a fallback rule. Everything from a `"` to the next `"` on the same line is `StringStart`, `StringText` and `StringEnd`, with no validation. Everything from a `'` to the next `'` on the line is a `Char`. A prefix is a separate `Ident`. Until layer 4, a digit and the letters, digits and underscores after it are one `Int`. Until layer 6, `"""` lexes as an empty string followed by another string, so a multi-line string lexes wrongly. Nothing depends on that: no fixture uses those forms, and no command lexes user code until the parser lands.

## Spec questions (decided by David, 2026-11-11)

All eight are approved: question 1 in its strict form, the other seven as proposed. Each rule goes in `spec/reference/lexical.md` with the tag shown. Every ID has 2 to 4 segments, and its second segment is a construct listed under `lex` in `spec/chapters.txt`. Question 1 adds one construct, `source`. Question 5 doesn't block the lexer.

**1. Source text (layer 3).** The chapter doesn't say what a source file is made of. Approved, as a new section "Source text" at the top of the chapter, with a `source` construct ("Source text: encoding, line endings, whitespace"):
- `r[lex.source.utf8]` A source file is UTF-8 text. A file that isn't valid UTF-8 is a compile error.
- `r[lex.source.bom]` A byte order mark at the start of a file is ignored by the compiler.
- `r[lex.source.line-end]` A line ends at a line feed, or at a carriage return followed by a line feed. A carriage return anywhere else is a compile error.
- `r[lex.source.canonical]` `quince` `fmt` writes every line ending as a line feed and removes a byte order mark. So, like any unformatted code, a file with CR LF line endings or a byte order mark compiles but fails `quince` `check` ([Toolchain](../tooling/toolchain.md)).
- `r[lex.source.whitespace]` Within a line, spaces and tabs are the only whitespace.
- `r[lex.source.ascii]` Outside literals and comments, source text is ASCII, so identifiers are ASCII letters, digits and underscores.

*Decision (David, 2026-11-11): strict.* Every Quince file has one canonical byte form. CR LF and a BOM are accepted, so such a file still compiles, but they count as formatting. This is the reversible direction: loosening it later breaks nothing, while the reverse would have made existing code fail `check`. A lone CR is almost always damage, so it is an error, not just unformatted. ASCII-only identifiers follow from the naming rules (`snake_case` has no case for `é`) and keep look-alike characters out of names. They're also the reversible direction: allowing more later breaks nothing.

*Rejected:* `fmt` keeps the BOM and the file's line endings, and `check` doesn't report them. That spares Windows checkouts made with `core.autocrlf=true`, but leaves two byte forms for the same file and closes off the strict choice later.

*Cost:* a Windows checkout that converts line endings fails `check` until `fmt` runs. The mitigation is a `.gitattributes` line, `*.qnc text eol=lf`, in every Quince project. Whether a `quince` command writes it when it creates a project is for M2's project-layout doc.

**2. Interpolation paths (layer 5).** `r[lex.lit.string.interp-path]` allows "a name or a field path", but the spec's own example (Items, `"{self.name} <{self.email}>"`) starts a path with `self`, which is a keyword, not a name. Allowing `self` and constants widens the rule's meaning, so the old ID is retired and replaced:
- Retire `lex.lit.string.interp-path`, and add `r[lex.lit.string.interp-root]`: "Inside braces only a path is allowed, never another expression: a name, `self` or a constant name, followed by any number of field names or tuple positions, with no spaces: `{name}`, `{user.name}`, `{self.email}`, `{id.0}`, `{MAX_RETRIES}`."
- A path can't start with a type (`{Config.MAX}`, `{Self.X}`). Bind the value first.

**3. Multi-line strings (layer 6).** `r[lex.lit.string.multiline]` leaves open what may follow the opening `"""`, where the closing one may sit, and what happens to a line indented less than the closing line. Approved, following Swift:
- `r[lex.lit.multiline-open]` After the opening `"""`, only spaces and tabs may come before the line break.
- `r[lex.lit.multiline-close]` The closing `"""` is the first thing on its line after its indentation, and code may follow it on that line (`""")`). Elsewhere, `"`, `""` and `"""` are text. So a content line can't start with `"""`: a `"""` string writes it `\"""`, and an `r"""` string can't hold it.
- `r[lex.lit.multiline-indent]` Every line between must start with exactly the closing line's indentation, the same spaces and tabs, except a line that holds only whitespace, which becomes empty. Otherwise it is a compile error.
- `r[lex.lit.multiline-value]` The value doesn't include the line break before the closing line, and every line break in it is a line feed, whatever the file uses.
- `r[lex.lit.multiline-forms]` Only `"""` and `r"""` are multi-line. A `"""` string takes the same escapes as a `"` string. `b"""` and tagged multi-line literals don't exist.

**4. Number forms (layer 4).** The examples don't settle the edge cases. Approved:
- `r[lex.lit.underscore]` In any numeric literal, including a float's fraction and exponent, an underscore may appear only between two digits. So `1_000` and `1_000.5` are valid, and `_1`, `1_`, `1__0`, `0x_FF` and `1_.5` are not.
- `r[lex.lit.int.prefix]` The base prefixes are lowercase (`0x`, `0b`, `0o`). Hex digits may be either case.
- `r[tool.fmt.hex-digits]`, in the Toolchain chapter's formatting rules: "`quince` `fmt` writes hex digits uppercase." The formatter doc cites it.
- `r[lex.lit.int.leading-zero]` A decimal literal, or a float's integer part, doesn't start with `0` unless it is `0`. So `007` and `00.5` are errors, rather than octal as in C, and `0.5` is fine.
- `r[lex.lit.float.shape]` A float has digits on both sides of its point (`1.0`, not `1.` or `.5`), and its exponent is a lowercase `e`, an optional sign and digits. `1E9` is an error whose fix lowercases the `E`.
- `r[lex.lit.no-suffix]` A numeric literal has no type suffix: its type comes from context.

**5. Code block labels (layer 7, wording only, no bump).** The spec and `std/API.md` don't label their code blocks, so "every code block parses" can't tell Quince from TOML, JSON and REPL transcripts. A skip list keyed by file and first line would break on every edit. Approved: label every block (`quince`, `toml`, `json`, `text`), and add a `spec-rules` check that fails on an unlabelled block.

**6. Doc comments (layer 2).** `r[lex.comment.doc]` doesn't say whether `////` is a doc comment. Approved: `r[lex.comment.doc-exact]` "A doc comment starts with exactly three slashes: `////` starts an ordinary comment", as in Rust.

**7. Escapes (layer 5).** Approved:
- `r[lex.lit.string.unicode-escape]` "`\u{...}` holds one to six hex digits naming a Unicode scalar value, so `\u{}`, surrogates (`\u{D800}`) and values above `\u{10FFFF}` are errors."
- `r[lex.lit.bytes.hex-escape]` "`\x` takes exactly two hex digits, and appears only in bytes literals."

**8. Control and bidirectional characters (layer 6).** The spec doesn't say which characters may appear raw inside literals and comments. The Trojan Source attack (CVE-2021-42574) hides code in bidirectional-format characters, and rustc denies them by default. Approved:
- `r[lex.source.control]` "Apart from tabs and line endings, a literal or comment can't contain a control character (U+0000 to U+001F, and U+007F) or a bidirectional-format character (U+202A to U+202E, U+2066 to U+2069). A `"` or `"""` string writes one as an escape. Raw and tagged literals and comments can't hold them."
- A carriage return not followed by a line feed falls under `r[lex.source.line-end]` (E0013), not this rule, so one character never gets both codes.

*Why:* these keep code from reading differently than it runs, as the ASCII rule does for names.

## Risks and trade-offs

- [A multi-line string with no closing line swallows the rest of the file, so the parser reports nothing after it] → E0005 points at the opening `"""`, and fixing it restores everything. A heuristic close (the first line that looks like code) would guess wrong in strings that hold code, which is a common use. Accepted.
- [An unclosed `/*` swallows the rest of the file in the same way] → E0003 points at it. Quince has no block comments, so this only happens to code written from habit, and the message says to use `//`.
- [`parse` fixtures pass against a lexer-only backend even when they aren't valid syntax] → Layer 1 puts the rule in the fixture README, review checks it, and the parser layer reruns every fixture.
- [Acceptance-only fixtures mark a rule covered before its value is tested] → Decision 11 limits what a lexer fixture may cite. Values are unit-tested now, and `run` fixtures cite the same rules from M6. The coverage report can't show the difference. Accepted until a test kind marks partial coverage.
- [Tokenized interpolation makes the lexer stateful] → One state, entered at `{` in an interpolating string and left at `}`, `"` or a line end. The invariant tests run through it, seeded from fixtures that use it.
- [The token contract turns out wrong for the parser] → Token boundaries also shape fixtures and, later, CST tests, so the cost grows with every layer. The parser doc comes next and can amend this one before layer 5 merges. `SyntaxKind` is internal to `quince_syntax`, so only that crate and the `lex/` fixtures change.
- [Fifteen codes is a lot for a lexer] → Each names one problem an agent can fix without reading the explain text. Codes are permanent, so merging two later would retire one. Review the table before layer 3.
- [Four spec bumps in one milestone] → Each bump records a real change in meaning, and the project's rule keeps each spec change in the PR with its code. A version number costs nothing.

## Rollback

Revert the stack's PRs. Nothing outside `quince_syntax`, `quince_diagnostics`, the runner, `AGENTS.md` and `spec/` changes, and no later work depends on it yet.

## For the parser doc

Found while writing this. None of them block the lexer.
- **Naming classes.** `r[lex.name.case]` names the styles but doesn't define them, and `r[lex.name.enforced]` makes the answer a compile error. Open cases:
  - `T` and `URL` fit both Pascal and screaming case;
  - it's unclear whether `_unused`, `x2`, `utf8_len`, `HTTPServer` and `a__b` are valid.

  Where the grammar accepts both `Name` and `NAME` (`primary`, `alt`), the parser has to pick one, or leave it to name resolution.
- **Booleans.** `true` and `false` are keywords, and the grammar's `literal` doesn't say it includes them, but a pattern `true =>` needs it to.
- **Line breaks before a closing bracket.** `r[tool.fmt.check]` says unformatted code compiles, so a call broken over lines without its trailing comma (`f(\n  a,\n  b\n)`) must parse. The line break before `)` can't end a statement there.
- **C names that are keywords.** A C function whose name is a keyword (POSIX `select`) can't be declared: `r[ffi.cbind.keyword-param]` covers parameters only, and `r[lex.no-contextual]` rules out reading `select` as a name inside `foreign`.
- **`;` ends a statement outright** (decided by David, 2026-11-11). A `;` isn't a line break, so the leading-dot continuation (`r[lex.continue.more]`) doesn't join across it: `a;` followed by a line `.b()` is two statements. E0002's fix, which deletes the `;`, would join them, so there it is `needs-review`.
- **Negative literals.** `r[lex.lit.int.neg]` says the minus is "directly before" the literal. Whether `- 1` counts decides whether the parser checks adjacency.
- **Rules this doc hands over** (decision 11): fixtures for `lex.keywords`, `lex.no-contextual`, `lex.no-shift-token`, `lex.grammar-rules`, `lex.ident-classes`, `lex.indentation`, `lex.comment.doc` and `lex.comment.doc-exact`. The formatter doc takes `lex.source.canonical`.
- **Coverage ratchet.** M0 left this for M1. Decide it once the parser adds real coverage.

## For later milestones

- **Lockfile hashes (M11).** CR LF files still compile (spec question 1), and a Windows checkout can convert line endings, so content hashes must be computed over git's tree or over normalized bytes, or a Windows checkout hashes differently. The M11 design doc should take this up.
- **`.gitattributes` for Quince projects (M2).** See question 1's cost: decide whether creating a project writes `*.qnc text eol=lf`.

## Tasks

Rule IDs in brackets are what each task's fixtures cite. Rules added by spec questions are cited only once approved.

**Layer 1: `m1/lexer-harness`**
- [x] `LineIndex` in `quince_diagnostics`, with the position rules' unit tests.
- [x] `Backend` takes `&[u8]`; the runner reads fixture bytes, decodes them with `String::from_utf8_lossy`, strips a BOM before the header, and numbers lines with `LineIndex`.
- [x] Fix-position checks in the runner.
- [x] Fixture README: `parse` fixtures are valid syntax apart from their expectations.

**Layer 2: `m1/lexer-tokens`**
- [x] Spec: the approved wording from question 6, and a minor version bump.
- [x] `SyntaxKind`, `Token`, `Lexed`, `lex_str`, trivia and the lossless invariant; unit tests for keywords, `continues_line()` and the grammar sync.
- [x] As tokens: punctuation with `>` alone, `;`, runs of characters that start no token, comments, doc comments and block comments, line endings and the BOM.
- [x] The fallback rule for literals not handled yet, numbers included (decision 12).
- [x] `compiler/syntax/README.md`: the token contract (decisions 2 to 8). Update `AGENTS.md`'s map.

**Layer 3: `m1/lexer-errors`**
- [x] Spec: the approved wording from question 1, the `source` construct, and a minor version bump.
- [x] `LexError`, `lex(&Decoded)` and `Lexed::diagnostics`; `SyntaxBackend` in place of `NoBackend`, and a runner check that each diagnostic names the fixture's file.
- [x] `decode_source`, `Decoded`; E0013, E0014; the replacement-column rule in `compiler/diagnostics/README.md`; switch the runner to `decode_source`, and update the fixture README's sentence on how invalid UTF-8 decodes (one U+FFFD per run, not per sequence). [`lex.source.utf8`, `lex.source.bom`, `lex.source.line-end`, `lex.source.whitespace`, `lex.source.ascii`]
- [x] `;` and unknown characters; E0001, E0002. [`lex.braces`]
- [x] Block comments; E0003. [`lex.comment.line`]
- [x] `compiler/syntax/README.md`: the error contract (decisions 2 and 9).

**Layer 4: `m1/lexer-numbers`**
- [x] Spec: the approved wording from question 4, `r[tool.fmt.hex-digits]`, and a minor version bump.
- [x] Numbers, the tuple-index rule, `literal::int_value` and `float_value`; E0008. [`lex.lit.int.forms`, `lex.lit.float.forms`, `lex.lit.underscore`, `lex.lit.int.prefix`, `lex.lit.int.leading-zero`, `lex.lit.float.shape`, `lex.lit.no-suffix`]

**Layer 5: `m1/lexer-strings`**
- [x] Spec: questions 2 and 7, retiring `lex.lit.string.interp-path`, and a bump.
- [x] Spec: `r[test.name-escapes]` (decided by David in review); test names don't interpolate. [`test.name`, `test.name-escapes`]
- [x] E0013: no fix for a CR right after another CR (a layer 3 fix, folded in by David).
- [x] String shape and escapes, `literal::string_value`; E0004, E0006. [`lex.lit.string.escapes`, `lex.lit.string.one-line`, `lex.lit.string.unicode-escape`]
- [x] Interpolation tokens; E0007. [`lex.lit.string.interp`, `lex.lit.string.interp-root`, `lex.lit.string.lone-brace`]

**Layer 6: `m1/lexer-literals`**
- [ ] Spec: questions 3 and 8, and a bump.
- [ ] Multi-line and raw multi-line strings, `StringNewline`, `StringIndent`; E0005, E0012. [`lex.lit.string.multiline`, `lex.lit.multiline-open`, `lex.lit.multiline-close`, `lex.lit.multiline-indent`, `lex.lit.multiline-forms`, `lex.lit.string.raw-multiline`]
- [ ] Raw strings. [`lex.lit.raw-string`]
- [ ] Bytes; E0011. [`lex.lit.bytes`, `lex.lit.bytes.hex-escape`]
- [ ] Tagged literals; E0010. [`lex.lit.tagged`, `lex.lit.tagged.raw`]
- [ ] Char literals; E0009. [`lex.lit.char`, `lex.lit.char.escapes`]
- [ ] Control and bidirectional characters; E0015. [`lex.source.control`]

**Layer 7: `m1/lexer-corpus`**
- [ ] Spec: label every code block (question 5), and the `spec-rules` check for unlabelled blocks.
- [ ] Corpus test over the spec and `std/API.md`.
- [ ] Mutation tests of the invariants.
