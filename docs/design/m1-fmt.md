# M1: Formatter

Status: draft, for design review. This is the third and last design doc of M1 (Syntax), after [the lexer](m1-lexer.md) and [the parser](m1-parser.md).

## Why

M1 ends when every `parse` and `fmt` fixture passes and `fmt` is idempotent on every fixture ([milestones](../milestones.md)). The parse, fuzz and lossless criteria are met. No formatter code and no `fmt` fixtures exist yet.

The formatter matters more to Quince than to most languages, for three reasons:
- `r[tool.fmt.check]` makes unformatted code fail `quince check`. So the formatter's output is the only accepted layout of every program. Changing it later makes existing code fail `check`.
- Agents run it after every edit. It must never change what a program means, never damage a file, and give the same output for the same tree, whatever the input's layout was.
- Several spec rules are rewrites that only `fmt` performs: field shorthand, positional first arguments, uppercase hex digits, merged imports and a removed `-> Unit`. Each one gives a construct one written form.

## Impact and approvals

- **Code:** `quince_syntax` gets a `fmt` module, and its README gets a Formatting section, the format's contract. The conformance runner gets a `fmt` mode and `Backend::format`. `quince_cli` gets `quince fmt` (decision 12). `fuzz` gets `check_format` and a second fuzz target.
- **Dependencies:** none. The layout engine is our own (decision 3).
- **Decisions for David:**
  - **A. Files with errors.** `fmt` refuses a file with any syntax error (decision 2). Only 15 of the 111 fixtures have no expected diagnostic, so M1's exit criterion "`fmt` is idempotent on every fixture" is read as: on every fixture `fmt` accepts, `fmt(fmt(x)) == fmt(x)`, and on every other fixture `fmt` refuses and writes nothing. The `fmt` fixtures this doc adds are what make the criterion bite. The alternative is formatting around errors, which decision 2 rejects.
  - **B. Rewrites that need the checker.** `r[item.arg.fmt-positional]` and the rewrite in `r[item.arg.named]` need the callee's parameters, which only M3's type checker knows. M1's formatter is syntax-only, and those two rules move to M3 (decision 8).
  - **C. `quince fmt` in M1.** The CLI gets `quince fmt <file>...` and `--check` on the files named, with no project walk, which needs M2's project model (decision 12).
- **Spec:** Spec questions, below, has seven questions, each with proposed wording. Each lands in the layer that enforces it, with a minor version bump.
- **Diagnostic codes:** none. Refusal reports the parser's diagnostics. The diagnostic `quince check` gives an unformatted file is M2's, when `check` is wired (decision 13).
- **Delivery:** a stack of seven PRs (decision 15).

## Context

- `quince_syntax` has the lexer, the parser, the lossless CST (`compiler/syntax/README.md`, "The CST" and "Node shapes"), and the case check. `parse(&Decoded) -> Parse` gives the root and every error. The tree's text is the input, byte for byte.
- The parser doc's decision 3 places trivia in the tree: comment lines directly above a node whose kind `attaches_comments()` go inside it, and other trivia goes in the innermost node that holds the tokens on both sides. The kinds that take comments are the declarations, `Import`, members, fields, variants, and match and select arms. Its "For the formatter doc" section lists what this doc must decide.
- The lexer splits each line ending into a `Newline` token, and each multi-line string's stripped indentation into `StringIndent` tokens (lexer decision 8), so the formatter can count blank lines and re-indent a string without changing its value.
- Every `;` is an E0002 and every block comment an E0003. So in a file without errors every comment is a `//` or `///` comment, and every comment ends its line.
- The nesting limit (parser decision 10) bounds the tree at 256 levels, so the formatter may recurse over it.
- The spec's formatting rules are in Toolchain (`r[tool.fmt.*]`). Others are scattered: `r[lex.source.canonical]`, `r[lex.line-end]`, `r[lex.continue.fn-header]`, `r[type.infer.required]`, `r[mod.import.one-line]`, `r[item.arg.named]` and `r[item.arg.fmt-positional]`.
- The conformance runner has `check`, `parse` and `run` modes, and only `parse` is wired. The fixture README says `fmt` arrives as a mode in M1.
- `quince_cli` handles only `--version`.

## Goals and non-goals

