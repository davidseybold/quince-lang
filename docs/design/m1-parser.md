# M1: Parser

Status: draft, not yet reviewed. The second design doc of M1 (Syntax), after [the lexer](m1-lexer.md). The formatter doc (`quince fmt`) follows it.

## Why

M1 ends when every `parse` and `fmt` fixture passes, `fmt` is idempotent on every fixture, a fuzzer finds no panics, and printing the CST gives back the input byte for byte ([milestones](../milestones.md)). The lexer gives a token stream. This doc covers the step from tokens to a tree: the parser, the lossless CST, and the diagnostics for syntax errors.

The tree matters more than the parser code. The formatter, `fix`, `query`, name resolution and the language server all read it, so its shape, and how trivia sits in it, gets harder to change with every consumer. The parser is also where most of the grammar is enforced (80 productions in `spec/grammar.ebnf`), along with the line-break rules, the operator rules and the naming rules. Several of those leave cases open, so this doc has spec questions.

## Impact and approvals

- **Code:** `quince_syntax` gets `parser`, `cst` and `validate` modules, and its README gets the CST contract. `SyntaxBackend::parse` runs the parser. `quince_conformance`'s coverage step gets a ratchet (decision 12).
- **Dependencies, waiting on David** (global `CLAUDE.md`: ask before adding a dependency):
  - **A. The CST.** Recommended: the `rowan` crate (decision 2). The alternative is a tree of our own, with no dependency.
  - **B. Snapshots.** Recommended: no snapshot crate, and unit tests with hand-written tree dumps, as the lexer has (decision 11). The repo plan's step 3 says "snapshot tests of the CST and of parse errors". This doc corrects that line.
  - **C. The fuzzer.** Recommended: extend the seeded mutation test to the parser, on stable Rust with no dependency, and count it as M1's fuzzer (decision 11). The alternative is a `cargo-fuzz` target, which needs nightly and `libfuzzer-sys`.
- **Spec, waiting on David:** Spec questions, below, has thirteen questions with proposed wording. Each rule lands in the layer whose code enforces it, and each such layer bumps the spec's minor version. A code whose rule waits on a question is registered only after approval.
- **Diagnostic codes:** eleven, E0016 to E0026, numbered provisionally (decision 10).
- **Delivery:** a stack of nine PRs (decision 13).

## Context

- `quince_syntax` has the lexer: `decode_source`, `lex(&Decoded) -> Lexed`, `lex_str`, `literal`, and one `u16` `SyntaxKind` enum for tokens, which the lexer doc said the parser's node kinds would join. The README holds the token contract and "Errors the lexer owns", which the parser must not report again.
- The lexer doc's decisions constrain this one directly:
  - decision 4: every line ending is a `Newline` token, and the parser decides what it means, with `SyntaxKind::continues_line()` as a helper;
  - decision 5: `>` is always alone, and the parser forms `>>` and `>=`;
  - decision 9: no error in a lossy text gets a fix.
