# M1: Lexer

Status: draft, not yet approved. The first design doc of M1 (Syntax). A parser doc and a formatter doc follow it.

## Why

M1 ends with every code block in the spec parsing, `quince fmt` idempotent, and a lossless CST that prints back the input byte for byte ([milestones](../milestones.md)). Every one of those needs a token stream first, and so does every later stage that reads source. The lexer is also where most of the spec's [Lexical structure](../../spec/reference/lexical.md) chapter is enforced, and where the first diagnostic codes and the first conformance backend arrive.

Getting the token contract right now matters more than the code, which is small. The parser, the formatter, `fix`, `query` and the language server all read tokens, and changing what a token is later means changing all of them.

## Impact and approvals

- **Code:** `compiler/syntax` (`quince_syntax`) gets its first modules and a `README.md` holding the token contract. `quince_diagnostics` gets a line index. `quince_conformance` gets its first backend.
- **Spec (needs David's approval):** the chapter leaves several things open that a lexer must decide. Spec questions, below, proposes wording for each. The approved wording lands in layer 1 with the code that enforces it, and bumps the spec to Draft v0.6 once.
- **Diagnostic codes:** E0001 to E0014, the first codes in the registry (decision 9).
- **Dependencies:** none. No `logos`, no `rowan`. Whether the CST uses `rowan` is the parser doc's question.
- **Delivery:** a stack of three PRs, each under about 800 changed lines (decision 12).

## Context

- `quince_syntax` is an empty crate with a one-line doc comment. It depends on nothing and must stay free of LLVM.
- The Lexical structure chapter has 42 rules. 24 of them are the lexer's (Tasks lists which). The rest belong to the parser (line continuation, `else` placement, negative literals, collection literals), or to type checking (literal types).
- The grammar (`spec/grammar.ebnf`) uses 42 quoted punctuation terminals and 38 keywords. Its prose terminals are `name`, `Name`, `NAME`, `c_name`, `path`, `literal`, `string`, `int`, `doc_comment` and `NEWLINE`.
- `quince_diagnostics` fixes how positions count (`compiler/diagnostics/README.md`): `[line, column]`, 1-based, columns in Unicode scalar values, lines ending at LF, CR LF or a lone CR, a leading BOM not counted. It says which line endings the lexer accepts is the spec's business. The spec doesn't say.
- The conformance runner has a `Backend` trait with `parse`, `check` and `run`, and ships `NoBackend`. `parse` fixtures run only the parser and must be valid syntax apart from their expected diagnostics (`tests/conformance/testdata/README.md`).
- `Fix::check` can't yet check that a fix's positions exist in the source. M0 left that for M1's backend.
- The spec and `std/API.md` hold 66 code blocks with no info string. Most are Quince. A few are TOML, JSON or REPL transcripts (`>>> 3.14159.to_fixed(2)`).

## Goals and non-goals

**Goals:**
- A lexer that turns any `&str` into a token stream whose texts concatenate to the input exactly, never panics, and reports every lexical error with an exact span and, where one is clear, a fix.
- A token contract the parser and formatter can build on without re-scanning text, documented in `compiler/syntax/README.md`.
- The first diagnostic codes, each with explain text and a conformance fixture, and the first wired conformance backend.

**Non-goals:**
- Deciding which line breaks end a statement. The parser decides that (decision 4).
- Checking naming case (`lex.name.*`), keywords used as names, or C names. They need parse context, so the parser doc places them.
- Checking a tagged literal's body (`url"..."` is a valid URL, `dur"30s"` a valid duration). That needs the literal's type, so type checking does it.
- Literal types and range checks (`let b: UInt8 = 300`). M3.
- Incremental relexing for the language server. Lexing a whole file is fast enough until M5 shows otherwise.
- A `quince` command that prints tokens. The parser doc may add a CST dump, which covers it.

## Decisions

### 1. Where it lives

The lexer is the `lexer` module of `quince_syntax`, with its literal handling in submodules. Literal values (decoded strings, integer values) are computed on demand by a `literal` module from token text, not stored in tokens.

*Alternative:* a separate `quince_lexer` crate. Rejected: nothing but the parser and formatter use tokens directly, they live in the same crate, and the repo plan puts the lexer there. Splitting later is cheap if a second consumer appears.

### 2. Lossless tokens

`lex(source: &str) -> Lexed`, where `Lexed` holds a `Vec<Token>` and a `Vec<LexError>`.
- A `Token` is a `SyntaxKind` and a byte length (`u32`). Start offsets are the running sum, so a token is 8 bytes. Files over 4 GiB are refused by the caller that reads them, as an I/O error, not a diagnostic.
- Every byte of the input belongs to exactly one token, so concatenating the token texts gives back the input. Comments, spaces and line breaks are tokens too (trivia).
- The lexer never fails and never stops early. An error produces a diagnostic and a token, and lexing goes on.
- `SyntaxKind` is one `u16` enum in `quince_syntax::kind`. The parser adds its node kinds to the same enum, as rowan-style CSTs need, but nothing here requires rowan.

*Alternative:* tokens without trivia, plus a side table of comments. Rejected: the formatter, `fix` and error recovery all need trivia in place, and the repo plan already chose a lossless CST.

### 3. Trivia kinds

- `Whitespace`: a run of spaces and tabs.
- `Newline`: one line ending, LF or CR LF. Each line ending is its own token, so the formatter can count blank lines and the parser can find line ends without scanning text.
- `Comment`: `//` up to, not including, the line ending.
- `DocComment`: `///` up to the line ending. It's a separate kind because the grammar names `doc_comment` as a terminal. Whether the parser treats it as trivia is the parser's call.
- A leading byte order mark is a `Bom` token, pending spec question 1.

### 4. Line breaks are tokens, and the parser decides what they mean

The lexer doesn't insert statement terminators, as Go's lexer does. It emits every line ending as a `Newline` token, and the parser decides whether one ends a statement.

The rules can't be applied from tokens alone:
- `r[lex.line-end]` continues a line that ends with a binary operator. A trailing `>` is greater-than in `a >` but closes type arguments in `fn get() -> Option<Int>` in an interface, where the line must end. Only the parser knows which.
- `r[lex.continue.signature]` applies only inside a `fn` signature.
- `r[lex.else-same-line]` needs to know the `else` follows a block, to give its fix.

So the lexer offers only a helper for the parser: `SyntaxKind::continues_line()`, true for the kinds that continue a line in every context (`,`, `(`, `[`, `{`, `=`, compound assignments, `=>`, `->`, `&&`, `||`, and the arithmetic and bit operators that are never anything else). The parser handles the two that depend on context, `>` and `*` (`uses *`). Comments are trivia here too: a line ends with its last token before any comment.

*Alternative:* Go-style terminator insertion in the lexer. Rejected for the reasons above: it would need a lexer that tracks generics and signatures, which is a parser.

### 5. Punctuation, and `>` always alone

The lexer takes the longest match among the grammar's punctuation terminals, with one exception: **`>` is always a token of its own.** The parser forms `>>` (shift) and `>=` from two `>`-led tokens with no trivia between them.
- `r[lex.no-shift-token]` already requires this for `>>`, so `List<List<Int>>` closes without a space.
- Doing the same for `>=` means `let x: Option<Int>= y` doesn't lex as `Option<Int` followed by `>=`. One mechanism covers both.
- `..=` is one token (`r[lex.grammar-rules]`), and so are `->`, `=>`, `==`, `!=`, `<=`, `<<`, `&&`, `||` and the compound assignments.
- `_` alone is `Underscore`. `_` followed by a letter, digit or `_` is an identifier.

*Alternative:* lex `>=` and `>>` greedily and have the parser split them where it expects `>`, as rustc does. Rejected: the spec forbids a `>>` token outright, and splitting is a second mechanism for the same problem.

### 6. Identifiers and keywords

- An identifier is an ASCII letter or `_`, then ASCII letters, digits and `_` (pending spec question 1 for non-ASCII). It's one `Ident` kind, whatever its case. `Ident` offers `case_class()` returning snake, Pascal, screaming or none, which the parser uses for `name`, `Name` and `NAME`, and which naming checks use later.
- The 38 keywords of `r[lex.keywords]` each have their own kind (`FnKw`, `SelfTypeKw` for `Self`, and so on). There are no contextual keywords (`r[lex.no-contextual]`), so the lexer has no context to track: `main`, `limit`, `TaskScope` and the tag names lex as identifiers everywhere.
- `c_name` lexes as an identifier too. Whether a C name may be a keyword is a question for the parser doc.

A test reads the keyword list from `spec/reference/lexical.md` through `quince_spec` (a dev-dependency) and fails if it differs from the lexer's table. A second test lexes every quoted terminal in `spec/grammar.ebnf` and checks each is one token, except `>>` and `>=`, which must be two. So the spec and the lexer can't drift apart silently.

### 7. Numbers

- `Int` covers `42`, `1_000_000`, `0xFF`, `0b1010` and `0o17`. `Float` covers `3.14`, `1e-9` and `2.5e10`. The exact forms are spec question 4.
- A `.` belongs to a float only when a digit follows it. So `1..10` is `1`, `..`, `10`, and `3.14159.to_fixed(2)` is a float then `.to_fixed`.
- **After a `.` token with no trivia between, only digits are lexed.** So `pair.0.1` is `pair` `.` `0` `.` `1`, not `pair` `.` `0.1`, and the grammar's `"." int` works for nested tuples. This is the only place the lexer looks back, and only one token.
- A minus sign is always its own token. `r[lex.lit.int.neg]` joins it to the literal in prefix position, which the parser knows and the lexer doesn't.
- A malformed number (`0b102`, `1__0`, `1_`, `0x`, `1e`, `007`) keeps its `Int` or `Float` kind and gets a diagnostic, so the parser sees a literal and reports nothing more.
- A number followed directly by a letter (`1u8`, `10px`) is one malformed `Int`. The message says literal types come from context (`let b: UInt8 = 1`), and the fix removes the suffix (`needs-review`).
- The lexer doesn't compute values. `literal::int_value` returns a `u128`, or `None` when it doesn't fit, for the type checker.

### 8. String literals

Every `"`-delimited literal has one shape:

```
StringStart  ( StringText | StringIndent | Interp... )*  StringEnd
```

- `StringStart` is the opening delimiter with its prefix: `"`, `"""`, `r"`, `r"""`, `b"`, or a tag such as `re"`. The parser reads the form from it.
- `StringText` is literal text, escapes included. Escapes are validated here and decoded later by `literal::string_value`.
- In a multi-line form, each content line's stripped indentation (the closing line's indentation, `r[lex.lit.string.multiline]`) is a `StringIndent` token. The formatter can re-indent a multi-line string by rewriting those tokens without changing its value.
- **Interpolation is tokenized**, in the forms that interpolate (`"` and `"""`): `InterpStart` (`{`), then the path as ordinary `Ident`, `SelfKw`, `Dot` and `Int` tokens, then `InterpEnd` (`}`). Since `r[lex.lit.string.interp-path]` allows only a name or field path, the lexer never has to lex an expression or a nested string, so this is one extra lexer state, not recursion. Names in strings are then real tokens in the CST, and rename, find-references and `query` treat them like any other name.
- `}` alone is text (`r[lex.lit.string.lone-brace]`).
- Raw, bytes and tagged forms produce only `StringText` between start and end, with no escapes (raw and tagged) or a reduced set (bytes, `r[lex.lit.bytes]`).
- A known tag is one of `url`, `re`, `dur`, `ts`. An identifier glued to `"` that isn't a tag (`foo"x"`, `br"x"`) is an unknown-tag error listing the tags. A keyword is never a tag, so `test"adds"` is `test` and a string.
- `Char` is one token, `'a'`, with the string escapes plus `\'` (`r[lex.lit.char.escapes]`).

