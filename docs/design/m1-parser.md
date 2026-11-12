# M1: Parser

Status: passed design review (six passes, the last two over the decisions recorded after approval). On 2026-11-11 David decided the three dependency questions, approved spec questions 2 to 12 as proposed, chose parentheses for question 14, allowed tuple positions as places for question 13, and approved question 1 as proposed. Every question is decided. This is the second design doc of M1 (Syntax), after [the lexer](m1-lexer.md). The formatter doc (`quince fmt`) follows it.

## Why

M1 ends when every `parse` and `fmt` fixture passes, `fmt` is idempotent on every fixture, every crash the parser's fuzz target has found is fixed with a regression test, and printing the CST gives back the input byte for byte ([milestones](../milestones.md)). The lexer gives a token stream. This doc covers the step from tokens to a tree: the parser, the lossless CST, and the diagnostics for syntax errors.

The tree matters more than the parser code. The formatter, `fix`, `query`, name resolution and the language server all read it, so its shape, and how trivia sits in it, gets harder to change with every consumer. The parser is also where most of the grammar is enforced (80 productions in `spec/grammar.ebnf`), along with the line-break rules, the operator rules and the naming rules. Several of those leave cases open, so this doc has spec questions.

## Impact and approvals

- **Code:** `quince_syntax` gets `parser`, `cst` and `validate` modules, and its README gets the CST contract. `SyntaxBackend::parse` runs the parser. `cargo xtask conformance` gets a coverage ratchet (decision 12).
- **Dependencies (decided by David, 2026-11-11):**
  - **A. The CST:** the `rowan` crate (decision 2). Layer 0 pinned `=0.17.0` (decided by David, 2026-11-11): the newest stable release, on the line rust-analyzer follows. The 0.16 line this doc was reviewed against ended in a yanked release, and 0.18 is a prerelease that moves trivia onto tokens. 0.17 depends on `countme`, `hashbrown`, `memoffset` (which builds with `autocfg`), `rustc-hash` and `text-size`, declares no minimum Rust version, and builds on 1.99.1.
  - **B. Snapshots:** none. Unit tests compare hand-written tree dumps (decision 11). Layer 0 corrects the repo plan's step 3, which asks for snapshot tests.
  - **C. The fuzzer:** a `cargo-fuzz` target, outside the workspace and outside CI (decision 11). It adds `libfuzzer-sys` to the fuzz crate only, and needs nightly Rust wherever it runs.
- **Spec (decided by David, 2026-11-11):** Spec questions, below, has fourteen questions, each with its approved wording. Each rule lands in the layer whose code enforces it, and each such layer bumps the spec's minor version.
- **Diagnostic codes:** eleven, E0016 to E0026, numbered provisionally (decision 10).
- **Delivery:** a stack of ten PRs (decision 13).

## Context

- `quince_syntax` has the lexer: `decode_source`, `lex(&Decoded) -> Lexed`, `lex_str`, `literal`, and one `u16` `SyntaxKind` enum for tokens. The lexer doc said the parser's node kinds would join that enum. The README holds the token contract and "Errors the lexer owns", which the parser must not report again.
- The lexer doc's decisions constrain this one directly:
  - decision 4: every line ending is a `Newline` token, and the parser decides what it means, with `SyntaxKind::continues_line()` as a helper;
  - decision 5: `>` is always alone, and the parser forms `>>` and `>=`;
  - decision 9: no error in a lossy text gets a fix.