**Goals:**
- `format`: a function from a parsed file with no errors to its canonical text, which depends only on the file's tree, comments and blank lines, never on the rest of its layout.
- It never changes a program's meaning: the output parses with no error to the same tokens, apart from the rewrites the spec asks for, and keeps every comment. It checks this on every run (decision 10).
- It is idempotent, on every input it accepts.
- A `fmt` fixture mode, and fixtures for every formatting rule M1 can enforce.
- `quince fmt` and `quince fmt --check` on named files.

**Non-goals:**
- The rewrites that need the callee (decision 8). M3's type-checker doc takes them.
- The diagnostic for unformatted code in `quince check` (`r[tool.fmt.check]`). M2 wires `check`.
- Formatting `quince.toml` (`r[dep.manifest.fmt]`), and walking a project's files. Both are M2's.
- Formatting the code in doc comments' examples. Doc comment text is kept as written.
- Formatting part of a file, or formatting as you type, for the language server. M5's doc decides what it needs: the same engine can format one declaration.
- Formatting a file with syntax errors (decision 2).
- Options. There is one format, with no settings.

## Decisions

### 1. Where it lives, and the API

The formatter is the `fmt` module of `quince_syntax`, as the repo plan has it, since it reads only the CST. Its parts are submodules:
- `doc`: the layout engine (decision 3), which knows nothing of Quince;
- `print`: one function per node kind, which turns the tree into the engine's documents;
- `comments`: placing comments and blank lines (decisions 5 and 6);
- `imports`: sorting, grouping and merging (decision 9);
- `verify`: checking the output (decision 10).

The API:
- `format(&Decoded, &FormatOptions) -> Result<String, FormatError>`, and `format_str` for text that never came from bytes, as with `parse` and `parse_str`.
- `FormatOptions` holds one field, the name of the project's own library, if there is one, which import grouping needs (decision 9). It isn't a style setting: M2's driver fills it from `quince.toml`.
- `FormatError` is one of: `Errors(Vec<SyntaxError>)`, the file's syntax errors; `Lossy`, the text isn't the file (decision 2); and `Internal(String)`, the check in decision 10 failed, which is a formatter bug.
- `format` parses the text itself, so no caller can format a tree from other text.

*Alternative:* a `quince_fmt` crate. Rejected for the reason the lexer and parser gave: the formatter needs `SyntaxKind` and the tree's internals, and the repo plan puts it in `quince_syntax`.

### 2. What `fmt` refuses

- **A lossy text.** When `Decoded::is_lossy()` is true the text isn't the file, so no writer may write it (lexer decision 9). `format` returns `Lossy` before parsing. Such a text always has an E0014 too, so this is a guard, not a separate case.
- **A file with any syntax error.** `format` returns the errors, and writes nothing. That includes errors that leave the tree whole, such as E0021 (case) or E0019 (parentheses needed): one rule, with nothing to judge per code. The CLI prints them (decision 12).

*Why:* formatting around errors means formatting a tree the parser guessed at. The formatter decides every line break, and joining or splitting lines in a `Skipped` region, or next to one, can change how the file parses once the error is fixed. gofmt, rustfmt and Prettier all refuse. Agents get the errors, fix them, and run `fmt` again.

*Alternative:* format every node outside `Skipped`, and print `Skipped` verbatim. Rejected for M1: it needs a rule for every boundary between formatted and verbatim text, and M1 has no consumer for it. The language server may want it, and M5's doc can add it. The cost is decision A: on most `parse` fixtures, idempotence is trivially true.

### 3. The layout engine

The formatter builds a document for the file, and the engine lays it out at 100 columns. It is Wadler's "prettier printer", the design Prettier uses. A document is built from:
- `text`, which never holds a line break;
- `line`, a space when its group is flat and a line break when it isn't; `soft_line`, nothing or a line break; `hard_line`, always a line break;
- `indent(doc)`, one level deeper on each line break inside it;
- `group(doc)`, laid out flat when all of it fits on the rest of the line, and broken otherwise;
- `if_break(broken, flat)`, for the trailing comma;
- `line_suffix(doc)`, text kept at the end of the current line, for a trailing comment (decision 5);
- `choice(first, second)`, which takes `first` when its first line fits, and `second` otherwise, for hugging (decision 7).

The engine lays groups out from the outside in: a group is flat only when everything in it fits, so the outer constructs break first. A `hard_line` breaks every group around it. The check that a group fits stops at the end of its line, so layout is linear in the document's size. `choice` checks only the first line of its first option, so nested choices stay linear too.

Indentation is two spaces per level, as in every spec example. Width is counted in Unicode scalar values (`char`s): one column each, a tab included. The engine never breaks inside a `text`, so a line holding a long string or name runs past 100 columns where nothing can break it (spec question 5).

