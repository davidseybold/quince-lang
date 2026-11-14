# M1: Formatter

Status: passed design review (five passes; the fifth returned "Ready to build"). On 2026-11-13 David approved decisions A, B and C and spec questions 1 to 11 as proposed. Question 9 was revised after that approval (pass 2 found its precedent only half right), and David confirmed the revision the same day. Each decision is tested against what other formatters do (Precedent, below). This is the third and last design doc of M1 (Syntax), after [the lexer](m1-lexer.md) and [the parser](m1-parser.md).

## Why

M1 ends when every `parse` and `fmt` fixture passes and `fmt` is idempotent on every fixture ([milestones](../milestones.md)). The parse, fuzz and lossless criteria are met. No formatter code and no `fmt` fixtures exist yet.

The formatter matters more to Quince than to most languages, for three reasons:
- `r[tool.fmt.check]` makes unformatted code fail `quince check`. So the formatter's output is the only accepted layout of every program, and changing it later makes existing code fail `check`.
- Agents run it after every edit. It must never change what a program means, never damage a file, and give the same output for the same tree, whatever the input's layout was.
- Several spec rules are rewrites: field shorthand, uppercase hex digits, merged imports. Each gives a construct one written form.

## Impact and approvals

- **Code:** `quince_syntax` gets a `fmt` module, and its README gets a Formatting section, the format's contract. The conformance runner gets a `fmt` mode and `Backend::format`. `quince_cli` gets `quince fmt` (decision 12). `fuzz` gets `check_format` and a second fuzz target. Two small parser changes come with spec questions 3, 4 and 8.
- **Dependencies:** none. The layout engine is our own (decision 3).
- **Decisions (approved by David, 2026-11-13):**
  - **A. Files with errors.** `fmt` refuses a file with any syntax error (decision 2), as every formatter checked does. Only 13 of the 111 fixtures have no expected diagnostic, so M1's exit criterion "`fmt` is idempotent on every fixture" becomes: on every fixture `fmt` accepts, `fmt(fmt(x)) == fmt(x)`, and on every other fixture `fmt` refuses and writes nothing. The `fmt` fixtures this doc adds are what make the criterion bite. Layer 1 rewords the criterion in `docs/milestones.md` to say so.
  - **B. The spec's examples follow the format.** Many spec examples don't match any one layout: some put `uses` on its own line in a header that fits on one line (`concurrency.md:18`, `:70`, `effects.md:116`), others keep it on the header line (`effects.md:69`); structs and enums are written on one line; `fn display(self) -> String { self.0 }` has a one-line body; trailing comments are aligned. The rules below are chosen on their merits and by precedent, not to reproduce every example. So the examples are illustrations, not the reference layout. After layer 6 lands, one spec PR runs `quince fmt` over the spec's code blocks, with David's approval, as a one-off edit: no test formats the spec ("Fixtures, not spec"). Many blocks won't parse, such as fragments, `{ ... }` placeholders and examples of errors, so `fmt` refuses them: that PR edits those by hand to match the format where the rules apply, and leaves the rest as they are. Each layer's PR lists the spec examples its rules would change.
  - **C. `quince fmt` in M1.** The CLI gets `quince fmt <file>...`, `quince fmt -` for standard input, and `--check`, with no project walk, since that needs M2's project model (decision 12).
- **Spec (approved by David, 2026-11-13, question 9 as revised):** Spec questions, below, has eleven questions, each with its wording. Four only clarify (2, 5, 10 and 11). Two reverse earlier choices on precedent: question 3 reopens the parser doc's question 5, and question 7 moves three rewrites out of `fmt`. Each lands in the layer that enforces it, with a minor version bump.
- **Diagnostic codes:** none. Refusal reports the parser's diagnostics. The diagnostic `quince check` gives an unformatted file is M2's, when `check` is wired (decision 13).
- **Delivery:** a stack of eight PRs (decision 15).

## Context

- `quince_syntax` has the lexer, the parser, the lossless CST (`compiler/syntax/README.md`, "The CST" and "Node shapes"), and the case check. `parse(&Decoded) -> Parse` gives the root and every error. The tree's text is the input, byte for byte.
- The parser doc's decision 3 places trivia in the tree: comment lines directly above a node whose kind `attaches_comments()` go inside it, and other trivia goes in the innermost node that holds the tokens on both sides. The kinds that take comments are the declarations, `Import`, members, fields, variants, and match and select arms. Its section "For the formatter doc" lists what this doc must decide.
- The lexer splits each line ending into a `Newline` token, and each multi-line string's stripped indentation into `StringIndent` tokens (lexer decision 8), so the formatter can count blank lines and re-indent a string without changing its value.
- Every `;` is an E0002 and every block comment an E0003. So in a file without errors, every comment is a `//` or `///` comment, and every comment ends its line.
- The nesting limit (parser decision 10) bounds the tree at 256 levels.
- The spec's formatting rules are in Toolchain (`r[tool.fmt.*]`). Others are scattered: `r[lex.source.canonical]`, `r[lex.line-end]`, `r[lex.continue.fn-header]`, `r[type.infer.required]`, `r[mod.import.one-line]`, `r[item.arg.named]` and `r[item.arg.fmt-positional]`.
- The conformance runner has `check`, `parse` and `run` modes, and only `parse` is wired. The fixture README says `fmt` arrives as a mode in M1.
- `quince_cli` handles only `--version`.

## Precedent

The guiding rule for this doc: where other formatters agree, Quince does the same; where Quince deviates, the decision says why. "Checked" means run on this machine (rustfmt 2021 and 2024 style, gofmt, rustc, swiftc), or read in the tool's documentation. "Reported" means a research summary that wasn't confirmed against a source, and is marked as such where a decision rests on it.