*Alternative:* each string as one token, with interpolation found later by re-scanning its text. Simpler for the lexer, but every consumer of names (resolution, rename, the language server) would need a special case for names inside strings. Rejected.

**Recovery.** A one-line string with no closing quote ends at its line end (`r[lex.lit.string.one-line]`), so one missing quote can't swallow the file. A broken interpolation (`{a + b}`, `{`, `{ name }`) becomes an `Error` token up to the next `}`, `"` or line end, and the string goes on. A multi-line string with no closing `"""` line runs to the end of the file. That one does swallow the file, and Risks covers it.

### 9. Diagnostics

The lexer reports `LexError { range, code, message, fixes }` in byte offsets. `quince_syntax` converts them to `Diagnostic` values through a `LineIndex`, which goes in `quince_diagnostics`, because that crate owns how positions count. The conformance runner reuses the same `LineIndex` for `Fix::check`.

Codes are numbered in order from E0001, with no ranges per stage. Ranges encode which stage reports a problem, which can change, and they run out.

| Code | Problem | Fix |
| --- | --- | --- |
| E0001 | A character that starts no token (`$`, `#`, a backtick, non-ASCII outside a literal or comment) | For a Unicode space such as U+00A0, replace it with a space (`safe`) |
| E0002 | A `;` (statements end at line breaks) | At a line end, delete it (`safe`) |
| E0003 | A block comment `/* */` (`r[lex.comment.line]`) | When it covers whole lines, rewrite it as `//` lines (`safe`) |
| E0004 | A one-line string, raw, bytes or tagged literal with no closing `"` | Insert `"` at the line end (`needs-review`) |
| E0005 | A multi-line string with no closing `"""` line | None |
| E0006 | An invalid escape: unknown (`\q`), `\'` in a string, `\}`, a bad `\u{...}`, a `\x` outside bytes | For `\'` and `\}`, delete the backslash (`safe`) |
| E0007 | An interpolation that isn't a name or field path | For spaces inside the braces, delete them (`safe`). The message suggests `\{` for a literal brace, or a `let` for an expression |
| E0008 | A malformed number (decision 7) | Removing a suffix (`needs-review`) |
| E0009 | An invalid char literal: empty, more than one scalar value, or unclosed | For more than one character, use `"` (`needs-review`) |
| E0010 | An unknown literal tag | None. The message lists the tags |
| E0011 | A non-ASCII character in a bytes literal | Replace it with the `\x` escapes of its UTF-8 bytes (`safe`) |
| E0012 | A multi-line string layout error (spec question 3) | None |
| E0013 | A carriage return not followed by a line feed (spec question 1) | Replace it with a line feed (`safe`) |
| E0014 | A file that isn't valid UTF-8 (spec question 1) | None |