*Alternatives:*
- A crate such as `pretty`. Rejected: the engine is about 400 lines, which we control and can shape to `line_suffix` and `choice`, against a new dependency.
- A gofmt-style formatter that keeps the author's line breaks and normalizes spacing. Rejected: `r[tool.fmt.width]` says the formatter decides every line break, so two layouts of one tree must format to the same text.

### 4. What the output depends on

The output depends on the tree's tokens, its comments, and where the input had blank lines. Nothing else in the input's layout counts: not its line breaks, not its indentation, not its spacing. So `f(\n  a,\n  b\n)` and `f(a, b)` format the same.

Blank lines (spec question 6):
- Between two declarations or members, exactly one blank line, whatever the input had. Between imports in one group none, and between groups one (decision 9).
- Between statements, fields, variants, arms and list items, one blank line where the input had one or more, and none where it had none.
- None after an opening bracket, before a closing one, or at the start of the file. The file ends with one line feed, unless it is empty.

Blank lines inside a block are the one piece of the author's layout that survives: they group statements, as paragraphs do, and no rule can recover that grouping from the tree.

### 5. Comments

The CST already holds every comment in place (parser decision 3). The formatter doesn't use which node holds a comment, only where it sits between significant tokens, with one exception, imports (decision 9). So this doc doesn't amend the tree's rules.

Each comment is one of two kinds:
- **A trailing comment** follows a significant token on its line. It is printed after that token, with one space before it, as a `line_suffix`, so it ends the line in the output too. When the formatter writes a `,` right after that token, the comment goes after the `,`. A trailing comment doesn't count toward the width, so adding or removing one never moves the code before it.
- **A comment line** starts its line. It is printed on its own line, indented as the token after it is, and keeps a blank line before it when the input had one. Comment lines before a closing bracket are indented as the bracket's contents.

Every comment ends a line, so a comment is a `hard_line`, which breaks every group around it. `f(a, // first` then a line `b)` gives `f(` and `a, // first` and `b,` on their own lines, then `)`.

A comment can't go where a line break can't. That holds in the input, which parsed. It holds in the output because the formatter keeps each comment between the same two significant tokens, and the parser's rules for where a break may go depend only on those tokens and the construct around them. Two rewrites change tokens, and they don't apply when a comment lies inside what they would change: `x: // c` followed by a line `x` keeps both `x`s, and `-> Unit // c` keeps its `-> Unit`. Decision 10 checks the rest.

Comment text is kept as written, apart from spaces and tabs at its end, which are removed. A doc comment is a comment line in its declaration's `DocComments`. Blank lines between the lines of one doc comment are removed, since they aren't part of its text.

### 6. Layout

These are the rules review checks the printers against. The README's Formatting section is their contract, with an example of each. Each printer is a short function over its node kind's children (Node shapes).

**One per line** (`r[tool.fmt.one-per-line]`). Each declaration, member, statement and match or select arm starts its own line. So a declaration's body, a block with a statement other than one expression, a `match` and a `select` always break.

**Lists.** Parameters, arguments, type arguments, generics, fields, variants, tuples, list and map literals, import names, patterns' fields, a `concurrent` spawn list, attribute arguments and `background`'s arguments are each a group: `(a, b, c)` when it fits, and otherwise one item per line, indented, with a trailing `,` (`r[tool.fmt.trailing-comma]`). Arms always break, and always end with `,`. A `uses` list and `with` handlers aren't bracketed, so they take no trailing comma: a long one breaks after its commas, indented, since a comma continues the line (`r[lex.line-end]`). An item that holds a comment or a doc comment breaks its list.

**Blocks.** `{` ends its line and `}` starts one. A block that holds one expression and no comment, outside a declaration, is a group: `{ x + 1 }` when it fits. That covers closures, the blocks of `if` and `else`, arms, and the blocks of `with`, `concurrent` and `background`. A declaration's body always breaks, so every declaration has one shape, and adding a statement to it never moves its header. `{}` stays `{}`.

**Signatures** (`r[lex.continue.fn-header]`: "`quince fmt` breaks a long signature before those words"). A `fn` header is its name, generics and parameters, then its tail: `-> T`, `uses E` and its contracts.
1. Each contract starts its own line, indented one level, as in Contracts' examples.
2. Otherwise the header is one line when it fits: `fn f(a: Int) -> Int uses Io {`.
3. When it doesn't, `-> T` and `uses E` each start a line, indented one level, as in Effects' examples, and the parameters stay on the first line if they fit there.
4. When they don't, the parameters break one per line, and `) -> T uses E {` follows them if it fits, and otherwise steps 3's lines follow `)`.
5. Whenever a tail part starts a line, `{` starts its own line, at the declaration's indentation.