| Question | What others do | Quince |
| --- | --- | --- |
| File with a syntax error | rustfmt refuses, exit 1 (checked); gofmt, Prettier, Black, zig fmt, dart refuse (reported) | Refuse (decision 2) |
| Who decides line breaks | rustfmt, Prettier, dart: the formatter, by width; gofmt: the author | The formatter (`r[tool.fmt.width]`) |
| Existing trailing comma forces a break ("magic trailing comma") | Black, Ruff yes (reported); dart's 3.7 style dropped it (reported) and made keeping it opt-in (checked); rustfmt, Prettier no | No: the formatter decides every break |
| Blank lines between declarations | gofmt keeps 0 or 1 between one-line declarations of one kind, adds one between kinds (checked) and before a multi-line function (reported); Black enforces them before and after `def` and `class`; rustfmt keeps 0 or 1 (checked) | One around any multi-line or documented declaration, else 0 or 1 as written (decision 4) |
| Blank lines in blocks | Everyone keeps 0 or 1 | Same |
| Declaration bodies on one line | rustfmt never (checked); gofmt keeps the author's | Never |
| `if` on one line | rustfmt only as a value with an `else` (checked); gofmt never | rustfmt's rule |
| `while`/`for` body on one line | Never (rustfmt, gofmt checked) | Never |
| Struct and enum field lists | rustfmt always breaks them in a declaration (checked) | Always break |
| Single-expression closure and arm blocks | rustfmt removes the braces when the body starts on the arm's line, and adds them when it can't (checked) | Arms: same (question 9). Closures need braces, so they stay on one line |
| Last-argument closure | rustfmt and Prettier keep it on the call's line (rustfmt checked) | Same (decision 7) |
| Method chains | rustfmt: one per line past a width; Prettier: a call-count heuristic | rustfmt's, by the full width |
| Long right-hand side | rustfmt: next line when it then fits on one line, else on the same line, breaking inside (checked) | Same |
| Trailing comment and width | Prettier doesn't count it (reported) | Doesn't count |
| Aligned trailing comments | gofmt aligns (checked); rustfmt, Prettier don't | Don't align |
| Width unit | Prettier, rustfmt, Black: display width, East Asian wide = 2 (reported) | Characters (question 5 says why) |
| Indentation | rustfmt, Black, zig: 4 spaces; Prettier, dart, swift-format: 2; gofmt: tabs | 2 spaces, as every spec example |
| Output check | Black checks the AST and a second pass by default (checked, AST part); Prettier only with `--debug-check` (reported) | Black's (decision 10) |
| Hex digits | Black: uppercase in numbers, lowercase in escapes since 24.1.0 (checked, from its changelog); rustfmt, gofmt keep them (checked) | Uppercase everywhere (question 2) |
| Field init shorthand | rustfmt has it, off by default (checked) | On, as the spec says (question 1) |
| `-> ()` removal | Not rustfmt (checked); Clippy's `unused_unit`, swift-format and ktlint lint rules (reported) | A `check` finding with a fix (question 7) |
| `- 1` with a space | Rust and OCaml treat it as `-1` (Rust checked); Swift makes it an error (checked) | Rust's (question 3) |
| Prelude names shadowable | Go (checked), Rust (reported) | Not needed by `fmt` once question 7 moves `-> Unit` |
| Import order | rustfmt 2024: `self` first, digit runs compared as numbers, `v2` before `v10` (checked); isort: std, third-party, first-party, first-party from configuration (reported) | Same (decision 9) |
| Merge imports of one package | isort merges by default (reported); rustfmt only with an unstable option | Merge (`r[mod.import.one-line]`) |
| Doc comment text | rustfmt keeps it (checked); gofmt normalizes lists and headings since Go 1.19 (checked) | Keep as written |
| Golden-file tests | gofmt `.input`/`.golden`, rustfmt `source`/`target` (reported) | `.qnc` and `.fmt` (decision 14) |
| Standard input | Black `-` (checked); the rest read it too (reported) | `quince fmt -` |
| `--check` exit | Black: 0, 1 if any file would change, 123 internal (checked) | 0, 1 would change, 2 refused or error |

## Goals and non-goals

**Goals:**
- `format`: a function from a parsed file with no errors, and `FormatOptions`, to its canonical text. Of the input's layout it reads only where comments and blank lines are, never its line breaks, indentation or spacing.
- It never changes a program's meaning: the output parses with no error to the same tree, apart from the rewrites the spec asks for, with every comment in the same place. It checks this on every run (decision 10).
- It is idempotent on every input it accepts.
- A `fmt` fixture mode, and fixtures for every formatting rule M1 enforces.
- `quince fmt` and `quince fmt --check` on named files and standard input.

**Non-goals:**
- Rewrites that need name resolution or types. Under question 7 they are `check` findings with fixes, in M2 or M3.
- The diagnostic for unformatted code in `quince check` (`r[tool.fmt.check]`). M2 wires `check`.
- Formatting `quince.toml` (`r[dep.manifest.fmt]`), and walking a project's files. Both are M2's.
- Formatting the code in doc comments' examples. Doc comment text is kept as written.
- Formatting part of a file, for the language server. M5's doc decides what it needs. The engine can format one declaration.
- Formatting a file with syntax errors (decision 2).
- Options. There is one format, with no settings.

## Decisions

### 1. Where it lives, and the API

The formatter is the `fmt` module of `quince_syntax`, as the repo plan has it, since it reads only the CST. Its parts are submodules:
- `doc`: the layout engine (decision 3), which knows nothing of Quince;
- `print`: one function per node kind, which turns the tree into the engine's documents;
- `comments`: placing comments and blank lines (decisions 4 and 5);
- `imports`: sorting, grouping and merging (decision 9);
- `verify`: checking the output (decision 10).

The API:
- `format(&Decoded, &FormatOptions) -> Result<String, FormatError>`, and `format_str` for text that never came from bytes, as with `parse` and `parse_str`. `format` parses the text itself, so no caller can format a tree from other text.
- `FormatOptions` holds one field, the name of the project's own library, if there is one, which import grouping needs (decision 9). It isn't a style setting: M2's driver fills it from `quince.toml`. goimports' `-local` and isort's first-party setting are the same input.
- `FormatError` is one of: `Errors(Vec<SyntaxError>)`, the file's syntax errors; `Lossy`, the text isn't the file (decision 2); `Unsupported(SyntaxKind)`, a node kind no printer handles yet, which exists only until layer 6 (decision 15); and `Internal(String)`, a check in decision 10 failed, which is a formatter bug.

*Alternative:* a `quince_fmt` crate. Rejected for the reason the lexer and parser gave: the formatter needs `SyntaxKind` and the tree's internals, and the repo plan puts it in `quince_syntax`.

### 2. What `fmt` refuses

- **A lossy text.** When `Decoded::is_lossy()` is true the text isn't the file, so no writer may write it (lexer decision 9). `format` returns `Lossy` before parsing. Such a text always has an E0014 too, so this is a guard.
- **A file with any syntax error.** `format` returns the errors and writes nothing. That includes errors that leave the tree whole, such as E0021 (case) or E0019 (parentheses needed). Precedent has no split to follow, since no other language's parser checks case, and one rule needs no list of codes to keep current.

*Why:* every formatter checked refuses. Formatting around errors means laying out a tree the parser guessed at, and since the formatter moves line breaks, that can change how the file parses once the error is fixed.

*Alternative:* format every node outside `Skipped`, and print `Skipped` verbatim. Rejected for M1: no formatter checked does it, and M1 has no consumer for it. M5's doc can add it for the language server. The cost is decision A.

### 3. The layout engine

The formatter builds a document for the file, and the engine lays it out at 100 columns. The design is Wadler's "prettier printer", which Prettier uses, and which Biome and Ruff implement in Rust with an engine of their own. A document is built from:
- `text`, which never holds a line break;
- `line`, a space when its group is flat and a line break when it isn't; `soft_line`, nothing or a line break; `hard_line`, always a line break;
- `indent(doc)`, one level deeper on each line break inside it;
- `group(doc)`, laid out flat when all of it fits on the rest of the line, and broken otherwise;
- `if_break(broken, flat)`, for the trailing comma;
- `line_suffix(doc)`, text kept at the end of the current line, for a trailing comment (decision 5);
- `choice(first, second)`, for hugging (decision 7).