Consecutive invalid characters make one `Error` token and one E0001, so a pasted line of emoji is one diagnostic, not thirty. A malformed literal keeps its literal kind, as decision 7 says for numbers, so the parser and later stages report nothing more about it (`r[diag.all-errors]`).

E0014 is reported by `quince_syntax::decode_source(&[u8])`, which every caller that reads a file uses before lexing, so the error has one home.

### 10. Wiring the conformance runner

The runner calls `quince_syntax` in-process, through a `SyntaxBackend` that implements `Backend`. This settles the M0 open question for `parse`:
- `quince_syntax` has no LLVM and builds in seconds, so xtask can depend on it.
- A subprocess would also test the CLI's JSON output, but that output's format is M5's decision. `run` fixtures, which need a built program, can still go through a subprocess when M6 or M7 decide.

Until the parser lands, `SyntaxBackend::parse` returns the lexer's diagnostics only. `check` and `run` stay unwired, so nobody writes a `check` fixture that passes only because nothing checks it.

Lexer fixtures go in `tests/conformance/testdata/lex/` in `parse` mode. They are written as valid programs apart from their expected diagnostics, as the fixture README already requires, so they keep passing when the parser arrives instead of failing on syntax the lexer couldn't see.

The runner changes in two ways:
- It reads fixtures as bytes and passes them to the backend, so E0014 can have a fixture. Headers and expectations are read from the lossy UTF-8 text. `Backend::parse` and `check` take `&[u8]`.
- It runs the position half of `Fix::check`, with the source text, on every fix a backend reports.