A closure's header and a function type follow the same steps. A `test` header can't break before `uses` or `{` today (spec question 4): until that's decided, only its `forall` parameters break.

**Expressions** (`r[lex.line-end]`: "a long expression breaks after its operator, never before").
- A binary expression that doesn't fit breaks after its operators, all those at one level at once, with the lines after the first indented one level. Nested levels are groups of their own.
- A chain of two or more method calls or fields that doesn't fit breaks before each `.`, indented (`r[lex.continue.more]`).
- `let`, `var`, assignment, `const` and a default value break after `=` and indent the value, unless the value is one that breaks well inside itself, a call, a collection, a closure, an `if`, a `match` or a block form, which stays on the `=` line.
- An arm breaks after `=>` the same way, keeping a block or a call on the arm's line.
- A prefix operator is written with nothing between it and its operand (spec question 3), and a range with nothing around `..` or `..=`, as in `0..3`. Other binary operators have one space on each side.
- Spacing elsewhere follows the spec's examples: no spaces inside `(`, `[`, `<` or a map's `{`, one inside a block's `{ x }`, `name: value` with one space after the colon, and none in an interpolation's braces.

**Multi-line strings.** Their text is kept. The formatter rewrites only their `StringIndent` tokens, which changes no value (lexer decision 8): the closing `"""` and every content line are indented one level deeper than the line the opening `"""` is on, and a line that holds only whitespace is written empty. Spaces after the opening `"""` are removed. A group that holds one breaks, except where it is hugged (decision 7).

### 7. Hugging

A call whose last argument is a closure or a multi-line string keeps that argument on the call's line and breaks only inside it, as long as the arguments before it fit on that line. So the spec's own examples keep their shape:

```
counts.update(word, default: 0, fn(n) {
  n + 1
})
let page = render(title, """
  <h1>{title}</h1>
  """)
```

A call whose only argument is a list or map literal hugs it the same way. Otherwise the call breaks its arguments one per line. This is a `choice`: the hugged layout when its first line fits, the broken list otherwise.

*Alternative:* no hugging. Rejected: every call that takes a closure would put the closure on a line of its own, and `r[item.arg.named]` moves the function-typed parameter last so that the closure reads like a block.

### 8. Rewrites

Each rewrite gives one construct one written form, and none changes meaning.

| Rule | Rewrite | Layer |
| --- | --- | --- |
| `r[lex.source.canonical]` | Every line ending is a line feed, inside multi-line strings too (`r[lex.lit.multiline-value]` makes that value-preserving), and a byte order mark is removed | 1 |
| `r[tool.fmt.hex-digits]` | Hex digits uppercase, in integer literals and, under spec question 2, in `\u{...}` and `\x` escapes | 1 |
| `r[lex.lit.neg-adjacent]` | A prefix minus is written with nothing after it (spec question 3) | 1 |
| `r[type.infer.required]` | `-> Unit` is removed from a declaration, a closure and a function type, under spec question 7 | 2 |
| `r[tool.fmt.field-shorthand]` | `name: name` becomes `name:`, in calls, struct literals and field patterns, under spec question 1 | 3 |
| `r[mod.import.one-line]` | Two imports of one package become one line (decision 9) | 5 |

Spaces, tabs and trailing whitespace outside literals are layout, not rewrites. A rewrite doesn't apply across a comment (decision 5).

**Rewrites that need the checker** (decision B). `r[item.arg.fmt-positional]` writes the first argument positionally "when the first argument may be positional". A named first argument may be for a later parameter, when the first has a default (`r[item.arg.default]`), and then it can't be positional. `r[item.arg.named]` drops the name of the argument for the last parameter when that parameter has a function type. Both need the callee's parameter list, and a method call needs its receiver's type to find the callee. So both wait for M3, where the checker resolves calls. M3's doc decides how: the likely shape is a pass that runs after a file type-checks and hands the formatter the calls to rewrite. In M1 the formatter writes each argument as it finds it, named or not.

### 9. Imports