The rules:
- Groups are laid out from the outside in: a group is flat only when everything in it fits on the rest of the line, so outer constructs break first.
- A `hard_line` breaks every group that holds it, up to the nearest `choice`. It doesn't pass through a `choice`.
- `choice` takes `first` when the text from the current position to `first`'s first line break fits, laying `first` out normally, including any `hard_line` in it. When `first` holds no line break, the measure runs on into the text after the `choice`, up to the next place a line may break. A `choice` nested inside `first` is resolved by its own rule while measuring. Each measure is bounded by the width, so this stays linear. Otherwise it takes `second`. So a hugged closure with three statements still hugs, as long as the call's line up to `{` fits.
- The option a `choice` takes is laid out with its own groups measured afresh, whatever the mode of the groups around it. A group around a hugging call may be flat, since it was measured only up to the `{`, as in `let t = a + orders.fold(0, fn(s, o) {`. The closure's statements are still measured and broken at 100 columns. After a `hard_line` printed inside a flat group, the rest of that group is measured again too. This is Prettier's `shouldRemeasure`. An engine unit test covers that case.
- The check that something fits stops at the first line break, so each check is bounded by the width.
- Documents are shared (`Rc`), so the two options of a `choice` share their parts. Nested choices, as in `f([g([h(...)])])`, never copy a subtree, and layout stays linear.
- The engine is iterative, with an explicit stack, as Prettier's printer is, so a deep document can't overflow the stack.

Indentation is two spaces per level, as in every spec example. Width is counted in Unicode scalar values (`char`s), one column each, a tab included (question 5). The engine never breaks inside a `text`, so a line holding a long string or name runs past 100 columns where nothing can break it.

*Alternatives:*
- A crate such as `pretty`. Rejected: the formatters that use this design in Rust own their engine, it is about 500 lines, and we can shape it to `line_suffix` and `choice`.
- A gofmt-style formatter that keeps the author's line breaks. Rejected: `r[tool.fmt.width]` says the formatter decides every line break, and so do rustfmt, Prettier and dart.

### 4. Blank lines

The output depends on the tree's tokens, its comments, and where the input had blank lines. Nothing else in the input's layout counts, so `f(\n  a,\n  b\n)` and `f(a, b)` format the same, and an existing trailing comma doesn't force a break, as in rustfmt and Prettier.

Blank lines follow Black, and gofmt where it agrees (spec question 6). gofmt adds a blank line before a multi-line function and between declarations of different kinds, and keeps the author's choice otherwise. Black puts them both before and after a function or class. Where two rules below apply, a removal wins: no blank line comes after an opening bracket or before a closing one, even next to a multi-line member.
- **Around a declaration or member** that spans more than one line once formatted, or has doc comments or attributes: exactly one blank line before and after it. Between two one-line declarations or members, such as `const` lines, an effect's operations or a `foreign` block's functions, one blank line where the input had one or more, and none where it had none. So an `effect` block stays a list of operations, and related constants stay grouped.
- **Imports:** none within a group, one between groups, and one after the last import.
- **Statements, fields, variants, arms and list items:** one blank line where the input had one or more. A blank line kept between list items doesn't break the list: it survives only when the list breaks for another reason.
- **Removed:** after an opening bracket, before a closing one, at the start of the file, between an attribute or a doc comment and what follows it, and anywhere inside an expression, a signature, or between `=` and its value. The file ends with one line feed, unless it is empty.

**Comments between declarations.** A comment line directly above a declaration belongs to it (parser decision 3), so the blank-line rule applies above the comment. A comment with a blank line after it stays separate: the blank line after it is kept, so the comment never attaches to the declaration below when the output is parsed again.

### 5. Comments

The CST already holds every comment in place. The formatter places each comment by the significant tokens on either side of it, not by the node that holds it, with one exception, imports (decision 9). So this doc doesn't amend the tree's rules. dart_style places comments by token the same way (reported).

Each comment is one of two kinds:
- **A trailing comment** follows a significant token on its line. It is printed after that token, with one space before it, as a `line_suffix`, so it ends the line in the output too. It doesn't count toward the width, so adding or removing one never moves the code before it. Trailing comments aren't aligned with each other: alignment would make editing one line rewrite its neighbours, which works against small diffs.
- **A comment line** starts its line. It is printed on its own line, indented as the token after it is. Comment lines before a closing bracket are indented as the bracket's contents.

Every comment ends a line, so a comment is a `hard_line`, which breaks every group around it. `f(a, // first` then a line `b)` gives `f(`, `a, // first`, `b,` and `)` on lines of their own.

**Why a comment can always keep its line break.** A `//` comment ends its line, so the input had a line break between the same two significant tokens, and parsed. The parser decides what a break means from the token before it and its role, the token after it, and whether it is in a list, a line context or a `fn` header (parser decision 4). The formatter keeps each comment, and so its break, between the same two tokens, so the break means what it meant in the input. Three refinements:
- **A `,` moves before the comment.** In `a` then `// c` then a line `, b`, the output is `a, // c`. A break after `,` always continues the line, so that's safe.
- **Rewrites don't apply across a comment.** Field shorthand (`x: // c` followed by a line `x` keeps both `x`s) and removing an arm's braces (question 9) are skipped when a comment lies inside what they change. Adding braces is always safe: in `0 => // c` followed by a line `a + b`, the comment ends up after `{`.
- **A pattern's negative literal** may hold a comment between `-` and its number (parser decision 5), and so may an expression's under question 3. The printer prints the trivia inside a `Literal`.

The formatter itself never breaks a line where the parser would read the break differently: before `(`, `[`, `?` or a binary operator; after `return`, `spawn` or a prefix operator; between `}` and `else`; before a body's `{` outside a `fn` header. Decision 6's rules only break where a break continues the construct. Decision 10 checks both claims on every run.

Comment text is kept as written, apart from spaces and tabs at its end, which are removed. A doc comment is a comment line in its declaration's `DocComments`, and blank lines between its lines are removed.

### 6. Layout

These are the rules review checks the printers against. The README's Formatting section is their contract, with an example of each. Each printer is a short function over its node kind's children (Node shapes).

**One per line** (`r[tool.fmt.one-per-line]`). Each declaration, member, statement and match or select arm starts its own line. So a `match` and a `select` always break.

**Declarations always break.** A function's body, a struct's fields, an enum's variants, and the members of an `impl`, `interface`, `effect` or `foreign` block are each one per line, even when they would fit on one line, as rustfmt does. So every declaration has one shape, and adding a field or a statement never rewrites the line above it. `{}` stays `{}`. A struct's `requires` follows its `}` on the same line.

**Lists.** Parameters, arguments, type arguments, generics, a variant's fields, tuples, list and map literals, import names, field patterns, a `concurrent` spawn list, attribute arguments and `background`'s arguments are each a group: on one line when they fit, and otherwise one item per line, indented, with a trailing `,` (`r[tool.fmt.trailing-comma]`). Arms always break and always end with `,`. A `uses` list and `with` handlers aren't bracketed, so they take no trailing comma (question 11): a long one breaks after its commas, indented, since a comma continues the line. An item that holds a comment breaks its list.

**Blocks** that aren't a declaration's body follow rustfmt:
- A `while` or `for` body, and the block of an `if` with no `else`, always break.
- An `if` with an `else` stays on one line when every branch holds one expression and no comment, and the `if` is a value: the right side of `=`, an argument, an item, an arm's body or a `return`'s operand. As a statement, or as a block's last expression, it breaks. So `let v = if x > 0 { 1 } else { 2 }` stays on one line.
- A closure whose block holds one expression and no comment stays on one line when it fits: `fn(n) { n + 1 }`. rustfmt removes such a closure's braces, which Quince's syntax requires.
- The blocks of `with`, `background` and `concurrent ... for` always break, as statement bodies.
- `}` and `else` share a line (`r[lex.else-same-line]`).