- Its section [For the parser doc](m1-lexer.md#for-the-parser-doc) hands over eleven items. Each is a decision or a spec question here.
- `spec/grammar.ebnf` has 80 productions, and each is a rule ID, `grammar.<production>`. The chapters the parser enforces most are Items (70 rules), Expressions (74), Statements (14) and Patterns (12), plus the lexical rules on line breaks, negative literals and naming.
- The conformance runner's `parse` mode runs only `SyntaxBackend::parse`, and a `parse` fixture must be valid syntax apart from its expected diagnostics. There are 38 fixtures, all under `lex/`. Beyond `let` and literals they use:
  - `fn` with parameters and return types that take type arguments (`-> List<Bytes>`);
  - `test`, and `const` with a type;
  - `impl` with a method that takes `self`;
  - type annotations (`let a: UInt8`);
  - list literals, ranges (`1..10`) and method calls (`3.14159.to_fixed(2)`);
  - a `///` inside a function body (`lex/control.qnc`).
- `cargo xtask ci` covers 36 of 962 rules.
- No test reads the spec (decided by David, 2026-11-11). No test parses the spec's code blocks, and none compares the parser with `spec/grammar.ebnf`.

## Goals and non-goals

**Goals:**
- A parser that turns any token stream into a CST whose text is the input. It never panics or loops, and it reports each syntax error once, with an exact span and a fix where one is clear.
- A CST contract, in `compiler/syntax/README.md`, that the formatter and later stages can build on.
- A `parse` fixture for every grammar production and for each syntax rule in Tasks, plus the fixtures the lexer handed over.

**Non-goals:**
- Formatting, and where `fmt` puts comments. The formatter doc decides that. This doc fixes only where trivia sits in the tree (decision 3), which the formatter doc may amend before layer 9 merges.
- A typed AST layer. Whichever stage first needs typed accessors (the formatter, or M2's name resolution) adds them over the CST. This doc fixes only the node kinds.
- Deciding what a dotted name means (`a.b.c` as a package path, a field access or a type path), or whether a non-binding name in a pattern is a constant, a variant or a struct. Name resolution does that in M2 (decision 7).
- Incremental reparsing. Whole-file parsing is fast enough until M5 shows otherwise.
- A `quince` command that prints the CST. The unit tests' dump function can back one later. The CLI's output is M5's to decide.
- Fixes that rename a declaration to the right case. A rename has to change every use, which needs name resolution (M2) and the multi-file `fix` (M5).

## Decisions

### 1. Where it lives, and the API

The parser is the `parser` module of `quince_syntax`, with a submodule for each grammar area: items, types, expressions, statements and patterns. The tree is the `cst` module. The checks that run on the tree after parsing (naming case, decision 7) are the `validate` module.

- `parse(&Decoded) -> Parse`, or `parse_str(&str)` for text that never came from bytes, as with `lex` and `lex_str`.
- `Parse` holds the root node and every syntax error, from the lexer, the parser and validation, in source order. `Parse::diagnostics` converts them through a `LineIndex`, as `Lexed::diagnostics` does.
- `LexError` becomes `SyntaxError`, since all three stages report the same shape (range, code, message, fixes). `codes::ALL` is split so the lexer's mutation test can still require every *lexer* code from its mutants.
- **No fixes in a lossy text.** `parse` clears every fix when `Decoded::is_lossy()` is true, once, after all three stages. So the rule covers every stage in `quince_syntax`, including fixes added later. `lex` keeps its own check for callers that lex without parsing. When M5's driver assembles a file's diagnostics from every stage, the rule moves there.

*Alternative:* a separate `quince_parser` crate. Rejected for the reason the lexer gave: the lexer, parser and formatter share `SyntaxKind`, and the repo plan puts them in one crate.

### 2. The CST

The tree is a `rowan` tree (decided by David, 2026-11-11). `rowan` is the tree rust-analyzer runs on, and it fixes nothing about the language: the kinds and the shape are Quince's. It has two layers:
- An immutable green tree: each node holds a kind, a text length and its children, and each token holds a kind and its text. Green nodes can be shared.
- A red layer over it, built on demand, that gives each node its parent and its offset. Consumers walk the red layer, and later use its token-at-offset lookups and tree editing (M5). Since 0.17, `rowan` edits only by building new green trees (`replace_with`, `splice_children`), as rust-analyzer now does: it has no in-place mutable trees.

Node kinds join tokens in `SyntaxKind`. Every lexer token appears in the tree, in order and with its text, trivia included, so the tree's text is the input exactly. The one exception is the joins: two CST tokens each hold two lexer tokens (decision 5). No node or token is empty, except the root of an empty file.

**The parser never builds the tree directly.** It emits events: start a node, finish a node, take a token, report an error. A tree builder turns the events and the lexer's tokens into the tree, placing trivia (decision 3). rust-analyzer separates the two the same way. So the parser doesn't depend on `rowan`, and it can parse ahead for a lookahead (decision 6) without building anything.

*Alternatives:*
- A tree of our own, a flat arena of about 400 lines. Rejected by David: it would need most of `rowan`'s features by M5.
- An AST with no trivia, plus a side table for comments. Rejected by the repo plan: `fmt`, `fix` and error recovery need trivia in place.

### 3. Trivia in the tree

The parser sees only significant tokens (decision 4). The builder places each trivia token:
- Trivia before a node's first token goes outside the node, before it, with one exception. Comments directly above a declaration, a field, an enum variant, a member or a match arm, with no blank line between, go inside that node, with their line endings. So moving or deleting a declaration takes its comments with it, as in rust-analyzer.
- Trivia after a node's last token goes outside it. So a comment at the end of a line belongs to the node that holds the line, such as the block, not to the expression before it.
- A doc comment isn't trivia where it documents something. The grammar names `doc_comment` as a terminal, so `DocComment` tokens are parsed as part of the declaration they document (decision 8). Anywhere else the builder places them like comments.
- The lexer's `Error` tokens, block comments, `Semicolon` tokens and the byte order mark are trivia. `Semicolon` is placed like a line ending, and the rest like comments.

The formatter doc decides where comments go in its output, and may amend these rules if it needs another attachment.

### 4. Line breaks

The parser reads a list of significant tokens: everything but `Whitespace`, `Newline`, `Comment`, `BlockComment`, `Bom`, `Error`, `Semicolon`, and doc comments that document nothing. Each one records the break before it:
- **none**;
- **line**, when a `Newline` comes between it and the previous significant token;
- **hard**, when a `Semicolon` does, even if a `Newline` does too. A `;` ends a statement outright (lexer README), so a hard break never continues a line.

**Where a break ends a statement.** In a line context, a token after a break doesn't continue the current construct, unless:
- the previous token's kind `continues_line()`;
- the previous token is a `>` or `*` the parser took as a binary operator. Those two depend on their role (lexer decision 4), and the parser knows the role when it takes them;
- the next token is `.` (`r[lex.continue.more]`);
- the parser is in a `fn` header and the next token is `->`, `uses`, `requires`, `ensures` or `{` (`r[lex.continue.fn-header]`, which replaces `r[lex.continue.signature]` under spec question 12). That covers a declaration's signature, a closure's header and a function type.

The parser checks that one function, `at_break()`, wherever a construct may end or go on: before a binary operator, a postfix operator, `else`, an optional part of a signature, and the operand of `return`. So `let x = a` followed by a line `- b` is two statements, and `return` on a line of its own is a bare return.

**Line contexts and list contexts** (spec question 3). Breaks matter only in line contexts: the file, a block, and the bodies of `impl`, `interface`, `effect` and `foreign`. Elsewhere a break is spacing: inside parentheses, square brackets, type arguments and generics, and the braces of a comma-separated list (fields, variants, map entries, import names, match and select arms, a `concurrent` spawn list). A `;` there is spacing too, apart from its E0002. A block inside a list is a line context again. So `f(\n  a,\n  b\n)` parses without its trailing comma, as `r[tool.fmt.check]` requires, since unformatted code compiles.

**A break that ends a statement too early** is one error, E0018, not two. The parser reports E0018 at the break and parses on as though the lines were joined when the next line starts with:
- a token that can't start a statement but would continue the construct the break ended: `=`, a compound assignment, `=>`, a binary operator that can't also start an expression, or `?`;
- `else` after an `if`'s block (`r[lex.else-same-line]`). This applies in list contexts too, where breaks are otherwise spacing, since the spec's rule has no exception;
- `{` where the construct the break ended needs its body next, as in `if x` followed by a line `{`. Outside a `fn` header, which `r[lex.continue.signature]` continues, a body's `{` sits on its construct's line.

A line that starts with `- b` or `(x)` isn't one of these: it can start a statement, so it is one, as `r[lex.line-end]` says.

E0018's fix replaces everything between the two tokens with one space (`safe`): the break, any blank lines, and the indentation. That joins the lines, as `r[lex.else-same-line]` asks. When any comment lies between the two tokens there is no fix, since joining would move code into the comment or delete it.

A hard break never joins. `a;` followed by a line `.b()` is an E0002 plus an E0016 at `.`, which no statement starts with. Deleting the `;` fixes both, which is why E0002's fix is `needs-review` there.

*Alternative:* Go's rule everywhere, so a line break before `)` ends the statement and `f(a\n)` is an error. Rejected: `r[tool.fmt.check]` says unformatted code compiles, and only the formatter adds trailing commas.

### 5. Tokens the parser joins

- **`>>` and `>=`** are formed from a `>` and the next token when no trivia sits between them (lexer decision 5). The parser checks adjacency, and the builder writes the pair to the tree as one token, `GtGt` or `GtEq`, holding both texts. They are CST-only kinds: the lexer never produces them, which keeps `r[lex.no-shift-token]`. `a > = b` is an "expected expression" error, as `a = = b` is.
- **A negative literal** (`r[lex.lit.int.neg]`, spec question 5) is a `-` in prefix position followed, with no trivia between, by an `Int` or `Float` that no postfix operator follows. The builder puts both tokens in one `Literal` node, so `-9_223_372_036_854_775_808` is one literal the type checker can range-check. `- 1` is a prefix minus applied to `1`. `-2.pow(2)` is `-(2.pow(2))`, as `r[expr.op.levels]` reads `-x.abs()`. In patterns, `[ "-" ] literal` follows the same adjacency rule.

### 6. Expressions

Expressions are parsed by precedence climbing over binding powers. The parser accepts any mix of operators and builds the tree the usual reading gives. Then it reports what the spec forbids, with the fix that adds the parentheses for that reading (`r[expr.op.parens-required]`). Accepting first keeps one slip to one error, and the tree is still useful to the formatter and the language server.

**The usual reading** is Quince's seven levels (`r[expr.op.levels]`). The operators with no level go where the spec's own examples put them:
- tightest first: postfix, prefix, `*` `/` `%`, `+` `-`, `<<` `>>`, `&`, `^`, `|`, `..` `..=`, comparisons and `is`, `&&`, `||`;
- so `flags & MASK == 0` reads `(flags & MASK) == 0`, `1 << n - 1` reads `1 << (n - 1)`, `a || b && c` reads `a || (b && c)`, and `r == 0..3` reads `r == (0..3)`, as the spec's examples write them.

What it reports:
- **E0019, parentheses needed**, with the fix that adds them for the usual reading (`needs-review`, since the author may have meant the other one):
  - `&&` mixed with `||` (`r[expr.op.logic.no-mix]`);
  - a bit operator as the operand of any other operator, or mixed with a different bit operator (`r[expr.op.bits.parens]`, `r[expr.op.bits.chain]`);
  - a shift as the operand of a shift, `a << b << c`, which `grammar.bits` doesn't allow;
  - a range as an operand (`r[expr.op.range.parens]`).
- **E0020, doesn't chain:**
  - a chained comparison (`r[expr.op.compare.no-chain]`). The fix is `a < b && b < c` (`needs-review`: it writes `b` twice, so a `b` with effects would run twice);
  - a chained range (`r[expr.op.range.no-chain]`), with no fix.
- **Jumps.** `jump` (`return`, `break`, `continue`, `spawn`) is an expression only at the top of an expression, so `a + return` is an E0016. `return` and `spawn` take everything to their right (`r[expr.op.keyword-forms]`). `spawn in` takes a postfix expression, which takes a following `(` or `[`, so `spawn in bg (x).run()` calls `bg`, as the grammar says.

**When `<` opens type arguments** (spec question 6). After a name or a path in an expression, the parser parses ahead to see whether type arguments closed by `>` follow. If they do, it checks the token after the `>`. `<` opens type arguments only when that token is `(`, `.`, `?`, `,`, `)`, `]`, `}` or a break. A `>` doesn't count, and neither does a `=` directly after the `>`. Otherwise `<` is less-than.
- So `json.decode<Order>(body)` and `Channel<Job>.new(16)` take type arguments.
- `a < b > c` and `x < Y >= z` are chained comparisons, E0020 with its fix.
- `a < b >> c` is a comparison with a shift as its operand, E0019, not type arguments followed by `> c`.
- Case can't decide this, since a type can be a C name such as `sqlite3` (`grammar.type_path`, as in `CBuf.new<Ptr<sqlite3>>()`).

Parsing ahead discards its events. The parser remembers the answer for every `<` it examines, nested ones included, so each `<` is examined once and the file stays linear.

**`{` where an expression is expected** starts a map literal (`r[expr.block.brace-map]`). `{}` there is E0025, whose fix writes `{:}`. The fix is `needs-review`, since an empty map may not be what the author meant: bare blocks don't exist (`r[expr.block.no-bare]`), so `{}` can't be the empty block or the unit value an author might have had in mind.

**Calls and struct literals** have one shape, `Call`, whatever is called. `User(id: 1)`, `UserId(1)`, `Some(x)` and `f(x)` all parse as a callee and an argument list, because only name resolution can tell a struct from a function. A `..base` argument is allowed only first (`r[item.struct.update]`).

**Assignment** is parsed as an expression followed by `=` or a compound assignment, then checked. The left side must be a place: a `var` binding's name or `self`, followed by field names, tuple or newtype positions, and indexes (`grammar.place`, `r[stmt.assign.place]`, spec question 13). Anything else is E0026. `x = y = z` and `if x = 1` are an E0016 at the second `=`, since assignment is a statement (`r[stmt.assign.no-chain]`).

### 7. Names and case

The CST has one `Name` node for a name being declared and one `NameRef` for a name being used, whatever its case.

Case changes the tree's shape in one place only: patterns. There, a name that doesn't start with an uppercase letter and stands alone binds a new variable (`r[pat.const-exhaustive]`: "a lowercase name always binds"). Any other name or dotted path is a path pattern, and name resolution (M2) reads it as a constant, a variant or a struct (`r[pat.const]`). So the parser never needs the edge cases of spec question 1.

Everywhere else the parser is case-blind:
- A type path is names separated by dots, whatever their case, since a type can be `Self` or a C name (`grammar.type_path`).
- A dotted chain in an expression is a `NameRef` followed by field steps. Name resolution decides whether `a.b` is a package path, a field, or a type with a constant.

That keeps case out of the parser, and lets code use a foreign name such as `SQLITE_OK` or `SDL_Init` wherever a name can go (spec question 1).

**The case check** (`r[lex.name.case]`, `r[lex.name.enforced]`) runs in `validate` over the finished tree. It checks a name only where it's declared: functions, parameters, bindings in `let`, `var`, `for` and patterns, fields, types, variants, generics, effects, constants, import aliases and the `ensures` binder. A use can't be wrong unless its declaration is, and the declaration gets the error.
- A name that isn't of the class its position takes, including a name that fits no class, is E0021. The message names the class and gives the name converted to it. There's no fix in M1 (see Non-goals).
- Names declared in a `foreign` block are skipped (`r[lex.name.foreign-exempt]`).
- Spec question 1 defines the classes.

A keyword where a name goes is an E0016 that names the keyword (`r[lex.keyword-not-name]`, `r[lex.ident-not-keyword]`). The lexer gives every keyword its own kind and none is contextual (`r[lex.no-contextual]`), so the parser never has to guess. A C function whose name is a keyword is spec question 4.

### 8. Declarations, doc comments and attributes

- **Declarations.** A declaration is its doc comments, its attributes, `pub`, and the item (`grammar.decl`). The doc comments are tokens in a `DocComments` node at its start, and the attributes are `Attribute` nodes. Fields, variants and variant fields take the same (`grammar.field`, `grammar.evariant`, `grammar.vfield`). So do impl items, interface methods, effect operations and foreign items (spec question 9).
- **Stray doc comments.** A doc comment that comes before nothing it can document is E0023, with a fix that makes it an ordinary comment (`safe`, spec question 10). Until E0023 lands in layer 7, such a doc comment is read as an ordinary comment, with no error. `lex/control.qnc` has one in a function body. Layer 7 adds its expectation, or turns it into a `//` comment if the fixture is about something else.
- **Attributes.** An attribute goes on its own line (`r[item.attr.own-line]`). One on its declaration's line is an E0016 whose fix inserts a line break (`safe`). Whether the attribute's name is known (`r[item.attr.unknown]`) is checked by a later stage, since the set depends on the edition.
- **Imports** come before declarations (`grammar.file`). An import after a declaration is an E0016.
- **What the grammar rejects is an E0016:** `pub` on a variant field (`grammar.vfield`, `r[item.enum.field-visibility]`), a `const` in an interface (`grammar.interface_decl`, `r[item.const.assoc.no-interface]`), and a wildcard import (`r[intro.excl.wildcard-imports]`).
- **What the grammar accepts goes to later stages,** each with a code and a fixture there: `pub` on an `impl` or a `test`, and `@test_only` on a method.

### 9. Error recovery

Every error goes through one rule: report it, then skip to a sync point, reporting nothing more until the parser gets there. The skipped tokens go in a `Skipped` node. That name avoids the lexer's `Error` token kind.

**Sync points.** Each open construct has its own:
- A list syncs at its next `,` and at its closing bracket.
- A line context syncs at its next break.
- Any construct syncs at a member start of a container around it (below), which closes it.

**A member start closes what can't hold it.** Some tokens, at the start of a line, can only start a member of a certain container. Such a token closes every open construct inside the innermost open container that can hold it, and each construct it closes that has an opening bracket gets an E0017 at that bracket. The member starts are:
- for the file: `fn` followed by a name, `struct`, `enum`, `type`, `interface`, `impl`, `effect`, `foreign`, `import`, `const` and `test`, and `pub` followed by one of those;
- for an `impl`, `interface`, `effect` or `foreign` body: `fn` followed by a name, `type` and `const`, and `pub` followed by one of those;
- for struct braces, enum braces and a variant's field list: `pub` followed by a name. A field or variant otherwise starts with a name, so these close at any other file member start. So a `pub` field on its own line never closes a struct, and a `pub` on a variant field stays the one E0016 of decision 8;
- for a block: `let`, `var` and `while`. These start statements but never expressions, so inside a block they can only sit in a nested block, never inside a list. `for` isn't one, since `concurrent(limit: 4) for x in xs { ... }` is an expression that can sit in a list.

So a missing `}` gives one E0017 at its `{` when the next declaration starts, rather than an error on every line after it. A missing `)` gives one E0017 when the next statement starts with one of those keywords. A declaration can't go in a block (`grammar.block`, `grammar.stmt`), so this never closes a block that was meant to stay open.

Doc comments and attributes aren't member starts: they belong to the declaration after them. A stray `///` or `@` never closes anything. At the start of a line, the check looks past attribute and doc-comment lines to the token after them, so a `@` line before a `fn` closes what the `fn` closes, with no E0016 at the `@`.

**Order and silence.** The member-start check runs before the parser reports "expected" at the start of a line, so a missing `)` before a `let` line is the E0017 alone. A construct closed while the parser is skipping after an error gets no E0017, since the error already reported covers that slip. So `f(a, b` followed by a line `g()` is one E0016 at `g`, expecting `,` or `)`, and the block's `}` later closes the `(` silently.

**Brackets.** A skip never stops inside a bracket it opened.
- A closing bracket that matches an open one further out closes everything inside it, with an E0017 at each inner opener. `f(a` followed by a line `}` is an E0017 at `(`.
- A closing bracket that matches no open bracket closes the innermost one in its place, with one E0016 that names the bracket expected. `f(a, b]` is one E0016.
- A closing bracket with nothing open is skipped, as an E0016.

**E0016** is the general syntax error: "expected X, found Y", where X lists what may go there in Quince terms ("an expression", "`,` or `)`", "a line break").
- When a list item is followed, on the next line, by something that starts another item, its fix inserts the missing `,` (`needs-review`).
- Two expressions on one line, such as adjacent literals (`"press "b" now"`, from the lexer doc), are an E0016 that expects a line break, with no hint for that shape (lexer decision 9).

**Errors the lexer already reported** aren't reported again (lexer README). Beyond that contract:
- An error the parser would report right after one of the lexer's `Error` tokens, with only trivia between, is dropped. So `let x = $` gives the lexer's E0001 alone, even though the `=` continues the line and the parser's error would fall on the next line. One slip, one error.
- An unclosed one-line literal (a string with no `StringEnd`, E0004, or a char with no closing `'`, E0009) ends its line's lists. Parentheses, square brackets and list braces opened on that line close at its end with no E0017, and nothing else on the line is reported. A block's `{` stays open, so in `if ok { log("done)` followed by a line `}`, the `}` closes the `if`'s block. So `print("hello)` is the E0004 alone.
- When a multi-line string or a block comment runs to the end of the file unclosed (E0005, E0003), the parser reports nothing at the end of the file. Everything it would report there, such as a missing `)` or `}`, is that slip.

**Loop safety.** Every parser loop takes a token or exits. A counter under `debug_assertions`, which fuzz builds turn on, panics when the parser looks ahead 256 times without taking a token, so the fuzz target and the unit tests see a loop as a panic.

**Slips and their diagnostics.** These are unit tests, and each row lands with the layer that parses its construct. Each later layer adds rows of its own.

| Slip | Diagnostics |
| --- | --- |
| `f(a, b` followed by a line `let y = 2` | E0017 at `(` |
| `f(a, b` followed by a line `g()` | E0016 at `g` |
| `struct S {` with `pub` fields on lines of their own | None |
| `struct S {` missing its `}`, followed by a line `pub fn g() {` | E0017 at the struct's `{` |
| `f(a b` followed by a line `fn g() {`, with the `)` and the block's `}` missing | E0016 at `b` alone. The missing closers show once it is fixed |
| `if ok { log("done)` followed by a line `}` | E0004 |
| `fn f() {` with no `}`, followed later by a line `fn g() {` | E0017 at the first `{` |
| `f(a` followed by a line `}` | E0017 at `(` |
| `f(a, b]` | E0016 at `]` |
| `f(` then lines `a` and `b`, then `)` | E0016 at `b`, with a fix that inserts `,` |
| `print("hello)` | E0004 |
| `let x = $` | E0001 |
| `let x` followed by a line `= 1` | E0018 |
| `}` followed by a line `else {` | E0018 |
| `if x` followed by a line `{` | E0018 |
| `a; b` | E0002 |
| `"press "b" now"` | E0016, expecting a line break |

### 10. Diagnostics

Codes continue from E0016 in the order they are registered, as the lexer's do. They are provisional until each one's layer merges. Messages describe Quince, the same way for every input (lexer decision 9).

The operators Quince leaves out (`r[expr.op.excluded]`) get no code or hint of their own: each is an E0016 that says what may go there. That covers `x++`, `a ** b`, `c ? a : b`, `x as Int` and `a ?? b`. `x?.y` and `--x` aren't errors at all: they are `?` followed by a field, and a double negation.

| Code | Problem | Fix | Rule from |
| --- | --- | --- | --- |
| E0016 | A token that can't go here: "expected X, found Y" | Insert a line break after an attribute on its declaration's line (`safe`); insert a missing `,` between list items on separate lines (`needs-review`); remove parentheses around a type that isn't a function type (`safe`, Q14). Otherwise none | |
| E0017 | An opening bracket with no closing one, at the opener | None | |
| E0018 | A line break that ends a statement too early, including before `else` or a body's `{` (decision 4) | Replace everything between the two tokens with a space (`safe`), unless a comment lies between them | Q3, Q12 |
| E0019 | Operators that need parentheses (decision 6) | Add the parentheses for the usual reading (`needs-review`) | |
| E0020 | A chained comparison or range | For comparisons, `a < b && b < c` (`needs-review`) | |
| E0021 | A name of the wrong case, or of no case, where it's declared | None in M1 | Q1 |
| E0022 | A tree deeper than the nesting limit (below) | None | |
| E0023 | A doc comment that documents nothing | Make it `//` (`safe`) | Q10 |
| E0024 | A struct with no fields written with braces | Remove the braces (`safe`) | Q11 |
| E0025 | `{}` where an expression is expected | `{:}` (`needs-review`) | |
| E0026 | An assignment whose left side isn't a place | None | Q13 |

**The nesting limit.** A deep enough input would overflow the stack, which aborts the process rather than panicking. The parser is recursive, and so are the tree's consumers, and perhaps dropping a tree. E0022 therefore bounds the depth of the tree, not just the parser's recursion:
- every node the parser opens counts, at every recursive entry point: expressions, types, patterns, blocks, `else if` chains and prefix operators;
- so does each step of a left-associative chain, such as `a + b + c` or `x.a().b()`, which loops in the parser but nests in the tree.

Past the limit, E0022 is reported at the token that goes past it, and the parser skips to the end of the innermost construct. Since the limit bounds the tree, consumers may recurse over it.
- Layer 9 sets the limit by measuring a debug build on a 2 MiB thread, the default for Rust test threads, with room to spare. It's at least 256.
- The deep-nesting tests run on spawned threads with that stack size.
- It's an implementation limit, so it goes in the README, not the spec.

### 11. Testing

- **Unit tests** in `quince_syntax` parse a text and compare a tree dump with hand-written expected text (decided by David, 2026-11-11). No snapshot crate, and no command that writes expected text from the parser's output, so each expectation is written before the code, as the core loop asks.
  - A dump has one line per node or token, indented by depth: the kind, plus the text for a token. It has no byte ranges, since text order gives positions and the lossless invariant is checked separately.
  - A dump without trivia keeps precedence and grouping tests short. The full dump is for tests about trivia.
  - Unit tests check the trees that `parse` fixtures can't see: precedence, where a line ends, where trivia sits, the joined tokens and negative literals.
- **Conformance fixtures** go in `tests/conformance/testdata/parse/`, a folder per grammar area, in `parse` mode. At least one fixture cites each grammar production (`grammar.<production>`, the repo plan's "one conformance test per grammar production"). Every code has one, and so does each rule in Tasks.
- **Coverage honesty.** A fixture cites a rule only when a parser that broke the rule would fail the fixture, as the lexer doc requires. A `parse` fixture sees only diagnostics, not the tree. So:
  - rules that forbid something are cited by fixtures that expect the error;
  - accepted forms are cited by fixtures that a parser rejecting them would fail;
  - rules about the tree's shape are unit-tested, and cited by fixtures only where a wrong tree gives a different diagnostic. They are `r[expr.op.levels]`, grouping, where a statement ends, and `r[lex.lit.int.neg]`, whose folding no diagnostic shows until M3 range-checks literals. M3's `check` fixtures and M6's `run` fixtures cite them.
- **The lexer's handed-over rules** get their fixtures here:
  - `lex.keywords` and `lex.ident-not-keyword`: each keyword where a name goes is an E0016;
  - `lex.no-contextual`: `main`, `limit` and `TaskScope` as ordinary names;
  - `lex.no-shift-token`: `List<List<Int>>` closes;
  - `lex.grammar-rules`: a `>>` shift, `..=`, and `x is A | B` (layer 5);
  - `lex.ident-classes`: names of each class in their positions (layer 8);
  - `lex.indentation`: the same program with odd indentation parses with no error;
  - `lex.comment.doc`: a doc comment on a declaration;
  - `lex.comment.doc-exact`: `////` in a function body with no E0023 (layer 7, once E0023 exists).
- **Invariants on every input:**
  - no panic and no loop (decision 9);
  - the tree's text is the input;
  - every lexer token is in the tree in order, apart from the joins (decision 5);
  - every error range lies inside the input, on character boundaries.
- **The fuzzer** (decided by David, 2026-11-11) is a `cargo-fuzz` target in `compiler/syntax/fuzz/`.
  - That crate is outside the workspace, with its own `[workspace]` table, as `cargo fuzz init` writes. So `cargo xtask ci` and stable builds never build it or its `libfuzzer-sys` dependency. Its `Cargo.lock` is committed, so a run can be reproduced.
  - It runs on nightly Rust, only where someone runs it. `rust-toolchain.toml` pins a stable release, so `AGENTS.md` gives the command as `cargo +nightly fuzz run parse`.
  - The check itself is a function in `quince_syntax`, `fuzz::check_parse(&[u8])`, which decodes the bytes, parses them and checks the invariants. Its module is compiled only for tests and under a `fuzz` feature, which the fuzz crate turns on, so a check that panics never reaches the crate's public API or a release build. The target is a few lines that call it, and its seed corpus is the fixtures. A unit test calls the same function on every fixture, so CI keeps the check compiling and passing as `quince_syntax` changes, and only the thin wrapper can break unseen. rust-analyzer splits its fuzz targets the same way (`syntax::fuzz`).
  - Fuzzing isn't a CI step, and no run has a set length (decided by David, 2026-11-11). Fuzzing is a way to find bugs, and a clean run proves nothing. So M1's exit asks that the target has been run before M1 closes, and that every crash it found is fixed and has a regression test: a unit test or a fixture, so CI guards against it coming back. The milestone's last PR records when the run was, how long it ran, the corpus size, and what it found.
  - rust-analyzer also builds its fuzz targets in a weekly scheduled workflow, without running them, so they can't rot. Quince has no such workflow (decided by David, 2026-11-11): the wrapper is a few lines, so whoever next runs the target fixes it first if it has broken.
  - The lexer's mutation test stays as it is.

### 12. The coverage ratchet

M0 left this to M1, and the parser brings the first real coverage. `tests/conformance/covered.txt` lists every covered rule ID, one per line, sorted. `cargo xtask conformance` fails when the list and the run disagree in either direction:
- a rule in the list that no passing fixture covers means coverage fell;
- a covered rule missing from the list means the list is stale.

`cargo xtask conformance --write` rewrites the list. Each PR's diff then shows the rules it covers or uncovers, which review can check against its fixtures. It's the same pattern as `spec-grammar --write`. It lands first, in its own PR below layer 0 (`m1/coverage-ratchet`, split out to keep layer 0 under 900 lines), so every parser layer shows its coverage this way.

*Alternative:* a number that may only go up. Rejected: a PR could uncover one rule and cover another, and the number wouldn't show it.

### 13. Delivery as a stack

Each layer builds and passes `cargo xtask ci` on its own.
- Layer 0 builds the tree with no parser, and tests it on hand-built events.
- Layer 1 wires the parser into `SyntaxBackend`, so from then on every fixture, `lex/` included, must parse. So layer 1 parses everything the 38 `lex/` fixtures use (Context), and fixes any fixture that turns out not to be valid syntax. Later layers complete each area.

The layers:
0. `m1/parser-cst`: `rowan`, pinned; `SyntaxError`, and `codes::ALL` split; events, the tree builder, the CST and trivia placement, tested on hand-built events; the tree dumps; the repo plan's step 3 corrected. The coverage ratchet comes before it, in its own PR, `m1/coverage-ratchet`.
1. `m1/parser-core`: the parser's core, wired in. It covers:
   - significant tokens, breaks and `at_break()` in line contexts;
   - recovery, E0016 and E0017;
   - `parse`, `Parse::diagnostics` and the lossy rule;
   - `fn` declarations with parameters (`self` included) and return types, `test`, `const`, and an inherent `impl` with methods (`impl User { ... }`). Layer 7 takes the rest of `impl`: generics, `for`, and `type` and `const` members;
   - type paths with type arguments;
   - blocks, `let` with a type;
   - literals, booleans (question 2), names, list literals, ranges, calls, method calls and fields;
   - the README's CST contract;
   - `SyntaxBackend::parse` runs the parser.
2. `m1/parser-types` (2a): the rest of types, generics, bounds and effect lists. `m1/parser-decls` (2b): struct, enum, newtype, alias and import declarations; attributes and doc comments; E0024.
3. `m1/parser-exprs`: every operator and level, E0019 and E0020; the joined `>>` and `>=`; type arguments in expressions; negative literals; tuples, maps and E0025; `..base`.
4. `m1/parser-stmts`: statements, assignment and E0026; list contexts; E0018; `if` and `else`.
5. `m1/parser-patterns`: patterns, `match`, `is` and `for`.
6. `m1/parser-keyword-forms`: closures, `concurrent`, `background`, `select`, `with`, `spawn` and contracts.
7. `m1/parser-items`: the rest of `impl`, `interface`, `effect` and `foreign`; members' doc comments and attributes; E0023.
8. `m1/parser-names`: `validate`, the case check and E0021.
9. `m1/parser-fuzz`: the fuzz crate, the nesting limit and E0022, and the deep-nesting tests.

Each spec question lands in the layer that enforces it, with a minor version bump (Tasks). Layers should come in at 500 to 900 changed lines. Layer 1 is the largest. If it runs past 1,000 lines, its expressions (lists, ranges, method calls) move to a layer 1b, and the `lex/` fixtures that need them wait for it. The wiring then moves to 1b too, since every fixture must parse once it lands.

## Spec questions

**Decided by David, 2026-11-11:** questions 1 to 12 are approved as proposed, question 13 allows tuple positions as places, and question 14 uses parentheses for function types, as written below.

Questions 1 to 5 come from the lexer doc's handover. The rest came up while reading the grammar. Each gives its approved wording and the tag it has. Every ID uses a construct already listed in `spec/chapters.txt`, and a question that changes a rule's meaning retires its ID and adds a new one (`AGENTS.md`, "Rule IDs").

**1. Naming classes (layer 8).** `r[lex.name.case]` names the styles but doesn't define them, and `r[lex.name.enforced]` makes the answer a compile error. `T` and `URL` fit two classes. Approved, in Lexical structure's Naming section:
- `r[lex.name.snake]` "A `snake_case` name is lowercase letters, digits and single underscores between them, and starts with a letter: `x2`, `utf8_len`. `_unused`, `trailing_` and `a__b` aren't `snake_case`."
- `r[lex.name.pascal]` "A `PascalCase` name starts with an uppercase letter and has no underscores. A single uppercase letter, such as `T`, is `PascalCase`. Capitals may follow each other, as in `HTTPServer`, but a name of two or more letters with no lowercase letter is `SCREAMING_SNAKE_CASE`, so a type is `Url`, not `URL`."
- `r[lex.name.screaming]` "A `SCREAMING_SNAKE_CASE` name is uppercase letters, digits and single underscores between them, starts with a letter, and has at least two letters: `MAX_RETRIES`, `HTTP2`."
- `r[lex.name.declared]` "The case is checked where a name is declared. A name that fits no class, such as `_unused` or `Sdl_init`, can't be declared outside a foreign block. A name declared in a foreign block can be used anywhere a name can, in its C spelling."

*Why:* every identifier falls into at most one class. Ruling out `_unused` keeps one way to write an unused name, `_` (`r[expr.closure.underscore]`). Allowing `HTTPServer` follows Rust, Go and Swift, and checking acronyms would need a word list. The parser doesn't depend on any of this (decision 7), so these definitions can change later without changing a tree.

*Consequence:* a single-letter constant (`const N: Int = 8`) is an E0021. It is written `const SIZE`.

*Alternative:* `T` as `SCREAMING_SNAKE_CASE`. Rejected: generic parameters are types.

The grammar's `primary` and `alt` list `name` and `NAME` but no `c_name`, so today a foreign function such as `SDL_Init` can be declared but not called. The last bullet fixes that in prose. Changing `primary` itself is optional, and the grammar's comment can say it.

**2. Booleans (layer 1).** `true` and `false` are keywords, and the grammar's `literal` doesn't say it includes them, but a pattern `true =>` needs it to. Approved: `r[lex.lit.bool]` "`true` and `false` are the literals of `Bool`", in Literals, so `literal` covers them.

**3. Line breaks inside lists (layer 4).** `r[lex.line-end]` is Go's rule. But `r[tool.fmt.check]` says unformatted code compiles, so `f(\n  a,\n  b\n)` without its trailing comma must parse. The spec doesn't say what a line break means where no statement can end. Approved, after `r[lex.continue.fn-header]` (question 12):
- `r[lex.continue.lists]` "Inside parentheses, square brackets, angle brackets and the braces of a comma-separated list, such as fields, variants, map entries and match arms, a line break is only spacing. A block inside them is a line context again, where the rules above apply. `else` still goes on the line of the closing `}`."

*Alternative:* Go's rule, which makes `f(a\n)` an error. Rejected because of `r[tool.fmt.check]`.

**4. C names that are keywords (layer 7).** A C function named `select` can't be declared. `r[ffi.cbind.keyword-param]` covers parameters only, and `r[lex.no-contextual]` rules out reading `select` as a name inside `foreign`. Approved: `r[ffi.cbind.keyword-name]` "A C function or type whose name is a Quince keyword can't be declared in v1. Wrap it in a C function with another name, built from `[native] sources`." This is the reversible choice: a renaming attribute can come later without breaking anything, and M12's `quince bind` doc can take it up.

**5. Negative literals (layer 3).** `r[lex.lit.int.neg]` says the minus is "directly before" the literal, but not whether a space may come between, or what a postfix operator after the literal does. Approved: `r[lex.lit.neg-adjacent]` "Directly before means with nothing between them, not even a space, and the literal isn't followed by a postfix operator. So `- 1` is the prefix minus applied to `1`, and `-2.pow(2)` is `-(2.pow(2))`, as `-x.abs()` is." So `Int.MIN` is written `-9_223_372_036_854_775_808`, and postfix still binds tighter than prefix everywhere.

**6. Type arguments in expressions (layer 3).** `r[expr.op.compare.no-chain]` says `json.decode<Order>(body)` "has only one reading", but not how a parser finds it. Case can't settle it, since a type can be a C name (`Ptr<sqlite3>`). Approved: `r[expr.op.type-args]` "In an expression, `<` after a name or path starts type arguments when what follows parses as type arguments closed by `>`, and the token after the `>` is `(`, `.`, `?`, `,`, `)`, `]`, `}` or a line break, and not a `=` directly after the `>`. Otherwise `<` is less-than."

So `a < b > c` stays a chained comparison with its fix, as the spec's example wants. One case still reads as type arguments: `f(a < b, c > (d))` calls `a<b, c>`. That reading then has to type-check as a generic call, and `f((a < b), c > (d))` gives the other one.

**7. The `ensures` binder (layer 6).** `contract = "requires" expr | "ensures" [ "(" name ")" ] expr` can read `ensures (x) - 1 > 0` two ways: as a condition on `x`, or as naming the result `x` with the condition `-1 > 0`. Approved: `r[contract.ensures-binder]` "`ensures` followed by a name in parentheses always names the return value. A condition that starts with a parenthesized name is written without the parentheses."

**8. Where a `uses` list ends (layer 2).** `effects = "*" | effect_ref { "," effect_ref }`, and `type_path` includes `c_name`, so in `handler: fn(Request) -> Response uses E, host: String` (`std/API.md`, `http.serve`), `host` could be read as a second effect. The same goes for `Pool.new`'s `open:` parameter, struct fields, and `Output = T` in a bound. Approved: `r[effect.uses-list]` "A comma continues a `uses` list only when a type path follows it that isn't followed by `:` or `=`. So a parameter, a field or a named bound argument after a function type ends its effects. In a list of types, a function type's effects take every type path after them. That applies to type arguments, tuple types and a function type's parameters, so `Map<String, fn() uses Io, Db>` gives the function both effects."

*Consequence:* a function type with effects that another type follows in such a list is written in parentheses (question 14). `(fn() uses Io, Int)` reads `Int` as a second effect, which is an error, so it is written `((fn() uses Io), Int)`, and `fn(fn() uses Io, Int) -> T` is written `fn((fn() uses Io), Int) -> T`. Neither the spec nor `std/API.md` has such a type today.

**9. Doc comments and attributes on members (layer 7).** In the grammar, `impl_item`, the members of `interface_decl` and `effect_decl`, and the items of `foreign_decl` take no doc comments, and the first three take no attributes. But `@deprecated` applies to "any public declaration", and `@unchecked` and `@verify` to functions. Approved: each of the four takes `{ doc_comment } { attribute NEWLINE }` before it, as `decl` does. For the grammar:
```
impl_item    = { doc_comment } { attribute NEWLINE } ( [ "pub" ] fn_decl | "type" Name "=" type | [ "pub" ] const_decl )
interface_decl = "interface" Name [ generics ] "{" { { doc_comment } { attribute NEWLINE } ( "type" Name | fn_sig { contract } ) NEWLINE } "}"
```
and the same for `effect_decl`'s `fn_sig` and `foreign_decl`'s items.

**10. A doc comment that documents nothing (layer 7).** `r[lex.comment.doc]` says doc comments attach to the next declaration, but not what happens when there's none, as with one inside a function body or at the end of a file. Approved: `r[lex.comment.doc-placement]` "A doc comment must come before a declaration, a field, a variant or a member. Anywhere else it is a compile error, whose fix makes it an ordinary comment." So documentation can't be lost quietly.

**11. `struct S {}` (layer 2).** `grammar.struct_decl` accepts empty braces, but `r[item.struct.unit]` says a struct with no fields "is declared without braces". Approved:
- retire `item.struct.unit`, and add `r[item.struct.no-fields]` "A struct with no fields is declared without braces, as in `pub struct ChannelClosed`, and its one value is written as its name, `ChannelClosed`. Writing `{}` is a compile error, whose fix removes the braces."
- change the grammar to `[ "{" field { "," field } [ "," ] "}" [ "requires" expr ] ]`. A fieldless struct then can't have a `requires`, which has nothing to constrain.

This gives one way to write it.

**12. Line breaks in closure headers and function types (layer 4).** `r[lex.continue.signature]` says "inside a `fn` signature". Approved: retire it, and add `r[lex.continue.fn-header]` "Inside any `fn` header, whether a declaration's signature, a closure's or a function type, a line that starts with `->`, `uses`, `requires`, `ensures` or `{` continues the header." So `fn(x: Int)` followed by a line `-> Int {` works the same in a closure as in a declaration, and there is one rule.

**13. Tuple positions as places (layer 4).** `grammar.place` allows field names and indexes, but not tuple positions. So `t.0 = 1` on a `var` tuple, or `id.0 = 2` on a `var` newtype (`r[item.newtype.access]`), is a syntax error, while `r[stmt.assign.place]` says "a field path from one". Decided by David, 2026-11-11: allow it. A position counts as a field, so the prose's "field path" already covers it. `r[stmt.assign.place]` keeps its ID, and only the grammar changes, to match it:
```
place       = ( name | "self" ) { "." name | "." int | "[" expr "]" }   // self only under var self
```
So `t.0 = 1` and `pair.0.1 = 2` assign one position of a `var` tuple, and `id.0 = 2` the inner value of a `var` newtype, as `user.name = "New"` assigns a field (`r[item.struct.field-assign]`).

*Alternative:* forbid it, retiring `stmt.assign.place` for a rule that says to build a new value. Rejected: allowing it is the reversible direction, since forbidding it later breaks code.

**14. Whose effects follow a returned function type (layer 2).** In `fn make() -> fn(Int) -> Int uses Io {`, the `uses Io` can belong to the returned function type or to `make`. The grammar has no parenthesized type, so neither reading can be forced. Decided by David, 2026-11-11:
- `r[effect.uses-return]` "When a function's return type is a function type written without parentheses, a `uses` after it belongs to the outer function." The same holds for closures, and for a function type whose return type is a function type.
- `r[type.fn-parens]` "A function type may be written in parentheses, which end it: `fn make() -> (fn(Int) -> Int uses Io) uses Net`. Only a function type takes them: `(Int)` is an error, since a single type needs no grouping." Its fix removes the parentheses.
- For the grammar, a `fn_type` production, used both bare and in parentheses:
```
type         = type_path [ type_args ] | "(" ")" | "(" type "," type { "," type } [ "," ] ")"
             | fn_type | "(" fn_type ")"
fn_type      = "fn" "(" [ type { "," type } [ "," ] ] ")" [ "->" type ] [ "uses" effects ]   // a trailing uses after a bare fn_type return belongs to the outer fn
```

So the three cases read:
```
fn make_plain() -> fn(Int) -> Int uses Net {            // make_plain uses Net
fn make() -> (fn(Int) -> Int uses Io) {                 // the returned function uses Io
fn make_logged() -> (fn(Int) -> Int uses Io) uses Net { // both
```

*Why:* every signature reads `-> T uses E`, so an agent reading the header expects the effects to be the declaration's. The parentheses keep a returned function's effects in the signature, where they can be seen, rather than in an alias elsewhere. An alias would also draw `r[item.alias.lint]`'s warning in a public API. Allowing parentheses only around function types keeps one spelling for every other type.

*Alternatives:*
- An alias, `type Transform = fn(Int) -> Int uses Io`, as the only way. Rejected: it hides the effects from the signature, and the linter flags it in public APIs.
- The innermost function type takes the `uses`, as a greedy parse would. Rejected: a declaration that returns a function would then need parentheses or an alias to state its own effects.

## Risks and trade-offs

- [The tree's shape is wrong for the formatter or name resolution] → Both arrive within M1 and M2, while the only consumers are tests. The formatter doc comes next and can amend this one, and the events keep the parser independent of the tree. Node kinds are internal to `quince_syntax`.
- [Parse fixtures can't see the tree, so a parser that builds the wrong tree passes them] → Decision 11 unit-tests tree shapes with hand-written dumps, and `check` and `run` fixtures in M3 and M6 cite the same rules. The coverage report can't show the difference, as the lexer doc already accepted.
- [Hand-written tree dumps are tedious, and a deliberate change to the tree's shape means editing many] → Dumps carry no ranges, and the dump without trivia keeps most of them short. Each test uses the smallest input that shows its point.
- [Line contexts and list contexts (spec question 3) differ from Go's rule, which the spec cites] → The proposal is Python's, and it only accepts more. If David prefers Go's rule, only `at_break()` changes, and `r[tool.fmt.check]` needs rewording.
- [Recovery reports too much or too little] → One rule with sync points scoped to their containers (decision 9), and a table of slips as tests. If a case needs a second patch, the memory `rule-over-heuristic` applies: propose a stricter spec rule instead.
- [Type arguments in expressions take one reading the author didn't mean] → Spec question 6 limits it to a parenthesized operand after the `>`, where the result must still type-check as a generic call.
- [The nesting limit rejects a valid program] → It also bounds left-associative chains, so a sum of a few hundred terms on one line is an E0022. No hand-written code comes near that. Generated code that does can split it across bindings. The limit is in the README, and raising it changes nothing else.
- [The fuzzer isn't in CI, so a panic can land between fuzz runs] → Unit tests, the slip table and the lexer's mutation test still run on every change. Every crash a fuzz run finds becomes a CI test.
- [Nothing builds the fuzz crate, so it can stop compiling unnoticed] → The check is in `quince_syntax` and CI runs it on every fixture (decision 11), so only the few-line wrapper can break. No scheduled build guards it (decided by David, 2026-11-11). Accepted.
- [Eleven codes] → Each names one problem an agent can fix from the message. Review the table before layer 1.
- [Layer 1 is large] → Decision 13 gives the split if it runs past 1,000 lines.
- [`rowan` changes or is abandoned] → It's rust-analyzer's own tree, and the events keep the parser independent of it.

## Rollback

Revert the stack's PRs. Nothing changes outside `quince_syntax`, the conformance runner and its fixtures, xtask's conformance step, `AGENTS.md`, `docs/repo-plan.md`, `docs/milestones.md` (M1's exit, reworded in this doc's PR) and `spec/`, and nothing depends on the parser until the formatter lands.

## For the formatter doc

- Where comments go in the output. Decision 3 fixes only where they sit in the tree.
- `lex.source.canonical` (lexer doc), `tool.fmt.*`, and `item.arg.fmt-positional` and `tool.fmt.field-shorthand`, which rewrite code.
- `fmt` must refuse a lossy text (`Decoded::is_lossy()`) and a file with parse errors, or say what it does with them.
- Writing a prefix minus with no space after it, which spec question 5 relies on.
- The `fmt` fixture mode in the fixture README.

## Tasks

Rule IDs in brackets are what each task's fixtures cite. Grammar productions are cited as `grammar.<production>`. Rules the spec questions add are cited once their layer puts them in the spec. A rule a question retires (`lex.continue.signature` under question 12, and `item.struct.unit` under question 11) is cited by its replacement, since the runner rejects a retired ID. The productions the questions change (`type`, `place`, `struct_decl`, `impl_item`, `interface_decl`, `effect_decl`, `foreign_decl`) keep their grammar IDs, since a widened or narrowed production isn't retired (`compiler/spec/README.md`, decided by David, 2026-11-11).

**Before building**
- [x] David decides questions A, B and C (2026-11-11).
- [x] Design review passes (six passes, the last two over the decisions recorded after approval).
- [x] David decides spec questions 1 to 14 (2026-11-11).

**Before layer 0: `m1/coverage-ratchet`**
- [x] The coverage ratchet and `cargo xtask conformance --write`; `AGENTS.md`.

**Layer 0: `m1/parser-cst`**
- [x] Add `rowan`, pinned after checking its dependencies and minimum Rust version: `=0.17.0`.
- [x] `SyntaxError` replaces `LexError`; split `codes::ALL` so the lexer's mutation test requires the lexer's codes: `codes::LEXER`, `codes::PARSER` and `codes::all()`.
- [x] Events, the tree builder and the CST; trivia placement (decision 3); the tree dumps, with and without trivia; tests on hand-built events. The README starts its CST section with the builder's rules.
- [x] Correct the repo plan's step 3 to match decision 11.

**Layer 1: `m1/parser-core`, `m1/parser-recovery`, `m1/parser-wiring`** (split in three by size, 2026-11-11: the core, then recovery, then the rest of the syntax and the wiring)
- [x] Spec: question 2, and a bump.
- [x] `parse`, `parse_str`, `Parse::diagnostics`; fixes cleared in a lossy text.
- [x] Significant tokens and breaks; `at_break()` in line contexts.
- [x] Recovery, E0016, E0017; the slip table's rows for this layer; nothing reported after an `Error` token, on the line of an unclosed one-line literal, or at the end of a file an unclosed string or comment swallowed.
- [x] `fn`, `test`, `const` and `impl` with methods; parameters including `self`; type paths with type arguments; blocks; `let` with a type; literals including booleans; names, list literals, ranges, calls, method calls and fields. [`grammar.file`, `grammar.fn_decl`, `grammar.fn_sig`, `grammar.params`, `grammar.block`, `grammar.list_lit`, `lex.no-contextual`, `lex.keywords`, `lex.keyword-not-name`, `lex.ident-not-keyword`, `lex.indentation`, `expr.block.last-newline`]
- [x] `SyntaxBackend::parse` runs the parser, and every `lex/` fixture still passes.
- [x] `compiler/syntax/README.md`: the CST contract. `AGENTS.md`'s map.

Layer 2 is two PRs, since types and declarations are separate pieces of work and together would run past 1,000 lines.

**Layer 2a: `m1/parser-types`**
- [x] Spec: questions 8 and 14, and a bump (v0.13).
- [x] Types, generics, bounds, effect lists; parenthesized function types (question 14); a closing `>` is never joined, so `>>` and `>=` close type arguments. Type arguments and generics are lists closed by `>`, so an error in them stays inside them. [`grammar.type`, `grammar.type_path`, `grammar.fn_type`, `grammar.type_args`, `grammar.type_arg`, `grammar.generics`, `grammar.generic`, `grammar.bounds`, `grammar.bound`, `grammar.bound_arg`, `grammar.effects`, `grammar.effect_ref`, `lex.no-shift-token`, `effect.uses-list`, `type.fn-parens`]
- [x] `effect.uses-return` is about the tree's shape, which no diagnostic shows, so it is unit-tested only (decision 11). M2's `check` fixtures cite it.
- [x] Trailing commas (decided by David, 2026-11-12): every bracketed list the formatter breaks across lines takes an optional trailing `,`, as `r[tool.fmt.trailing-comma]` adds one. The grammar's `fn_type`, `type_args`, `generics` and `bound` gain `[ "," ]` here, `attribute` in 2b, and the pattern and `concurrent` lists in layers 5 and 6. `effects` and `with` aren't bracketed, so a trailing `,` there stays an error.

**Layer 2b: `m1/parser-decls`**
- [x] Spec: question 11, and a bump (v0.14). The `@test` attribute became `@test_only` (decided by David, 2026-11-12): `test` is a keyword, so `@test` couldn't parse.
- [x] Struct, enum, newtype, alias and import declarations; attributes; doc comments; E0024. [`grammar.struct_decl`, `grammar.enum_decl`, `grammar.newtype_decl`, `grammar.alias_decl`, `grammar.import`, `grammar.attribute`, `item.attr.own-line`, `item.enum.field-visibility`, `lex.comment.doc`, `intro.excl.wildcard-imports`, `item.struct.no-fields`]
- [x] Attribute look-past (decision 9): at the start of a line, the member-start check looks past attribute and doc-comment lines, so `g(1,` then a line `@attr` then `fn h() {}` closes the call with one E0017 and no E0016 at `@`.
- [x] Decided by David, 2026-11-12: an attribute's arguments take a trailing `,` (layer 2a's trailing-comma decision), and blank lines may come between an attribute and its declaration. `r[item.attr.placement]` is retired for `r[item.attr.own-line]`, since its meaning widened, which reads "An attribute goes on its own line above the declaration it applies to. Blank lines may come between them."

**Layer 3: `m1/parser-exprs`**
- [ ] Spec: questions 5 and 6, and a bump.
- [ ] Every operator and level; E0019, E0020. [`expr.op.parens-required`, `expr.op.logic.no-mix`, `expr.op.bits.parens`, `expr.op.bits.chain`, `expr.op.compare.no-chain`, `expr.op.range.no-chain`, `expr.op.range.parens`, `expr.op.excluded`, `grammar.bits`]
- [ ] Type arguments in expressions; negative literals, unit-tested only (decision 11).
- [ ] The joined `>>` and `>=` (decision 5): the parser's join step, used by the shift and comparison operators. Moved from layer 2, since only expressions join tokens.
- [ ] Postfix forms, tuples, maps, `{}` and E0025, `..base`. [`lex.lit.collections`, `lex.lit.empty-map`, `expr.block.brace-map`, `item.struct.update`, `intro.excl.macros`]

**Layer 4: `m1/parser-stmts`**
- [ ] Spec: questions 3, 12 and 13, and a bump.
- [ ] Statements, places, E0026. [`grammar.stmt`, `grammar.place`, `stmt.assign.stmt`, `stmt.assign.place`, `stmt.assign.no-chain`, `stmt.loop.forms`]
- [ ] Line and list contexts, E0018, `if` and `else`. [`lex.line-end`, `lex.continue.more`, `lex.continue.fn-header`, `lex.continue.lists`, `lex.else-same-line`, `expr.block.no-bare`]
- [ ] `member_holder` peeks the line's leading kinds once, so its cost doesn't grow with the number of open frames; a deep-nesting test of valid blocks stays under `LOOKAHEAD_LIMIT`. Blocks first nest in this layer, and today each open block costs a look on every `stop()` (found in layer 1's review).

**Layer 5: `m1/parser-patterns`**
- [ ] Spec: `variant_pat` and `pfields` take an optional trailing `,` (layer 2a's trailing-comma decision).
- [ ] Patterns, `match`, guards, `is`, `for`. [`grammar.pattern`, `grammar.match_expr`, `expr.match.arm-syntax`, `expr.match.arm-comma`, `pat.negative`, `pat.const`, `expr.range.pattern`, `lex.grammar-rules`]

**Layer 6: `m1/parser-keyword-forms`**
- [ ] Spec: question 7, and a bump. `concurrent`'s arguments take an optional trailing `,` (layer 2a's trailing-comma decision).
- [ ] Closures, `concurrent`, `background`, `select`, `with`, `spawn`, `return`, contracts. [`grammar.closure`, `grammar.concurrent`, `grammar.background`, `grammar.select`, `grammar.with_expr`, `grammar.jump`, `grammar.contract`, `expr.op.keyword-forms`]

**Layer 7: `m1/parser-items`**
- [ ] Spec: questions 4, 9 and 10, and a bump.
- [ ] Spec (wording approved by David, 2026-11-12): `r[lex.comment.doc]` gains "A doc comment after code on its line documents nothing." So `let x = 1 /// note` gets E0023.
- [ ] The rest of `impl` (generics, `for`, `type` and `const` members), `interface`, `effect` and `foreign`; members' doc comments and attributes; E0023; `lex/control.qnc`'s doc comment. [`grammar.impl_decl`, `grammar.interface_decl`, `grammar.effect_decl`, `grammar.foreign_decl`, `item.const.assoc.no-interface`, `lex.comment.doc-exact`]

**Layer 8: `m1/parser-names`**
- [ ] Spec: question 1, and a bump.
- [ ] `validate` and the case check; E0021. [`lex.name.case`, `lex.name.enforced`, `lex.name.foreign-exempt`, `lex.ident-classes`]

**Layer 9: `m1/parser-fuzz`**
- [ ] `fuzz::check_parse` and its unit test over every fixture; the fuzz crate and its thin target; the command in `AGENTS.md`.
- [ ] The nesting limit, measured; E0022; the deep-nesting tests on spawned threads.
- [ ] Every grammar production cited by a passing fixture.
- [ ] Before M1 closes: run the fuzz target, and fix every crash it finds, each with a regression test.