Imports come before declarations (`grammar.file`), and the formatter rewrites them as one block (`r[tool.fmt.imports]`, `r[mod.import.one-line]`):
- **Merge.** Imports of the same package path, neither with `as`, become one line: `import std.log` and `import std.log.{Log}` become `import std.log.{self, Log}` (`r[mod.import.self]`). A name imported twice is written once. An import with `as` is never merged, and two lines that import one package under different names are left for M2's checker.
- **Group.** The standard library first (paths that start with `std`), then other packages, then the project's own library, named in `FormatOptions` (decision 1). With no name given, there's no third group. The groups are separated by one blank line.
- **Sort.** Within a group, by package path, comparing names one at a time in byte order. Inside braces, `self` first, then the names in byte order.
- **Comments.** Comment lines directly above an import are inside its node (parser decision 3) and move with it. A comment line with a blank line after it, among the imports, moves with the import below it. A trailing comment stays on its import's line. When two imports merge, their comment lines go above the merged line, in source order, and a trailing comment becomes a comment line there, so none is lost.

This is the only place the formatter reorders code, and the only place the tree's comment attachment matters to it.

### 10. Checking the output

Before `format` returns, it parses its own output and checks:
- the output has no syntax error;
- its significant tokens equal the input's, after the rewrites in decision 8 are applied to the input's tokens, ignoring a `,` before a closing bracket, and with the imports compared as a set of what they import;
- its comments equal the input's, in order outside the imports, and as a multiset within them.

If a check fails, `format` returns `Internal`, and the CLI writes nothing and asks for a bug report. So a formatter bug can stop `fmt`, but it can't damage a file. Black checks its output the same way. Parsing twice costs little next to an agent's edit loop.

The check doesn't guard the rewrites themselves, since it applies the same rewrite to both sides. They are small, and each has unit tests. It guards the layout, which is the large and risky part: a line break where one can't go, a lost comment, a token printed twice.

Idempotence isn't checked on every run, since it costs a third parse and a second format. Tests and the fuzz target check it (decision 14).

### 11. No typed AST

The parser doc left typed accessors over the CST to the first stage that needs them. The formatter doesn't: each printer walks its node's children in order, which is what layout needs, using a few helpers (`child_of_kind`, `tokens`, `has_comment`). M2's name resolution asks questions of a node ("this function's parameters"), which is what typed accessors are for, so M2's doc decides them, and whether to generate them.

### 12. The CLI

`quince fmt <file>...` formats each file in place. `quince fmt --check <file>...` writes nothing, and lists each file whose formatted text differs.
- Exit code 0 when every file is formatted (or was formatted), 1 when `--check` found an unformatted file or any file was refused, and 2 for a usage error. A refused file is left as it was, and the others are still formatted.
- A refused file's errors go to standard error, one line each: `path:line:column: CODE message`. That's provisional. The JSON output of every command is M5's, and so is a shared human format.
- A file is written only when its text changes, through a temporary file in the same folder that is then renamed, so an interrupted run never leaves half a file.
- No arguments is a usage error in M1. M2 makes `quince fmt` with no arguments format the project, `testdata/` excluded, along with `quince.toml`.

*Alternative:* no CLI in M1, only fixtures. Rejected: each milestone ends in a command that works on real code ([milestones](../milestones.md)), and `quince fmt` on files needs nothing M2 adds.

### 13. Unformatted code in `quince check`

`r[tool.fmt.check]` makes unformatted code fail `check`. M2 wires `check`, so M2's doc adds the diagnostic, cited by `check` fixtures. This doc fixes what it needs from the formatter: `format` is a pure function of the text and `FormatOptions`, so `check` can compare a file with its formatted text, and the fix can be the edits between them. A file `fmt` refuses already fails `check` with its syntax errors.

### 14. Testing

- **Engine unit tests** lay out hand-built documents at small widths, so each rule of decision 3 has a test that doesn't depend on Quince.
- **Printer unit tests** format a text and compare it with hand-written expected text, as the parser's tree dumps are hand-written: no snapshot crate, and no command that writes expected text from the formatter's output.
- **`fmt` fixtures** (below) cite the rules in decision 8's table and `tool.fmt.*`.
- **Every fixture.** `fuzz::check_format(&[u8])` decodes and formats, and, when `format` succeeds, checks that it is idempotent and that the output has no line ending but LF. When it refuses, it checks that the parse had errors. `format` never returns `Internal`. A unit test runs it on every fixture, `fmt` expected outputs included, as the parser's fuzz check runs. That is how M1's idempotence criterion is checked (decision A).
- **The fuzz target** `format` calls `check_format`. Random bytes rarely parse with no error, so its seed corpus is the `fmt` fixtures and their expected outputs, and the error-free `parse` fixtures. As for the parser, a run happens before M1 closes, and every crash it finds gets a regression test.