**Signatures** (`r[lex.continue.fn-header]`: "`quince fmt` breaks a long signature before those words"). The steps apply to the outermost header's tail only, never to a function type inside it, so in `fn make() -> fn(Int) -> Int`, the inner `-> Int` stays with its function type.
1. Each contract starts its own line, indented one level, as in Contracts' examples.
2. Otherwise the header is one line when it fits: `fn f(a: Int) -> Int uses Io {`.
3. When it doesn't, `uses E` starts its own line, indented one level (`effects.md:116`).
4. When the line before it still doesn't fit, `-> T` starts its own line too. `effects.md:158` breaks before `->` in a header that fits, so the format changes that example (decision B).
5. When `fn name<generics>(params)` alone doesn't fit, the parameters break one per line, and `) -> T uses E {` follows them when it fits, with steps 3 and 4 applied to it otherwise. When the generics alone don't fit on the first line, they break as a list first, and the parameters after them.
6. Whenever `uses`, `->` or a contract starts a line, `{` starts its own line, at the declaration's indentation.

A closure's header follows the same steps. A `test` header follows them for `forall` and `uses` once question 4 is decided, and until then only its `forall` parameters break.

**Expressions** (`r[lex.line-end]`: "a long expression breaks after its operator, never before").
- A binary expression that doesn't fit breaks after its operators, all those at one level at once, with the lines after the first indented one level. Nested levels are groups of their own.
- A chain of two or more calls or fields that doesn't fit breaks before each `.`, indented (`r[lex.continue.more]`), keeping a leading run of plain names and fields on the first line: `self.items` stays together. That's rustfmt's rule, applied at the full width rather than rustfmt's separate 60-column chain width, since one width is one rule.
- **A right-hand side**, after `=` in `let`, `var`, an assignment, a `const` or a default value, follows rustfmt: the whole on one line when it fits; else the right side alone on the next line, indented, when it fits there on one line; else the right side on the `=` line, breaking inside itself. So `let s = """` stays on its line, a long call breaks its arguments, and a long sum moves to the next line when it fits there.
- **An arm's body** follows the Rust style guide (question 9): on the arm's line when it fits there, or when its first line does and it is a kind that breaks inside itself: a call, a closure, a collection, an `if`, a `match`, a block form or a multi-line string. Any other body that doesn't fit, such as a long binary expression, goes in a block, `=> {` with the body on the lines after it and `},`. So a body never starts on the line after `=>`.
- A prefix operator has nothing between it and its operand, and a range nothing around `..` or `..=`, as in `0..3`. Other binary operators have one space on each side.

**Spacing**, following the spec's examples: no spaces inside `(`, `[`, `<`, a map's braces (`{"a": 1}`) or import braces (`{self, Log}`); one space inside a one-line block, closure or spawn list (`{ n + 1 }`); one space after `:` and `,`, none before; none inside an interpolation's braces.

**Multi-line strings.** Their text is kept. The formatter rewrites only their `StringIndent` tokens, which changes no value (lexer decision 8): the closing `"""` and every content line are indented one level deeper than the line the opening `"""` is on, and a line that holds only whitespace is written empty. Spaces after the opening `"""` are removed. No formatter checked re-indents strings, but none has a lexer that marks the indentation the value drops, so for them it would change the value. For Quince it doesn't, and code moved into a deeper block would otherwise keep a string at its old indentation.

### 7. Hugging

A call whose last argument is a closure, a multi-line string, or a call that itself hugs, named or not, keeps that argument on the call's line and breaks only inside it, when the arguments before it fit on that line and none of the arguments holds a comment. A call whose only argument is a list or map literal hugs it the same way. Otherwise the call breaks its arguments one per line. rustfmt and Prettier both hug the last argument (rustfmt checked):

```
let total = orders.fold(Money.ZERO, fn(sum, order) {
  let line = order.price * order.quantity
  sum + line
})
```

This is a `choice` (decision 3): the hugged layout when its line fits up to the opening of the hugged argument, the broken list otherwise. So an outer call around a hugging one, as in `foo(bar, items.map(fn(i) { ... }))`, hugs the inner call, to any depth: each level is a `choice` measured on its own line, and documents are shared, so nesting costs nothing extra.

*Alternative:* no hugging. Rejected: every call that takes a closure would put the closure on a line of its own, and `r[item.arg.named]` moves the function-typed parameter last so that the closure reads like a block.

### 8. Rewrites

Each rewrite gives one construct one written form, and none changes meaning. Each is purely syntactic: it needs nothing but the file's tree.

| Rule | Rewrite | Layer |
| --- | --- | --- |
| `r[lex.source.canonical]` | Every line ending is a line feed, inside multi-line strings too (`r[lex.lit.multiline-value]` makes that value-preserving), and a byte order mark is removed | 1 |
| `r[tool.fmt.hex-digits]` | Hex digits uppercase in integer literals, and under question 2 in the `\u{...}` and `\x` escapes the lexer decodes: never in raw strings, tagged literals or comments, and never in `\\x`, an escaped backslash | 1 |
| `r[tool.fmt.shorthand]` (question 1) | `name: name` becomes `name:`, in calls, struct literals and field patterns | 4 |
| `r[tool.fmt.arm-braces]` (question 9) | A one-expression arm is written without braces when it starts on the arm's line, and in braces when it can't | 4 |
| `r[mod.import.one-line]` | Two imports of one package become one; `import p.{self}` alone becomes `import p` (question 10) | 5 |

Spaces, tabs and trailing whitespace outside literals are layout, not rewrites. Under question 3, `- 1` written as `-1` is layout too, since both are one literal. A rewrite doesn't apply across a comment (decision 5).

**Rewrites that need names or types.** `r[item.arg.fmt-positional]` writes the first argument positionally "when the first argument may be positional", which depends on the callee's parameters: a named first argument may be for a later parameter when the first has a default (`r[item.arg.default]`). `r[item.arg.named]`'s rewrite needs to know that the last parameter has a function type. `r[type.infer.required]`'s removal of `-> Unit` needs to know that `Unit` is the prelude's, since a generic parameter named `Unit` (`fn f<Unit>() -> Unit`), or a type a package declares, changes what it means. None of these is syntactic. No formatter checked does a rewrite that needs names or types: rustfmt leaves `-> ()` alone (checked), and Clippy, swift-format and ktlint make the same rewrites lint rules with fixes (reported). Question 7 proposes the same for Quince: each becomes a `check` finding, treated like unformatted code, with a `safe` fix that `quince fix` applies. So `fmt` stays a function of one file, which formats a file that doesn't type-check, and needs no project to run.

### 9. Imports