### 11. Testing

- **Unit tests** in `quince_syntax` compare token dumps (`kind "text"` per line) with hand-written expected text, and check decoded values through `literal`. No snapshot crate.
- **Spec sync tests** (decision 6): keywords against the spec, punctuation against the grammar.
- **Corpus:** every unlabeled code block in the spec chapters and `std/API.md` lexes with no errors. Blocks that aren't Quince are skipped by a list in the test, keyed by file and first line, until spec question 5 is settled.
- **Invariants on every input:** no panic; the token texts concatenate to the input; no token is empty; every error range lies inside the input on character boundaries. These run over the corpus and over seeded mutations of it (inserted, deleted and swapped characters from a small dependency-free PRNG), bounded so `cargo test` stays under a second for this crate. A `cargo-fuzz` target needs nightly and a dependency, so it waits for the parser doc, which owns the fuzzing exit criterion.
- **Conformance fixtures:** at least one per lexer rule in Tasks, and one per diagnostic code, as `cargo xtask diagnostics` requires.

A fixture cites a rule only when it would fail if the lexer broke that rule. In `parse` mode, a fixture for an accepted form (`0b1010`, a multi-line string) checks that the form is accepted, not its value. Values are unit-tested until `run` fixtures exist in M6, which then cite the same rules.