**The `fmt` fixture mode.** The fixture README gains:
- `// mode: fmt` formats the fixture. It must parse with no diagnostics, and `//~` expectations are an error in this mode.
- A sibling `<name>.fmt` holds the expected output, byte for byte. With no `.fmt` file, the fixture is expected to be formatted already, so the output must equal the fixture. That mirrors `.stdout`.
- The expected output, formatted again, must equal itself, so every `fmt` fixture checks idempotence directly.
- A `.fmt` file with no `.qnc` file of the same name is an error, as for `.stdout`, and so is a `.fmt` file beside a fixture in another mode.
- The fixture's header is part of its text, so the expected output starts with the same header. `.gitattributes` already keeps bytes as they are, so a fixture can hold CR LF or a byte order mark for `r[lex.source.canonical]`.

`Backend` gains `format(&self, path, source) -> Option<Result<Vec<u8>, Vec<Diagnostic>>>`. `SyntaxBackend` implements it, with an `Internal` error turned into a fixture failure.

**Coverage honesty.** A `fmt` fixture cites a rule only when a formatter that broke it would fail the fixture, as for the lexer and parser. A rule about where a break may go, such as `r[lex.line-end]`, is cited by a fixture whose expected output breaks a long expression after its operator.

### 15. Delivery as a stack

Each layer builds and passes `cargo xtask ci` on its own. Until the last layer, `format` returns `Internal("unsupported: <kind>")` for a node kind no printer handles yet. `check_format` treats that as a refusal, so the idempotence check covers whatever the layers so far support. Layer 6 removes the variant's last use, and from then on `check_format` requires it never to happen.

0. `m1/fmt-engine`: the layout engine and its unit tests. No Quince.
1. `m1/fmt-core`: `format`, `FormatOptions`, `FormatError`, refusal; comments and blank lines; the canonical bytes, hex digits and prefix minus; the check of decision 10; printers for the file, `fn` with simple parameters, blocks, `let`, literals, names and calls, enough for the first fixtures; the `fmt` fixture mode and `Backend::format`; `check_format` over every fixture; the README's Formatting section. Spec questions 2 and 3.
2. `m1/fmt-decls`: every declaration and member, attributes, doc comments, signatures, generics, fields, variants, types, `-> Unit`. Spec questions 4, 6 and 7.
3. `m1/fmt-exprs`: every expression, lists and trailing commas, hugging, chains, field shorthand. Spec questions 1 and 5.
4. `m1/fmt-blocks`: statements, `if` and `else`, `match` and `select`, closures, the concurrency forms, patterns, multi-line strings.
5. `m1/fmt-imports`: decision 9.
6. `m1/fmt-cli`: `quince fmt` and `--check`; the fuzz target and its seeds; `AGENTS.md`; the fuzz run.

Layers should come in at 500 to 900 changed lines, not counting fixtures' expected outputs. Layer 1 is the largest. If it runs past 1,000 lines, the fixture mode moves to a layer 1b, with layer 1's fixtures.

## Spec questions

Each gives proposed wording. Every ID uses a construct already in `spec/chapters.txt`, and a question that changes a rule's meaning retires its ID and adds a new one (`AGENTS.md`, "Rule IDs").

**1. Field shorthand outside struct literals (layer 3).** `r[tool.fmt.field-shorthand]` rewrites `profile: profile` in a struct literal. But the parser can't tell a struct literal from a call (parser decision 6), and `r[item.arg.shorthand]` gives calls the same shorthand. Field patterns have it too (`Rect(width:, ..)`). The spec also says the shorthand passes "a binding of the same name", which leaves open what `handler:` means when `handler` is a function. Proposed: retire `tool.fmt.field-shorthand`, and add:
- `r[tool.fmt.shorthand]` "`quince` `fmt` rewrites `name: name` to `name:` in struct literals, arguments and field patterns, so each has one form."
- In `r[item.arg.shorthand]`, keeping its ID since it only says what was meant: "`name:` alone means `name: name`, whatever `name` refers to, so `handle(req, bg:)` means `handle(req, bg: bg)`."

*Why:* a formatter that rewrites some `x: x` and not others needs the checker to tell them apart, and two forms of one call is what the rule exists to prevent.