Imports come before declarations (`grammar.file`), and the formatter rewrites them as one block (`r[tool.fmt.imports]`, `r[mod.import.one-line]`):
- **Merge.** Imports of the same package path, neither with `as`, become one import: `import std.log` and `import std.log.{Log}` become `import std.log.{self, Log}` (`r[mod.import.self]`). A name imported twice is written once. `import p.{self}` alone is written `import p` (question 10). An import with `as` is never merged, and two that import one package under different names are left for M2's checker. A merged list of names is a list (decision 6), and breaks like one.
- **Group.** The standard library first (imports whose first name is `std`), then other packages, then the project's own library, named in `FormatOptions` (decision 1). With no name given, there's no third group. Each group is separated by one blank line.
- **Sort** (question 10). Within a group, by package path, comparing names one at a time. Names compare as rustfmt's 2024 style compares them: runs of digits as numbers, so `http_v2` comes before `http_v10`, which matters because a second major version is imported by its manifest key (`r[mod.import.major]`); anything else in byte order. Two names equal that way, such as `a01` and `a1`, compare in byte order, so the order is total. Inside braces, `self` first, then the names, compared the same way.
- **Comments.** Comment lines at the very start of the file, before the first import, stay at the top: they are a file header, such as a fixture's directives, even though the tree puts them in the first import's node. Other comment lines above an import move with it, whether a blank line separates them or not. A trailing comment stays on its import's line. When two imports merge, their comment lines go above the merged one in source order, and a trailing comment becomes a comment line there, so none is lost.

This is the only place the formatter reorders code.

### 10. Checking the output

Before `format` returns, it checks its output, as Black does:
- **No errors.** The output parses with no syntax error.
- **The same tree.** The output's tree, with trivia removed, equals the input's tree with trivia removed: the same node kinds, nested the same way, with the same token texts. Before comparing, the input tree has the rewrites of decision 8 applied: hex digits uppercased, shorthand, and its import block replaced by the merged, sorted block. Two differences are ignored on both sides instead, since whether the formatter makes them depends on its layout: a trailing `,` before a closing bracket, and braces around an arm body that is one expression, so `=> { e }` equals `=> e`. A multi-line string compares by its value (`literal::string_value`), not its `StringIndent` text.
- **The same comments, in the same places.** Each comment is recorded with the significant tokens before and after it, and the two sequences must match, outside the imports in order, and within the import block as a multiset. A trailing `,`, and the braces around an arm body that is one expression, are ignored in that record, as in the tree comparison, so `b // c` followed by `)`, which becomes `b, // c`, records the same. A comment allowed to move past a `,` (decision 5) is recorded after it on both sides. The file header's comments, kept at the top (decision 9), compare in order as the start of the file. Comments compare without trailing spaces and tabs. Doc comments are significant tokens, so the tree comparison trims them the same way.
- **Stable.** Formatting the output again gives the output. That second format runs without these checks, so it doesn't recurse.

If a check fails, `format` returns `Internal`, and the CLI writes nothing and asks for a bug report. So a formatter bug can stop `fmt`, but can't damage a file.

The tree comparison is what catches a line break in the wrong place: `let x = a` then a line `- b`, or `f` then a line `(x)`, keeps the same tokens but parses as two statements, so its tree differs. A comparison of tokens alone would miss it. Black compares ASTs for the same reason. The parser's tree dump without trivia gives the comparison.

The check doesn't guard the rewrites themselves, since it applies them to the input. They are small, purely syntactic, and each has unit tests. It guards the layout, the large and risky part.

*Cost:* two more parses and one more format per file. Black pays the same by default and offers `--fast` to skip it. Quince parses a large file in milliseconds, so there's no `--fast`.

### 11. No typed AST

The parser doc left typed accessors over the CST to the first stage that needs them. The formatter doesn't. Each printer walks its node's children in order, printing each token and child as it meets it, so a printer can't forget one, which is the mistake an accessor-based printer can make. A few helpers find what a layout rule needs (`child_of_kind`, `has_comment`). M2's name resolution asks questions of a node ("this function's parameters"), which is what typed accessors are for, so M2's doc decides them, and whether to generate them.

### 12. The CLI

- `quince fmt <file>...` formats each file in place. `quince fmt -` reads standard input and writes standard output, as Black's `-` does, for editors and pipes.
- `quince fmt --check <file>...` writes nothing, and prints the path of each file whose formatted text differs, one per line, on standard output.
- `quince fmt --check -` reads standard input, writes nothing to standard output, and exits 1 when the text would change.
- **Exit codes:** 0 when every file is formatted, or was, and nothing was refused; 1 when `--check` found a file that would change; 2 when any file was refused, couldn't be read or written, or hit `Internal`, and for a usage error. A refused file is left as it was, and the others are still formatted. Refused standard input writes nothing to standard output. Black uses 123 for an internal error. One code for "fmt couldn't do its job" costs a script the ability to tell a usage error from a refused file without reading standard error, which is acceptable while JSON output (M5) is the interface for tools.
- **Errors** go to standard error, one line each: `path:line:column: CODE message`. That's provisional: JSON output for every command is M5's.
- **Writing.** A file is written only when its text changes. It is written to a temporary file in the same folder as its target, given the original's permissions, and renamed over it, so an interrupted run never leaves half a file. A symbolic link is followed, and its target is written. A file named twice is formatted once.
- No file arguments is a usage error in M1. M2 makes `quince fmt` with no arguments format the project, `testdata/` excluded, along with `quince.toml`, as `cargo fmt` does over rustfmt.

*Alternative:* no CLI in M1, only fixtures. Rejected: each milestone ends in a command that works on real code ([milestones](../milestones.md)), and `quince fmt` on files needs nothing M2 adds.

### 13. Unformatted code in `quince check`

`r[tool.fmt.check]` makes unformatted code fail `check`. M2 wires `check`, so M2's doc adds the diagnostic, cited by `check` fixtures, and question 7's findings join it in M2 and M3. This doc fixes what M2 needs from the formatter: `format` is a function of the text and `FormatOptions` alone, so `check` can compare a file with its formatted text, and the fix can be the edits between them. A file `fmt` refuses already fails `check` with its syntax errors.

### 14. Testing

- **Engine unit tests** lay out hand-built documents at small widths, so each rule of decision 3 has a test that doesn't depend on Quince, `choice` with a `hard_line` inside included.
- **Printer unit tests** format a text and compare it with hand-written expected text, as the parser's tree dumps are hand-written: no snapshot crate, and no command that writes expected text from the formatter's output.
- **Deep inputs.** Formatting inputs nested to the limit runs on 2 MiB threads, as the parser's deep-nesting tests do, and within a time bound. The printer recurses over the tree, which the limit bounds. The engine doesn't recurse.
- **`fmt` fixtures** (below) cite the rules in decision 8's table and `tool.fmt.*`.
- **Every fixture.** `fuzz::check_format(&[u8])` decodes and formats. When `format` succeeds, it checks that the result is idempotent and has no line ending but LF. When it returns `Errors`, it checks that the parse had errors. `Unsupported` is accepted until layer 6, which removes the variant. `Internal` always fails. A unit test runs it on every fixture, `fmt` expected outputs included, as the parser's fuzz check runs. That checks M1's idempotence criterion (decision A).
- **The fuzz target** `format` calls `check_format`. Random bytes rarely parse with no error, so its seed corpus is the `fmt` fixtures and their expected outputs, and the error-free `parse` fixtures. As for the parser, a run happens before M1 closes, and every crash it finds gets a regression test.