- Its section [For the parser doc](m1-lexer.md#for-the-parser-doc) hands over eleven items. Each is a decision or a spec question here.
- `spec/grammar.ebnf` has 80 productions, and each is a rule ID, `grammar.<production>`. The chapters the parser enforces most are Items (70 rules), Expressions (74), Statements (14) and Patterns (12), and the lexical rules on line breaks, negative literals and naming.
- The conformance runner's `parse` mode runs only `SyntaxBackend::parse`, and a `parse` fixture must be valid syntax apart from its expected diagnostics. There are 38 fixtures, all under `lex/`, and they use `fn`, `let`, `test`, `const` and `impl`. `cargo xtask ci` covers 36 of 962 rules.
- No test reads the spec (decided by David, 2026-11-11). No test parses the spec's code blocks, and no test compares the parser with `spec/grammar.ebnf`.

## Goals and non-goals

**Goals:**
- A parser that turns any token stream into a CST whose text is the input, never panics, never loops, and reports each syntax error once, with an exact span and a fix where one is clear.
- A CST contract, in `compiler/syntax/README.md`, that the formatter and later stages can build on.
- A `parse` fixture for every grammar production and for each syntax rule in Tasks, and the fixtures the lexer handed over.

**Non-goals:**
- Formatting, and where `fmt` puts comments. The formatter doc decides that. This doc fixes only where trivia sits in the tree (decision 3), which the formatter doc may amend before layer 9 merges.
- A typed AST layer. Whichever stage first needs typed accessors (the formatter or M2's name resolution) adds them over the CST. This doc fixes only the node kinds.
- Deciding what a dotted name means (`a.b.c` as a package path, a field access or a type path). Name resolution does that in M2 (decision 7).
- Incremental reparsing. Whole-file parsing is fast enough until M5 shows otherwise.
- A `quince` command that prints the CST. The unit tests' dump function can back one later. The CLI's output is M5's to decide.
- Fixes that rename a declaration to the right case. A rename has to change every use, which needs name resolution (M2) and the multi-file `fix` (M5).

## Decisions

### 1. Where it lives, and the API

The parser is the `parser` module of `quince_syntax`, with a submodule for each grammar area: items, types, expressions, statements, patterns. The tree is the `cst` module. The checks that run on the tree after parsing (naming case, decision 7) are the `validate` module.

- `parse(&Decoded) -> Parse`, or `parse_str(&str)` for text that never came from bytes, as `lex` and `lex_str` are.
- `Parse` holds the root node and every syntax error, from the lexer, the parser and validation, in source order. `Parse::diagnostics` converts them through a `LineIndex`, as `Lexed::diagnostics` does.
- `LexError` becomes `SyntaxError`, since all three stages report the same shape (range, code, message, fixes). The rename is in layer 1.
- **No fixes in a lossy text.** `parse` clears every fix when `Decoded::is_lossy()` is true, once, after all three stages. So the rule covers every stage in `quince_syntax`, including fixes added later. `lex` keeps its own check for callers that lex without parsing. When M5's driver assembles a file's diagnostics from every stage, the rule moves there.

*Alternative:* a separate `quince_parser` crate. Rejected for the reason the lexer gave: the lexer, parser and formatter share `SyntaxKind` and live in one crate in the repo plan.

### 2. The CST

The tree has two layers, as rust-analyzer's does:
- An immutable green tree: each node is a kind, a text length and its children, and each token is a kind and its text. Green nodes can be shared.
- A red layer over it, built on demand, that gives each node its parent and its offset. Consumers walk the red layer.

Node kinds join tokens in `SyntaxKind`. Every token from the lexer appears in the tree in order, trivia included, so the tree's text is the input exactly. Two CST tokens are joins of lexer tokens (decision 5). No node or token is empty except the root of an empty file.

**The parser never builds the tree directly.** It emits events: start a node, finish a node, take a token, report an error. A tree builder turns the events and the lexer's tokens into the tree, placing trivia (decision 3). This is how rust-analyzer separates the two. So the parser doesn't depend on the tree implementation, and it can parse ahead for a lookahead (decision 6) without building anything.

**`rowan` or our own (David decides, question A).**
- *Recommended: `rowan`.* It's the green and red tree rust-analyzer runs on, already shaped like this decision: `u16` kinds, a builder that takes start, token and finish calls, offsets, parents, token-at-offset lookups and tree editing. The language server and `fix` (M5) need exactly those, and replacing a tree later means changing every consumer. Its cost is one dependency with a few small ones of its own. Its version is pinned when layer 1 adds it, and checked then.
- *Alternative: our own tree.* A flat, immutable arena of nodes with child ranges and precomputed offsets, about 400 lines with tests. It's enough for the parser, the formatter and M2. It can't share subtrees, has no editing API, and would need most of `rowan`'s features by M5.
- Since the parser emits events, only the builder and the tree's consumers depend on the choice. In M1 those are the tests and the formatter, so switching stays cheap until M2 starts.

*Alternative:* an AST with no trivia, plus a side table for comments. Rejected by the repo plan: `fmt`, `fix` and error recovery need trivia in place.

### 3. Trivia in the tree

The parser sees only significant tokens (decision 4). The builder places each trivia token:
- Trivia before a node's first token goes outside the node, before it, with one exception: comments directly above a declaration, a field, an enum variant or a match arm, with no blank line between them and it, go inside that node, with their line endings. So moving or deleting a declaration takes its comments with it, as rust-analyzer does.
- Trivia after a node's last token goes outside it. So a comment at the end of a line belongs to the node that holds the line, such as the block, not to the expression before it.
- A doc comment isn't trivia. The grammar names `doc_comment` as a terminal, so `DocComment` tokens are parsed as part of the declaration they document (decision 8).
- The lexer's `Error` tokens, block comments and the byte order mark are trivia, placed like comments.

The formatter doc decides where comments go in its output, and may amend these rules if it needs another attachment.

### 4. Line breaks

The parser reads a list of significant tokens: everything but `Whitespace`, `Newline`, `Comment`, `BlockComment`, `Bom`, `Error` and `Semicolon`. Each one records the break before it:
- **none**;
- **line**, when a `Newline` comes between it and the previous significant token;
- **hard**, when a `Semicolon` does. A `;` ends a statement outright (lexer README), so a hard break never continues a line.

**Where a break ends a statement.** In a line context, the next token doesn't continue the current construct when a break comes before it, unless:
- the previous token's kind `continues_line()`;
- the previous token is a `>` or `*` the parser took as a binary operator. Those two depend on their role (lexer decision 4), and the parser knows the role when it takes them;
- the next token is `.` (`r[lex.continue.more]`);
- the parser is in a `fn` header, and the next token is `->`, `uses`, `requires`, `ensures` or `{` (`r[lex.continue.signature]`; spec question 12 asks whether closures and function types count).

The parser checks that one function, `at_break()`, wherever a construct may end or may go on: before a binary operator, a postfix operator, `else`, an optional part of a signature, and the operand of `return`. So `let x = a` then a line `- b` is two statements, and `return` on a line of its own is a bare return.

**Line contexts and list contexts** (spec question 3). Breaks matter only in line contexts: the file, a block, and the bodies of `impl`, `interface`, `effect` and `foreign`. Inside parentheses, square brackets, type arguments and generics, and the braces of a comma-separated list (fields, variants, map entries, import names, match and select arms, a `concurrent` spawn list), a break is spacing. A block inside a list is a line context again. So `f(\n  a,\n  b\n)` parses without its trailing comma, which `r[tool.fmt.check]` requires, since unformatted code compiles.

**A break that ends a statement too early** is one error, E0018, not two. When the next line starts with a token that can't start a statement but would continue the construct the break ended (`=`, a compound assignment, `=>`, a binary operator that can't also start an expression, `else` or `?`), the parser reports E0018 at the break and parses on as though the lines were joined. A line that starts with `- b` or `(x)` isn't one of these: it can start a statement, so it is one, as `r[lex.line-end]` says. Its fix moves the break after that token (`safe`). This covers `r[lex.else-same-line]`, whose spec text asks for exactly that fix, and slips like `let x` then a line `= 1`. A hard break never joins: `a;` then a line `.b()` is an E0002 and an error at `.`, which no statement starts with. Deleting the `;` fixes both, which is why E0002's fix is `needs-review` there.

*Alternative:* Go's rule everywhere, so a line break before `)` ends the statement and `f(a\n)` is an error. Rejected: `r[tool.fmt.check]` says unformatted code compiles, and only the formatter adds trailing commas.

### 5. Tokens the parser joins

- `>>` and `>=` are formed from a `>` and the next token when no trivia sits between them (lexer decision 5). The parser checks adjacency, and the builder writes them to the tree as one token, `GtGt` or `GtEq`, holding both texts. They are CST-only kinds: the lexer never produces them, which keeps `r[lex.no-shift-token]`. `a > = b` is an "expected expression" error, as `a = = b` is.
- **A negative literal** (`r[lex.lit.int.neg]`, spec question 5) is a `-` followed with no trivia by an `Int` or `Float`, where a prefix operator may go. The builder puts both tokens in one `Literal` node, so `-9_223_372_036_854_775_808` is one literal the type checker can range-check, and `- 1` is a prefix minus applied to `1`. In patterns, `[ "-" ] literal` follows the same rule.

### 6. Expressions

Expressions are parsed by precedence climbing over binding powers. The parser accepts any mix of operators, builds the tree the usual reading gives, then reports what the spec forbids, with the fix that adds the parentheses for that reading (`r[expr.op.parens-required]`). Accepting first keeps one slip to one error, and the tree is still useful to the formatter and the language server.

**The usual reading** is Quince's seven levels (`r[expr.op.levels]`) plus, for the operators with no level, the places the spec's own examples put them:
- tightest first: postfix, prefix, `*` `/` `%`, `+` `-`, `<<` `>>`, `&`, `^`, `|`, `..` `..=`, comparisons and `is`, `&&`, `||`;
- so `flags & MASK == 0` reads `(flags & MASK) == 0`, `1 << n - 1` reads `1 << (n - 1)`, `a || b && c` reads `a || (b && c)`, and `r == 0..3` reads `r == (0..3)`, as the spec's examples write them.

What it reports:
- E0019, **parentheses needed**: `&&` mixed with `||` (`r[expr.op.logic.no-mix]`); a bit operator as the operand of any other operator, or mixed with another bit operator (`r[expr.op.bits.parens]`, `r[expr.op.bits.chain]`); a range as an operand (`r[expr.op.range.parens]`). The fix adds the parentheses for the usual reading (`needs-review`, since the author may have meant the other one).
- E0020, **doesn't chain**: a chained comparison (`r[expr.op.compare.no-chain]`), whose fix is `a < b && b < c` (`needs-review`: it writes `b` twice, so a `b` with effects would run twice), and a chained range (`r[expr.op.range.no-chain]`), with no fix.
- `jump` (`return`, `break`, `continue`, `spawn`) is an expression only at the top of an expression, so `a + return` is an E0016. `return` and `spawn` take everything to their right (`r[expr.op.keyword-forms]`).

**When `<` opens type arguments** (spec question 6). After a name or a path in an expression, the parser parses ahead to see whether type arguments closed by `>` follow. If they do, `<` opens them; otherwise it's less-than. Parsing ahead runs the type-argument parser with its events discarded, and remembers the answer per token, so the whole file stays linear. Case helps: a type starts with a `PascalCase` name, `Self`, `(`, `fn` or a package name followed by `.` and a `PascalCase` name, so `a < b` fails at `b` at once.

**`{` where an expression is expected** starts a map literal (`r[expr.block.brace-map]`). `{}` there is E0025, whose fix writes `{:}` (`needs-review`: it may have been meant as an empty list).

**Calls and struct literals** have one shape, `Call`, whatever is called. `User(id: 1)`, `UserId(1)`, `Some(x)` and `f(x)` all parse as a callee and an argument list, because only name resolution can tell a struct from a function. A `..base` argument is allowed only first (`r[item.struct.update]`).

**Assignment** is parsed as an expression followed by `=` or a compound assignment, then checked: the left side must be a place, a `var` binding name or `self` followed by field names and indexes (`grammar.place`). Anything else is E0026. `x = y = z` and `if x = 1` are E0016 at the second `=`, since assignment is a statement (`r[stmt.assign.no-chain]`).

### 7. Names and case

The CST has one `Name` node for a name being declared and one `NameRef` for a name being used, whatever its case. Case changes the tree's shape in only three places, where the grammar uses it:
- **patterns**: a `snake_case` name binds, a `SCREAMING_SNAKE_CASE` one is a constant, and a `PascalCase` one starts a variant or struct pattern (`r[pat.const]`, `r[pat.const-exhaustive]`);
- **type paths**: `[ name "." ] Name { "." Name }`, so the parser knows where a type ends;
- **type arguments in expressions** (decision 6).

Everywhere else, a dotted chain is a `NameRef` followed by field steps, and name resolution (M2) decides whether `a.b` is a package path, a field, or a type with a constant. That keeps case out of the expression parser, and lets code use a foreign name such as `SQLITE_OK` or `SDL_Init` wherever a name can go (spec question 1).

**The case check** (`r[lex.name.case]`, `r[lex.name.enforced]`) runs in `validate` over the finished tree, only where a name is declared: functions, parameters, bindings in `let`, `var`, `for` and patterns, fields, types, variants, generics, effects, constants, import aliases and the `ensures` binder. A use can't be wrong unless its declaration is, and the declaration gets the error. A name of the wrong class is E0021, which says the class the position takes and gives the name converted to it. No fix in M1 (see Non-goals). Names declared in a `foreign` block are skipped (`r[lex.name.foreign-exempt]`). Spec question 1 defines the classes.

A keyword where a name goes is an E0016 that names the keyword (`r[lex.keyword-not-name]`, `r[lex.ident-not-keyword]`). The lexer gives every keyword its own kind and none is contextual (`r[lex.no-contextual]`), so the parser never has to guess. A C function whose name is a keyword is spec question 4.

### 8. Declarations, doc comments and attributes

- A declaration is its doc comments, its attributes, `pub`, and the item. The doc comments are tokens in a `DocComments` node at its start, and the attributes are `Attribute` nodes (`grammar.decl`). Fields, variants and variant fields take the same (`grammar.field`, `grammar.evariant`, `grammar.vfield`), and so do impl items, interface methods, effect operations and foreign items if spec question 9 is approved.
- A doc comment that comes before nothing it can document is E0023, with a fix that makes it an ordinary comment (`safe`), if spec question 10 is approved.
- An attribute goes on its own line (`r[item.attr.placement]`). One on the line of its declaration is an E0016 whose fix inserts a line break (`safe`). Whether the attribute's name is known (`r[item.attr.unknown]`) is checked by a later stage, since the set depends on the edition.
- Imports come before declarations (`grammar.file`). An import after a declaration is an E0016.
- The parser accepts what the grammar accepts, and leaves the rest to later stages: `pub` on an item that can't be public, `@test` on a method, `pub` on a variant field (`r[item.enum.field-visibility]`), or a `const` in an interface (`r[item.const.assoc.no-interface]`). Each of those needs a code and a fixture in the stage that checks it.

### 9. Error recovery

Every error goes through one rule: report, then skip to a sync point, with nothing more reported until the parser gets there. The tokens skipped go in an `Error` node.
- **Sync points**, innermost first: the next break in a line context; the closing bracket of the current list, or its next `,`; and the next token at the start of a line that only starts a declaration: `fn` followed by a name, `struct`, `enum`, `type`, `interface`, `impl`, `effect`, `foreign`, `import`, `const`, `test`, `pub`, `@` or `///`. A block can't hold a declaration (`r[expr.block.positions]`), so a declaration keyword at the start of a line inside an unclosed block closes the block. Then a missing `}` gives one E0017 at its `{`, not an error for every line after it.
- **Brackets** are matched as the parser goes. An opener with no closer is E0017 at the opener. A stray closer is an E0016.
- **E0016** is the general syntax error: "expected X, found Y", where X lists what may go there in Quince terms ("an expression", "`,` or `)`", "a line break"). Two expressions on one line, such as adjacent literals (`"press "b" now"`, lexer doc), are an E0016 that expects a line break, with no hint for that shape (lexer decision 9).
- **Errors the lexer already reported** aren't reported again (lexer README). Beyond that contract:
  - An error the parser would report right after one of the lexer's `Error` tokens, with only spaces between, is dropped. So `let x = $` gives the lexer's E0001 alone, not E0001 and "expected expression". One slip, one error.
  - When a multi-line string or a block comment runs to the end of the file unclosed (E0005, E0003), the parser reports nothing at the end of the file, since everything it would report (missing `)` or `}`) is that slip.
- **Loop safety.** Every parser loop takes a token or exits. A debug-build counter panics when the parser looks ahead 256 times without taking a token, so the mutation tests (decision 11) catch a loop as a panic.

### 10. Diagnostics

Codes continue from E0016 in the order they are registered, as the lexer's do. They are provisional until each one's layer merges, and a code whose rule waits on a spec question is registered only after approval. Messages describe Quince, the same way for every input (lexer decision 9). In particular the operators Quince leaves out (`r[expr.op.excluded]`: `++`, `--`, `**`, `??`, `?.`, the ternary and `as` casts) get no code or hint of their own: each is an E0016 that says what may go there.

| Code | Problem | Fix | Waits on |
| --- | --- | --- | --- |
| E0016 | A token that can't go here: "expected X, found Y" | For an attribute on its declaration's line, insert a line break (`safe`). Otherwise none | |
| E0017 | An opening bracket with no closing one, at the opener | None | |
| E0018 | A line break that ends a statement too early, including before `else` (decision 4) | Move the break after the token that starts the next line (`safe`) | Q3, Q12 |
| E0019 | Operators that need parentheses (decision 6) | Add the parentheses for the usual reading (`needs-review`) | |
| E0020 | A chained comparison or range | For comparisons, `a < b && b < c` (`needs-review`) | |
| E0021 | A name of the wrong case where it's declared | None in M1 | Q1 |
| E0022 | Nesting deeper than the limit (below) | None | |
| E0023 | A doc comment that documents nothing | Make it `//` (`safe`) | Q10 |
| E0024 | A struct with no fields written with braces | Remove the braces (`safe`) | Q11 |
| E0025 | `{}` where an expression is expected | `{:}` (`needs-review`) | |
| E0026 | An assignment whose left side isn't a place | None | Q13 |

**The nesting limit.** The parser is recursive, so deep enough nesting would overflow the stack, which the fuzzer would find. Nesting deeper than 256 brackets, blocks or prefix operators is E0022 at the token that goes past it, and the parser skips to the end of that construct. 256 runs well inside a 2 MiB test thread, which a unit test checks. It's an implementation limit, so it goes in the README, not the spec. No hand-written program comes near it.

### 11. Testing

- **Unit tests** in `quince_syntax` parse a text and compare a tree dump with hand-written expected text: one line per node or token, indented by depth, with kind, range and token text, trivia included. They check the trees `parse` fixtures can't see: precedence, where a line ends, where trivia sits, the joined tokens and negative literals. No snapshot crate, and no command that writes expected text from the parser's output, so each expectation is written before the code, as the core loop asks (David decides, question B).
- **Conformance fixtures** go in `tests/conformance/testdata/parse/`, a folder per grammar area, in `parse` mode. Every grammar production is cited by at least one fixture (`grammar.<production>`, the repo plan's "one conformance test per grammar production"), every code has one, and so does each rule in Tasks.
- **Coverage honesty.** A fixture cites a rule only when a parser that broke it would fail the fixture, as the lexer doc requires. A `parse` fixture sees only diagnostics, not the tree. So:
  - rules that forbid something are cited by fixtures that expect the error;
  - accepted forms are cited by fixtures that a parser rejecting them would fail;
  - rules about the tree's shape (precedence, grouping, where a statement ends) are unit-tested, and cited by fixtures only where a wrong tree gives a different diagnostic. `run` fixtures in M6 cite them again.
- **The lexer's handed-over rules** get their fixtures here:
  - `lex.keywords` and `lex.ident-not-keyword`: each keyword where a name goes is an E0016;
  - `lex.no-contextual`: `main`, `limit` and `TaskScope` as ordinary names;
  - `lex.no-shift-token` and `lex.grammar-rules`: `List<List<Int>>` closes, `>>` shifts, `..=` and `|` in a pattern;
  - `lex.ident-classes`: names of each class in their positions;
  - `lex.indentation`: the same program with odd indentation parses with no error;
  - `lex.comment.doc` and `lex.comment.doc-exact`: a doc comment on a declaration, and `////` anywhere.
- **Invariants on every input:** no panic, no loop (decision 9), the tree's text is the input, every lexer token is in the tree in order, and every error range lies inside the input on character boundaries.
- **The fuzzer** (David decides, question C). Recommended: extend `compiler/syntax/src/tests/mutations.rs` to parse every mutant, and add token-level mutations (delete, duplicate or swap a token, or insert the text of a random token kind), which reach parser states that byte mutations rarely do. The mutations run over every fixture, `lex/` and `parse/`, and check the invariants. This is a mutation fuzzer, on stable Rust with no dependency, in CI on every change, and it's what meets the exit criterion. A unit test also parses 300 nested brackets and the like on a 2 MiB thread. *Alternative:* a `cargo-fuzz` target, which finds more over long runs but needs nightly and `libfuzzer-sys`, so it would run outside `cargo xtask ci`. It can be added later without changing anything here.
- **Time.** The mutation tests should stay under a second in debug. Their count per fixture is set to fit.

### 12. The coverage ratchet

M0 left this to M1, and the parser is the first real coverage. `tests/conformance/covered.txt` lists every covered rule ID, one per line, sorted. `cargo xtask conformance` fails when the list and the run disagree in either direction:
- a rule in the list that no passing fixture covers means coverage fell;
- a covered rule missing from the list means the list is stale.

`cargo xtask conformance --write` rewrites the list. Each PR's diff then shows the rules it covers or uncovers, which review can check against its fixtures. That's the same pattern as `spec-grammar --write`.

*Alternative:* a number that may only go up. Rejected: a PR could uncover one rule and cover another, and the number wouldn't show it.

### 13. Delivery as a stack

Each layer builds and passes `cargo xtask ci` on its own. The parser is wired into `SyntaxBackend` in layer 1, so from then on every fixture, `lex/` included, must parse. Layer 1 therefore covers everything the 38 `lex/` fixtures use, and fixes any of them that turns out not to be valid syntax.

1. `m1/parser-core`: `SyntaxError`, events, the tree builder and the CST (question A); `parse`, `Parse::diagnostics` and the lossy rule; trivia placement; `at_break()`; recovery and E0016, E0017; `fn`, `test` and `const` declarations, blocks, `let`, literals and calls, enough for the `lex/` fixtures; the README's CST contract; `SyntaxBackend::parse` runs the parser.
2. `m1/parser-types`: types, type paths, generics, bounds, effects; the joined `>>` and `>=`; struct, enum, newtype, alias, import and attribute declarations; doc comments.
3. `m1/parser-exprs`: every operator, precedence, E0019, E0020; type arguments in expressions; negative literals; postfix forms, lists, maps, tuples and E0025.
4. `m1/parser-stmts`: statements, assignment and E0026, line contexts and list contexts, E0018, `if` and `else`.
5. `m1/parser-patterns`: patterns, `match`, `is`, `for`.
6. `m1/parser-keyword-forms`: closures, `concurrent`, `background`, `select`, `with`, `spawn`, contracts.
7. `m1/parser-items`: `impl`, `interface`, `effect`, `foreign`; spec questions 4 and 9; E0023, E0024.
8. `m1/parser-names`: `validate`, the case check, E0021; spec question 1.
9. `m1/parser-fuzz`: the token mutations, the nesting limit and E0022, and the coverage ratchet.

Each spec question lands in the layer that enforces it, with a minor version bump. Layers should come in at 500 to 900 changed lines. Layer 1 is the riskiest for size, since the tree, the builder and recovery all arrive with it. If its estimate goes past 1,000 lines, the CST and builder split out first, tested on hand-built events, with the parser wired in the layer above.

## Spec questions

Questions 1 to 5 come from the lexer doc's handover. The rest came up while reading the grammar. Each gives proposed wording and the tag it would have.

**1. Naming classes (layer 8).** `r[lex.name.case]` names the styles but doesn't define them, and `r[lex.name.enforced]` makes the answer a compile error. `T` and `URL` fit two classes. Proposed, in Lexical structure's Naming section:
- `r[lex.name.snake]` "A `snake_case` name is lowercase letters, digits and single underscores between them, and starts with a letter: `x2`, `utf8_len`. `_unused`, `trailing_` and `a__b` aren't names."
- `r[lex.name.pascal]` "A `PascalCase` name starts with an uppercase letter and has no underscores. A single uppercase letter, such as `T`, is `PascalCase`. Two capitals may follow each other, as in `HTTPServer`, but a name of two or more letters with no lowercase letter is `SCREAMING_SNAKE_CASE`, so a type is `Url`, not `URL`."
- `r[lex.name.screaming]` "A `SCREAMING_SNAKE_CASE` name is uppercase letters, digits and single underscores between them, starts with a letter, and has at least two letters: `MAX_RETRIES`, `HTTP2`."
- `r[lex.name.declared]` "The case is checked where a name is declared. A name declared in a foreign block can be used anywhere a name can, in its C spelling."
- *Why:* every identifier then falls into at most one class, so the parser's pattern rule (decision 7) has one reading. Ruling out `_unused` keeps one way to write an unused name, `_` (`r[expr.closure.underscore]`). Allowing `HTTPServer` follows Rust, Go and Swift, and checking acronyms would need a word list.
- *Alternative:* `T` as `SCREAMING_SNAKE_CASE`. Rejected: generic parameters are types.
- The grammar's `primary` and `alt` list `name` and `NAME` but no `c_name`, so today a foreign function such as `SDL_Init` can be declared but not called. The last bullet fixes that in prose. Changing `primary` itself is optional, and the grammar's comment can say it.

**2. Booleans (layer 1).** `true` and `false` are keywords, and the grammar's `literal` doesn't say it includes them, but a pattern `true =>` needs it to. Proposed: `r[lex.lit.bool]` "`true` and `false` are the literals of `Bool`", in Literals, so `literal` covers them.

**3. Line breaks inside brackets (layer 4).** `r[lex.line-end]` is Go's rule, but `r[tool.fmt.check]` says unformatted code compiles, so `f(\n  a,\n  b\n)` without its trailing comma must parse. The spec doesn't say what a line break means where no statement can end. Proposed, after `r[lex.continue.signature]`:
- `r[lex.break.lists]` "Inside parentheses, square brackets, angle brackets and the braces of a comma-separated list, such as fields, variants, map entries and match arms, a line break is only spacing. A block inside them is a line context again, where the rules above apply."
- *Alternative:* Go's rule, which makes `f(a\n)` an error. Rejected because of `r[tool.fmt.check]`.

**4. C names that are keywords (layer 7).** A C function named `select` can't be declared: `r[ffi.cbind.keyword-param]` covers parameters only, and `r[lex.no-contextual]` rules out reading `select` as a name inside `foreign`. Proposed: `r[ffi.cbind.keyword-name]` "A C function or type whose name is a Quince keyword can't be declared in v1. Wrap it in a C function with another name, built from `[native] sources`." This is the reversible choice: a renaming attribute can come later without breaking anything, and M12's `quince bind` doc can take it up.

**5. Negative literals (layer 3).** `r[lex.lit.int.neg]` says the minus is "directly before" the literal. Proposed: `r[lex.lit.neg-adjacent]` "Directly before means with nothing between them, not even a space: `- 1` is the prefix minus applied to `1`." So `Int.MIN` is written `-9_223_372_036_854_775_808`, and the formatter writes no space after a prefix minus anyway.

**6. Type arguments in expressions (layer 3).** `r[expr.op.compare.no-chain]` says `json.decode<Order>(body)` "has only one reading", but not how the parser finds it. In `[a < B, C > d]`, both readings are valid syntax. Proposed: `r[expr.type-args.parse]` "In an expression, `<` after a name or path starts type arguments when what follows parses as type arguments closed by `>`. Otherwise it is less-than. So a list of comparisons such as `[a < B, C > d]` needs parentheses." In that example the false reading is a syntax error, since nothing can follow `a<B, C>` but `d`. A silent change needs comparisons of `PascalCase` values whose second operand is parenthesized, as in `[a < B, C > (d)]`, and even then the call to `a` must type-check.

**7. The `ensures` binder (layer 6).** `contract = "requires" expr | "ensures" [ "(" name ")" ] expr` can read `ensures (x) - 1 > 0` as a condition on `x`, or as naming the result `x` with the condition `-1 > 0`. Proposed: `r[contract.ensures-binder]` "`ensures` followed by a name in parentheses always names the return value. A condition that starts with a parenthesized name is written without the parentheses."

**8. Effects in a list (layer 2).** `fn() uses Io, Db` inside a comma-separated list, as in `Map<String, fn() uses Io, Db>`, can end its effects at `Io` or at `Db`. Proposed: `r[effect.list-greedy]` "A `uses` list takes every effect after it that is separated by commas, so a function type with effects inside a list of types comes last, or is named with a type alias." A wrong reading is a type error, not a silent change, since a type in the effect list isn't an effect.

**9. Doc comments and attributes on members (layer 7).** `impl_item`, the members of `interface_decl` and `effect_decl`, and the items of `foreign_decl` take no doc comments or attributes in the grammar. But `@deprecated` applies to "any public declaration", `@unchecked` and `@verify` to functions, and the spec's foreign example puts `@cleanup` and `@borrowed` on foreign items (the grammar does allow attributes there). Proposed: each of the four takes `{ doc_comment } { attribute NEWLINE }` before it, as `decl` does. For the grammar:
```
impl_item    = { doc_comment } { attribute NEWLINE } ( [ "pub" ] fn_decl | "type" Name "=" type | [ "pub" ] const_decl )
interface_decl = "interface" Name [ generics ] "{" { { doc_comment } { attribute NEWLINE } ( "type" Name | fn_sig { contract } ) NEWLINE } "}"
```
and the same for `effect_decl`'s `fn_sig` and `foreign_decl`'s items.

**10. A doc comment that documents nothing (layer 7).** `r[lex.comment.doc]` says doc comments attach to the next declaration, but not what happens when there's none, as with one inside a function body or at the end of a file. Proposed: `r[lex.comment.doc-placement]` "A doc comment must come before a declaration, a field, a variant or a member. Anywhere else it is a compile error, whose fix makes it an ordinary comment." So documentation can't be lost quietly.

**11. `struct S {}` (layer 2).** `grammar.struct_decl` accepts empty braces, but `r[item.struct.unit]` says a struct with no fields "is declared without braces". Proposed: reword `r[item.struct.unit]`'s first clause to "A struct with no fields is declared without braces, and writing `{}` is a compile error whose fix removes them", and change the grammar to `[ "{" field { "," field } [ "," ] "}" [ "requires" expr ] ]`. That's one way to write it. A fieldless struct then can't have a `requires`, which has nothing to constrain.

**12. Line breaks in closure headers and function types (layer 4).** `r[lex.continue.signature]` says "inside a `fn` signature". Proposed: reword it to "inside any `fn` header: a declaration's signature, a closure's, or a function type". So `fn(x: Int)` then a line `-> Int {` works the same in a closure as in a declaration, and there is one rule.

**13. Tuple positions as places (layer 4).** `grammar.place` allows field names and indexes, but not tuple positions, so `t.0 = 1` on a `var` tuple is a syntax error, while `r[stmt.assign.place]` says "a field path from one". Proposed: either add `"." int` to `place`, if a tuple position counts as a field, or add "A tuple position isn't a place: build a new tuple" to `r[stmt.assign.place]`. No preference beyond saying which. Allowing it is the reversible direction, since forbidding it later breaks code.

## Risks and trade-offs

- [The tree's shape is wrong for the formatter or name resolution] → Both arrive within M1 and M2, while the only consumers are tests. The formatter doc comes next and can amend this doc, and the events keep the tree implementation behind the builder. Node kinds are internal to `quince_syntax`.
- [Parse fixtures can't see the tree, so a parser that builds the wrong tree passes them] → Decision 11: tree shapes are unit-tested with hand-written dumps, and `run` fixtures in M6 cite the same rules. The coverage report can't show the difference, as the lexer doc already accepted.
- [Line contexts and list contexts (spec question 3) differ from Go's rule, which the spec cites] → The proposal is Python's, and it only accepts more. If David prefers Go's rule, only `at_break()` changes, and `r[tool.fmt.check]` needs rewording.
- [Recovery reports too much or too little] → One rule (decision 9) with sync points from the grammar, not per-error heuristics. If a case needs a second patch, the memory `rule-over-heuristic` applies: propose a stricter spec rule instead.
- [Parsing ahead for type arguments is slow on pathological input] → The answer is remembered per token (decision 6), so each token is examined a bounded number of times.
- [The nesting limit rejects a valid program] → 256 levels is far beyond hand-written or generated code. The limit is in the README, and raising it changes nothing else.
- [Eleven codes, two waiting on spec questions] → Each names one problem an agent can fix from the message. Review the table before layer 1.
- [Layer 1 is large] → Decision 13 gives the split if its estimate goes past 1,000 lines.
- [`rowan` changes or is abandoned] → It's rust-analyzer's own tree, and the events keep the parser independent of it.

## Rollback

Revert the stack's PRs. Nothing outside `quince_syntax`, the conformance runner and its fixtures, `AGENTS.md`, `docs/repo-plan.md` and `spec/` changes, and nothing depends on the parser until the formatter lands.

## For the formatter doc

- Where comments go in the output. Decision 3 fixes only where they sit in the tree.
- `lex.source.canonical` (lexer doc), `tool.fmt.*`, and `item.arg.fmt-positional` and `tool.fmt.field-shorthand`, which rewrite code.
- `fmt` must refuse a lossy text (`Decoded::is_lossy()`) and a file with parse errors, or say what it does with them.
- Writing a prefix minus with no space after it, which spec question 5 relies on.
- The `fmt` fixture mode in the fixture README.

## Tasks

Rule IDs in brackets are what each task's fixtures cite. Grammar productions are cited as `grammar.<production>`. Rules added by spec questions are cited only once approved.

**Before building**
- [ ] Design review passes.
- [ ] David decides questions A, B and C, and spec questions 1 to 13.
- [ ] Correct the repo plan's step 3 to match decision 11.

**Layer 1: `m1/parser-core`**
- [ ] `SyntaxError` replaces `LexError`; `parse`, `parse_str`, `Parse::diagnostics`; fixes cleared in a lossy text.
- [ ] Events, the tree builder and the CST; trivia placement (decision 3); the unit tests' tree dump.
- [ ] Significant tokens and breaks; `at_break()` for line contexts.
- [ ] Recovery, E0016, E0017; nothing reported after an `Error` token or at the end of a file an unclosed string or comment swallowed.
- [ ] `fn` and `test` declarations with plain parameters and types, `const`, blocks, `let`, literals including booleans (question 2), names, calls. [`grammar.file`, `grammar.fn_decl`, `grammar.block`, `lex.no-contextual`, `lex.keywords`, `lex.indentation`, `expr.block.last-newline`]
- [ ] `SyntaxBackend::parse` runs the parser, and every `lex/` fixture still passes.
- [ ] `compiler/syntax/README.md`: the CST contract. `AGENTS.md`'s map.

**Layer 2: `m1/parser-types`**
- [ ] Types, type paths, type arguments, generics, bounds, effect lists (question 8); joined `>>` and `>=`. [`grammar.type`, `grammar.type_args`, `grammar.generics`, `grammar.bounds`, `grammar.effects`, `lex.no-shift-token`]
- [ ] Struct, enum, newtype, alias and import declarations; attributes; doc comments; E0024 (question 11). [`grammar.struct_decl`, `grammar.enum_decl`, `grammar.import`, `grammar.attribute`, `item.attr.placement`, `lex.comment.doc`, `lex.comment.doc-exact`]

**Layer 3: `m1/parser-exprs`**
- [ ] Every operator and level; E0019, E0020. [`expr.op.levels`, `expr.op.parens-required`, `expr.op.logic.no-mix`, `expr.op.bits.parens`, `expr.op.bits.chain`, `expr.op.compare.no-chain`, `expr.op.range.no-chain`, `expr.op.range.parens`, `expr.op.excluded`, `lex.grammar-rules`]
- [ ] Type arguments in expressions (question 6); negative literals (question 5). [`lex.lit.int.neg`]
- [ ] Postfix forms, tuples, lists, maps, `{}` and E0025, `..base`. [`lex.lit.collections`, `lex.lit.empty-map`, `expr.block.brace-map`, `item.struct.update`]

**Layer 4: `m1/parser-stmts`**
- [ ] Questions 3, 12 and 13, and a bump.
- [ ] Statements, places, E0026. [`grammar.stmt`, `grammar.place`, `stmt.assign.stmt`, `stmt.assign.no-chain`, `stmt.loop.forms`]
- [ ] Line and list contexts, E0018, `if` and `else`. [`lex.line-end`, `lex.continue.more`, `lex.continue.signature`, `lex.else-same-line`, `expr.block.no-bare`]

**Layer 5: `m1/parser-patterns`**
- [ ] Patterns, `match`, guards, `is`, `for`. [`grammar.pattern`, `grammar.match_expr`, `expr.match.arm-syntax`, `expr.match.arm-comma`, `pat.negative`, `pat.const`, `expr.range.pattern`]

**Layer 6: `m1/parser-keyword-forms`**
- [ ] Question 7, and a bump.
- [ ] Closures, `concurrent`, `background`, `select`, `with`, `spawn`, `return`, contracts. [`grammar.closure`, `grammar.concurrent`, `grammar.background`, `grammar.select`, `grammar.with_expr`, `grammar.jump`, `grammar.contract`, `expr.op.keyword-forms`]

**Layer 7: `m1/parser-items`**
- [ ] Questions 4, 9 and 10, and a bump.
- [ ] `impl`, `interface`, `effect`, `foreign`; E0023. [`grammar.impl_decl`, `grammar.interface_decl`, `grammar.effect_decl`, `grammar.foreign_decl`]

**Layer 8: `m1/parser-names`**
- [ ] Question 1, and a bump.
- [ ] `validate` and the case check; E0021. [`lex.name.case`, `lex.name.enforced`, `lex.name.foreign-exempt`, `lex.ident-classes`, `lex.keyword-not-name`, `lex.ident-not-keyword`]

**Layer 9: `m1/parser-fuzz`**
- [ ] Token-level mutations over every fixture; the invariants; the deep-nesting test.
- [ ] The nesting limit; E0022.
- [ ] The coverage ratchet and `cargo xtask conformance --write`.
- [ ] Every grammar production cited by a passing fixture.