### 12. Delivery as a stack

Each layer builds and passes `cargo xtask ci` on its own:

1. `m1/lexer-tokens`: the approved spec wording and the bump to v0.6; `SyntaxKind`, `Token`, `lex`; trivia, identifiers, keywords, punctuation and numbers; `decode_source`; `LineIndex`; `SyntaxBackend` and the runner changes; E0001 to E0003, E0008, E0013 and E0014; `compiler/syntax/README.md`.
2. `m1/lexer-literals`: strings, interpolation, multi-line, raw, bytes, tagged and char literals, `literal` decoding; E0004 to E0007 and E0009 to E0012.
3. `m1/lexer-corpus`: the corpus and mutation tests.

The spec edits all go in layer 1, so the spec changes version once. The rules they add for strings are covered when layer 2 lands.

## Spec questions (need David's approval)

Each proposed rule would go in `spec/reference/lexical.md` with the tag shown. Layer 1 needs questions 1, 4 and 6. Layer 2 needs 2, 3 and 7. Question 5 doesn't block the lexer.

**1. Source text.** The chapter doesn't say what a source file is made of. Proposed, as a new section "Source text" at the top of the chapter:
- `r[lex.source.utf8]` A source file is UTF-8 text. A file that isn't valid UTF-8 is a compile error.
- `r[lex.source.bom]` A byte order mark at the start of a file is ignored, and `quince` `fmt` removes it.
- `r[lex.source.line-end]` A line ends at a line feed, or at a carriage return followed by a line feed, and `quince` `fmt` writes line feeds. A carriage return anywhere else is a compile error.
- `r[lex.source.whitespace]` Spaces and tabs are the only whitespace between tokens.
- `r[lex.source.ascii]` Outside string literals, char literals and comments, source text is ASCII, so identifiers are ASCII letters, digits and underscores.

*Why:* agents and editors on Windows produce CR LF, so rejecting it would fail on correct code, while a lone CR is almost always damage. ASCII-only identifiers follow from the naming rules (`snake_case` has no case for `é`) and keep look-alike characters out of names.