**The `fmt` fixture mode.** gofmt's tests pair an `.input` file with a `.golden` one, and rustfmt's a `source` file with a `target` one (reported). The fixture README gains:
- `// mode: fmt` formats the fixture. It must parse with no diagnostics, and `//~` expectations are an error in this mode.
- A sibling `<name>.fmt` holds the expected output, byte for byte, named for what it holds as `.stdout` is. With no `.fmt` file, the fixture must already be formatted, so the output must equal the fixture.
- The expected output, formatted again, must equal itself, so every `fmt` fixture checks idempotence directly.
- A `.fmt` file with no `.qnc` file of the same name is an error, as for `.stdout`, and so is a `.fmt` file beside a fixture in another mode.
- The fixture's header is part of its text, so the expected output starts with the same header (decision 9 keeps it at the top). `.gitattributes` keeps bytes as they are, so a fixture can hold CR LF or a byte order mark for `r[lex.source.canonical]`.

`Backend` gains `format(&self, path, source) -> Option<Result<Vec<u8>, FormatFailure>>`, where `FormatFailure` is either the diagnostics of a refused file or an internal error's message. The runner reports either as the fixture's failure. `SyntaxBackend` implements it.

**Coverage honesty.** A `fmt` fixture cites a rule only when a formatter that broke it would fail the fixture. A rule about where a break goes, such as `r[lex.line-end]`, is cited by a fixture whose expected output breaks a long expression after its operator.

### 15. Delivery as a stack

Each layer builds and passes `cargo xtask ci` on its own. Until layer 6, `format` returns `Unsupported` for a node kind no printer handles yet, and `check_format` accepts it, so the idempotence check covers what the layers so far support.

- **`m1/neg-literal-spacing`** (parser, under question 3): a minus before a number is part of the literal whatever trivia lies between, in expressions as in patterns. Its own PR, since it changes the parser and its unit tests.
- **`m1/test-header-lines`** (parser, under question 4): `forall`, `uses` and `{` lines continue a `test` header. Removes parser layer 9a's E0018 for them, and updates `parse/items/test_decl.qnc`.
- **`m1/unit-type`** (parser, under question 8): `()` leaves the grammar's `type`. These three parser PRs come before layer 0 and don't depend on each other.
0. `m1/fmt-engine`: the layout engine and its unit tests. No Quince.
1. `m1/fmt-core`: `format`, `FormatOptions`, `FormatError`, refusal; comments and blank lines; the canonical bytes and hex digits; the generic list printer, with groups and trailing commas; printers for the file, `fn` with parameters, blocks, `let`, literals, names, calls and fields; the output check; the `fmt` fixture mode and `Backend::format`; `check_format` over every fixture; the README's Formatting section; M1's exit criterion reworded (decision A). Spec questions 2, 5 and 11.
2. `m1/fmt-decls`: every declaration and member, attributes, doc comments, signatures, generics, fields, variants, types; blank lines between declarations. Spec question 6.
3. `m1/fmt-exprs`: every expression, closures, chains, right-hand sides, multi-line strings, hugging.
4. `m1/fmt-blocks`: statements, `if` and `else`, `match` and `select`, patterns, the concurrency forms, field shorthand, arm braces. Spec questions 1 and 9.
5. `m1/fmt-imports`: decision 9. Spec question 10.
6. `m1/fmt-cli`: `quince fmt`, `-` and `--check`; `Unsupported` removed; the fuzz target and its seeds; `AGENTS.md`; the fuzz run.

Question 7 changes no code in M1: its findings land in M2 and M3. Its spec change lands with layer 1, so `fmt` never claims rules it doesn't enforce. Layers should come in at 500 to 900 changed lines, not counting fixtures' expected outputs. Layer 1 is the largest: if it runs past 1,000 lines, the fixture mode moves to a layer 1b, with layer 1's fixtures. Layer 1's `fn` printer has no signature rules until layer 2, so its fixtures keep headers short.

## Spec questions

Each gives proposed wording. Every ID uses a construct already in `spec/chapters.txt`, and a question that changes a rule's meaning retires its ID and adds a new one (`AGENTS.md`, "Rule IDs").

**1. Field shorthand outside struct literals (layer 4).** `r[tool.fmt.field-shorthand]` rewrites `profile: profile` in a struct literal. The parser can't tell a struct literal from a call (parser decision 6), and `r[item.arg.shorthand]` gives calls the same shorthand. Field patterns have it too (`Rect(width:, ..)`). The spec also says the shorthand passes "a binding of the same name", which leaves open what `handler:` means when `handler` is a function. Proposed:
- retire `tool.fmt.field-shorthand`, and add `r[tool.fmt.shorthand]` "`quince` `fmt` rewrites `name: name` to `name:` in struct literals, arguments and field patterns, so each has one form."
- In `r[item.arg.shorthand]`, keeping its ID since it says what was meant: "`name:` alone means `name: name`, whatever `name` refers to, so `handle(req, bg:)` means `handle(req, bg: bg)`."

*Precedent:* in Rust and JavaScript, `Foo { x }` means `x: x`, whatever `x` resolves to (reported). rustfmt has the rewrite, off by default (checked). The spec already chose to make it the format.

**2. Hex digits in escapes (layer 1).** `r[tool.fmt.hex-digits]` says hex digits are written uppercase, and `r[lex.lit.int.prefix]` that they "may be either case", but neither says whether escapes count. Proposed, keeping the ID, since every spec example already writes escapes uppercase: "`quince` `fmt` writes hex digits uppercase, in integer literals and in `\u{...}` and `\x` escapes."

*Precedent:* Black uppercases hex digits in numbers but lowercases them in escapes (checked). That split follows Python's own `repr`, which writes `\xab`. Quince has no such convention to match, and one rule for every hex digit is simpler to learn and to check.

**3. `- 1` is `-1` (parser PR, before layer 0). Reopens the parser doc's question 5.** That question made `- 1` a prefix minus applied to `1`, and `-1` a literal. The formatter writes no space after a prefix operator, so `- 1` would become `-1` and change the tree. Usually the value is the same, but not at each signed type's minimum: `- 128` as an `Int8` is a minus applied to an out-of-range literal, a compile error, and `-128` is valid. The same goes for `- 0` as an unsigned type. So `fmt` would turn errors into valid code, which its output check would have to allow.

*Precedent:* Rust compiles `let a: i8 = - 128;` with no error (checked), and OCaml's parser folds `- 1` into a negative constant (reported): the space doesn't matter. Swift makes a space after a prefix operator an error (checked). Quince's own patterns already fold `- 5` into `-5` (parser decision 5, "as in Rust").

Proposed: retire both `lex.lit.int.neg` and `lex.lit.neg-adjacent`, since the first says "directly before" and the second defines it, and add `r[lex.lit.neg-literal]` "A minus in prefix position before an integer or float literal is part of the literal, whatever spaces, line breaks or comments lie between them, unless a postfix operator follows the literal. So `Int.MIN` can be written `-9_223_372_036_854_775_808`, `- 1` and `-1` are the same literal, and `-2.pow(2)` is `-(2.pow(2))`, as `-x.abs()` is." (Revised after approval, found in review pass 2: the approved wording left `lex.lit.int.neg` saying "directly before". The meaning David approved is unchanged.) The parser puts any trivia between them inside the `Literal`, as it does in patterns. A break after a prefix `-` is already spacing, so a `Literal` can span lines: the formatter drops spaces and line breaks inside it, and keeps a comment there with the line break it needs. Then `fmt` writing `-1` is layout, not a rewrite, and Quince has one rule for negative numbers in expressions and patterns.