**2. Hex digits in escapes (layer 1).** `r[tool.fmt.hex-digits]` says hex digits are written uppercase, and `r[lex.lit.int.prefix]` that "hex digits may be either case", but doesn't say whether escapes count. Proposed, keeping the ID, since every spec example already writes escapes uppercase: "`quince` `fmt` writes hex digits uppercase, in integer literals and in `\u{...}` and `\x` escapes."

**3. A prefix minus before a number (layer 1).** `- 1` is a prefix minus applied to `1` (`r[lex.lit.neg-adjacent]`). The formatter writes a prefix operator with nothing after it, so `- 1` becomes `-1`, a negative literal. The value is the same, with one exception: `- 9_223_372_036_854_775_808` is an out-of-range literal under a minus, a compile error from M3, and `-9_223_372_036_854_775_808` is `Int.MIN`. So `fmt` turns that one error into valid code. Proposed, added to `r[lex.lit.neg-adjacent]`: "`quince` `fmt` writes a prefix minus with nothing after it, so in formatted code a minus directly before a number is always part of the literal."

*Alternative:* keep the space, `- 1`, when the operand is a number. Rejected: two spellings of minus one, and a space that means something.

**4. Line breaks in `test` headers (layer 2, from the parser stack).** `r[lex.continue.fn-header]` covers `fn` headers only, so a `test` header with `forall` and `uses` can't break before `uses` or `{`: parser layer 9a makes either an E0018. A long test header can then break only inside its `forall` parameters. Proposed, a new rule after it: `r[lex.continue.test-header]` "Inside a `test` header, a line that starts with `uses` or `{` continues the header, so `quince` `fmt` breaks a long test header as it does a signature." The parser's E0018 for those two words goes.

*Alternative:* leave it. Test headers are rarely long, and their names, the long part, can't break anyway.

**5. Lines past 100 columns (layer 3).** `r[tool.fmt.width]` says "100-column limit". A string, a name, a comment or a path longer than that can't be broken, and the rule doesn't say what a column is. Proposed, keeping the ID, since the rule can only have meant this: "100-column limit, counted in characters; the formatter decides all line breaks. A line runs past 100 columns only where no line break may go, as in a long string, and a comment at the end of a line doesn't count toward it."

**6. Blank lines (layer 2).** "The formatter decides all line breaks" doesn't say whether it keeps an author's blank lines, and the output depends on the answer (decision 4). Proposed, a new rule after `r[tool.fmt.width]`: `r[tool.fmt.blank-lines]` "Declarations and members are separated by one blank line. Inside a block or a list, one blank line is kept where the source had one or more, and none after an opening bracket or before a closing one."

*Alternative:* keep blank lines between declarations as written, up to one, as rustfmt does. Rejected: it gives two layouts of one file, and between declarations the grouping is already the declarations.

**7. Can a package declare a prelude name (layer 2)?** `r[type.infer.required]` has `fmt` remove `-> Unit`. If a package can declare a type named `Unit`, or a file can shadow a prelude name some other way, `-> Unit` may mean that type, and removing it changes the program. Neither Packages nor the Prelude chapter says. Proposed, in the Prelude chapter: `r[prelude.reserved]` "A package can't declare a top-level name the prelude has, so `Unit`, `Option` and every other prelude name mean the same thing in every file." That keeps the rewrite syntactic, and a name read anywhere means what the prelude table says.

*Alternative:* allow shadowing, and move the `-> Unit` rewrite to M3, after name resolution.

## Risks and trade-offs