**2. Interpolation paths.** `r[lex.lit.string.interp-path]` allows "a name or a field path", but the spec's own example (Items, `"{self.name} <{self.email}>"`) starts a path with `self`, which is a keyword, not a name. Proposed rewording:
- `r[lex.lit.string.interp-path]` stays as is in meaning, and a new `r[lex.lit.string.interp-root]` says: "A path starts with a name, `self` or a constant name, and has no spaces: `{self.email}`, `{MAX_RETRIES}`."

**3. Multi-line strings.** `r[lex.lit.string.multiline]` leaves open what may follow the opening `"""`, where the closing one may sit, and what happens to a line indented less than the closing line. Proposed, following Swift:
- `r[lex.lit.string.multiline.open]` Only a line break may follow the opening `"""`.
- `r[lex.lit.string.multiline.close]` The closing `"""` is the first thing on its line after its indentation, and code may follow it on that line (`""")`).
- `r[lex.lit.string.multiline.indent]` Every line between must start with exactly the closing line's indentation, the same spaces and tabs, except a line that holds only whitespace, which becomes empty. Otherwise it is a compile error.
- `r[lex.lit.string.multiline.value]` The value doesn't include the line break before the closing line, and every line break in it is a line feed, whatever the file uses.

**4. Number forms.** The examples don't settle the edge cases. Proposed:
- `r[lex.lit.int.underscore]` An underscore may appear only between two digits, so `1_000` is valid and `_1`, `1_`, `1__0` and `0x_FF` are not.
- `r[lex.lit.int.prefix]` The base prefixes are lowercase (`0x`, `0b`, `0o`). Hex digits may be either case, and `quince` `fmt` writes them uppercase.
- `r[lex.lit.int.leading-zero]` A decimal literal other than `0` doesn't start with `0`, so `007` is an error rather than an octal number as in C.
- `r[lex.lit.float.shape]` A float has digits on both sides of its point (`1.0`, not `1.` or `.5`), and its exponent is `e`, an optional sign and digits.
- `r[lex.lit.no-suffix]` A numeric literal has no type suffix: its type comes from context.

**5. Code block labels (wording only, no version bump).** The spec and `std/API.md` don't label their code blocks, so the M1 exit criterion "every code block parses" needs a skip list for the TOML, JSON and REPL blocks. Proposed: label every block (`quince`, `toml`, `json`, `text`), so the corpus test reads only `quince` blocks and an unlabeled block is an error. This could wait for the parser doc.

**6. Doc comments.** `r[lex.comment.doc]` doesn't say whether `////` is a doc comment. Proposed: `r[lex.comment.doc.exact]` "A doc comment starts with exactly three slashes: `////` starts an ordinary comment", as in Rust.

**7. Unicode escapes.** Proposed: `r[lex.lit.string.unicode-escape]` "`\u{...}` holds one to six hex digits naming a Unicode scalar value, so surrogates (`\u{D800}`) and values above `\u{10FFFF}` are errors."

## Risks and trade-offs