*Alternative:* Swift's rule, making `- 1` an error. Rejected: it makes spacing a syntax error, which `r[tool.fmt.check]` avoids ("unformatted code compiles").

**4. Line breaks in `test` headers (parser PR, before layer 0).** `r[lex.continue.fn-header]` covers `fn` headers only, so parser layer 9a makes a line that starts with `forall`, `uses` or `{` in a `test` header an E0018. A long test header then can't break at all outside its parameters, and a long name can't break anyway. Proposed, a new rule after it: `r[lex.continue.test-header]` "Inside a `test` header, a line that starts with `forall`, `uses` or `{` continues the header, so `quince` `fmt` breaks a long test header as it does a signature." The parser's E0018 for those words goes.

*Alternative:* leave it, since test headers are rarely long.

**5. What a column is (layer 1).** `r[tool.fmt.width]` says "100-column limit", but not what a column is, or what happens with a string longer than 100 columns. Proposed, keeping the ID, since the rule can only have meant this: "100-column limit, counted in characters; the formatter decides all line breaks. A line runs past 100 columns only where no line break may go, as in a long string, and a comment at the end of a line doesn't count toward it."

*Precedent, and why Quince deviates:* Prettier, rustfmt and Black count display width, where an East Asian wide character is two columns (reported). That width comes from a Unicode table that changes with each Unicode release. Under `r[tool.fmt.check]`, a compiler upgrade that changed a character's width could make formatted code fail `check`. Characters never change. Code outside literals and comments is ASCII (`r[lex.source.ascii]`), trailing comments don't count, and so the difference shows only in a line whose string holds wide characters. Not counting trailing comments is Prettier's choice (reported).

**6. Blank lines (layer 2).** "The formatter decides all line breaks" doesn't say whether it keeps an author's blank lines, and the output depends on the answer (decision 4). Proposed, a new rule after `r[tool.fmt.width]`: `r[tool.fmt.blank-lines]` "No blank line comes at the start of the file, after an opening bracket, before a closing one, after an attribute or a doc comment, between imports of one group, or inside an expression or a signature. Otherwise, a declaration or member that spans several lines, or has doc comments or attributes, has one blank line before and after it; import groups are separated by one blank line, and one comes between the last import and the first declaration; and elsewhere one blank line is kept where the source had one or more." (Reworded after approval, found in review pass 2, to say what decision 4 says: the approved wording put the bracket exception after the rule it overrides, and left out imports and attributes.)

*Precedent:* Black, and gofmt where it agrees (decision 4). Keeping the author's choice between one-line declarations keeps most `effect` and `foreign` blocks in the spec as written. Two change: `effects.md:308`'s `fn run(...)` runs past 100 columns, so it breaks and gets blank lines around it, and `interop.md:24`'s `@borrowed` function gets one before it.

**7. Rewrites that need names or types become `check` findings (layer 1's spec change, code in M2 and M3).** Three rules make `fmt` do something it can't do from one file's tree (decision 8). Proposed:
- retire `item.arg.fmt-positional`, and add `r[item.arg.positional-first]` "When the first argument may be positional, writing its name is reported by `quince` `check` like unformatted code (`r[tool.fmt.check]`), with a fix that removes it. So `http.serve(8080``, handler: h)` is the one form, even when the first parameter's name carries meaning: `Channel<Job>.new(16)`, `log.json(cfg.log_level)`."
- retire `item.arg.named`, and add `r[item.arg.names]` with its text, ending "...it is written without its name, whether it is a closure or a named function, and `quince` `check` reports the other form like unformatted code, with a fix that rewrites it."
- retire `type.infer.required`, and add `r[type.infer.annotations]` with its text, the parenthesis reading "(leaving one out means Unit, and `quince` `check` reports a written `-> Unit` like unformatted code, with a fix that removes it; the same holds in a function type such as `fn(Int) uses Log`)".

*Precedent:* formatters are syntax-only, and these rewrites are lint fixes elsewhere: Clippy's `unused_unit`, swift-format's and ktlint's unit-return rules (reported). For Quince the effect on CI is the same, since `check` fails on both, and `fmt` stays a function of one file, which formats a file that doesn't type-check and needs no project.

*Alternative:* keep them in `fmt`, which then needs the project and the type checker, and formats a file differently when another file has a type error. It would also need a ruling on shadowing for `-> Unit`, which the first draft proposed (prelude names reserved), and which still missed generic parameters.

**8. The unit type has two spellings (parser PR, before layer 0).** Found while writing question 7. The grammar's `type` takes `"(" ")"` as the unit type, and the prelude names it `Unit`, so `Result<(), E>` and `Result<Unit, E>` are two forms. The spec's examples write `Unit`, and none writes `()` as a type. Proposed: remove `"(" ")"` from `type`, so `()` is only the value, as `r[type.unit]` ("`Unit`: The empty value, written `()`") already reads, and add to it, keeping its ID since the type's name doesn't change: "The type is written `Unit`, and its one value `()`." A `()` where a type goes is an E0016. No fixture uses it.

*Precedent:* Kotlin writes the type `Unit` and has no `()`; Rust has only `()`; Swift has both, `Void` and `()`, and swift-format has a rule to pick one (reported). Quince's examples already follow Kotlin.

*Alternative:* leave both, and have question 7's finding cover `-> ()` too.

**9. Braces around a one-expression arm (layer 4).** `r[expr.match.arm-syntax]` allows `pattern => expr,` and `pattern => { block },`, so `0 => { 1 },` and `0 => 1,` are two forms of one arm. Revised after approval, and confirmed by David, 2026-11-13 (review pass 2 found the approved version only half of rustfmt's rule). Proposed, a new rule: `r[tool.fmt.arm-braces]` "`quince` `fmt` writes an arm whose body is a single expression without braces when the expression starts on the arm's line, and in braces when it has to start on the next one. A map literal stays in braces, since `=> {` starts a block (`r[expr.match.arm-block]`)."

*Precedent:* the Rust style guide, rustfmt's default (checked: `0 => { 1 }` becomes `0 => 1`, and a body too long for the arm's line gets braces, `0 => {` with the body on the next line). The approved version removed the braces always, and broke after `=>`, which is rustfmt's non-default setting. With braces a body never starts on the line after `=>`, so a long arm reads like a short block.

**10. Import order, and `{self}` alone (layer 5).** `r[tool.fmt.imports]` says "sorted" without an order, and `import p.{self}` alone is a second form of `import p`. Proposed, keeping the ID, since "sorted" needed an order: "Imports sorted and grouped (standard library, then packages, then local). Paths and names are compared name by name, with runs of digits compared as numbers, so `http_v2` comes before `http_v10`, and inside braces `self` comes first." And added to `r[mod.import.self]`: "`import p.{self}` alone is written `import p`."

*Precedent:* rustfmt's 2024 style (checked).

**11. Trailing commas where a list isn't bracketed (layer 1).** `r[tool.fmt.trailing-comma]` says "all multi-line lists", but a `uses` list and `with` handlers take no trailing comma (parser layer 2a). Proposed, keeping the ID, since the parser already reads it this way: "Trailing commas required in every multi-line bracketed list, such as arguments, parameters, fields, variants, collections, and arms." ("such as" added after approval, found in review pass 2, so the list doesn't read as complete.)