- [The format is wrong in ways only real code shows, and changing it later makes formatted code fail `check`] → Until 1.0 the format can change, at the cost of one `fmt` run per project. The spec's own examples are the reference style (decision 6), and review compares the printers with them.
- [A comment lands where a line break can't go, and the output doesn't parse] → Decision 5 argues it can't, and decision 10 checks every run, so the worst case is a refused file and a bug report, never a damaged one. The fuzz target looks for such inputs.
- [Hugging and `choice` make layout slow on deep inputs] → `choice` checks one line of its first option, so layout stays linear, and the nesting limit bounds the depth. A unit test formats an input near the limit within a time bound, as the parser's deep-nesting tests do.
- [Refusing files with errors makes the idempotence criterion trivial on most fixtures] → Decision A. The `fmt` fixtures and the error-free fixtures carry it, and layer 1's PR states the count.
- [Two rules wait for M3, so M1's formatter leaves some calls in a form M3's will rewrite] → Every file then changes once, when M3 lands. There is no Quince code outside this repo yet.
- [Seven spec questions] → Each is small, and questions 2 and 5 only say what the rule already meant.
- [The check in decision 10 duplicates the rewrites, so it can't see a wrong rewrite] → Rewrites are unit-tested, and each is a few lines.
- [Layer 1 is large] → Decision 15 gives the split.

## Rollback

Revert the stack's PRs. Nothing changes outside `quince_syntax`, the conformance runner and its fixtures, `quince_cli`, `AGENTS.md`, the fuzz crate and `spec/`, and nothing depends on the formatter until M2 wires `check`.

## For later milestones

- **M2:** the unformatted-code diagnostic in `check` (decision 13); `quince fmt` over a project, `testdata/` excluded; `quince.toml` formatting; `FormatOptions` filled from the manifest; whether typed accessors come with name resolution (decision 11).
- **M3:** `r[item.arg.fmt-positional]` and `r[item.arg.named]`'s rewrite (decision 8). If spec question 7 allows shadowing, the `-> Unit` rewrite too.
- **M5:** JSON output for `fmt`; formatting part of a file for the language server; whether the language server formats around errors (decision 2).

## Tasks

Rule IDs in brackets are what each task's fixtures cite.

**Before building**
- [ ] Design review passes.
- [ ] David decides A, B and C.
- [ ] David decides spec questions 1 to 7.

**Layer 0: `m1/fmt-engine`**
- [ ] The document type and the layout engine: groups, the three line kinds, `indent`, `if_break`, `line_suffix`, `choice`; width in `char`s; unit tests at small widths.

**Layer 1: `m1/fmt-core`**
- [ ] Spec: questions 2 and 3, and a bump.
- [ ] `format`, `format_str`, `FormatOptions`, `FormatError`; refusal of lossy text and syntax errors.
- [ ] Comments and blank lines (decisions 4 and 5).
- [ ] Line feeds, the byte order mark, hex digits, prefix minus. [`lex.source.canonical`, `tool.fmt.hex-digits`, `lex.lit.neg-adjacent`]
- [ ] Printers for the file, `fn` with parameters and a return type, blocks, `let`, literals, names, calls and fields; `Internal("unsupported")` for the rest. [`tool.fmt.one-per-line`]
- [ ] The output check (decision 10).
- [ ] The `fmt` fixture mode, `Backend::format`, and the fixture README.
- [ ] `fuzz::check_format`, and its unit test over every fixture.
- [ ] `compiler/syntax/README.md`: the Formatting section. `AGENTS.md`'s map.

**Layer 2: `m1/fmt-decls`**
- [ ] Spec: questions 4, 6 and 7, and a bump. Question 4 also removes the parser's E0018 for `uses` and `{` lines in a `test` header.
- [ ] Every declaration and member; attributes on their own lines, with no blank line after; doc comments; generics, bounds, fields, variants; types and function types.
- [ ] Signatures (decision 6). [`lex.continue.fn-header`, `lex.continue.test-header`]
- [ ] Blank lines between declarations. [`tool.fmt.blank-lines`]
- [ ] `-> Unit` removed. [`type.infer.required`]

**Layer 3: `m1/fmt-exprs`**
- [ ] Spec: questions 1 and 5, and a bump.
- [ ] Every expression; binary expressions broken after their operators; chains; `=` and `=>`. [`lex.line-end`, `lex.continue.more`, `tool.fmt.width`]
- [ ] Lists and trailing commas; hugging (decision 7). [`tool.fmt.trailing-comma`]
- [ ] Field shorthand. [`tool.fmt.shorthand`]

**Layer 4: `m1/fmt-blocks`**
- [ ] Statements; `if` and `else`; `match` and `select` arms; closures; `concurrent`, `background`, `with`, `spawn`; patterns. [`tool.fmt.one-per-line`, `tool.fmt.trailing-comma`]
- [ ] Multi-line strings re-indented through their `StringIndent` tokens.

**Layer 5: `m1/fmt-imports`**
- [ ] Merging, grouping and sorting, with comments; the local group from `FormatOptions`, unit-tested. [`tool.fmt.imports`, `mod.import.one-line`, `mod.import.self`]

**Layer 6: `m1/fmt-cli`**
- [ ] `quince fmt` and `--check` (decision 12), tested by running the binary on a temporary folder.
- [ ] No printer returns `unsupported`; `check_format` requires it.
- [ ] The `format` fuzz target and its seeds; the command in `AGENTS.md`.
- [ ] Before M1 closes: run both fuzz targets, and fix every crash each finds, with a regression test. The PR records the runs.