- [A multi-line string with no closing line swallows the rest of the file, so the parser reports nothing after it] → E0005 points at the opening `"""`, and fixing it restores everything. A heuristic close (the first line that looks like code) would guess wrong in strings that hold code, which is a common use. Accepted.
- [`parse` fixtures pass against a lexer-only backend even when they aren't valid syntax] → The fixture README already requires valid syntax, review checks it, and the parser layer reruns every fixture.
- [Acceptance-only fixtures mark a rule covered before its value is tested] → Decision 11: values are unit-tested now, and `run` fixtures cite the same rules from M6. The coverage report can't show the difference. Accepted until a test kind marks partial coverage.
- [Tokenized interpolation makes the lexer stateful] → One state, entered at `{` in an interpolating string and left at `}`, `"` or a line end. The invariant tests run through it.
- [The token contract turns out wrong for the parser] → The parser doc comes next and can amend this one before layer 2 merges. `SyntaxKind` is internal to `quince_syntax`, so only that crate changes.
- [Fourteen codes is a lot for a lexer] → Each names one problem an agent can fix without reading the explain text. Codes are cheap and never reused, but merging two later would retire one. Review the table before layer 1.

## Rollback

Revert the stack's PRs. Nothing outside `quince_syntax`, `quince_diagnostics`, the runner and `spec/reference/lexical.md` changes, and no later work depends on it yet.

## For the parser doc

Found while writing this. None of them block the lexer.
- `true` and `false` are keywords, and the grammar's `literal` doesn't say it includes them, but a pattern `true =>` needs it to.
- `r[tool.fmt.check]` says unformatted code compiles, so a call broken over lines without its trailing comma (`f(\n  a,\n  b\n)`) must parse. The line break before `)` can't end a statement there.
- A C function whose name is a keyword (POSIX `select`) can't be declared: `r[ffi.cbind.keyword-param]` covers parameters only, and `r[lex.no-contextual]` rules out reading `select` as a name inside `foreign`.
- A single uppercase letter (`T`, `N`) is both `Name` and `NAME`. Where the grammar accepts either (`primary`, `alt`), the parser has to pick one, or leave it to name resolution.
- `r[lex.lit.int.neg]` says the minus is "directly before" the literal. Whether `- 1` counts decides whether the parser checks adjacency.
- The coverage ratchet M0 left for M1: decide once the parser adds real coverage.

## Tasks

Rule IDs in brackets are what each task's fixtures cite. Rules added by the spec questions get their IDs once approved.

**Layer 1: `m1/lexer-tokens`**
- [ ] Spec: the approved wording from questions 1, 4 and 6, and the bump to Draft v0.6.
- [ ] `SyntaxKind`, `Token`, `Lexed`, `lex`, trivia and the lossless invariant. [`lex.indentation`, `lex.braces`]
- [ ] `decode_source`, BOM and line endings; E0013, E0014. [`lex.source.*`]
- [ ] Identifiers, `case_class`, keywords, the keyword sync test. [`lex.keywords`, `lex.no-contextual`, `lex.ident-classes`]
- [ ] Punctuation, `>` alone, the grammar sync test; E0001, E0002. [`lex.grammar-rules`, `lex.no-shift-token`]
- [ ] Comments and doc comments; E0003. [`lex.comment.line`, `lex.comment.doc`]
- [ ] Numbers, the tuple-index rule, `literal::int_value`; E0008. [`lex.lit.int.forms`, `lex.lit.float.forms`]
- [ ] `LineIndex` in `quince_diagnostics`, and `LexError` to `Diagnostic`.
- [ ] `SyntaxBackend`; the runner reads bytes and checks fix positions.
- [ ] `compiler/syntax/README.md`: the token contract (decisions 2 to 8). Update `AGENTS.md`'s map.

**Layer 2: `m1/lexer-literals`**
- [ ] String shape and escapes, `literal::string_value`; E0004, E0006. [`lex.lit.string.escapes`, `lex.lit.string.one-line`, `lex.lit.string.unicode-escape`]
- [ ] Interpolation tokens; E0007. [`lex.lit.string.interp`, `lex.lit.string.interp-path`, `lex.lit.string.interp-root`, `lex.lit.string.lone-brace`]
- [ ] Multi-line and raw multi-line strings, `StringIndent`; E0005, E0012. [`lex.lit.string.multiline`, `lex.lit.string.multiline.*`, `lex.lit.string.raw-multiline`]
- [ ] Raw strings. [`lex.lit.raw-string`]
- [ ] Bytes; E0011. [`lex.lit.bytes`]
- [ ] Tagged literals; E0010. [`lex.lit.tagged`, `lex.lit.tagged.raw`]
- [ ] Char literals; E0009. [`lex.lit.char`, `lex.lit.char.escapes`]

**Layer 3: `m1/lexer-corpus`**
- [ ] Corpus test over the spec and `std/API.md`, with the skip list or, if question 5 is approved, labels.
- [ ] Mutation tests of the invariants.