## Risks and trade-offs

- [The format is wrong in ways only real code shows, and changing it later makes formatted code fail `check`] → Until 1.0 the format can change, at the cost of one `fmt` run per project. Each rule follows a precedent or says why it doesn't.
- [A comment or a line break lands where it changes meaning] → Decision 5 argues it can't, and decision 10 compares trees on every run, so the worst case is a refused file and a bug report. The fuzz target looks for such inputs.
- [Hugging makes layout slow on deep inputs] → `choice` measures one line, documents are shared, and the engine is iterative. A test formats inputs nested to the limit within a time bound.
- [Refusing files with errors makes the idempotence criterion trivial on most fixtures] → Decision A. The `fmt` fixtures and the error-free fixtures carry it, and layer 1's PR states the count.
- [Two questions reverse earlier choices: question 3 reopens a decided spec question, and question 7 moves rewrites the spec gives to `fmt`] → Each is argued from precedent and the formatter's needs. If either is declined, decision 8 and the output check change, not the engine or the stack.
- [Spec examples drift from the format] → Decision B: one spec PR formats them once `quince fmt` exists, and each layer lists what it would change.
- [The output check applies the rewrites to its input, so it can't see a wrong rewrite] → Rewrites are syntactic, small and unit-tested.
- [Eleven spec questions] → Four only clarify wording. Each is small and lands with the layer that needs it.
- [Layer 1 is large] → Decision 15 gives the split.

## Rollback

Revert the stack's PRs. Nothing changes outside `quince_syntax`, the conformance runner and its fixtures, `quince_cli`, `AGENTS.md`, `docs/milestones.md`, the fuzz crate and `spec/`, and nothing depends on the formatter until M2 wires `check`. The three parser PRs revert on their own.

## For later milestones

- **M2:** the unformatted-code diagnostic in `check` (decision 13), and question 7's `-> Unit` finding once names resolve; `quince fmt` over a project, `testdata/` excluded; `quince.toml` formatting; `FormatOptions` filled from the manifest; typed accessors (decision 11).
- **M3:** question 7's argument findings, once calls resolve.
- **After layer 6:** one spec PR formats the spec's code blocks (decision B).
- **M5:** `quince fix` formats each file it changes, so one command reaches the canonical form under question 7, as `go fix` prints formatted code (found in review pass 2); JSON output for `fmt`; formatting part of a file for the language server; whether the language server formats around errors (decision 2).

## Tasks

Rule IDs in brackets are what each task's fixtures cite.

**Before building**
- [x] Design review, pass 1: Ready with changes. Its findings are addressed in this revision.
- [x] Design review, pass 2: Ready with changes. Its findings are addressed in this revision.
- [x] Design review, pass 3: Ready with changes. Its findings are addressed in this revision.
- [x] Design review, pass 4: Ready with changes. Its findings are addressed in this revision.
- [x] Design review passes (pass 5: Ready to build).
- [x] David decides A, B and C (2026-11-13: approved as proposed).
- [x] David decides spec questions 1 to 11 (2026-11-13: approved as proposed).
- [x] David confirms question 9 as revised after approval (2026-11-13).

**Parser PRs, before layer 0**
- [ ] `m1/neg-literal-spacing`: question 3, and a bump. The parser's unit tests for `- 1`. The doc comments that cite the retired IDs (`parser/exprs.rs` and `tests/exprs.rs`) cite the new one. [`lex.lit.neg-literal`]
- [ ] `m1/test-header-lines`: question 4, and a bump. [`lex.continue.test-header`]
- [x] `m1/unit-type`: question 8, and a bump. [`type.unit`, `grammar.type`]

**Layer 0: `m1/fmt-engine`**
- [ ] The document type and the iterative engine: groups, the three line kinds, `indent`, `if_break`, `line_suffix`, `choice`, shared documents; width in `char`s; unit tests at small widths.

**Layer 1: `m1/fmt-core`**
- [ ] Spec: questions 2, 5, 7 and 11, and a bump.
- [ ] `format`, `format_str`, `FormatOptions`, `FormatError`; refusal of lossy text and syntax errors.
- [ ] Comments and blank lines (decisions 4 and 5).
- [ ] Line feeds, the byte order mark, hex digits. [`lex.source.canonical`, `tool.fmt.hex-digits`]
- [ ] The list printer, with groups and trailing commas; printers for the file, `fn` with parameters and a return type, blocks, `let`, literals, names, calls and fields. [`tool.fmt.one-per-line`, `tool.fmt.trailing-comma`, `tool.fmt.width`]
- [ ] The output check (decision 10).
- [ ] The `fmt` fixture mode, `Backend::format`, `FormatFailure`, and the fixture README.
- [ ] `fuzz::check_format`, its unit test over every fixture, and the deep-input tests.
- [ ] `compiler/syntax/README.md`: the Formatting section. `AGENTS.md`'s map. `docs/milestones.md`: M1's exit criterion reworded (decision A).

**Layer 2: `m1/fmt-decls`**
- [ ] Spec: question 6, and a bump.
- [ ] Every declaration and member, broken one per line; attributes on their own lines; doc comments; generics, bounds, fields, variants; types and function types.
- [ ] Signatures (decision 6). [`lex.continue.fn-header`, `lex.continue.test-header`]
- [ ] Blank lines between declarations. [`tool.fmt.blank-lines`]

**Layer 3: `m1/fmt-exprs`**
- [ ] Every expression; binary expressions broken after their operators; chains; right-hand sides. [`lex.line-end`, `lex.continue.more`]
- [ ] Closures; multi-line strings re-indented through their `StringIndent` tokens; hugging (decision 7).

**Layer 4: `m1/fmt-blocks`**
- [ ] Spec: questions 1 and 9, and a bump.
- [ ] Statements; `if` and `else`; `match` and `select` arms; `concurrent`, `background`, `with`, `spawn`; patterns. [`tool.fmt.one-per-line`]
- [ ] Field shorthand and arm braces. [`tool.fmt.shorthand`, `tool.fmt.arm-braces`]

**Layer 5: `m1/fmt-imports`**
- [ ] Spec: question 10, and a bump.
- [ ] Merging, grouping and sorting, with comments and the file header; the local group from `FormatOptions`, unit-tested. [`tool.fmt.imports`, `mod.import.one-line`, `mod.import.self`]

**Layer 6: `m1/fmt-cli`**
- [ ] `quince fmt`, `-` and `--check` (decision 12), tested by running the binary on a temporary folder: exit codes, permissions kept, a symbolic link followed.
- [ ] `Unsupported` removed; `check_format` then requires every error-free input to format.
- [ ] The `format` fuzz target and its seeds; the command in `AGENTS.md`.
- [ ] Before M1 closes: run both fuzz targets, and fix every crash each finds, with a regression test. The PR records the runs.
