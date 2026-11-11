# Quince Language Spec

## 1. Overview and design principles

Quince is a statically typed, automatically memory-managed, general-purpose language designed so that both AI agents and humans can write, read, and verify code with as little hidden context as possible. Every rule below serves one goal: a reader should be able to understand a function correctly from the function itself.

### Target domain

Quince is built for services and tools: web servers and APIs, command-line tools, data pipelines, and automation. It relies on compiler-inserted reference counting and a green-thread runtime, and accepts their small runtime costs in exchange for never making anyone manage memory by hand.

- **Non-goals:** operating systems, drivers, and embedded devices, which can't host a runtime; and latency-critical work such as game engines and audio, where every allocation and free must be under the programmer's control.
- **Future option:** arenas or regions (memory allocated and freed as a group) for hot paths that can't afford per-object reference counting. The runtime and Quince IR should not rule this out, but it is not part of version 1.
- **Ruled out:** an ownership system like Rust's. It would reach every domain but reintroduces the complexity this language exists to avoid.

### Core principles

1. **Locality.** Everything that affects a function's behavior is visible in its signature, its body, and the declarations of the types and effects it names: its types, its effects, its failure modes, and its contracts.
2. **One way to do it.** Where two constructs would overlap, the language keeps one, and where it keeps two (such as is and match) the spec says when to use each. Fewer choices means fewer wrong choices.
3. **The compiler is a collaborator.** Diagnostics are precise, machine-readable, and come with suggested fixes. The compiler can be queried, not just run.
4. **Correctness is stated, then checked.** Types, exhaustive matching, contracts, and executable examples turn intent into something the toolchain verifies.
5. **Nothing changes meaning silently.** Published code never changes behavior under a dependent. Changes to the shape of a public API (types, signatures, effects and contracts) are detected mechanically.
6. **Familiar surface.** Syntax stays close to TypeScript, Rust, and Go so knowledge transfers. Novelty goes into semantics and tooling, not punctuation.

### Deliberately excluded

| Feature | Why it's out |
| --- | --- |
| r[intro.excl.exceptions] Exceptions | Failure must appear in the return type |
| r[intro.excl.null] `null` / `nil` | Replaced by `Option<T>` |
| r[intro.excl.macros] Macros and compile-time metaprogramming | Code must mean what it appears to mean |
| r[intro.excl.user-operators] User-defined operators and impure overloading | Each operator calls one fixed, pure prelude interface ([Prelude and special interfaces](#operators-on-user-types)), so a + b can never do I/O |
| r[intro.excl.overloading] Function name overloading | Every name resolves to exactly one definition; an interface method resolves through the impl the argument types select |
| r[intro.excl.implicit-conversions] Implicit conversions | All conversions are explicit calls |
| r[intro.excl.global-state] Global mutable state | Shared state is passed explicitly as a `Ref<T>` parameter, and reading or writing it requires `uses State` |
| r[intro.excl.reopen-types] Monkeypatching / reopening types | A type is fully defined in one place |
| r[intro.excl.wildcard-imports] Wildcard imports | Every imported name is listed |
| r[intro.excl.inheritance] Inheritance | Composition and interfaces only |
| r[intro.excl.fmt-config] Formatter configuration | One canonical layout |

## 2. Lexical structure and syntax

r[lex.braces] Quince uses C-family syntax with braces, semicolon-free statements, and a fixed set of 38 keywords. r[lex.indentation] Indentation never carries meaning, so a misapplied diff can't silently change program structure.

r[lex.line-end] A line break ends a statement unless the line ends with a binary operator, a comma or an opening bracket, as in Go. A long expression therefore breaks after its operator, never before, and `quince` `fmt` always formats it that way: a line that starts with `- b` is a new statement, not a continuation.

r[lex.continue.more] A line also continues when it ends with `=`, a compound assignment such as `+=`, `=>` or `->`, and when the next line's first token is `.`, so a method chain can lead each line with its dot. No statement can start with `.`, so this adds no ambiguity. r[lex.else-same-line] `else` must sit on the line of the closing `}`; a line that starts with `else` is an error whose fix joins it. r[lex.continue.signature] Inside a `fn` signature, a line that starts with `->`, `uses`, `requires`, `ensures` or `{` continues the signature, so `quince` `fmt` breaks a long signature before those words, as in [Effects](#5-effects-and-capabilities) and [Contracts](#requires-and-ensures).

### Keywords

r[lex.keywords] `fn` `let` `var` `type` `struct` `enum` `interface` `impl` `match` `is` `if` `else` `for` `in` `while` `return` `break` `continue` `import` `pub` `uses` `requires` `ensures` `test` `forall` `with` `concurrent` `spawn` `background` `foreign` `true` `false` `self` `Self` `const` `effect` `select` `as`

r[lex.no-contextual] There are no contextual keywords: every reserved word means the same thing in every position, and no other word is special, apart from main, the tagged-literal prefixes ([Literals](#literals)), the named arguments of keyword forms (limit:, settle:, on\_error:), the case names that @wire(case: ...) takes (camel, snake, kebab, pascal, upper\_snake), and TaskScope in a background header. r[lex.keyword-not-name] A keyword therefore can't name a binding, parameter, field or method. A wire name that is a keyword is written with `@wire` ([Prelude and special interfaces](#serialization)).

r[stmt.loop.forms] There is exactly one loop over collections (`for x in xs`) and one conditional loop (`while`). No `do-while`, no C-style `for`, no `loop`. r[stmt.loop.no-labels] There are no loop labels: to leave nested loops at once, move them into a function and `return`.

r[expr.ret.value] `return expr` ends the function with `expr`. r[expr.ret.bare] A bare `return` is allowed only in a function that returns `Unit`, where it means `return ()`.

### Bindings

- r[stmt.bind.let-var] `let` declares an immutable binding; `var` declares a mutable one.
- r[stmt.bind.shadow.same-scope] Shadowing a binding in the same scope is a compile error. That rules out let x = x?, so the suggested fix gives the unwrapped value its own name (let user = user\_result?). r[stmt.bind.shadow.nested] Shadowing in a nested scope is allowed but flagged by the linter.
- r[stmt.bind.var-param] A parameter can be marked `var`, as in `fn sum<I: Iterator<Item = Int>>(var it: I)` `-> Int`. The function gets its own mutable copy, and the caller's value never changes. This is not an inout parameter, and it avoids `var it = it`, which the shadowing rule forbids.

### Constants

```
const MAX_RETRIES: Int = 3
const EMAIL_PATTERN: Regex = re"^[^@]+@[^@]+$"
const DEFAULT_CONFIG: Config = Config.parse(DEFAULT_CONFIG_TEXT).expect("built-in config is valid")
```

- r[item.const.decl] `const` declares a top-level constant. r[item.const.typed-pure] Its type is always written, and its initializer must be pure ([Effects](#5-effects-and-capabilities)).
- r[item.const.eval.literal] A literal, including a tagged literal, is evaluated at compile time. r[item.const.eval.lazy] Any other initializer runs once, on first use, and the result is safe to share across tasks. Because initializers are pure, the order in which constants are first used never matters.
- r[item.const.no-ref] A const's type can't contain a reference type ([Types](#3-type-system)) anywhere inside it, in fields, payloads or closure captures. The compiler derives this the same way it derives whether a type can cross a task boundary, and the error names the field path that holds the reference. State the whole program shares, such as a request counter, a cache or a registry, belongs in an effect handler installed in `main` ([Effects](#5-effects-and-capabilities)), so every function that touches it says so in its `uses` clause.
- r[item.const.panic.site] A panic in an initializer is reported at the first use site. r[item.const.panic.warn] `quince` `check` warns about any initializer that can panic.
- r[item.const.assoc] A type's own constants are declared in its `impl` block, in the type's package: `impl Int { pub const MAX: Int = 9_223_372_036_854_775_807 }`. They follow every rule above. r[item.const.assoc.qualified] They are always read through the type's name, as `Int.MAX`, `Float.PI` or `Status.OK`, and inside the impl as `Self.MAX`, never as a bare `MAX`. r[item.const.assoc.no-interface] Interfaces can't declare constants in v1.

### Literals

| Kind | Examples | Notes |
| --- | --- | --- |
| Integer | r[lex.lit.int.forms] `42`, `1_000_000`, `0xFF`, `0b1010`, `0o17` | r[lex.lit.int.default] Default type `Int` (64-bit, overflow panics, never wraps). r[lex.lit.int.context] Takes another integer type from context, as in let b: UInt8 = 200, and a literal that doesn't fit that type is a compile error. r[lex.lit.int.neg] A minus sign directly before a literal in prefix position is part of it, so Int.MIN can be written -9\_223\_372\_036\_854\_775\_808 |
| Float | r[lex.lit.float.forms] `3.14`, `1e-9` | r[lex.lit.float.default] Default type `Float` (64-bit). r[lex.lit.float.context] Takes Float32 from context the same way |
| String | `"hello {name}"` | r[lex.lit.string.interp] UTF-8; interpolation with {} is the only formatting syntax. r[lex.lit.string.interp-path] Inside braces only a name or a field path is allowed ({name}, {user.name}), never another expression. A field path may read a newtype or tuple position, as in {id.0}. r[lex.lit.string.lone-brace] Only { opens interpolation, so a lone } is plain text and needs no escape. r[lex.lit.string.escapes] Escapes: \\n \\t \\\\ \\" \\{ \\r \\0 \\u{1F600}. r[lex.lit.string.one-line] A "..." string stays on one line. r[lex.lit.string.multiline] A multi-line string sits between """ lines: it interpolates, starts on the line after the opening """, and drops the closing line's indentation from every line, as in Swift and Kotlin. r[lex.lit.string.raw-multiline] r""" ... """ is its raw form, which may contain " and { freely |
| Raw string | `r"C:\path"` | r[lex.lit.raw-string] No escapes, no interpolation; it can't contain a quote, so text with quotes uses r""" |
| Char | `'a'`, `'\n'`, `'\u{1F600}'` | r[lex.lit.char] Type Char: one Unicode scalar value. r[lex.lit.char.escapes] Same escapes as strings, plus \\' |
| Bytes | `b"GET /"`, `b"\x00\xFF"` | r[lex.lit.bytes] Type Bytes: ASCII characters, \\x escapes and the escapes \\n \\r \\t \\0 \\\\ \\". No interpolation |
| Tagged literal | `url"https://..."`, `re"[a-z]+"`, `dur"30s"` | r[lex.lit.tagged] Validated at compile time; a fixed, language-defined set of tags (v1: url, re, dur, ts). r[lex.lit.tagged.raw] The body is raw, as in a raw string: no escapes and no interpolation, so re"\\d+\\.\\d+" and re"a{2}" mean what they say |
| Collections | `[1, 2, 3]`, `{"a": 1}` | r[lex.lit.collections] List and Map. r[lex.lit.empty-map] The empty map is {:}, so it can't be mistaken for an empty block. r[lex.lit.no-set] Sets have no literal: Set.from(\[1, 2, 3\]) |

### Naming

- r[lex.name.case] Types and enum variants: `PascalCase`. Functions, bindings, fields, packages, files: `snake_case`. Constants: `SCREAMING_SNAKE_CASE`.
- r[lex.name.enforced] Naming case is enforced by the compiler, not the linter. r[lex.name.foreign-exempt] Names declared inside a foreign block are exempt and keep their exact C spelling, so they match the library's documentation ([Interop with C](#11-interop)).

### Arguments

```
counts.update(word, default: 0, fn(n) { n + 1 })
http.serve(8080, handler: fn(req) { handle(req, bg:) })
```

- r[item.arg.first-positional] The first argument may be positional. r[item.arg.named] Every later argument is written with its parameter name, except the argument for the last declared parameter when that parameter has a function type: it is written without its name, whether it is a closure or a named function, and quince fmt rewrites either form into this one. r[item.arg.shorthand] As in struct literals, `name:` alone passes the binding of the same name, so `handle(req, bg:)` means `handle(req, bg: bg)`. r[item.arg.fmt-positional] When the first argument may be positional, `quince` `fmt` writes it positionally, so `http.serve(8080``, handler: h)` is the one form. That holds even when the first parameter's name carries meaning: `Channel<Job>.new(16)`, `log.json(cfg.log_level)`.
- r[item.arg.receiver] On a method call the receiver doesn't count, so the first argument after it may be positional: `counts.update(word, default: 0, ...)`. r[item.arg.fn-value-positional] A call through a function value, such as `handler(req)` or `f(a, b)`, is always positional, because function types have no parameter names.
- r[item.arg.order] Named arguments are passed in declaration order, so each call has one canonical form.
- r[item.arg.default] A parameter may have a default value, which must be a literal, an empty list or map literal (`[]` or `{:}`), including a tagged literal such as dur"30s", or the name of a const, including a type's constant such as Int.MAX: `fn retry(times: Int = 3)`. A caller can leave it out.
- r[item.arg.semver] Adding a parameter with a default is not a breaking change ([Dependencies and publishing](#enforced-semantic-versioning)). Renaming any parameter is.

### Type arguments and construction

```
let order = json.decode<Order>(body)?
let jobs = Channel<Job>.new(16)
let user = User(id: UserId(1), email:, name: "Ada", created_at: now)
```

- r[type.type-arg.angle] After a name or path in an expression, `<` starts type arguments when the tokens up to its matching `>` parse as types and that `>` is directly followed by `(` or `.`. Otherwise `<` is less-than. Type names are always `PascalCase`, so in `f(a < b, c > (d))` neither `b` nor `c` can be a type, and the call has two comparison arguments.
- r[expr.op.compare.no-chain] Comparison operators don't chain: `a < b > c` is a syntax error, and the suggested fix is `a < b && b < c`. So `json.decode<Order>(body)` has only one reading.
- r[lex.no-shift-token] The lexer never makes `>>` one token, so `List<List<Int>>` closes without a space.
- r[type.type-arg.optional] Type arguments are optional wherever inference finds them ([Inference rules](#inference-rules)).
- r[type.type-arg.construct] A `PascalCase` name followed by `(` builds a value: a struct (`User(id:, name: "Ada")`), a newtype (`UserId(1)`) or, through its enum's name, a variant (`Shape.Rect(width: 2.0, height: 3.0)`). Functions are `snake_case`, so this is never a function call.
- r[type.type-arg.no-brace-construct] Braces never build a value of a user type: `{` starts a block or a map literal. So a struct value can appear in an `if`, `while`, `for`, `match` or `with` head without parentheses: `if p == Point(x: 0, y: 0) { ... }`.

### Operators and precedence

r[expr.op.levels] Operators bind in seven levels, tightest first. r[expr.op.parens-required] Where a row says an expression must be parenthesized, it can't be an operand of a different operator without parentheses, and the compiler's fix adds the parentheses that give the usual reading. So every expression that compiles reads one way.

| Level | Operators | Grouping | Notes |
| --- | --- | --- | --- |
| 1 Postfix | `.name  .method()  f()  xs[i]  ?` | Left to right | `-x?` is `-(x?)` |
| 2 Prefix | `-  !  ~` | Right to left | `-x.abs()` is `-(x.abs())` |
| 3 Multiplicative | `*  /  %` | Left to right |  |
| 4 Additive | `+  -` | Left to right |  |
| 5 Range | `..  ..=` | r[expr.op.range.no-chain] Doesn't chain | r[expr.op.range.parens] Must be parenthesized as an operand (see [Ranges](#ranges)) |
| 6 Comparison | `==  !=  <  <=  >  >=  is` | Doesn't chain | r[expr.op.is-pattern] `is` takes a pattern on its right ([Patterns](#patterns)) |
| 7 Logical | `&&  \|\|` | r[expr.op.logic.short-circuit] Left to right, short-circuit | r[expr.op.logic.no-mix] The two can't be mixed without parentheses |
| Apart | `&  \|  ^  <<  >>` | r[expr.op.bits.chain] `&`, `\|` and `^` chain only with themselves | r[expr.op.bits.parens] No level: must be parenthesized as an operand of any other operator, and its own operands are prefix or postfix expressions |

```
flags & MASK == 0        // error: write (flags & MASK) == 0
a || b && c              // error: write a || (b && c)
0 <= i < n               // error: write 0 <= i && i < n
1 << n - 1               // error: write 1 << (n - 1)
```

r[expr.op.keyword-forms] `return`, `break`, `continue` and `spawn` are keyword forms, not operators; `return expr` and `spawn expr` take everything to their right.

Operand types. r[expr.op.no-conversion] No operator converts its operands, so `Int + Float` and `Int32 + Int64` are errors whose fix names the conversion method ([Types](#built-in-types)). User types take part only through the interfaces in [Operators on user types](#operators-on-user-types).

| Operators | Built-in operands | Failure |
| --- | --- | --- |
| `+  -  *  /  %` | r[expr.op.arith.operands] Two values of the same integer or float type | r[expr.op.arith.panic] Integers panic on overflow, on division by zero and on Int.MIN / -1, and / rounds toward zero. r[expr.op.arith.float] Floats follow IEEE |
| Prefix `-` | r[expr.op.neg.operands] Signed integers and floats, not unsigned ones | r[expr.op.neg.panic] -Int.MIN panics |
| `==  !=  <  <=  >  >=` | r[expr.op.compare.operands] Two values of the same type, with Eq or Ord | Never fails |
| `&&  \|\|  !` | r[expr.op.logic.operands] Bool only | Never fails |
| `&  \|  ^  ~` | r[expr.op.bits.operands] Values of the same integer type | Never fails |
| `<<  >>` | r[expr.op.shift.operands] An integer, shifted by a count of any integer type | r[expr.op.shift.panic] Panics when the count is negative or not below the bit width. r[expr.op.shift.semantics] Bits shifted out are dropped, and >> keeps the sign of a signed value |

r[expr.op.plus-numeric] `+` is numeric only. r[expr.op.plus-suggest] Strings join with interpolation (`"{first}{last}"`) and lists with `.concat()`, and the compiler suggests both. r[expr.op.float-total-order] Because comparisons use `Eq` and `Ord`, floats compare in their total order ([Types](#built-in-types)): `NaN == NaN` is true and `-0.0 == 0.0` is false. r[expr.op.float-eq-warn] `quince` `check` warns when a float is compared with `==` against a literal.

r[expr.op.excluded] Not in Quince: the ternary `c ? a : b` (use `if`), `++` and `--` (`+= 1`), `**` (`x.pow(n)`), `??` and `?.` (`unwrap_or`, `?`, `map`), a pipe operator, and casts with `as`.

### Ranges

r[expr.range.semantics] `a..b` runs from `a` up to but not including `b`, and `a..=b` includes `b`. r[expr.range.type] Both are values of the prelude type `Range<T>`, where `T` is one integer type for both ends.

- r[expr.range.empty] When `a >= b` (or `a > b` for `..=`), the range is empty; it never panics. r[expr.range.no-overflow] `0..=Int.MAX` iterates to the end without overflowing.
- Ranges drive loops (`for i in 0..n`) and are passed as values, as in `xs.slice(1..3)`. Their methods are listed in [the standard library API](../std/API.md#ranges).
- r[expr.range.pattern] Both forms are patterns too: `1..=9 => "digit"`. r[expr.range.pattern.char] A pattern range may also be over `Char`, as in `'a'..='z' => ...`, and exhaustiveness counts its scalar values; a `Range<T>` value is always over an integer type. r[expr.range.pattern.exhaustive] Exhaustiveness is checked over integer ranges, so `0..=127` and `128..=255` cover a `UInt8`.
- r[expr.range.no-open] There are no open ranges (`a..`, `..b`, `..`), because `..` already means the remaining fields in a pattern and the copied fields in `User(..user, name: "New")`.
- r[expr.index.int] `xs[i]` takes only an `Int`; another integer type is converted first, as in `xs[n.to_int64()]`. r[expr.index.slice] A slice is the method call `xs.slice(r)`, so `[]` has one meaning.
- r[expr.range.precedence] `0..n + 1` is `0..(n + 1)`, and a range must be parenthesized as an operand: `r == (0..3)`.

### Blocks, statements and expressions

r[expr.block.expr-stmt] `if`, `match`, closures and the keyword forms (`concurrent`, `background`, `select`, `with`) are expressions. Declarations, loops and assignments are statements.

- r[expr.block.value] A block's value is its last line when that line is an expression, and `()` otherwise. r[expr.block.last-newline] The last statement needs no newline before `}`, so a one-line block may hold a single statement: `Some(m) => { total = total + m },`. r[expr.block.fn-value] A function returns its body's value, as `word_counts` does in [Mutation](#mutation).
- r[expr.if.type] `if` with an `else` has its branches' common type. r[expr.if.no-else] Without an `else`, its type is `Unit` and its branch must be `Unit`: `let sign = if n < 0 { -1 } else { 1 }`. r[expr.block.unit-context] Where the expected type is `Unit` (an `if` without `else`, a loop body, the body of a function that returns `Unit`, or an `if` or `match` used as a statement), a last expression is a statement: its value is discarded under the unused-value rules ([Errors and failure](#expected-failures-result-and-option)). So `if ok { seen.insert(x) }` compiles, an unused `Result` is still an error, and the arms of a `match` used as a statement need not share a type.
- r[expr.match.arm-syntax] A `match` arm is `pattern => expr,` or `pattern => { block },`. r[expr.match.arm-block] In an arm, `{` starts a block. r[expr.match.arm-comma] Every arm ends with a comma, a block arm included.
- r[expr.block.no-bare] There are no bare blocks. r[expr.block.brace-map] Anywhere else, `{` where an expression is expected starts a map literal. r[expr.block.positions] Blocks appear only as the bodies of functions, closures, `if`, `else`, match arms, `for`, `while`, `test` and the keyword forms. r[expr.block.keyword-bodies] The bodies of `with` and `background`, like `select` arms, are ordinary blocks of the enclosing function: `return`, `?`, `break` and `continue` leave them, and a `background` block first cancels its tasks and waits for them. r[expr.block.concurrent-tasks] A `concurrent` block's spawned expressions are tasks, not blocks of the function.

* r[stmt.loop.stmt] `for` and `while` are statements, and `break` takes no value. r[stmt.loop.for-sources] `for` iterates any `Iterator`, and a `List`, `Set` or `Map` directly (a map's entries as `(key, value)` pairs). r[stmt.loop.for-string] A `String` or `Bytes` value needs `.chars()`, `.bytes()` or `.iter()`, so the unit is always written. r[stmt.loop.fanout-sources] `concurrent(limit:) for` takes the same things.
* r[expr.jump.expr] `return`, `break` and `continue` are expressions, so they can stand in a match arm (`None => return,`). r[type.never.block] A block whose last statement or expression can't complete normally can't complete normally itself, and neither can a `while true` with no `break` aimed at it. Such a block has type `Never`, so a function returning `Summary` may end in a `while true` loop that it leaves only through `return`.
* r[stmt.assign.stmt] Assignment is a statement: `place = expr`, or `place op= expr` with `+=` `-=` `*=` `/=` `%=`, meaning `place = place op expr`. r[stmt.assign.place] A place is a `var` binding, a field path from one, or an index into one (`xs[i] = v`). r[stmt.assign.no-chain] There is no `x = y = z`, no assignment inside a condition, and no `++` or `--`.
* r[expr.closure.syntax] A closure is `fn(params) -> T uses E { body }`. r[expr.closure.infer] Parameter types, the return type and `uses` can be left out when the expected type gives them: `fn(n) { n + 1 }`. r[expr.closure.no-expected] With no expected type, the parameter types are written, and the return type and the effects are inferred from the body: `Callback.local(fn(sent: CInt, total: CInt) { done.set(sent.to_int64()) })` needs no `uses`. r[expr.closure.underscore] An unused parameter of a closure or function may be named `_`.
* r[expr.fn-value.named] A named function, or an associated function such as `Url.parse`, can be passed where a function type is expected: `lines.map(parse_user)`. Its type carries its effects. r[expr.fn-value.no-method] A method is passed through a closure, `fn(u) { u.display() }`, so a function value never hides which receiver it holds. r[expr.fn-value.variant] An enum variant with a payload is a function from its fields, in declaration order, so `map_err(AppError.Serve)` passes the variant as a function.

### Formatting rules (enforced by `quince` `fmt`)

- r[tool.fmt.one-per-line] One declaration, statement, or match arm per line.
- r[tool.fmt.trailing-comma] Trailing commas required in all multi-line lists, arguments, fields, and arms.
- r[tool.fmt.width] 100-column limit; the formatter decides all line breaks.
- r[tool.fmt.imports] Imports sorted and grouped (standard library, then packages, then local).
- r[tool.fmt.check] Unformatted code compiles but fails `quince` `check`, which CI runs by default.

### Comments

- r[lex.comment.line] `//` line comments only. No block comments, which break easily under partial edits.
- r[lex.comment.doc] `///` doc comments attach to the next declaration and may contain executable examples (see [Testing](#executable-examples-in-doc-comments)).

### Attributes

r[item.attr.fixed] Attributes are a fixed set defined by the language. Packages cannot define their own, so attributes can never become a form of metaprogramming. r[item.attr.unknown] An unknown attribute is a compile error, and new attributes arrive only through editions ([Packages and modules](#editions)).

| Attribute | Applies to | Meaning |
| --- | --- | --- |
| r[item.attr.unchecked] `@unchecked` | Functions | Skip contract checks in release builds ([Contracts](#requires-and-ensures)) |
| r[item.attr.verify] `@verify` | Functions | Attempt static proof of contracts with `quince` `verify` ([Contracts](#static-verification-optional)) |
| r[item.attr.extensible] `@extensible` | Enums | Variants may be added without a major version; callers must include a wildcard arm ([Dependencies and publishing](#enforced-semantic-versioning)) |
| r[item.attr.pinned] `@pinned` | Foreign libraries | Always call from the same OS thread ([Interop with C](#calling-c)) |
| r[item.attr.shareable] `@shareable` | Foreign resource types | The C handle is safe to use from several threads, so its Resource may cross task boundaries. Unverifiable, so it is listed with the package's native dependencies ([Interop with C](#trust)) |
| r[item.attr.cleanup] `@cleanup(fn, limit: dur)` | Foreign types | The C function that releases the type, and the cleanup time limit (10 seconds when left out), so it can be held in a Resource ([Interop with C](#memory-across-the-boundary)) |
| r[item.attr.ownership] `@borrowed`, `@owned(fn)` | Foreign functions that return a CString or a Ptr\<T> whose T has no @cleanup | Who owns the returned memory: C, or Quince, which frees it with fn when it is dropped. One of the two is required ([Interop with C](#memory-across-the-boundary)) |
| r[item.attr.deprecated] `@deprecated("why", use: path)` | Any public declaration | Warn on use and name the replacement; `quince` `fix` rewrites calls automatically when the replacement takes the same arguments |
| r[item.attr.opaque] `@opaque` | Structs and enums | No automatic Eq, Hash, debug form, test generator, Encode or Decode, for handle-like types ([Prelude and special interfaces](#serialization)) |
| r[item.attr.test] `@test` | Top-level declarations | Compiled only by `quince` `test`; naming it from code that isn't a test is a compile error. Impls of a `@test` type are test-only too, without their own `@test` ([Testing](#unit-tests)) |
| r[item.attr.wire] `@wire("name")`, `@wire(case: camel)`, `@wire(tag: "kind")` | Fields and variants (name); structs and enums (case, tag) | Set the wire name, the naming case, or the enum tag field used by Encode and Decode ([Serialization](#serialization)) |
| r[item.attr.test-default] `@test_default(f)` | Package effects | The handler a test gets when it reaches this effect with no `with` around it; `f` takes no arguments and returns a handler ([Effects](#handlers-in-tests-and-production)) |

r[item.attr.placement] An attribute goes on its own line directly above the declaration it applies to.

## 3. Type system

r[type.nominal] Quince's type system is static, sound, and nominal, with full type annotations required at every declaration boundary and inference allowed only inside function bodies. r[type.no-any] There is no `any` type and no unchecked cast.

### Built-in types

| Type | Description |
| --- | --- |
| `Int`, `Int8`...`Int64`, `UInt8`...`UInt64` | r[type.int] Integers; overflow is a panic (see [Errors and failure](#panics-bugs)), and checked\_add etc. return Option. r[type.int-alias] Int is another name for Int64, not a separate type. r[type.no-uint] There is no plain UInt; use UInt64 |
| `Float`, `Float32` | r[type.float] IEEE 754 floats; Float is another name for Float64. r[type.float.total-order] Arithmetic follows IEEE, but Eq, Ord and Hash use a total order (NaN equals NaN, and -0.0 sorts before 0.0), so floats can be map keys and always sort the same way |
| `Bool` | r[type.bool] `true` / `false` |
| `String` | r[type.string] Immutable UTF-8 text. r[type.string.no-index] There is no s\[i\]; .bytes() and .chars() say which unit you mean |
| `Bytes` | r[type.bytes] Immutable byte sequence |
| `List<T>`, `Map<K, V>`, `Set<T>` | r[type.collections] Values: passing or assigning never shares a mutable alias (see [Mutation](#mutation)) |
| `Option<T>` | r[type.option] `Some(value)` or `None` |
| `Result<T, E>` | r[type.result] `Ok(value)` or `Err(error)` |
| `Unit` | r[type.unit] The empty value, written `()` |
| `Never` | r[type.never] The type of an expression that never produces a value: return, break, continue, unreachable, todo and Process.exit. It has no values and fits wherever any type is expected (see [Inference rules](#inference-rules)) |
| Char | r[type.char] One Unicode scalar value; .chars() yields them |
| Tuples | r[type.tuple] Fixed-size groups such as (Int, String), written (1, "a") and taken apart with let (n, s) = t or match, or read by position with t.0 and t.1 |
| Function types | r[type.fn] fn(A) -> B, with uses E for effects ([Effects](#effect-variables)) |
| Duration, Timestamp, Url, Regex | r[type.tagged] The values of the tagged literals dur, ts, url and re ([Lexical structure](#literals)) |
| Range\<T> | r[type.range] The value of a..b or a..=b, over one integer type ([Ranges](#ranges)) |

r[type.num.div-round] Integer division `/` and remainder `%` round toward zero, so `-7 / 2` is `-3` and `-7 % 2` is `-1`. r[type.num.conversions] Conversions between numeric types are explicit methods: widening ones (`n.to_int64()`) always succeed, narrowing ones (`n.to_int32()`) return `Option`, and `f.to_int()` returns `None` for NaN, infinities and out-of-range values.

### Structs

```
pub struct User {
  id: UserId,
  email: Email,
  name: String,
  created_at: Timestamp,
}
```

- r[item.struct.init-all] All fields must be initialized at construction. No default constructors, no partially built values. r[item.struct.unit] A struct with no fields is declared without braces, as in `pub struct ChannelClosed`, and its one value is written as its name, `ChannelClosed`.
- r[item.struct.literal] A struct is built with its name and every field named, in declaration order: `User(id: UserId(1), email:, name: "Ada", created_at: now)`. r[item.struct.shorthand] A field name with nothing after the colon takes the binding of the same name, as in patterns, and `quince` `fmt` rewrites `email: email` to that form. r[item.struct.update] `..base`, written first, copies every field not written after it: `User(..user, name: "New")`. r[item.enum.construct] A variant with a payload is built the same way, shorthand included: `SqliteError.Open(code:)`.
- r[item.struct.value] Structs are values, like collections: passing or assigning one copies it (see [Mutation](#mutation)). r[item.struct.let-immutable] On a `let` binding a struct never changes, and updates build a new value with `User(..user, name: "New")`. r[item.struct.field-assign] On a `var` binding, `user.name = "New"` assigns one field, which is shorthand for `user = User(..user, name: "New")`.
- r[item.struct.field-private] Each field is private unless marked `pub`, as in `pub struct Account { pub id: AccountId, balance: Money }`. r[item.struct.literal-visibility] A struct literal or a `Type(..x)` update can be written only where every field is visible, so a type with private fields can only be built through its own functions. Invariants are stated once, with `requires` on the struct ([Contracts](#requires-and-ensures)).

### Enums (algebraic data types)

```
pub enum Shape {
  Circle(radius: Float),
  Rect(width: Float, height: Float),
  Point,
}
```

r[item.enum.named-fields] Variant payloads use named fields so call sites read unambiguously: `Shape.Rect(width: 2.0, height: 3.0)`. r[item.enum.field-visibility] A variant's fields always have its enum's visibility, so other packages can build and match a `pub` enum's variants, and a variant field is never marked `pub`: writing it is a compile error.

r[item.enum.recursive] An enum may contain itself, directly or inside a collection: `enum Tree { Leaf, Node(left: Tree, value: Int, right: Tree) }`. Enums are values, so no box or pointer is written.

### Newtypes

r[item.newtype.distinct] `struct UserId(Int)` creates a distinct type that does not unify with `Int`. r[item.alias.lint] Plain aliases (`type Name = String`) are allowed but flagged by the linter in public APIs, since they hide meaning without adding safety.

r[item.newtype.access] `UserId(1)` builds a newtype and `id.0` reads its inner value. r[item.newtype.visibility] The inner value follows `pub` like a field: `pub struct UserId(pub Int)` lets other packages build and read it, while `pub struct Email(String)` leaves both to the package's own functions, which is how a newtype guards an invariant.

### Mutation

r[type.mut.value-semantics] All built-in collections and structs have value semantics. r[type.mut.shared-in-ref] Mutable state shared between functions lives only in a reference type (listed under [Reference types](#mutation)), and reading or writing one requires the `State` effect ([Effects](#5-effects-and-capabilities)), except where its row in the Reference types table below says otherwise. A signature therefore shows which state a function can reach, through its parameter types, and whether it touches that state, through `uses State`. A constant can't hold a reference type ([Items](#constants)), so there is no global mutable state to reach any other way.

**Local mutation.** r[type.mut.local] On a `var` binding, collections and struct fields can be changed in place:

```
fn word_counts(text: String) -> Map<String, Int> {
  var counts: Map<String, Int> = {:}
  for word in text.split_whitespace() {
    counts.update(word, default: 0, fn(n) { n + 1 })
  }
  counts
}
```

- r[type.mut.no-alias] Passing or assigning a value never creates a shared alias. Changing `counts` can't affect any other variable, so a function that mutates locally is still pure from the outside.
- r[type.mut.rc-cow] Every heap value carries a reference count. A mutating method updates in place when the count is one, and copies first otherwise; the copy is unique, so later changes to it are in place again. r[type.mut.move-last-use] The compiler moves a value at its last use, so building a collection in a loop never copies. r[type.mut.c-memory-scope] Values that own C memory are the one exception: they are released at the end of their scope, never at last use ([Interop with C](#memory-across-the-boundary)). r[type.mut.copy-report] `quince` `check` reports every update that may copy, and names the alias that causes it ([Memory](#memory-management)).
- r[type.mut.var-self] A method that changes its receiver declares `var self`, as in `fn push(var self, item: T)`. r[type.mut.var-self.call] It can be called only on a `var` binding or on a field path that starts at one (`state.items.push(x)`). On a `let` binding the call is a compile error, and the suggested fix makes the binding `var`. r[type.mut.var-self.rebind] A call computes the updated value and rebinds it, the same shorthand as `user.name = ...`, so it updates in place when the value is unique. The built-in `push`, `insert` and `update` are declared this way, and the query API prints `var self` in every signature, so whether a method mutates is always visible.
- r[expr.closure.capture-value] Closures capture values, never `var` bindings by reference. So a closure can't call a `var self` method on a value it captured; it copies the value into a local `var` first. r[expr.closure.no-assign-capture] Assigning to a captured binding is a compile error too, and the suggested fix uses `fold` or a `for` loop.

**Shared state.** State that must outlive a call or be seen by several functions lives in a `Ref<T>`:

```
fn record_hit(stats: Ref<Stats>, path: String) -> Stats uses State {
  stats.modify(fn(s) { Stats(..s, hits: s.hits + 1) })
}
```

- **Reference types.** r[type.ref.identity] A reference type has identity: copying one shares the same underlying state. These are the only types with reference semantics:

  | Type | Holds | Needs `uses State` | Crosses a task boundary |
  | --- | --- | --- | --- |
  | r[type.ref.ref] `Ref<T>` | One mutable value, within a single task | `get`, `set`, `modify` | No |
  | r[type.ref.atomic] `Atomic<T>` | One value updated atomically | Every read and write | Yes |
  | r[type.ref.concurrent-map] `ConcurrentMap<K, V>` | A map that tasks can update concurrently | Every read and write | Yes |
  | r[type.ref.channel] `Channel<T>` | A queue between tasks | Every operation | Yes |
  | r[type.ref.task-scope] `TaskScope` | A scope that owns long-lived tasks ([Concurrency](#long-lived-work-fire-and-forget)) | No: `spawn` is not an effect | Yes |
  | r[type.ref.resource] `Resource<T>` | C-owned memory with guaranteed cleanup ([Interop with C](#memory-across-the-boundary)) | No: close() carries its foreign block's effects, and ptr, keep and is\_closed need none | Only if its foreign type is `@shareable` |
  | r[type.ref.pool] `Pool<T, Err, uses E>` | A bounded set of values, such as connections, that tasks check out one at a time ([Interop with C](#memory-across-the-boundary)) | Yes: checking out carries the open function's effects plus State, and waiting is a cancellation point | Yes, even when T can't |
  | r[type.ref.callback] Callback\<F> | A Quince closure that C code may call later ([Interop with C](#callbacks)) | No: its calls carry its foreign block's effects | No |
  | r[type.ref.cbuf] `CBuf<T>` | C-allocated memory that C writes through and Quince reads ([Interop with C](#generated-bindings)) | No: creating, reading and writing it are pure | No |
  | r[type.ref.net] `Outbound`, `Inbound`, `Listener` | A network connection or listening socket ([Types the built-in effects use](../std/API.md#types-the-built-in-effects-use)) | No: their operations are `Net` and `Listen` operations | Yes |

  r[type.ref.pure-pass] Passing, storing or capturing a reference is pure. r[type.ref.pure-create] So is creating one, as in `Ref.new(0)`, since a fresh reference can't be observed without `uses State`. r[type.ref.state-access] Only reading or writing through it requires `uses State`, so a function with no `uses` clause always returns the same result for the same arguments. r[type.ref.no-merge] Same means equal under `Eq`: two calls that each create a `Ref` return different references, so the compiler never merges such calls. r[type.ref.release-pure] Releasing memory or a `Resource` is not an effect either, wherever the last reference drops: a resource's C cleanup runs without its foreign block's effects being required there ([Interop with C](#memory-across-the-boundary)).

  r[type.ref.in-fields] Reference types may appear in struct fields, enum payloads and closure captures. r[type.ref.crossing] Whether a type can cross a task boundary is a property the compiler derives: a type can cross unless it contains a `Ref<T>`, `Resource<T>`, `Callback<F>` or `CBuf<T>` (other than a `@shareable` one, or one held inside a `Pool`) anywhere inside it. r[type.ref.crossing.report] The query API reports this property for every type, and a rejected `spawn` names the field path that holds the reference.
- r[type.ref.no-cross] A `Ref<T>`, or any value that contains one, cannot cross a task boundary ([Concurrency](#shared-state)). State shared between tasks uses `Atomic<T>`, `ConcurrentMap<K, V>` or `Channel<T>`.

### Generics

- r[item.generic.bounds] Type parameters with interface bounds: `fn max<T: Ord>(a: T, b: T) -> T`.
- r[item.generic.multi-bounds] Several bounds are joined with `+`: `fn show_sorted<T: Ord + Display>(xs: List<T>) -> String`. r[item.generic.assoc-binding] A bound can fix an interface's associated type, as in Rust: `fn sum<I: Iterator<Item = Int>>(var it: I) -> Int` or `fn total<T: Add<T, Output = T>>(xs: List<T>) -> T`.
- r[item.iface.type-params] Interfaces may take type parameters: `interface From<T> { fn from(value: T) -> Self }`, and effect parameters ([Effects](#effect-variables)): `interface Iterator<uses E>`.
- r[item.iface.assoc-types] Interfaces may declare associated types, which each implementation fixes:

```
pub interface Iterator<uses E> {
  type Item
  fn next(var self) -> Option<Self.Item> uses E
}
```

- r[prelude.iter.next-var-self] `next` takes `var self`, so it advances the iterator it is called on (see [Mutation](#mutation)). r[prelude.iter.for-copy] `for x in it` iterates over a copy of `it`, so `it` doesn't need to be `var` and is unchanged after the loop. That holds when the iterator's source is a value, such as a list or a range. r[prelude.iter.shared-source] An iterator over an effect or a reference type, such as `Fs.read_lines` or `Channel.iter()`, shares its source, so every copy pulls from the same stream.
- r[type.tuple.derived] The compiler provides Eq, Ord, Hash, Encode, Decode and test generators for tuples of any size, so the standard library needs no per-size implementations.
- r[item.generic.excluded] Deliberately excluded: higher-kinded types (keeps type errors readable), variadic generics (`concurrent` and tuples are handled by the compiler instead), and specialization (which implementation runs must never depend on code elsewhere).
- Type arguments can be written at a call site, as in `json.decode<Order>(body)` or `Channel<Job>.``new(16)`, and are optional wherever inference finds them ([Type arguments and construction](#type-arguments-and-construction)).

### Interfaces

```
pub interface Display {
  fn display(self) -> String
}

impl Display for Email {
  fn display(self) -> String { self.0 }
}

impl Display for User {
  fn display(self) -> String { "{self.name} <{self.email}>" }
}
```

- r[item.iface.explicit] Implementation is always explicit with `impl`. A type never satisfies an interface by accident.
- r[item.iface.receiver] An impl's method takes the same receiver as the interface declares, `self` or `var self`. r[effect.handler.receiver] The one exception is a handler, which implements each operation of an `effect` as a method with a `self` receiver. r[effect.handler.uses] That method's `uses` may name the effects the effect is built on ([Effects](#declaring-and-inferring)), `State`, and the effect it handles. r[effect.handler.outer-call] A call to that effect from inside the handler goes to the handler that was in effect where the `with` installed it, never back to the handler itself. So a narrowing handler such as `net.only` passes allowed calls through, and a fake such as `fixed_clock` keeps its time in a reference type. r[effect.handler.uses-subset] It may name fewer of them, down to none.
- r[item.iface.in-scope] A method that comes from an interface can be called only when the interface is in scope, either defined in the same package or imported. r[item.iface.ambiguous] If two interfaces in scope define the same method name for a type, the call is a compile error, and the suggested fix names the interface: `Display.display(u)`. r[item.iface.ambiguous-inherent] The same holds when a type's own method and an interface method in scope share a name: the call is an error until it is written `User.display(u)` or `Display.display(u)`, so a new method in a dependency can never silently change which body runs. Picking an `impl` by argument type, as `Money.from(x)` does across several `From<T>` impls, is ordinary generic dispatch, not overloading.
- r[item.iface.orphan] An `impl` must live in the package that defines the type or the package that defines the interface (the orphan rule), so every implementation can be found in one of two places.
- r[item.iface.no-default] Interface methods have no bodies, default or otherwise. Shared behavior goes in a regular generic function.
- r[item.iface.no-dyn] There are no interface-typed values (no `dyn Interface`). Interfaces are used only as generic bounds, so the implementation that runs is always known from the types at the call site. To hold handlers of different types in one list, use a struct of closures or an enum.

### Operators on user types

r[prelude.user-op.interfaces] Arithmetic operators on user types come from six prelude interfaces, and each operator always calls the same one: `+` calls `Add`, `-` calls `Sub`, `*` calls `Mul`, `/` calls `Div`, `%` calls `Rem`, and unary `-` calls `Neg`. r[prelude.user-op.builtin-impls] The built-in numeric types implement them, and a user type opts in with an ordinary `impl`:

```
pub interface Sub<R> {
  type Output
  fn sub(self, rhs: R) -> Self.Output
}

impl Sub<Money> for Money {
  type Output = Money
  fn sub(self, rhs: Money) -> Money { ... }
}

impl Mul<Int> for Money {
  type Output = Money
  fn mul(self, rhs: Int) -> Money { ... }
}
```

- **Pure.** r[prelude.user-op.pure] The operator interfaces take no effect parameter, like `Eq` and `Ord`, so an operator can't do I/O or touch shared state. It can still panic, as `Int` overflow does.
- **Comparison.** r[prelude.user-op.compare] `==` and `!=` always come from `Eq`, and `<`, `<=`, `>` and `>=` from `Ord`. r[prelude.user-op.ne] There are no separate comparison interfaces, and `a != b` always means `!(a == b)`.
- **The left operand picks the impl.** r[prelude.user-op.left-picks] `R` is the right operand's type, so `impl Mul<Int> for Money` allows `amount * 3` and nothing else. r[prelude.user-op.swap-fix] `3 * amount` and `amount * amount` are compile errors, and for the first one the suggested fix swaps the operands. r[prelude.user-op.output] `Output` lets an operator return another type, as `Timestamp - Timestamp` returns `Duration`.
- **A fixed set.** r[prelude.user-op.fixed] No other operator can be overloaded. `&&`, `||`, `!`, `?`, the bitwise operators, ranges, field access and calls keep their built-in meaning, and a program can't define new operators.
- **Compound assignment.** r[prelude.user-op.compound] `a += b` always means `a = a + b`, and the same holds for every other operator, so there is nothing more to implement.
- **Tooling.** r[prelude.user-op.query] The query API reports which impl an operator expression calls, so a tool can go from `a - b` straight to its body.

### Automatic equality and hashing

- r[prelude.auto.eq-hash] Every struct and enum implements `Eq` and `Hash` automatically, field by field, when all of its fields do. r[prelude.auto.builtin-eq-hash] The built-in `List`, `Map`, `Set`, `Option`, `Result` and tuples have `Eq` and `Hash` whenever their element types do. r[prelude.auto.ref-no-eq] Reference types have neither, so a struct that holds one has no automatic `Eq` or `Hash`. There is nothing to write and nothing to forget. r[prelude.auto.manual-eq] Writing `impl Eq` by hand turns off the automatic `Hash`, because the two must agree for maps and sets to work: the type has no `Hash` until it writes one, and the error on a missing `Hash` says why.
- r[prelude.auto.encode] Every struct and enum whose fields are all `pub` also implements `Encode` and `Decode` automatically, when all of its field types do (see [Serialization](#serialization)).
- r[prelude.auto.debug] Every value also has a built-in debug form, which assertions, panic messages and `quince` `test` print. It is not `Display` and can't be overridden.
- r[prelude.dbg.behavior] `dbg(x)` writes `x`'s debug form and its source location to standard error and returns `x`, so it can wrap any expression. r[prelude.dbg.pure] It is not an effect and is allowed in any function, pure ones included, so adding a print while debugging never changes a signature. r[prelude.dbg.release] `quince` `check` rejects `dbg` in release builds, and `quince` `publish` refuses a package that contains one.
- r[prelude.auto.no-ord-display] `Ord` and `Display` are never automatic, because field order and formatting are real choices. Write an `impl` for them.
- r[prelude.auto.opaque] `@opaque` turns off the automatic `Eq`, `Hash`, debug form, test generator, `Encode` and `Decode`, for types whose fields shouldn't be compared or shown, such as handles and secrets.

### Serialization

A struct or enum whose fields are all `pub` implements the std interfaces `Encode` and `Decode` automatically when all of its field types do, the same way it gets `Eq` and `Hash`. r[prelude.wire.field-visibility] Only the fields' visibility counts, not the type's own, so a private struct whose fields are all `pub` gets them too. r[prelude.wire.data-model] The std formats (`json`, `toml`, `form`) all read and write through one format-neutral data model, so a type that encodes to one of them encodes to all of them. There is no derive to write and no hand-written codec to drift out of date.

```
@wire(case: camel)
pub struct Order {
  @wire("type")
  pub kind: OrderKind,
  pub customer_id: CustomerId,
  pub items: List<Item>,
  pub note: Option<String>,
}

@wire(tag: "kind")
pub enum OrderKind {
  Pickup,
  Delivery(address: Address),
}

let order = json.decode<Order>(body)?   // an Order: decode returns Result<Order, DecodeError>, and ? unwraps it
let text = json.encode(order)           // String
```

- **Wire names.** r[prelude.wire.name.default] A field or variant appears on the wire under its Quince name. r[prelude.wire.name.explicit] `@wire("name")` on a field or variant sets its wire name exactly, which is also how keyword names such as `type`, `in` and `match` are written. r[prelude.wire.case] `@wire(case: camel)` on a type renames every field and variant that has no `@wire` of its own; the cases are `camel`, `snake`, `kebab`, `pascal` and `upper_snake`.
- **Shapes.** r[prelude.wire.shape.struct] A struct is an object. r[prelude.wire.shape.collections] `List` and `Set` are arrays, tuples are fixed-length arrays, and `Bytes` is a base64 string. r[prelude.wire.shape.map] A `Map` is an object when its key is `String` or a newtype over `String`, and an array of `[key, value]` pairs otherwise. r[prelude.wire.shape.newtype] Newtypes are transparent: `UserId(Int)` is a plain number.
- **Optional fields.** r[prelude.wire.option] An `Option` field decodes to `None` when it is missing or `null`, and `None` is left out on encode. r[prelude.wire.required] Every other field must be present.
- **Enums.** r[prelude.wire.enum.unit] An enum whose variants carry no data is a string. r[prelude.wire.enum.tagged] Any other enum is an object with a `"type"` tag naming the variant, with the variant's fields beside it; `@wire(tag: "kind")` renames the tag. r[prelude.wire.enum.tag-clash] A variant field with the same wire name as the tag is a compile error.
- **Numbers.** r[prelude.wire.number.int] `Int` decodes only from a whole number within the `Int64` range. r[prelude.wire.number.float] `Float` decodes from any number. r[prelude.wire.number.nan] Encoding a `NaN` or infinite `Float` panics, because JSON can't represent it.
- **Decoding is checked.** r[prelude.wire.unknown-fields] Unknown fields are ignored. r[prelude.wire.invariants] Every `requires` invariant on a decoded struct is checked. r[prelude.wire.decode-error] A failure is a `DecodeError(path:, expected:, found:)`, where the path points into the input, as in `$.items[3].price: expected Int, found "12"`.
- **Writing your own.** r[prelude.wire.custom] An `impl Encode` or `impl Decode` replaces the automatic one, for example to write a `Timestamp` as an RFC 3339 string. r[prelude.wire.private-fields] A type with private fields gets no automatic impl, because decoding would build it without going through its own functions; its package writes an `impl Decode` that calls its constructor. r[prelude.wire.opaque] `@opaque` turns both off. r[prelude.wire.formats] Other formats implement std's `Encoder` and `Decoder` interfaces against the same data model. r[prelude.wire.pure] Encoding and decoding are pure.
- **Dynamic data.** `json.Value` holds JSON whose shape isn't known ahead of time.
- **Tooling.** r[prelude.wire.query] `quince` `query wire <Type>` prints the type's wire shape. r[prelude.wire.semver] `quince` `publish` treats any change to the wire shape of a public type as breaking ([Dependencies and publishing](#enforced-semantic-versioning)), and when the change is a renamed field, its suggested fix adds `@wire("old_name")` to keep the old wire name.

### Methods

- r[item.method.impl] Methods and associated functions are declared in an `impl` block for the type, in the type's own package: ` impl User {  ``fn create(email: Email) -> Result<User, UserError>` `{ ... } }`. r[item.method.call] A function with a `self` or `var self` receiver is called as a method (`u.display_name()`); one without is called on the type (`User.create(email)`, `Config.parse(text)`). r[item.method.visibility] An inherent method or associated function is visible outside the package only when marked `pub`. r[item.method.impl-no-pub] Methods in an interface impl or a handler take no `pub`: they are visible wherever both the type and the interface or effect are.
- r[effect.op-call] Effect operations are called through the effect's name: `Log.info("started")`, `Db.query(sql)`. The handler that runs is the one `with` installed ([Effects](#handlers-in-tests-and-production)).
- A struct literal may write a field whose value is a binding of the same name once: `Dashboard(profile:, orders:)`. r[tool.fmt.field-shorthand] `quince` `fmt` rewrites `profile: profile` to that form, so each literal has one canonical layout.

### Inference rules

- r[type.infer.required] Required annotations: function parameters, return types (leaving one out means Unit, and quince fmt removes a written -> Unit, so there is one form; the same holds in a function type such as `fn(Int) uses Log`), struct fields, constants.
- r[type.infer.inferred] Inferred: local `let`/`var` bindings, closure parameters when passed directly to a typed parameter, generic arguments at call sites.
- r[type.infer.locality] If inference would need information from more than one statement away, the compiler asks for an annotation instead of guessing.
- **Never.** r[type.never.type] `return`, `break`, `continue`, `unreachable(...)`, `todo(...)` and `Process.exit(...)` have type `Never`, so each fits in any branch or match arm: `None => return Err(e)` sits beside an arm that yields a value. r[type.never.fn] A function can declare `-> Never`, as in `fn fatal(msg: String) -> Never uses Console, Process`, and the compiler rejects it if the end of its body is reachable. r[type.never.while-true] A `while true` with no `break` aimed at it can't complete ([Statements](#keywords)), so a server loop can be typed `-> Never`.

## 4. Errors and failure

Quince separates expected failures, which are values in the return type, from bugs, which panic and stop the program. r[err.panic.no-catch] There are no exceptions and no way to catch a panic in ordinary code: a panic ends its task, and only the scope that owns the task decides what happens next ([Concurrency](#9-concurrency)). Cancellation is a third case, separate from both: it unwinds a task but is neither an error value nor a bug ([Cancellation](#cancellation)).

### Expected failures: `Result` and `Option`

- r[err.result-option] A function that can fail returns `Result<T, E>`. A function that may have no answer returns `Option<T>`.
- Error types are usually enums, so callers can match on every failure mode:

```
pub enum ParseError {
  Empty,
  InvalidChar(at: Int, found: Char),
  TooLong(max: Int),
}
```

- r[err.unused-result] An unused `Result` is a compile error. r[err.discard] To discard one deliberately, write `let _ = save(file)`. r[err.unused-value] An unused value of any other type except `Unit` is a warning, which `let _ =` also silences. r[err.var-self-value] A value returned by a `var self` method, such as the `Option` from `Map.insert`, raises no warning. r[stmt.bind.underscore] \_ is not a binding, so it can be written any number of times in one scope.

### The `?` operator

- r[err.try.unwrap] `expr?` unwraps `Ok`/`Some` or returns early with the `Err`/`None`.
- r[err.try.from] The error must convert to the enclosing function's error type through an explicit `impl From<A> for B`. There is no automatic widening, so every error path can be traced by searching for its `From` impl.
- `.map_err(f)` and `.ok_or(err)` cover cases with no `From` impl. r[err.try.same-type] When the error type already equals the function's error type, `?` needs no `From` impl. r[err.try.closure] Inside a closure, `?` returns from the closure, not from the enclosing function. r[err.try.option] `?` on an `Option` works only in a function that returns `Option`; in one that returns `Result`, the suggested fix is `.ok_or(e)?`.

### Exhaustive `match`

- r[expr.match.exhaustive] Every `match` must cover every variant. A missing case is a compile error that lists the missing variants.
- r[expr.match.wildcard-foreign] A wildcard arm (`_`) on a non-extensible enum from another package triggers a warning, because a later major version could add variants, and the wildcard would silently swallow them when you upgrade.
- r[expr.match.guard] Guards are allowed: `n if n > 0 => ...`. r[expr.match.guard-exhaustive] Arms with guards don't count toward exhaustiveness.
- r[expr.match.empty-type] A case whose type has no values needs no arm. A `Result<T, Never>` can't be `Err`, so matching it needs only `Ok(v)`, and code generic over an error type can be given `Never` to say it can't fail.

### Patterns

- r[pat.variant-fields] A variant's named fields are matched by name: `Shape.Rect(width: w, height: h)`. r[pat.shorthand] A field name with nothing after the colon binds a variable of the same name: `Shape.Rect(width:, height:)`.
- r[pat.rest] `..` matches the remaining fields: `Shape.Rect(width:, ..)`. r[pat.all-fields] Every field is either named or covered by `..`.
- r[pat.forms] Literals (`0`, `"GET"`), nested patterns (`Ok(Some(x))`) and alternatives (`Shape.Circle(..) | Shape.Point`) are allowed. r[pat.alt-names] Every alternative binds the same names. r[pat.negative] A numeric literal pattern or range bound may start with `-`, as in `-5 => ...`.
- r[pat.const] A constant is a pattern too. A `NAME`, such as `MINZ_BUF_ERROR`, or a type constant, such as `Status.OK`, matches a value equal to it under `Eq`. r[pat.const-exhaustive] Constants count like literals for exhaustiveness, and a lowercase `name` always binds a new variable.
- r[pat.qualified-variant] A variant is always written with its enum's name, in patterns and in expressions alike: `Shape.Rect`, `LoadError.Orders`, `Command.Stop`. r[pat.bare-variants] The prelude's `Some`, `None`, `Ok` and `Err` are the only bare variant names.
- r[pat.struct] A struct is matched the way a variant is, by its type name and named fields: `User(name:, ..)`. Every rule above applies to it.

### Pattern tests with `is`

r[expr.is.bool] `expr is Pattern` evaluates to `Bool` and can be used anywhere an expression can. r[expr.is.scope] Inside an `if` condition, variables bound by the pattern are in scope in the true branch, in the right side of a following && and in the body of a while the test heads (never through || or !):

```
if response is Ok(page) {
  render(page)
}
```

`is` complements `match`: use it to check or extract one case, and `match` when every case needs handling.

### Panics (bugs)

A panic means the program has a bug, not that something in the world went wrong. These panic:

- r[err.panic.arith] Integer overflow, division by zero, and a shift count outside the bit width
- r[err.panic.index] Out-of-bounds indexing with `xs[i]` (use `xs.get(i)` for an `Option`)
- r[type.map.no-index] Maps have no `m[k]` at all, because a missing key is an expected case, not a bug. `m.get(k)` returns an `Option`.
- r[err.panic.contract] A failed `requires` or `ensures` contract ([Contracts](#requires-and-ensures))
- r[err.panic.explicit] An explicit `unreachable("reason")` or `todo("reason")`. Both have type Never ([Types](#built-in-types)), so either can stand in for any value while code is still being written

r[err.panic.message] Every panic message includes the source location and, for contracts, the values that violated the condition. r[err.panic.task] A panic stops the current task; the scope that owns the task may log the panic or restart it, as its FailurePolicy says ([Concurrency](#long-lived-work-fire-and-forget)), but cannot inspect or resume it. That scope boundary is the only place a panic is handled.

## 5. Effects and capabilities

r[effect.pure-default] Functions are pure by default: a function with no `uses` clause returns the same result for the same arguments and changes nothing its caller can observe. r[effect.uses-required] Any function that touches the outside world or shared state declares it with a `uses` clause, and the compiler rejects calls to effectful functions from a context that doesn't have that effect.

### Built-in effects

| Effect | Grants |
| --- | --- |
| r[effect.net] `Net` | Outbound network connections |
| r[effect.listen] `Listen` | Accepting inbound connections |
| r[effect.fs] `Fs` | Reading and writing files |
| r[effect.clock] `Clock` | Reading the current time, sleeping |
| r[effect.random] `Random` | Nondeterministic randomness |
| r[effect.env] `Env` | Environment variables and command-line arguments |
| r[effect.process] `Process` | Spawning subprocesses and ending the program early |
| r[effect.console] `Console` | Reading standard input; writing standard output and standard error |
| r[effect.state] `State` | Reading or writing a reference type whose row in the [Reference types](#mutation) table needs it |

Each built-in effect's operations, such as Clock.now() and Fs.read\_text(path), are listed in [Built-in effect operations](#built-in-effect-operations). r[effect.log-not-builtin] Logging is not built in. The standard library declares it as a package effect, `pub effect Log uses Console, Clock { ... }`, so a function that logs says `uses Log`, and a test can capture its logs with `with Log = ...` without touching standard output.

r[effect.decl] Packages declare their own effects with `effect`, naming the built-in effects each one is built on and listing its operations. r[effect.handler.def] A handler is any value whose type implements the operations with `impl Db for T`, and only those bodies may perform the underlying effects. r[effect.handler.crossing] A handler's type must be able to cross task boundaries ([Types](#mutation)), since tasks inherit handlers ([Concurrency](#9-concurrency)); that's why `PgDb` holds a `Pool` rather than a single connection:

`pg`, `PgConn` and `PgError`, here and in [Interop with C](#11-interop), stand for a hypothetical third-party Postgres package. They are not part of std.

```
pub effect Db uses Net, State {
  fn query(sql: String) -> Result<Rows, DbError>
  fn execute(sql: String) -> Result<Int, DbError>
}

pub struct PgDb { pool: Pool<PgConn, PgError, uses Net> }

impl Db for PgDb {
  fn query(self, sql: String) -> Result<Rows, DbError> uses Net, State { ... }
  fn execute(self, sql: String) -> Result<Int, DbError> uses Net, State { ... }
}
```

- r[effect.op-uses] Calling a `Db` operation needs `uses Db`. r[effect.no-expand] A caller with `uses Db` doesn't also list `Net`; `quince` `query signature` shows what `Db` is built on.
- r[effect.interface] The operations are the effect's whole interface: code outside the package reaches `Db` only through them, and they are what `with Db = ...` installs, in tests and in `main`.
- r[effect.keyword-decl-only] `effect` appears only in declarations. [Effect variables](#effect-variables) are written `uses E`, so the keyword never appears in a generic parameter list.
- r[effect.no-generics] Effects and their operations take no type parameters in v1. Handlers are installed at runtime and there is no `dyn`, so a generic operation such as `Cache.get<T>(key)` would have no implementation the compiler could pick. A cache that holds many types stores `Bytes` or `json.Value`, and an ordinary generic function gives it a typed wrapper.
- r[effect.copy] `effect UsersDb = Db` declares a distinct effect with the same operations. A program with two databases declares one copy per database, writes `uses UsersDb, OrdersDb`, calls `UsersDb.query(sql)`, and installs each with its own `with`. The type checker keeps the two apart, and a signature says which database a function touches. r[effect.copy-handler] A handler for `Db` (such as `PgDb`) handles every copy. When the set is known only at runtime (shards, one database per tenant), use one effect whose operations take the key, such as `TenantDb.query(tenant, sql)`. r[effect.no-index] Effects can't be indexed by a value.

### Declaring and inferring

```
pub fn sync_users(api: ApiClient, store: UserStore) -> Result<Int, SyncError>
  uses Net, Db, Log
{ ... }
```

- r[effect.declare-all] Every function declares its effects, public or private, so a reader never has to open a callee to learn what a function can do. r[effect.fix-uses] `quince` `fix` writes in a missing or out-of-date `uses` clause, so nobody works it out by hand.
- r[effect.unused-warn] Declaring an effect a function doesn't use is a warning, so signatures stay honest in both directions. r[effect.unused-pub] A `pub` function is exempt, because adding an effect later is a breaking change ([Dependencies and publishing](#enforced-semantic-versioning)), and r[effect.fix-pub-review] `quince` `fix` marks adding an effect to a `pub` signature `needs-review`.
- r[effect.closure-type] Closures carry effects in their type: `fn(Int) -> Int uses Log`. r[effect.pure-param] A parameter type with no `uses` accepts only pure closures. A function that should accept effectful ones declares an [effect variable](#effect-variables).

### Effect variables

r[effect.effvar.decl] A function that takes a closure and calls it declares an effect variable among its generic parameters with `uses`, and puts that variable in its own `uses` clause. The function then has exactly the effects of the closure it was given:

```
impl<T> List<T> {
  fn map<U, uses E>(self, f: fn(T) -> U uses E) -> List<U> uses E
}

pub fn retry<T, Err, uses E>(attempts: Int, f: fn() -> Result<T, Err> uses E)
  -> Result<T, Err>
  uses E, Clock
```

- r[effect.effvar.inferred] **Inferred at every call, never written there.** `xs.map(fn(x) { x * 2 })` is pure. `xs.map(fn(u) { describe(u) })` calls `describe`, which uses `Log`, so the caller needs `Log`. `keys.map(fn(k) { cache.get(k) })` reads a `Ref`, so it uses `State`; reference types need no extra rule.
- r[effect.effvar.opaque] **Opaque inside the function.** The body can call `f` but can't perform an effect from `E` itself or test what `E` holds. A higher-order function's behavior can't depend on which effects its caller passed in, so its signature tells a reader everything it can do.
- r[effect.effvar.combine] **Combining.** A `uses` clause can mix variables and named effects (`uses E, Clock`). Two closure parameters can share one variable or take one each (`<uses E1, uses E2>` and `uses E1, E2`). An effect variable stands for a set of effects; there is no syntax for subtracting effects from one.
- r[effect.effvar.subsumption] Subsumption. A closure whose effects are a subset of the expected ones is accepted, so a pure closure fits wherever an effectful one is expected. r[effect.effvar.union] When several closures meet in one type, inference takes the union of their effects: a list holding a route that uses `Db` and one that uses `Net` has type `List<Route<uses Db, Net>>`.
- r[effect.effvar.types] **Structs and enums** that store a closure take effect parameters the same way: `struct Route<uses E> { path: String, handler: fn(Request) -> Response uses E }`. So does `serve`: `pub fn serve<uses E>(port: Int, handler: fn(Request) -> Response uses E) -> Result<Unit, ServeError> uses Listen, E`. This is abridged: std.http's `serve` also takes `host:` and `max_body:`, which have defaults.
- r[effect.effvar.iface] **Interfaces** may take effect parameters, like `Iterator<uses E>` in [Prelude and special interfaces](#interfaces-1), so `impl Iterator<uses Fs> for FileLines` can read a file one line at a time. r[effect.effvar.default-pure] Leaving out an effect argument means no effects: a `T: Iterator` bound accepts only pure iterators. r[effect.effvar.pure-ifaces] `Eq`, `Ord`, `Hash` and `Display` take no effect parameter, and neither do the operator interfaces ([Prelude and special interfaces](#operators-on-user-types)), so comparing, hashing, printing and arithmetic never have effects.
- r[effect.effvar.contracts] **Contracts** must be pure, so a `requires` or `ensures` can't call a closure whose type includes an effect variable.
- r[effect.effvar.diagnostic] **Diagnostics.** When a closure is rejected because its parameter type has no `uses`, the error names the closure's effects and the call in it that brings in each one. If the function is in the caller's own package, the suggested fix adds an effect variable to it.

### Capabilities at runtime

r[exec.cap.main] Each effect is backed by a capability that the runtime supplies only to `main`:

```
fn main() -> Result<Unit, AppError> uses * { ... }
```

- r[exec.cap.static] A uses clause is a static permission. r[exec.cap.context] At runtime, the capability behind each effect is implicit context that travels with each call. The compiler guarantees it is present wherever a uses clause says so, and nothing is passed by hand. r[exec.cap.with] A with block replaces that context for the code inside it.
- r[exec.cap.inherit] Tasks inherit this context: a spawned task keeps the handlers in effect where it was spawned ([Concurrency](#9-concurrency)), and a C callback uses those of the call that registered it ([Interop with C](#callbacks)). r[exec.cap.no-capture] Nothing else captures handlers: a closure's effects go to the handlers in effect where it is called, and a lazy `Iter`'s to those in effect where it is pulled, even when it was created inside a different `with`.
- r[exec.cap.manifest-limits] A program's entry in quince.toml (\[cmd.\<name>\]) can narrow its capabilities, e.g., `Net` limited to `api.example.com` or `Fs` limited to `./data`. r[exec.cap.violation] A violation is not a panic: the operation returns Err(NetError.PermissionDenied) or Err(FsError.PermissionDenied), and every std error type built on Net or Fs carries that error, directly or inside the NetError or FsError it wraps, and its message names the manifest line.
- r[exec.cap.dependency] A dependency can never use an effect its declared API doesn't list, so auditing a package means reading its signatures. r[exec.cap.query-effects] quince query effects \<path> prints a function's effects with every package effect expanded to the built-in effects beneath it, which is the view to audit.
- r[exec.main.err-exit] If `main` returns `Err(e)`, the runtime prints `e`'s `Display` to standard error and exits with code 1. r[exec.main.panic-exit] If the main task panics, the runtime prints the panic report to standard error and exits with code 101. r[exec.main.process-exit] `Process.exit(code)` cancels every running scope, waits for cleanup, then exits with `code`. It flushes standard output and standard error first. r[exec.main.signature] `main` is written `fn main()` or `fn main() -> Result<Unit, E>`, where `E` implements `Display`, with `uses *` on either.

**Planned after v1: finer-grained limits.** Effects in signatures stay coarse (`uses Net`, not `uses Net("host")`): hosts are often dynamic, and per-host effects would churn signatures and turn new endpoints into breaking changes. Finer control will come later through one more runtime mechanism, which v1's runtime and manifest format must not rule out:

- Per-dependency limits in the manifest (e.g., an analytics package may reach only its own events host and no files), as a defense against compromised dependencies.

### Handlers in tests and production

Tests can replace an effect's implementation for a block:

```
test "expired sessions are removed" {
  with Clock = testing.fixed_clock(ts"2026-01-01T00:00:00Z") {
    assert(purge_sessions(store).len() == 3)
  }
}
```

A handler is an ordinary value whose type implements an effect's operations (see [package effects](#declaring-and-inferring)). r[effect.with.install] `with` installs handlers for the code inside it, in tests and in production. This is how a program configures its database, logger and other services:

```
fn main() -> Result<Unit, AppError> uses * {
  let cfg = load_config("app.toml")?
  with Db = PgDb.connect(cfg.db, max: 20)?,
       Log = log.json(cfg.log_level) {
    http.serve(cfg.port, handler: route).map_err(AppError.Serve)
  }
}
```

- r[effect.with.package-main-only] Outside test code, a `with` for a package effect may appear only in `main`. A package that builds several programs, such as a server and a worker, gives each its own small package with one main ([Packages](#packages)), so a program's production wiring always sits in one function an agent can read, and "which `Db` is this?" is always answered there.
- r[exec.main.no-package-effects] `main` gets no package effects from `uses *`. r[exec.main.handler-proof] The compiler proves that every package effect reachable from `main` is covered by a `with`; a missing one is a compile error that names the call path.
- r[exec.main.uses-star] `uses *` is allowed only on `main`. Anywhere else it is a compile error, and `quince` `fix` writes the real `uses` clause. Tests opt into real effects with `test "name" uses Net`.
- r[test.package-effect-proof] A test that reaches a package effect with no `with` around it is a compile error that names the call path, the same proof as for `main`. The suggested fix wraps the call in `with Db = ...`. Built-in effects keep their default fakes ([Testing](#unit-tests)); package effects have none, because no fake database can be guessed. r[test.default-handler] An effect's own package may name one with `@test_default(f)` on the effect declaration, where `f` takes no arguments and returns a handler. std.log names `capture_log`, so a test of code that logs needs no `with Log`.
- r[effect.with.nested-warn] Outside tests, a `with` nested inside another `with` for the same effect is a warning, because an override deep in the call stack hides which handler runs.
- r[effect.with.query-handlers] `quince` `query handlers <fn>` prints which handler each of a function's effects resolves to from each program's main.
- r[effect.with.builtin-anywhere] A `with` for a built-in effect can narrow it anywhere, in any function: `with Net = net.only("api.example.com") { run_plugin(input) }`.
- r[effect.handler.no-continuation] Handlers are a handler type's value, not a continuation: they can't resume, abort or capture the code that called them. `with` is the only form of effect handler in the language.

## 6. Contracts and testing

Contracts state what a function promises; tests and examples check it. r[contract.runtime] Contracts are checked at runtime in debug and test builds, and in release builds unless a function opts out with `@unchecked` after profiling. r[contract.cost-warn] Because contracts usually run in production, quince check warns about any contract that calls a standard-library function documented as slower than constant time, such as xs.sorted(), or a function of your own that contains a loop or recursion.

### `requires` and `ensures`

```
pub fn withdraw(account: Account, amount: Money) -> Result<Account, WithdrawError>
  requires amount > Money.ZERO
  ensures(r) match r {
    Ok(a) => a.balance == account.balance - amount,
    Err(_) => true,
  }
{ ... }
```

- r[contract.requires] `requires` constrains the caller; a violation panics and blames the call site.
- r[contract.ensures] `ensures` constrains the function; a violation panics and blames the function. r[contract.ensures-name] `ensures(r)` names the return value `r` (any name works) for use in the condition.
- r[contract.pure] Contract expressions must be pure: no effects and no mutation. Reading a reference type needs `uses State`, so a contract can't read shared state such as `stats.get()`; it constrains only arguments and the return value.
- r[contract.pub-visibility] The contracts of a `pub` function may name only what its callers can see: its parameters, `pub` fields and `pub` functions. Otherwise it is a compile error, and the suggested fix adds a `pub` pure accessor. A `requires` blames the caller, so the caller must be able to read it through `quince` `query signature`. Invariants on private fields belong in the struct's own `requires`.
- r[contract.var-self] In a method that takes `var self`, `ensures` sees `self` as it is after the call. r[contract.no-old] There is no way to name the value from before the call, since there is no `old()`.
- r[contract.invariant] Struct invariants use `requires` on the struct and are checked after every construction and update: `pub struct Interval { pub start: Int, pub end: Int } requires start <= end`. r[contract.invariant-update] Each assignment to a field of a var binding is a separate update, so a change that moves two fields together is written as one: `r = Interval(..r, start: 10, end: 20)`.
- r[contract.iface] Interface methods may declare `requires` and `ensures`. r[contract.iface-inherit] Every `impl` inherits them unchanged and can't add or remove any, so a method's contract is always read in one place: the interface.

### Executable examples in doc comments

```
/// Splits a full name into first and last.
///   >>> split_name("Ada Lovelace")
///   Some(("Ada", "Lovelace"))
///   >>> split_name("")
///   None
pub fn split_name(full: String) -> Option<(String, String)> { ... }
```

r[test.doc.run] `quince` `test` runs every example. A doc example that fails is a test failure, so documentation can't drift from behavior. r[test.doc.compare] The line after a `>>>` line is a Quince expression, compared with the result using `==`, and examples run under the same effect fakes as tests.

### Unit tests

- r[test.unit-location] `test "description" { ... }` blocks live in the same file as the code they test, after the code. r[test.name] A test's name is plain text: it doesn't interpolate, so braces in it need no escape. r[test.attr] A top-level declaration marked `@test`, such as a fake handler, is compiled only by `quince` `test`. r[test.attr-pub] A `pub` `@test` declaration in a library is also visible to the test code of packages that depend on it, so a library can ship a fake for its own effect. r[test.attr-use] Naming it from code that isn't a test is a compile error. r[test.cmd] Program packages are no exception: `quince` `test` runs the tests in every `cmd/<name>/` folder too.
- r[test.fakes] Tests may use any effect, but `Net`, `Clock`, and `Random` are replaced with deterministic fakes unless a test opts in, written test "name" uses Net { ... }. r[test.uses] A test needs no `uses` for any other effect: its body may use every effect, and `uses` on a test only swaps a fake for the real one. r[test.clock-sleep] Under every fake clock, the default one included, `Clock.sleep` moves fake time forward instead of waiting, so a `with_timeout` in a test fires without real waiting. r[test.clock-virtual] A test has one fake clock, shared by all its tasks, and its time is virtual: a sleep blocks its task until fake time reaches the sleep's deadline, and fake time jumps to the earliest pending deadline only when every task in the test is blocked. So a task that can still run never times out because another task slept. r[test.clock-timeout] A timeout that fires during a sleep moves fake time only to the timeout's own deadline, which is what Clock.monotonic() then reads. r[test.default-fakes] The default fake clock starts at `ts"2000-01-01T00:00:00Z"`, the default `Net` refuses every connection with `NetError.Refused`, and the default `Random` is seeded from the run's seed.
- r[test.fakes-fs-console] `Fs` and `Console` are faked by default too: `Fs` is an empty in-memory file system and `Console` output is captured. r[test.fakes-other] Env is empty, Listen accepts only in-memory connections from the same test, and Process is refused unless the test opts in. r[test.deterministic] A test with no opt-ins is fully deterministic, so the same seed gives the same run. r[test.opt-in-report] Opting into a real effect (`uses Net`, `uses Fs`) gives that up, and the test report says which tests did. r[test.foreign-opt-in] Foreign code ignores fakes, so a test that reaches a foreign block must opt in to each faked effect the block declares ([Interop with C](#testing-foreign-code)).
- r[test.assert-values] Assertions show the full values on both sides of a failed comparison.

r[diag.query.without-tests] Unit tests make files longer, so `quince` `query source <file> --without-tests` returns the file with its test blocks folded away. An agent reading code to change one function doesn't pay for every test in its context.

### Integration tests

Integration tests check how a package behaves from the outside, through its public API only. They live in a top-level `tests/` directory instead of beside the code they test.

- r[test.itest.packages] Each file directly in `tests/`, and each folder tests/\<name>/, is compiled as a separate test package that depends on the library ([Packages](#packages)), so it can use only `pub` names, exactly as an outside user would.
- They don't break when internals are rewritten, which makes them the main safety net for refactoring.
- r[test.itest.same-syntax] Same `test` block syntax, effect fakes, and property tests as unit tests. r[test.itest.flags] `quince` `test` runs both; `quince` `test --unit` or `--integration` runs one kind.

### Property tests

```
test "parse inverts display" forall (n: Int) {
  assert(Int.parse(n.display()) == Ok(n))
}
```

- r[test.prop.forall] `forall` generates inputs from the types; every type has a derived generator unless it is marked @opaque or has private fields. r[test.prop.filter] Generated values are filtered against the type's requires invariants with a retry limit, and when the limit is reached the test reports which invariant it couldn't satisfy instead of passing.
- r[test.prop.custom] `impl Generate for T` replaces the derived generator, or provides one for an `@opaque` type or a type with private fields, usually by calling the type's own constructor. r[test.prop.discard] A generated case that breaks a `requires` of the function under test is discarded, not counted as a failure, and the report says how many were discarded.
- r[test.prop.contracts] Contracts on the function under test act as extra assertions on every generated case.
- r[test.prop.shrink] Failing cases are shrunk to a minimal input and saved to `.``quince``/regressions`, which reruns them on every later test run.

### Static verification (optional)

r[contract.verify.opt-in] `quince` `verify` attempts to prove contracts with an SMT solver. It is opt-in per function with `@verify`, reports which contracts it proved, and never blocks a build unless the project config makes it required.

## 7. Modules, packages, and versioning

r[mod.namespace] A package is one namespace, and a project holds one library package plus its programs and tests ([Packages](#packages)): files organize code but never change what a name is called or how it is imported. r[mod.name-sources] Every name used in a file is either defined in the package, listed in an import, or in the prelude ([Prelude and special interfaces](#modules-and-imports)). r[mod.pkg.one-version] Each dependency, meaning one repository folder at one major version, resolves to one exact version, pinned in the lockfile by content hash, so nothing a dependent relies on changes without a change to its lockfile.

### Modules and imports

```
import std.http.{Response}
import std.json
import acme_billing.{Invoice, InvoiceError}
```

- r[mod.import.explicit] Imports list names explicitly; `import std.json` brings in the package name only, used as `json.decode(...)`. r[mod.import.fn-qualified] A function is always called through its package, as `json.decode(body)` or `http.get(url)`, so every call says where it comes from. r[mod.import.by-name-kinds] Only types, interfaces, effects and constants are imported by name; importing a function by name is a compile error whose fix imports its package instead. r[mod.import.prelude-fns] The prelude's free functions (`assert`, `todo`, `unreachable` and `dbg`) are called bare. r[mod.import.self] To use both the package name and some of its names, write one line, `import std.log.{self, Log}`. r[mod.import.one-line] A package appears in at most one import line, and `quince` `fmt` merges two lines into that form. r[mod.import.as] When two packages in one file end in the same name, one is renamed for the file with `as`, which only a package import takes: `import json as acme_json` beside `import std.json`. r[mod.import.major] A second major version is imported by its manifest key, as `import http_v2` ([The manifest](#the-manifest)); no import or type name contains `@`. r[mod.import.test-only] An import used only by `test` blocks is allowed at the top of the file and raises no unused-import warning.
- r[prelude.only] The prelude is the only set of names a file can use without importing them, and the [table of prelude names](#prelude-names) lists every one. r[prelude.contents] It holds `Option`, `Result` and their variants, the number, text, time and collection types, the core and operator interfaces, the built-in effects, `TaskScope`, `FailurePolicy`, `ScopeFailed`, `assert`, `todo`, `unreachable` and `dbg`. r[prelude.not-ref-types] The other reference types of [Types](#mutation) are not in it: `Ref`, `Atomic`, `ConcurrentMap`, `Channel` and `Pool` are imported from std.sync, and `Resource`, `Callback` and the C types from std.ffi. r[prelude.fixed] The language fixes the prelude, and it changes only in an edition. r[prelude.criterion] A name is in the prelude only if the language's syntax or the compiler needs it, so any name that isn't imported is a language name. Everything else, including test helpers such as `fixed_clock` from std.testing, is imported.
- r[mod.extension] Source files use the `.qnc` extension.
- r[mod.unique-names] Every top-level name is unique within its package, and its import path is the package, never the file. r[mod.no-reexport] There are no re-exports, so a name has exactly one import path, and moving a definition to another file never changes it.
- r[mod.pkg.program] A program is a package of its own in `cmd/<name>/`, holding the one `main` for that binary and depending on the library package beside it, as in Go. r[mod.pkg.main-location] `main` may appear only in such a package, and `quince` `build` builds every program in the project.
- r[mod.pkg.program-path] A package is a program exactly when its folder is `cmd/<name>/`. The path decides; no manifest field makes or unmakes a program. This holds for a project with only one program too, so adding a second program never moves code.
- r[mod.pkg.main-outside] A `main` outside `cmd/<name>/` is a compile error with its own error code. r[mod.pkg.main-fix] `quince` `fix` moves it to `cmd/<package>/`, using the package name from `quince``.toml`.
- r[mod.pkg.binary-name] A program's binary is named `<name>`, after its folder. Build flags may choose where the binary is written, never what it is called, so every build of a commit produces the same names.
- r[mod.intra-package] Files in the same package may refer to each other freely. r[mod.no-cycles] Circular dependencies between packages are a compile error.
- r[mod.private-default] Everything is private unless marked `pub`.
- r[diag.query.signature-location] Since an import doesn't name a file, `quince` `query signature <name>` and the language server return the file and line where any name is defined.

### Packages

r[mod.pkg.project] A project is a folder with a `quince``.toml` manifest at its root, naming the project, its edition, its dependencies, the capability limits of its programs ([Program execution](#capabilities-at-runtime)), and the native libraries it links ([Interop with C](#linking)). Its format is under [The manifest](#the-manifest). r[mod.pkg.one-manifest] One manifest and one lockfile cover every package in the project, so all of them build with the same edition and the same dependency versions. r[dep.lock.contents] The lockfile records the exact version and content hash of every dependency, direct or indirect.

- **Library.** r[mod.pkg.library-files] Every source file in the project except those under `cmd/`, `tests/` and folders whose names start with a dot, and except any `testdata/` folder, whose files are never compiled, so a tool can keep `.qnc` fixtures there. r[mod.pkg.library-name] Its import name is the `name` in `quince``.toml`. r[mod.pkg.program-import] A program imports it by that name, `import <name>.{...}`, and a program folder may share the library's name, since folders are not namespaces. r[mod.pkg.one-library] A project has at most one library, and it has none when every file is in a program.

* **Programs.** Each `cmd/<name>/` ([Packages](#packages)). r[mod.pkg.program-visibility] A program sees only the library's `pub` names, exactly as a dependent does. r[mod.pkg.program-no-import] Nothing imports a program, and programs never import each other, so code that two programs share goes in the library.

- **Test packages.** r[mod.pkg.test-packages] Each file directly in `tests/`, and each folder `tests/<name>/`, is one test package ([Testing](#integration-tests)). A folder lets several test files share helpers. r[mod.pkg.test-visibility] A test package sees only the library's `pub` names.

* r[mod.pkg.dependents] **Dependents** import only the library. Programs and test packages are never part of a project's public API, and `quince` `publish` compares only the library's.

- r[mod.pkg.test-deps] Dependencies used only by tests (test packages, `test` blocks and `@test` declarations) are listed in their own section of the manifest. They never become dependencies of anyone who depends on the project.

* r[mod.pkg.nested-manifest] A `quince``.toml` anywhere below the project root is a compile error. r[mod.pkg.multi-project] A repository with several libraries holds several projects, each in its own folder with its own tags, and a dependent names one with `dir` ([The manifest](#the-manifest)). r[mod.pkg.workspace-room] The manifest format must leave room for a later workspace that groups them.

### The manifest

r[dep.manifest.strict] `quince.toml` is TOML with the tables below and nothing else. r[dep.manifest.unknown] An unknown table or key is a compile error that names its line, so a misspelled key never silently does nothing. r[dep.manifest.fmt] `quince fmt` also formats `quince.toml`: tables in the order of the example below, several `[cmd.*]` tables and the keys within a table as written.

```
[project]
name = "users"              # required: the library's import name, lowercase snake_case
edition = "2027"            # required: the only edition in v1
allow_native = ["sqlite"]   # optional: native dependencies this project trusts (Interop with C)

[dependencies]
pg = { repo = "github.com/acme/quince-pg", version = "1.4" }
http = { repo = "github.com/acme/quince-http", version = "2.1" }
shared = { path = "../shared" }
sqlite = { repo = "github.com/acme/quince-sqlite", version = "0.5" }

[test-dependencies]
fake_smtp = { repo = "github.com/acme/fake-smtp", version = "0.3" }

[native]                    # optional: C libraries (Interop with C, Linking)
link = ["minz"]

[cmd.users]                 # optional: capability limits for cmd/users/ (Program execution)
net = ["db.internal:5432", "*.example.com"]
fs = ["./data"]
```

- r[dep.manifest.project] `[project]` is required, and so are its name and edition. r[dep.manifest.no-version] The manifest holds no version of its own: a project's versions are its git tags. Every other table is optional.
- r[dep.no-registry] There is no central registry. A dependency comes from a git repository or a folder. r[dep.repo] `repo` is the repository's path without a scheme, fetched over HTTPS, and `version` is the minimum version needed: `"MAJOR.MINOR"` or `"MAJOR.MINOR.PATCH"`, where a missing part is 0. r[dep.min-select] A minimum selects the lowest tag at or above it in its major, and `quince update` and `quince add` always write the full `MAJOR.MINOR.PATCH`. r[dep.tag] A version is the repository's git tag `vMAJOR.MINOR.PATCH`, and the lockfile pins each one by content hash ([Content addressing](#content-addressing)). r[dep.dir] A project in a subfolder of its repository adds `dir`: `textutil = { repo = "github.com/acme/tools", dir = "textutil", version = "0.3" }`. r[dep.dir-tags] Its versions are the tags `textutil/vMAJOR.MINOR.PATCH`, as in Go, while tags without a prefix belong to the project at the repository's root, so every project in a repository has its own versions.
- r[dep.key] The key is the name that code imports, and it must be the dependency's own name, the `name` in its `[project]` table, so a package is called the same in every project that uses it. r[dep.rename] Any other key is a compile error, except where two dependencies would share a name: then one must be renamed, and only to the form the compiler suggests and `quince fix` writes, `<name>_v<N>` for another major version of the same repository (`pg_v2`) and `<owner>_<name>` for packages from different repositories that share a name, all of them renamed so that no name depends on which was added first (`acme_json` and `zeta_json` for github.com/acme/json and github.com/zeta/json; the owner is lowercased, with `-` and `.` turned into `_`). r[dep.key-major] A key's major version is the major of its `version`. r[dep.identity] Within a build, a package is identified by its repository, folder and major version, so every manifest that lists it shares one package.
- r[dep.path] `path` names another project on this machine, relative to this manifest. It is built from the folder as it stands and isn't hashed into the lockfile. r[dep.path-override] An entry may hold `path` beside `repo`, `dir` and `version`, as in Cargo: builds of this project use the folder, while dependents, the lockfile and `publish` see only the repository. r[dep.path-publish] A project with an entry that has only `path` can't be published. r[dep.lock.file] The lockfile is `quince.lock` beside `quince.toml`. r[dep.lock.commit] The toolchain writes it, and every project commits it, libraries included. r[dep.lock.format] It is TOML with one `[[package]]` table per dependency in the build, holding `repo`, `dir`, `major`, `tag` and one `hash` over the package's definition hashes (Content addressing). r[dep.lock.path] A dependency with only `path` is listed with `path` alone.
- r[dep.test-only] Names under `[test-dependencies]` may be imported by test packages, and by a library or program file only when every use is inside a `test` block or an `@test` declaration. Any other use is a compile error. r[dep.test-unused] Such an import is never reported as unused in a non-test build.
- r[dep.manifest.cmd-folder] Each `[cmd.<name>]` table must name an existing `cmd/<name>/` folder, and a program without one has no limits. r[dep.manifest.cmd-net] `net` lists the hosts it may connect to, as `host`, `host:port` or `*.domain`. r[dep.manifest.cmd-fs] `fs` lists the paths it may read or write, relative to the working directory at start, and each entry covers everything below it. r[dep.manifest.path-normalize] Entries, and every path the program opens, are made absolute against that directory, normalized (`.` and `..` removed) and resolved through symlinks before they are compared, so an absolute path inside an allowed folder is allowed and a symlink can't lead out of one. r[dep.manifest.limit-default] A missing key means no limit, and `[]` allows nothing.
- r[dep.manifest.future-room] The format leaves room for a later `[workspace]` table, a caching proxy or checksum database for repositories, and per-dependency limits ([Program execution](#capabilities-at-runtime)).

### Version resolution

r[dep.mvs.min] Quince uses minimal version selection, as Go does, so the same manifest always builds the same dependency graph without a solver.

- A manifest states the minimum version it needs of each dependency (`version = "1.4"`).
- r[dep.mvs.one-per-major] A build uses exactly one version per major version of each package: the highest minimum that anything in the graph asks for. A newer release is never picked up unless some manifest asks for it.
- r[dep.mvs.majors-distinct] Different major versions are different packages. `http` and `http_v2` (major 2 of the same repository) can both be in one build, and `http.Response` and `http_v2.Response` are distinct types; a mismatch error names each with its repository and major version.
- r[dep.update.raise] `quince` `update` raises the version of each direct dependency within its major, rewriting that dependency's `version` in `quince.toml` and the lockfile, and reports what changed in each dependency's public API. r[dep.update.indirect] Indirect versions follow from the manifests that ask for them, so `update` and `add` change no other file, and both list every version that moved, including indirect ones. r[dep.update.skip-breaking] A tag that the API check refuses (Enforced semantic versioning) is skipped: they take the newest tag in the major that passes, and name each skipped tag and the change that broke it. r[dep.update.major-explicit] Moving to a new major version is always an explicit edit to the manifest.

### Content addressing

- r[dep.hash.definition] Each published definition is identified by a hash of its normalized syntax tree plus the hashes of everything it depends on. r[dep.hash.recursive] Mutually recursive definitions are hashed together as one group.
- r[dep.hash.verify] Versions are chosen by name and version ([Version resolution](#version-resolution)), then verified against the lockfile's hashes, so a republished or tampered package can't change what an existing lockfile builds. Hashes also key the build cache ([Toolchain](#incremental-compilation)). r[dep.lock.root-only] Only the root project's lockfile is read; a dependency's own lockfile is ignored. r[dep.lock.build] `quince build` writes a missing lockfile and stops on one that no longer matches the manifests, naming `quince update`.
- r[dep.hash.text-source] Source stays in ordinary text files; hashes are computed by the toolchain and stored in the lockfile.

### Enforced semantic versioning

- r[dep.semver.compare] `quince` `publish` compares the new public API with the previous version: types, signatures, effects, and contracts.
- r[dep.semver.breaking] Removing or changing anything public, adding an effect, or strengthening a `requires` clause is breaking and needs a major version bump. Publishing refuses otherwise. r[dep.publish.version] Publishing picks the version from that comparison, the smallest bump it allows over the latest tag (major, minor or patch), or a larger one the author asks for, then creates and pushes that git tag. r[dep.publish.first] The first tag is v0.1.0 unless the author names another. r[dep.semver.bump] The bump follows from the comparison: a public API identical to the latest tag's allows a patch; any addition, such as a new public name, method, impl, optional field, parameter with a default, or variant of an `@extensible` enum, needs a minor; and a breaking change needs a major. r[dep.semver.pre-1] Below v1.0.0 the same rules hold, so a breaking change after v0.3.2 is published as v1.0.0. r[dep.publish.needs-review] A [needs-review finding](#enforced-semantic-versioning) is reported and doesn't stop the publish. r[dep.publish.prefix] In a subfolder project, `publish` creates the prefixed tag (The manifest). r[dep.publish.git] `publish` tags the current commit, refuses uncommitted changes in the project's folder, and pushes the tag to the git remote `origin`. r[dep.publish.refuse] It refuses a project with no library, since nothing can depend on it (its programs are released with ordinary git tags), and a library that contains `dbg` or `todo`. r[dep.semver.dep-check] Adding or updating a dependency repeats the same comparison for every tag it picks, against the newest earlier tag in that major version that passed this check, and refuses a tag that breaks it, naming the change. So a tag pushed by hand can't bring a breaking change into a minor version.
- r[dep.semver.effvar] Removing an effect variable, or adding a named effect beside one (`uses E` to `uses E, Log`), is breaking. r[dep.semver.effvar-add] Adding an effect variable to a closure parameter that was pure is not, since every existing caller passes a pure closure and still type-checks.
- r[dep.semver.wire] For a public type that implements `Encode` or `Decode`, any change to its wire shape is breaking: renaming a field or variant without `@wire`, changing a `@wire` name, case or tag, adding a required field, or removing a field. Adding an `Option` field is not.
- r[dep.semver.receiver] Changing a public method's receiver between `self` and `var self` is breaking in either direction.
- r[dep.semver.variant] Adding an enum variant is breaking unless the enum is marked `@extensible`, which in turn forces callers to include a wildcard arm.
- r[dep.semver.contract] Any change to a contract's normalized syntax tree counts as breaking (reformatting doesn't), because whether a new `requires` is stronger than the old one can't be decided in general. r[dep.semver.iface-method] Adding a method to a public interface is breaking too. `quince` `publish` can't see a behavior change behind an unchanged signature; doc examples and integration tests are the guard there.
- r[dep.semver.derived] `quince` `publish` also compares the properties the compiler derives for each public type: automatic `Eq`, `Hash`, `Encode` and `Decode`, whether it can cross a task boundary, and whether it can be held in a `const`. Losing any of them is breaking, for example by adding a field whose type has no `Hash`. r[dep.semver.pub-field] Adding a field to a struct whose fields are all `pub` is breaking, because it breaks struct literals outside the package. r[dep.semver.impl-review] Adding an impl to a public type is reported as needs-review, because it can create a method-name clash ([Items](#interfaces)) in a dependent. So is adding a `pub` method, for the same reason.

### Editions

- r[mod.edition.per-package] Language changes ship in editions (e.g., `edition = "2027"`), chosen per package.
- r[mod.edition.interop] Packages on different editions link together freely.
- r[mod.edition.migrate] `quince` `migrate` rewrites code to a new edition mechanically; any change it can't make automatically is a compile error with a suggested fix.

## 8. Toolchain

r[tool.cli.single-binary] The toolchain is a single binary, `quince`, whose every command can emit structured JSON. It is designed to be driven by an agent as much as by a person.

### Commands

| Command | Does |
| --- | --- |
| `quince` `check` | r[tool.cli.check] Type-checks, lints, and verifies formatting; no code generation; warnings don't fail it unless --deny warnings is passed, which CI does by default |
| `quince` `build` | r[tool.cli.build] Compiles a binary or library |
| `quince` `run` | r[tool.cli.run] Builds and runs `main` |
| `quince` `test` | r[tool.cli.test] Runs unit tests, integration tests, doc examples, property tests, and saved regressions |
| `quince` `fmt` | r[tool.cli.fmt] Applies the canonical format |
| `quince` `fix` | r[tool.cli.fix] Applies every machine-applicable suggested fix |
| `quince` `query` | r[tool.cli.query] Answers questions about the codebase ([Query API](#query-api)) |
| `quince` `verify` | r[tool.cli.verify] Attempts static proof of `@verify` contracts |
| `quince` `publish` | r[tool.cli.publish] Checks semver against the previous version, then creates and pushes the version's git tag |
| `quince` `migrate` | r[tool.cli.migrate] Moves a package to a newer edition |
| `quince` `bind` | r[tool.cli.bind] Generates raw foreign declarations from a C header ([Interop with C](#generated-bindings)) |
| `quince` `update` | r[tool.cli.update] Raises dependency minimums and reports API changes ([Dependencies and publishing](#version-resolution)) |
| `quince` `explain` | r[tool.cli.explain] Prints the full explanation of an error code |
| `quince` `add` | r[tool.cli.add] Adds a dependency at its newest version and lists every version it raised ([Dependencies and publishing](#version-resolution)) |

r[tool.perf.targets] Performance targets: `quince` `check` on a single changed file in a 500,000-line project should finish in under 1 second, and an incremental build in under 5 seconds.

### Structured diagnostics

r[diag.json] With `--format json`, every diagnostic has a stable code, an exact span, a plain-language message, and zero or more fixes expressed as text edits:

```
{
  "code": "E0412",
  "severity": "error",
  "message": "match on `FetchError` is missing variant `Timeout`",
  "span": { "file": "src/fetch.qnc", "start": [42, 3], "end": [47, 4] },
  "fixes": [
    {
      "description": "add an arm for `Timeout`",
      "applicability": "needs-review",
      "edits": [{ "start": [46, 5], "end": [46, 5], "text": "FetchError.Timeout(after:) => todo(\"handle timeout\"),\n" }]
    }
  ],
  "docs": "quince explain E0412"
}
```

- r[diag.applicability] `applicability` is `safe` (apply automatically), `needs-review`, or `placeholder` (inserts a `todo`).
- r[diag.edits] An edit replaces the text from `start` up to `end` with `text`: when `start` equals `end` it inserts, and an empty `text` deletes.
- r[diag.edit-file] An edit may name a `file`, a path relative to the project root; without one it applies to the diagnostic's span file. An edit at `[1, 1]` to a file that doesn't exist creates it.
- r[diag.code-stable] Error codes are stable across editions; r[diag.explain] `quince` `explain <code>` prints a full explanation with examples.
- r[diag.all-errors] The compiler reports all independent errors in one run, ordered so fixing the first rarely invalidates the rest.

### Query API

`quince` `query` exposes what the compiler knows, so an agent can look things up instead of guessing:

| Query | Returns |
| --- | --- |
| `members <Type>` | r[diag.query.members] Fields, methods, and implemented interfaces, with signatures |
| `signature <path>` | r[diag.query.signature] Full signature, effects, contracts, and doc comment |
| `callers <path>` / `callees <path>` | r[diag.query.callers] The call graph around a function |
| `impls <Interface>` | r[diag.query.impls] Every type implementing an interface |
| `type-at <file:line:col>` | r[diag.query.type-at] The inferred type of an expression |
| `effects <path>` | r[diag.query.effects] Each of a function's effects, with the call chain that brings it in and every package effect expanded to the built-in effects beneath it |
| `search <type pattern>` | r[diag.query.search] Functions matching a type shape, e.g., `String -> Result<Int, _>` |
| `dependents <path>` | r[diag.query.dependents] Everything that would break if a definition changed |
| `wire <Type>` | r[diag.query.wire] The JSON shape a type encodes to and decodes from, with wire names and which fields are optional |
| source \<file> \[--without-tests\] | r[diag.query.source] A file's source, optionally with test blocks folded away ([Testing](#unit-tests)) |
| `handlers <fn>` | r[diag.query.handlers] Which handler each of a function's effects resolves to, from each program's main ([Effects](#handlers-in-tests-and-production)) |

r[diag.query.lsp] The same queries are served over a long-running language server for editors and agents that keep a session open.

## 9. Concurrency

r[conc.structured] Quince uses structured concurrency on green threads: every task belongs to a scope, and a scope doesn't exit until all its tasks have finished or been cancelled. r[conc.no-async] There is no `async` or `await`; blocking calls look like ordinary calls, and the effect system already shows which ones do I/O.

### Concurrent blocks

```
fn load_dashboard(user: UserId) -> Result<Dashboard, LoadError>
  uses Net
{
  let (profile, orders) = concurrent {
    spawn fetch_profile(user),
    spawn fetch_orders(user),
  }?
  Ok(Dashboard(profile:, orders:))
}
```

- r[conc.block] A `concurrent` block runs its `spawn`ed tasks in parallel and evaluates to `Result<(A, B, ...), E>`: `Ok` holds each task's value, in the order written. Write `concurrent { ... }?` to unwrap it like any other `Result`.
- r[conc.error-type] Every task that can fail returns `Result` with the same error type `E`. When the error types differ, convert them in the spawn expression, for example `spawn fetch_orders(user).map_err(LoadError.Orders)`; there is no automatic widening ([Errors and failure](#the--operator)). r[conc.plain] A task that returns a plain value can't fail, and a block whose tasks all return plain values evaluates to a plain tuple. r[conc.mixed] When some tasks return `Result` and others plain values, the block evaluates to `Result<(A, B, ...), E>`. `Ok` holds each `Result` task's value unwrapped and each plain value as it is, and only the `Result` tasks need the same `E`. r[conc.static-type] Whether a task can fail is decided by its static type at the spawn site: a value of a type parameter is a plain value even when a caller instantiates it with a `Result`, so a generic function behaves the same for every type. The same holds for failure policies.
- r[conc.err-cancel] If any task returns `Err`, the block cancels its siblings, waits for them to stop, and evaluates to that `Err`. r[conc.panic] If a task panics, the block cancels its siblings the same way and then panics the enclosing task. r[conc.cancelled-silent] Siblings the block cancelled report nothing (see [Cancellation](#cancellation)).
- r[conc.settle] `concurrent(settle: true) { ... }` runs every task to the end instead. It never cancels on an `Err`, and it evaluates to a tuple of `Result`s in the order written, so each task may have its own error type. r[conc.settle-panic] A panic still cancels the siblings and panics the enclosing task, because a panic is a bug.
- r[conc.no-outlive] A task can never outlive its block, so when a function returns, all work it started is finished.
- r[conc.not-effect] Starting tasks is not an effect. Because nothing escapes the scope, a function's signature already describes everything it can do.

### Fan-out over a collection

To run one task per item of a list, put `for` after `concurrent`:

```
fn fetch_all(urls: List<Url>) -> Result<List<Page>, FetchError>
  uses Net
{
  concurrent(limit: 10) for url in urls {
    spawn fetch(url)
  }
}
```

- r[conc.fanout.result] The block evaluates to `Result<List<T>, E>`, with the results in the same order as the input. It follows the same rules as a `concurrent` block: one shared error type, and the first `Err` cancels the remaining tasks. r[conc.fanout.plain] When every task returns a plain value, the block can't fail and evaluates to a plain `List<T>`, as a `concurrent { }` block gives a plain tuple.
- r[conc.fanout.limit] `limit` caps how many tasks run at once. r[conc.fanout.limit-required] It is required, so every fan-out states how many tasks run at once. r[conc.fanout.limit-type] It takes any `Int` expression, with `requires limit >= 1`.
- r[conc.fanout.settle] `settle: true` works here too. `concurrent(limit: 10, settle: true) for ...` never cancels on an `Err` and evaluates to `List<Result<T, E>>`.
- r[conc.fanout.one-spawn] The body holds exactly one `spawn`. It can bind values before it, like any block, but it can't spawn twice or return early.

### Long-lived work (fire and forget)

Work that must outlive the current function is spawned into a longer-lived scope passed in as a `TaskScope` value:

```
fn main() -> Result<Unit, AppError> uses * {
  with Log = log.json(log.Level.Info) {
    let served = background (bg: TaskScope, on_error: FailurePolicy.LogAndContinue) {
      http.serve(8080, handler: fn(req) { handle(req, bg:) })
    }
    served.map_err(AppError.Scope)?.map_err(AppError.Serve)
  }
}

fn handle(req: Request, bg: TaskScope) -> Response uses Net {
  let response = build_response(req)
  spawn in bg send_analytics(req.url, event: "page_view")
  response
}
```

- r[conc.background.taskscope-param] Only a function that takes a `TaskScope` parameter can start work that outlives it, so this is visible in the signature.
- r[conc.background.spawn-in] `spawn in bg expr` starts `expr` as a task in the scope `bg`. It follows the same rule as `spawn` inside `concurrent`: the whole expression is evaluated in the new task. r[conc.background.spawn-type] The form has type `Unit`, and `expr` must be `Unit` or `Result<Unit, E>` where `E` implements `Display`, so no task's result is silently thrown away. r[conc.background.scope-value] The scope can be any `TaskScope` value, such as `self.bg`. `spawn` is a keyword, so a `TaskScope` has no `spawn` method.
- r[conc.background.policy] The scope's owner sets the failure policy with `on_error`, which takes a `FailurePolicy`: `FailurePolicy.LogAndContinue`, `FailurePolicy.Restart(max: n)`, or `FailurePolicy.FailScope` (the default). r[conc.background.log-policy] `LogAndContinue` writes through the `Log` effect, so the enclosing function needs `uses Log`. r[conc.background.log-import] `Log` comes from std.log, and this is the one place the language names a std effect, so code that uses this policy imports it: `import std.log.{Log}`.
- r[conc.background.failure] A failure is a task that panics or returns `Err`. r[conc.background.policy-semantics] Under `LogAndContinue` the failure is logged and the scope carries on. Under `Restart(max: n)` the task is started again, up to n times. The count is per spawned task. Under `FailScope`, or once the restarts run out, the scope cancels its remaining tasks and fails.
- r[conc.background.result] A `background` block evaluates to `Result<T, ScopeFailed>`, where `T` is the body's value. r[conc.background.scope-failed] `ScopeFailed` names the spawn site of the task that failed and holds its panic message or the `Display` text of its error. Cancellation is never a failure (see [Cancellation](#cancellation)).
- r[conc.background.exit] When a `background` block exits, it cancels remaining tasks and waits for them, giving clean shutdown.
- r[conc.background.owned] A task with no owning scope is impossible by construction. r[conc.background.no-escape] A `background` block's value can't contain a `TaskScope` (a compile error that names the field path), and spawning into a scope whose block has already exited panics, naming the spawn site and the `background` line. r[conc.background.store] Storing a scope in a handler or a struct for the block's lifetime is fine.
- r[conc.background.handlers] A spawned task keeps the handlers that were in effect where it was spawned, even after it outlives that `with` block. If a test runs ` with Net = fake {  ``handle(req, bg:)` `}` and `handle` spawns analytics work into `bg`, that work uses the fake. A handler therefore lives as long as the longest task that captured it.

### Cancellation

- r[conc.cancel.cooperative] Cancellation is cooperative. r[conc.cancel.stop] A cancelled task stops at its next cancellation point (listed in [Cancellation](#cancellation)), and execution doesn't continue past it. r[conc.cancel.unwind] Instead the task unwinds to the scope that cancelled it, running `Resource` cleanup and closing any scopes it opened on the way ([Interop with C](#memory-across-the-boundary)). r[conc.cancel.not-error] Cancellation is never an error value: `?` never sees it, and no `Result` type has to carry it.
- r[conc.cancel.points] The cancellation points are every effectful call, including package effects, since they are built on the others; every operation that waits (`receive`, `send` on a full channel, and `select`); and the checks the compiler inserts at the top of every loop iteration and at every function entry. r[conc.cancel.state] A `State` operation that doesn't wait is never a cancellation point (this means a reference-type method; a package effect's operation is always a cancellation point, even when the effect is built only on State), so an update to a `Ref` or `Atomic` is never stopped halfway. Waiting changes nothing, so stopping a task while it waits is always safe.
- r[conc.cancel.no-catch] Code can't catch or suppress cancellation. r[conc.cancel.scope-sees] Only the scope that cancelled a task sees it: a `concurrent` block discards the siblings it cancelled, a `background` block treats cancelled tasks as shut down, and `with_timeout` reports it as a value ([Cancellation](#cancellation)). r[conc.cancel.not-failure] Cancellation is not a failure, so an `on_error` policy never sees it. This keeps cancellation out of every error type, and there is no catch-all for an agent to write that would swallow it by accident.
- r[conc.cancel.pure] Pure code needs no explicit check: the compiler-inserted checks stop a long pure computation too, so every task can be cancelled. A pure function stays pure ([Effects](#5-effects-and-capabilities)), because a cancelled computation's result is never observed. r[conc.cancel.preempt] The same checks let the scheduler preempt a long-running task, so it can't starve the others.
- r[conc.timeout.scope] `time.with_timeout(dur``"5s", fn() { ... })` is a scope that cancels its contents when the time runs out. r[conc.timeout.result] It evaluates to `Result<T, TimedOut>`, where `T` is the body's value, so the caller must handle the timeout. r[conc.timeout.own-timer] It reports only its own timer: if an enclosing scope cancels the task while the body runs, the cancellation keeps unwinding past `with_timeout` and never becomes `Err(TimedOut)`. r[conc.timeout.clock] It needs `uses Clock`, because it reads the time, so under `quince` `test` the timer runs on the fake clock and fires at the same point on every run.
- r[conc.cancel.cleanup-shield] Cleanup that runs while a task unwinds is shielded from cancellation, so a graceful close (flushing a buffer, sending a goodbye) finishes. r[conc.cancel.cleanup-limit] It has a time limit, 10 seconds unless the resource declares another ([Interop with C](#memory-across-the-boundary)). Past the limit the cleanup itself is cancelled, and the resource's C-level release still runs.
- r[conc.cancel.cleanup-panic] A panic in cleanup doesn't stop the unwinding. The remaining cleanups still run, and the task ends as panicked, whether it was unwinding from a cancellation or from an earlier panic. The owning scope handles it like any other panic, and the report lists every panic that occurred.

### Shared state

- r[conc.share.values] Values are immutable by default and can be shared freely between tasks.
- A `Ref<T>` cannot cross a task boundary, and neither can any value that contains one ([Reference types](#mutation)). r[conc.share.spawn-reject] The compiler rejects capturing such a value in a `spawn`, and the error names the field that holds the reference.
- r[conc.share.channels] Tasks communicate through `Channel<T>` or explicitly concurrent types such as `Atomic<T>` and `ConcurrentMap<K, V>`. r[conc.share.state] Every operation on them requires `uses State`. r[conc.share.no-races] Data races are impossible in safe code.
- r[conc.chan.capacity] A `Channel<T>` has a fixed capacity, set when it is created. r[conc.chan.send] `ch.send(v)` waits while the channel is full and returns `Err(ChannelClosed)` once it has been closed. r[conc.chan.receive] `ch.receive()` waits for a value and returns `Option<T>`, which is `None` once the channel is closed and empty. r[conc.chan.close] `ch.close()` closes it, and values already sent can still be received.
- r[conc.share.modify-pure] The closures passed to `Ref.modify`, `Atomic.modify` and `ConcurrentMap.update` must be pure. r[conc.share.modify-atomic] The update is applied only after the closure returns, so a cancellation inside the closure leaves the reference unchanged, and an update is never applied halfway.

### Waiting on several channels

r[conc.select.wait] `select` waits on several channel operations at once and runs the arm of the first one that is ready:

```
fn worker(jobs: Channel<Job>, control: Channel<Command>) uses State, Net {
  while true {
    select {
      job = jobs.receive() => match job {
        Some(j) => process(j),
        None => return,
      },
      cmd = control.receive() => match cmd {
        Some(Command.Stop) => return,
        Some(_) => (),
        None => {
          for j in jobs.iter() { process(j) }
          return
        },
      },
    }
  }
}
```

- r[conc.select.arm] Each arm is `pattern = operation => expression`, where the operation is a `receive()` or a `send(v)` on a channel. Nothing else can be an arm. r[conc.select.irrefutable] An arm's pattern must be irrefutable, usually a plain name. A refutable one is a compile error, and its fix binds a name and matches in the body. r[conc.select.closed] A `receive()` on a closed, empty channel is ready at once and yields `None`, so a loop that keeps selecting on it spins. On `None`, leave the loop and drain the other channel with `.iter()`. r[conc.select.body] An arm's body is an ordinary block of the enclosing function, not a closure, so it may assign the function's `var`s and `return` from the function.
- r[conc.select.value] The `select` evaluates to the value of the arm that ran, so every arm has the same type.
- r[conc.select.random] When several arms are ready at once, one is picked at random. r[conc.select.seeded] Under `quince` `test` the pick is part of the seeded schedule, so a failure replays exactly.
- r[conc.select.no-default] There is no timeout arm and no default arm. For a deadline, wrap the `select` in `with_timeout`; for shutdown, cancel the task. A `select` is a cancellation point like any other wait.

### Deterministic testing

r[test.dtest.seeded] Under `quince` `test`, the scheduler runs tasks in a seeded, reproducible order and explores several orderings per test. r[test.dtest.replay] A failure prints its seed, and `quince` `test --seed <n>` replays it exactly. r[test.dtest.real-one] A test that opts into a real effect runs one ordering, so its real I/O happens once. How many orderings the other tests explore is set by the toolchain.

### Calling C

Green threads complicate foreign calls ([Interop with C](#11-interop)), so the runtime handles them specially:

- r[ffi.thread-pool] The runtime runs foreign calls on a separate thread pool, so a blocking C call can't stall other tasks.
- r[ffi.pinned] `@pinned` marks a foreign library that must always be called from the same OS thread (e.g., GUI toolkits).
- r[ffi.cancel-after] A task blocked in a foreign call is cancelled only after the call returns.
- r[ffi.callback-cancel] Cancellation never interrupts a callback that C calls into Quince ([Interop with C](#callbacks)). It is delivered once the foreign call returns. r[ffi.callback-panic] A panic that reaches a callback's boundary aborts the process, with a message naming the callback, because unwinding through C frames is undefined behavior.

## 10. Compilation

r[tool.build.native] Quince compiles ahead of time to native code through LLVM, producing a single self-contained binary that includes the runtime (scheduler and cycle collector). r[tool.build.static] On Linux the binary is fully static, unless a native library is available only as a shared library, which quince build reports; on macOS and Windows it links only the operating system's own libraries. r[tool.build.no-jit] There is no interpreter or JIT.

### Backend decision

r[tool.build.llvm-only] LLVM is the only backend, for debug and release builds alike. A second, faster debug backend (as Zig and Rust have pursued) is not planned: it is years of work for modest gains, since type checking, monomorphization, and linking take much of the build time too. Fast feedback comes instead from `quince` `check` (no code generation), per-definition caching, and debug builds that skip optimization. Quince IR is kept backend-independent so a Cranelift debug backend can be added later if real-world build times require it.

### Pipeline

1. **Parse** source into a syntax tree.
2. **Check** types, effects, exhaustiveness, and formatting. `quince` `check` stops here.
3. **Lower** to Quince IR, a mid-level representation where generics are monomorphized, contracts become explicit checks, reference-count operations are inserted and optimized, and `concurrent` blocks become runtime calls.
4. **Generate** LLVM IR, with reference counting already explicit, so no GC stack maps or LLVM statepoints are needed.
5. **Optimize and emit** native object code, then link with the runtime.

### Memory management

r[mem.rc] Quince manages memory with reference counting that the compiler inserts in Quince IR, as Koka (Perceus) and Lean 4 do. There is no tracing collector for ordinary values.

- **Counts the compiler removes.** r[mem.borrow] A parameter the callee doesn't store is borrowed and touches no count, and a value is moved, not counted, at its last use. Most calls touch no counts at all. r[mem.c-scope] A value that owns C memory is never released before the end of its scope, because C may still hold its pointer ([Interop with C](#memory-across-the-boundary)).
- **Freeing and reuse.** r[mem.free] A value is freed as soon as its last reference goes away, at a point the code determines. r[mem.reuse] When a unique value is consumed by an update that builds a value of the same shape, its memory is reused in place.
- **Across tasks.** r[mem.atomic-shared] Counts are non-atomic by default. A value becomes shared when it crosses a task boundary (captured by `spawn`, sent on a channel, or stored in an `Atomic<T>` or `ConcurrentMap<K, V>`), and only shared values use atomic count operations. r[mem.mark-shared] The first crossing marks everything reachable from the value as shared, which takes time proportional to its size, so sending a large map on a channel is not free. r[mem.const-shared] Constants are marked shared when they are initialized.
- **Cycles.** r[mem.no-value-cycles] Plain values can't form cycles, because a value can't be changed through an alias. Only reference types (`Ref<T>`, `Atomic<T>`, `ConcurrentMap<K, V>`, Channel\<T>, TaskScope, Pool\<T, Err, uses E> and Callback\<F>) can, by holding a value that refers back to them. r[mem.cycle-collector] The runtime runs an incremental cycle collector over reference-type objects only. r[mem.cycle-per-task] It collects each task's own Refs inside that task, and the types that cross tasks in a background task, so it never stops all tasks at once.
- r[mem.copy-report] **Hidden copies are reported.** `quince` `check` and the query API ([Query API](#query-api)) list every update that may copy instead of updating in place, and name the alias that causes it.
- **Large frees.** Freeing a large structure takes time proportional to its size. r[mem.large-free] Above a size threshold, the runtime hands the rest of the free to a background task, so a request handler isn't stalled. r[mem.resource-not-deferred] Resource cleanup is never deferred this way: any Resource inside the structure is cleaned up at the drop point, before the rest is handed off.

### Build profiles

| Profile | LLVM optimization | Overflow checks | Contracts | Debug info |
| --- | --- | --- | --- | --- |
| r[tool.profile.debug] `debug` (default) | O0 | On | On | Full |
| r[tool.profile.release] `release` | O2, ThinLTO | On | On, except `@unchecked` | Line tables |

r[tool.profile.overflow] Overflow checks stay on in release builds because overflow is defined as a panic ([Errors and failure](#panics-bugs)), not an optimization opportunity.

### Targets

| Tier | Targets | Guarantee |
| --- | --- | --- |
| r[tool.target.tier1] 1 | x86-64 and ARM64 on Linux, macOS, Windows | Built and fully tested on every release |
| r[tool.target.tier2] 2 | WebAssembly (`wasm32`) | Builds; concurrency limited until WebAssembly stack switching is widely available |

r[tool.target.cross] Cross-compilation is built in: `quince` `build --target linux-arm64` works from any tier 1 host with no separate toolchain install, for packages with no foreign dependencies. r[tool.target.cross-foreign] Cross-compiling foreign code needs the target's C libraries, and macOS targets need Apple's SDK.

### Incremental compilation

r[tool.incr.cache] Compiled output is cached per definition, keyed by a hash of the definition's own body plus the signature hashes of everything it calls. r[tool.incr.body] Changing a function's body recompiles only that function and what it was inlined into; changing a signature also recompiles its callers. The full content hash from [Content addressing](#content-addressing), which covers every dependency, identifies definitions in the lockfile.

### Debugging

r[tool.debug.dwarf] Binaries emit DWARF (or PDB on Windows), so standard debuggers work. r[tool.debug.locations] Panic traces and the deterministic test scheduler ([Testing](#deterministic-testing)) report Quince source locations, never generated code.

## 11. Interop

r[ffi.c-abi] Quince interoperates with other languages only through the C ABI. Any library that exposes a C interface (including ones written in Rust, Zig, Swift, or C++) can be called; direct interop with Rust generics or JavaScript is out of scope.

### Generated bindings

r[ffi.cbind.generate] `quince` `bind <header.h>` reads a C header and generates raw declarations. Bindings are generated rather than hand-written, because a mistyped foreign declaration corrupts memory silently.

```
foreign "sqlite3" uses Fs {
  @cleanup(sqlite3_close, limit: dur"5s")
  type sqlite3

  fn sqlite3_open(path: CString, db: Ptr<Ptr<sqlite3>>) -> CInt
  fn sqlite3_close(db: Ptr<sqlite3>) -> CInt
  fn sqlite3_changes(db: Ptr<sqlite3>) -> CInt
  @borrowed
  fn sqlite3_errmsg(db: Ptr<sqlite3>) -> CString
}

pub struct Db { handle: Resource<sqlite3> }

pub fn open(path: String) -> Result<Db, SqliteError> uses Fs {
  let out = CBuf.new<Ptr<sqlite3>>()
  let c_path = CString.try_from(path).ok_or(SqliteError.NulInPath)?
  let code = sqlite3_open(c_path, db: out.ptr())
  if code != 0 { return Err(SqliteError.Open(code:)) }
  Ok(Db(handle: Resource.adopt(out.get())))
}

impl Db {
  pub fn close(self) -> Result<Unit, SqliteError> uses Fs {
    let code = self.handle.close()
    if code != 0 { return Err(SqliteError.Close(code:)) }
    Ok(())
  }
}
```

- r[ffi.block] A `foreign` block names the library and the effects its code performs. r[ffi.effects] Callers of anything built on it see those effects like any others. r[ffi.pure-block] A block without uses declares pure functions. r[ffi.cbind.all-effects] Like every foreign effect, that is a promise the compiler can't check, so quince bind writes every built-in effect on the block and the author removes the ones the library doesn't use.
- r[ffi.multi-block] A package may have several `foreign` blocks for one library, each with its own effects, for example a pure block for one-shot functions and a `uses State` block for a stream. r[ffi.namespace] They share one namespace, so a type declared in one is visible in the others. r[ffi.attr-consistent] A library attribute such as `@pinned` must appear on every block for that library or on none.
- r[ffi.not-pub] Raw declarations use C types (`CInt`, `CString`, `Ptr<T>`) and cannot be `pub`. The package that declares them must wrap them in a safe API using `Result` and ordinary Quince types. r[ffi.resource-not-pub] A Resource\<T> whose T is a foreign type can't appear in a pub signature either; hold it in a field of a struct, as Db does.
- r[ffi.type] `type name` inside a `foreign` block declares an opaque C type. r[ffi.cleanup] `@cleanup(fn, limit: dur)` above it names the C function that releases it and the cleanup time limit, 10 seconds when left out. r[ffi.cleanup-signature] The cleanup function must be declared in a foreign block of the same package and take exactly one parameter, Ptr\<T>. r[ffi.cleanup-return] It may return any C type, and res.close() returns that value; a wrong signature is a compile error on the @cleanup line.
- r[ffi.cresource.adopt] `Resource.adopt(p)` turns a `Ptr<T>` into a `Resource<T>`, and works only for a type with `@cleanup`. r[ffi.cresource.ptr] `res.ptr()` lends the raw pointer for one call. r[ffi.cresource.ptr-arg-only] Like buf.ptr() and bytes.lend(), it may only be written as an argument of a foreign call; binding it to a name is a compile error. r[ffi.cresource.close] res.close() runs the cleanup now and returns the cleanup function's result, res.is\_closed() tells whether cleanup has run, and res.keep(x) moves a Callback, CBuf or CString into the resource, which releases it right after the C cleanup runs, so the end of the local scope no longer releases it.
- r[ffi.cbuf.def] `CBuf<T>` is C-allocated memory with identity (a reference type, [Types](#mutation)), and is the only way to pass an out-parameter, so Quince memory never escapes to C. r[ffi.cbuf.api] CBuf.new\<T>() holds one value and CBuf.alloc\<T>(n) an array; buf.get(), buf.set(v) and buf.ptr() reach it, and CBuf.from\_bytes(b) and buf.to\_bytes(n) copy UInt8 buffers. r[ffi.cstring.convert] `CString.try_from(s)` converts text for C (`None` when it holds a NUL byte), and `cstr.to_string()` converts it back as a `Result`, since C strings may not be UTF-8.
- r[ffi.lend] `bytes.lend()` lends a `Bytes` value in place for one call, as a `ConstPtr<UInt8>`. r[ffi.lend-const] `quince` `bind` emits `ConstPtr<T>` for a `const T*` parameter, and only such a parameter accepts a lent pointer, so C can't write into Quince memory. r[ffi.no-string-lend] A `String` is never lent, since C strings need a trailing NUL byte; `CString.try_from` copies it.
- r[ffi.ownership] A foreign function that returns a `CString` must say who owns it. r[ffi.borrowed] `@borrowed` means C keeps it: Quince never frees it, and it must be converted with `to_string()` before the next call into that library. r[ffi.owned] `@owned(free_fn)` means Quince frees it with `free_fn` when it is dropped. r[ffi.ownership-required] Leaving both out is a compile error, and `quince` `bind` leaves them out because a header can't say. r[ffi.ownership-ptr] The same rule covers a returned `Ptr<T>` whose `T` has no `@cleanup`. r[ffi.cstring.owned] A `CString` made by `CString.try_from` is always owned by Quince.
- r[ffi.ptr-ops] The pointer operations are `Ptr.null()`, `p.is_null()`, and `p.read()` and `p.write(v)` for C types declared in the block.
- r[ffi.package-only] Everything in this list works only in the package that declares the `foreign` block. r[ffi.no-unsafe] There is no `unsafe` keyword: the native mark and [`allow_native`](#trust) are the audit surface, and `quince` `query` lists every raw operation in a package.

### C types

r[ffi.ctype.fixed-alias] A C type whose width is the same on every target is another name for a Quince number type, so literals, constants and conversions work as usual: `const MINZ_BUF_ERROR: CInt = -5`, `n.to_int32()`, `c.to_int64()`. r[ffi.ctype.target-width] A C type whose width depends on the target is a type of its own, so code that compiles on one target compiles on all of them. r[ffi.ctype.pub] The fixed-width names may appear in `pub` signatures, since each is only another name for a Quince type. Target-width C types, raw foreign declarations and `Resource<foreign T>` may not.

| C type | Quince type |
| --- | --- |
| signed char, unsigned char | r[ffi.ctype.char] CSChar = Int8, CUChar = UInt8 |
| short, unsigned short | r[ffi.ctype.short] CShort = Int16, CUShort = UInt16 |
| int, unsigned int | r[ffi.ctype.int] CInt = Int32, CUInt = UInt32 |
| long long, unsigned long long | r[ffi.ctype.longlong] CLongLong = Int64, CULongLong = UInt64 |
| float, double | r[ffi.ctype.float] CFloat = Float32, CDouble = Float |
| int8\_t to uint64\_t | r[ffi.ctype.stdint] Int8 to UInt64 |
| char, long, unsigned long, size\_t, ssize\_t, intptr\_t | r[ffi.ctype.own] Own types: CChar, CLong, CULong, CSize, CSSize, CIntPtr |

r[ffi.ctype.target-convert] Each target-width type is built with a checked conversion such as `CSize.try_from(n) -> Option<CSize>`, accepts integer literals that fit in 32 bits, and converts back with `to_int64()` (signed) or `to_uint64()` (unsigned), which never fails on any target.

r[ffi.cbind.params] `quince` `bind` writes pointer and callback parameters this way:

| C parameter | Quince parameter |
| --- | --- |
| `T*` | r[ffi.cbind.ptr] `Ptr<T>` |
| `const T*` | r[ffi.cbind.const-ptr] `ConstPtr<T>` |
| `T**` (an out-parameter) | r[ffi.cbind.out-param] `Ptr<Ptr<T>>`, passed as a `CBuf<Ptr<T>>`'s `ptr()` |
| `const char*` | r[ffi.cbind.cstring] `CString`, lent for the call |
| A function pointer followed by its `void*` context | r[ffi.cbind.callback] One `Callback<fn(...)>` parameter, its arguments in C types |

r[ffi.cbind.param-names] Parameter names come from the header, since every argument after the first is named at the call. r[ffi.cbind.keyword-param] A C parameter whose name is a keyword gets `_` appended (`fn` becomes `fn_`). r[ffi.cbind.callback-name] The one `Callback` parameter made from a function pointer and its context takes the function pointer's name. r[ffi.cbind.unnamed] A parameter the header leaves unnamed becomes `arg1`, `arg2`, and so on by position. r[ffi.cbind.returns] Return types follow the same table: a returned `T*` becomes `Ptr<T>`, and a returned `const char*` or `char*` becomes `CString`. r[ffi.cbind.ownership-left] Both need `@borrowed` or `@owned`, which `quince bind` leaves for the author (Generated bindings).

### Trust

r[ffi.trust.native] A package containing a `foreign` block is native, which the toolchain sees in its source. r[ffi.trust.allow] A project must allow each native dependency by name in its manifest:

```
[project]
allow_native = ["sqlite"]
```

r[ffi.trust.unlisted] Building with an unlisted native dependency is an error. The effect declared on a foreign block can't be verified, so this list is the audit surface: everything else is checked by the compiler.

### Linking

r[ffi.link.table] The `[native]` table in `quince``.toml` names every C library the project links and any C sources it builds:

```
[native]
link = ["minz"]            # found with pkg-config, then the system linker's search path
sources = ["native/*.c"]   # optional: C files built with the toolchain's C compiler
include = ["native"]       # header folders for sources and quince bind
```

- r[ffi.link.match] Every library named by a `foreign` block must match a `link` entry or the library built from `sources`, which takes the project's name. Otherwise it is a compile error that names the missing manifest line. r[ffi.link.own-only] `[native]` covers only the project's own foreign blocks: a native dependency's link entries and sources carry into the final binary without being listed again.
- r[ffi.trust.transitive] `allow_native` lists every native package in the build by its own name, whether it is a direct, indirect or test-only dependency; a project never lists itself.
- r[ffi.link.ship-sources] A published package ships its `sources` and builds them on the user's machine. r[ffi.link.missing] A package that only links needs the library installed, and `quince` `build` names the missing library and the pkg-config name it tried.

### Memory across the boundary

- r[ffi.cmem.no-escape] **Quince** **memory never escapes to C.** Data is copied into C-allocated buffers, or lent in place for the duration of one call (bytes.lend(), above) (Quince never moves memory, so this needs no pinning). C code can never hold a pointer to Quince memory after the call returns, because reference counts can't see it.
- **C-owned memory is a `Resource<T>`.** r[ffi.cmem.resource-once] A resource declares its cleanup function and is released exactly once: by res.close(), at the end of a with\_resource call, or when its last reference is dropped, whichever comes first. r[ffi.cmem.with-resource] with\_resource hands its body the resource itself and closes it when the body ends, even on error, panic, or cancellation:

```
let changes = ffi.with_resource(Resource.adopt(out.get()), fn(db) {
  sqlite3_changes(db.ptr())
})
```

- r[ffi.cmem.drop-close] Dropping the last reference to an open resource closes it then and there, a moment reference counting makes exact ([Memory](#memory-management)). So a wrapper such as `Db` is correct without any close call, and its `close` method exists for callers that want the C error code. r[ffi.cmem.pinned-cleanup] For a `@pinned` library the cleanup is queued onto that library's thread. r[ffi.cmem.cycle-warn] Only a Resource freed when the cycle collector breaks its cycle ([Memory](#memory-management)) logs a warning, since the moment of cleanup is then unpredictable, and `quince` `check` warns about any reference type that can hold a Resource in a cycle.
- r[ffi.cmem.use-after-close] Using a resource after it is closed is a bug: `res.ptr()` or `res.close()` on a closed resource panics, naming the foreign type and the line that closed it. r[ffi.cmem.escape-warn] `quince` `check` warns when a Resource, or a value holding one, is returned from or escapes a `with_resource` body.
- r[ffi.cmem.scope-release] A value that owns C memory (a `Resource`, `CBuf`, `Callback` or `CString`, or a value holding one) is never released before the end of the scope that holds it, even after its last use, because C may still hold its pointer. r[ffi.cmem.drop-order] On every exit (normal, `?`, panic, cancellation), locals are released in reverse order of creation, and the fields of a dropped struct in declaration order. r[ffi.cmem.keep] To tie a callback to the handle C stores it in, use `res.keep(cb)`. The same is how to hand C memory it keeps, such as a string it stores: a `CString` passed directly is lent for that call only, so keep it on the resource with `res.keep(cstr)`.
- r[ffi.cmem.cleanup-limit] Cleanup is shielded from cancellation and has a time limit, 10 seconds unless the resource's declaration sets another. Past the limit the cleanup is cancelled, and the C-level release still runs ([Cancellation](#cancellation)).

**Sharing resources between tasks.** r[ffi.cresource.crossing] A `Resource<T>` can't cross a task boundary unless its foreign type is marked `@shareable`, a promise that the C handle is safe to use from several threads. r[ffi.pool.def] For the usual case, a handle that isn't, the standard library's `Pool<T, Err, uses E>` holds up to `max` of them and hands each one to a single task at a time:

```
let pool = Pool.new(20, healthy: fn(conn) { conn.is_open() }, fn() { pg.connect(url) })
let rows = pool.checkout(fn(conn) { conn.query("SELECT 1", params: []) })??
```

- r[ffi.pool.checkout] `pool.checkout` waits for a free value (a cancellation point), lends it to the closure, and returns it afterwards. r[ffi.pool.closure-crossing] The closure is checked like a spawned task: what it captures and what it returns must be able to cross a task boundary. r[ffi.pool.lent] The lent value can't, so it can't be returned, stored or kept after the closure ends, and two tasks never hold it at once. r[ffi.pool.panic] If the closure panics or is cancelled, the value is closed and replaced rather than returned. r[ffi.pool.close-drop] Here and below, the pool closes a value by dropping it, so a Resource inside it closes then; v1 has no close hook. r[ffi.pool.err] An `Err` that the closure returns is an ordinary value, since the pool can't see inside `R`, so the value goes back to the pool. r[ffi.pool.healthy] On every return the pool first calls `healthy` on the value, and closes a value it rejects instead of keeping it, so a connection that died during the closure is never handed out again. r[ffi.pool.healthy-required] `healthy` is required, so every pool states its policy; `fn(_) { true }` keeps every value.
- r[ffi.pool.crossing] The pool itself can cross task boundaries, so a handler such as `PgDb` holds one and every task that inherits the handler shares it.

### Callbacks

r[ffi.callback.def] C code may call back into Quince functions passed to it as a Callback\<F>: a resource that keeps the closure alive until it is released, so C can store it beyond one call, as event loops and sqlite hooks do. r[ffi.callback.effects] `F` carries the callback's effects like any function type, as in `Callback<fn(CInt, CInt) uses State>`, and they must be among those declared on its `foreign` block. r[ffi.callback.no-poly] So a `pub` wrapper fixes a callback's effects, and can't take an effect-polymorphic callback. r[ffi.callback.handlers] A callback runs with the effect handlers that were in effect at the call that registered it, so a test that registers one under a fake gets the fake on every later call. r[ffi.callback.param] A foreign parameter typed `Callback<fn(...)>` receives a C function pointer plus its context, which `quince` `bind` sets up from the header. r[ffi.callback.release] `cb.unregister()` or the end of its scope releases it, unless `res.keep(cb)` has moved it into a resource, whose cleanup then releases it.

- r[ffi.callback.new] `Callback.new(f)` is for C that may call at any time, from any thread. r[ffi.callback.new-crossing] The closure's captures must be able to cross a task boundary, as for `spawn`, and each call runs as a short task in the scope that registered it. r[ffi.callback.new-threads] The runtime registers each calling thread on first use.
- r[ffi.callback.local] `Callback.local(f)` is for C that calls back only during a call the task itself is making, such as progress reports, sort comparators and row callbacks. r[ffi.callback.local-ref] The closure may capture a `Ref`, and it runs as part of the task that created it. r[ffi.callback.local-abort] A call from anywhere else aborts the process with a message naming the callback.

```
// C: int minz_stream_set_progress(minz_stream *stream, minz_progress_fn on_progress, void *user);
foreign "minz" uses State {
  @cleanup(minz_stream_free)
  type minz_stream

  fn minz_stream_set_progress(
    stream: Ptr<minz_stream>,
    on_progress: Callback<fn(CInt, CInt) uses State>,
  ) -> CInt
  fn minz_stream_free(stream: Ptr<minz_stream>)
}

let done = Ref.new(0)
let progress = Callback.local(fn(sent: CInt, total: CInt) { done.set(sent.to_int64()) })
check(minz_stream_set_progress(stream.ptr(), on_progress: progress))?
stream.keep(progress)
```

Runtime rules for foreign calls (separate thread pool, `@pinned`, cancellation) are in [Calling C](#calling-c).

### Testing foreign code

C code does real I/O whatever handler is installed, so a test fake can't stand in for it. r[ffi.ffitest.opt-in] A test that reaches a `foreign` block must opt in to each effect the block declares that tests fake by default: a test that calls into `foreign "sqlite3" uses Fs` needs `test "..." uses Fs`, and the compile error names the block and the opt-in to add. r[ffi.ffitest.pure] Pure blocks and `uses State` blocks need nothing. r[ffi.ffitest.step] The scheduler counts each foreign call as one step. A test that should not touch real I/O replaces the wrapper's own effect with a fake ([Effects](#handlers-in-tests-and-production)).

## 12. Open questions

Only the name remains open. Every design question raised in the v0.1 and v0.2 reviews has been decided in this draft, and the reviews record each decision.

- [x] **Name: Quince.** Decided 2026-11-10, before the first public release. "Lucid" was the working title, dropped because it collides with an active Princeton research language for network data planes (with a verification-focused successor, VeriLucid) and an old 1970s dataflow language. A web search found no language or developer tool named Quince. The command is quince, the manifest is quince.toml, and source files use .qnc.

## Appendix: Expression grammar

`{ x }` means zero or more, `[ x ]` optional. r[lex.grammar-rules] Three lexer rules apply: `>>` is never one token (the parser joins two adjacent `>` for a shift), `..=` is one token, and `|` inside a pattern is alternation, so `x is A | B` tests for either variant.

```ebnf
expr        = jump | logic | range | bits
jump        = "return" [ expr ] | "break" | "continue"
            | "spawn" [ "in" postfix ] expr   // a TaskScope value: bg, self.bg

logic       = compare { "&&" compare }
            | compare "||" compare { "||" compare }   // never mixed with &&
compare     = arith [ cmp_op arith | "is" pattern ]    // never chained
cmp_op      = "==" | "!=" | "<" | "<=" | ">" | ">="
range       = arith ( ".." | "..=" ) arith
bits        = prefix "&" prefix { "&" prefix }
            | prefix "|" prefix { "|" prefix }
            | prefix "^" prefix { "^" prefix }
            | prefix ( "<<" | ">>" ) prefix

arith       = term { ( "+" | "-" ) term }
term        = prefix { ( "*" | "/" | "%" ) prefix }
prefix      = ( "-" | "!" | "~" ) prefix | postfix
postfix     = primary { "." ( name | NAME ) [ type_args ] [ args ] | "." int | args | "[" expr "]" | "?" }
// type_path covers None, ChannelClosed, Int.MAX's Int and PgDb.new's PgDb; type_path args builds a newtype or a prelude variant: UserId(1), Some(x)
args        = "(" [ arg { "," arg } [ "," ] ] ")"
arg         = expr | name ":" [ expr ]   // name: alone passes the binding name

primary     = literal | path [ type_args ] | type_path [ type_args ] | NAME | "self" | "(" expr ")" | tuple
            | list_lit | map_lit | struct_lit | closure
            | if_expr | match_expr | concurrent | background | select | with_expr
tuple       = "(" ")" | "(" expr "," expr { "," expr } [ "," ] ")"
list_lit    = "[" [ expr { "," expr } [ "," ] ] "]"
map_lit     = "{" ":" "}" | "{" entry { "," entry } [ "," ] "}"
entry       = expr ":" expr
struct_lit  = type_path [ type_args ] "(" [ ".." expr "," ] field_init { "," field_init } [ "," ] ")"
field_init  = name ":" [ expr ]
closure     = "fn" "(" [ cparam { "," cparam } [ "," ] ] ")" [ "->" type ] [ "uses" effects ] block
cparam      = ( name | "_" ) [ ":" type ]
if_expr     = "if" expr block [ "else" ( if_expr | block ) ]
match_expr  = "match" expr "{" { arm "," } "}"
arm         = pattern [ "if" expr ] "=>" ( block | expr )

block       = "{" { stmt NEWLINE } [ stmt ] "}"   // a last stmt that is an expr is the value
stmt        = "let" pattern [ ":" type ] "=" expr
            | "var" name [ ":" type ] "=" expr
            | place assign_op expr
            | "for" pattern "in" expr block
            | "while" expr block
            | expr
place       = ( name | "self" ) { "." name | "[" expr "]" }   // self only under var self
assign_op   = "=" | "+=" | "-=" | "*=" | "/=" | "%="
```

### Declarations, types and patterns

r[lex.ident-classes] `name` is a `snake_case` identifier, `Name` a `PascalCase` one and `NAME` a `SCREAMING_SNAKE_CASE` one. r[lex.ident-not-keyword] None of them can be a keyword. r[lex.c-name] `c_name` is any C identifier, kept in its C spelling, and appears only for what a `foreign` block declares. `path` is `name { "." name }`, a package path such as `std.http`. `literal`, `string`, `int` and `doc_comment` are the tokens of [Lexical structure](#2-lexical-structure-and-syntax).

```ebnf
file         = { import NEWLINE } { decl NEWLINE }
import       = "import" path ( "." names | [ "as" name ] )   // as: packages only
names        = "{" name_ref { "," name_ref } [ "," ] "}"
name_ref     = "self" | name | Name | NAME
decl         = { doc_comment } { attribute NEWLINE } [ "pub" ] item
attribute    = "@" name [ "(" arg { "," arg } ")" ]
item         = fn_decl | struct_decl | newtype_decl | enum_decl | alias_decl | interface_decl
             | impl_decl | effect_decl | const_decl | foreign_decl | test_decl

fn_decl      = fn_sig { contract } block
fn_sig       = "fn" name [ generics ] "(" [ params ] ")" [ "->" type ] [ "uses" effects ]
generics     = "<" generic { "," generic } ">"
generic      = Name [ ":" bounds ] | "uses" Name
bounds       = bound { "+" bound }
bound        = type_path [ "<" bound_arg { "," bound_arg } ">" ]
bound_arg    = type_arg | Name "=" type   // Item = Int, Output = T
params       = param { "," param } [ "," ]
param        = [ "var" ] "self" | [ "var" ] ( name | "_" ) ":" type [ "=" expr ]   // closures: "_" too
contract     = "requires" expr | "ensures" [ "(" name ")" ] expr
struct_decl  = "struct" Name [ generics ] [ "{" [ field { "," field } [ "," ] ] "}" [ "requires" expr ] ]   // no braces: no fields
field        = { doc_comment } { attribute NEWLINE } [ "pub" ] name ":" type
newtype_decl = "struct" Name [ generics ] "(" [ "pub" ] type ")"
enum_decl    = "enum" Name [ generics ] "{" [ evariant { "," evariant } [ "," ] ] "}"
evariant     = { doc_comment } { attribute NEWLINE } variant
variant      = Name [ "(" vfield { "," vfield } [ "," ] ")" ]
vfield       = { doc_comment } { attribute NEWLINE } name ":" type   // no pub: the enum's visibility
alias_decl   = "type" Name [ generics ] "=" type
interface_decl = "interface" Name [ generics ] "{" { ( "type" Name | fn_sig { contract } ) NEWLINE } "}"
impl_decl    = "impl" [ generics ] [ type_path [ type_args ] "for" ] type "{" { impl_item NEWLINE } "}"
impl_item    = [ "pub" ] fn_decl | "type" Name "=" type | [ "pub" ] const_decl
effect_decl  = "effect" Name ( [ "uses" effects ] "{" { fn_sig NEWLINE } "}" | "=" type_path )
const_decl   = "const" NAME ":" type "=" expr
foreign_decl = "foreign" string [ "uses" effects ] "{" { ( { attribute NEWLINE } ( c_fn | "type" c_name ) ) NEWLINE } "}"
c_fn         = "fn" c_name "(" [ c_name ":" type { "," c_name ":" type } [ "," ] ] ")" [ "->" type ]
test_decl    = "test" string [ "forall" "(" params ")" ] [ "uses" effects ] block
effects      = "*" | effect_ref { "," effect_ref }      // "*" only on main
effect_ref   = type_path | Name                          // an effect or an effect variable

type         = type_path [ type_args ] | "(" ")" | "(" type "," type { "," type } [ "," ] ")"
             | "fn" "(" [ type { "," type } ] ")" [ "->" type ] [ "uses" effects ]
type_args    = "<" type_arg { "," type_arg } ">"
type_arg     = type | "uses" effects   // uses comes last and takes the rest of the list
type_path    = [ name "." ] Name { "." Name } | "Self" { "." Name } | c_name   // a type declared in a foreign block

pattern      = alt { "|" alt }
alt          = "_" | [ "-" ] literal [ ( ".." | "..=" ) [ "-" ] literal ] | name | NAME | type_path "." NAME | tuple_pat | variant_pat
tuple_pat    = "(" pattern "," pattern { "," pattern } [ "," ] ")"
variant_pat  = type_path [ "(" ( pattern { "," pattern } | pfields ) ")" ]
pfields      = ".." | pfield { "," pfield } [ "," ".." ]
pfield       = name ":" [ pattern ]

concurrent   = "concurrent" [ "(" arg { "," arg } ")" ]
               ( "{" spawn { "," spawn } [ "," ] "}" | "for" pattern "in" expr block )
spawn        = "spawn" expr
background   = "background" "(" name ":" "TaskScope" [ "," arg ] ")" block
select       = "select" "{" { pattern "=" expr "=>" ( block | expr ) "," } "}"
with_expr    = "with" handler { "," handler } block
handler      = type_path "=" expr
```

## Appendix: Prelude

The prelude is every name a file can use without an import ([Prelude and special interfaces](#modules-and-imports)). Signatures are written the way `quince` `query signature` prints them. r[prelude.hash-eq] A type parameter written `T: Hash` on a key type also requires `Eq`, which every type with `Hash` has. r[prelude.ord-eq] Likewise `T: Ord` implies `T: Eq`, and a type can implement `Ord` only when it has `Eq`. A `fn` with no body, a `{ ... }` body, or a one-line `{ fn ... }` interface or impl, here and in examples elsewhere in the spec, is a signature listing, not compilable code.

### Prelude names

| Name | Kind |
| --- | --- |
| Int, Int8…Int64, UInt8…UInt64, Float, Float32, Bool, Char, String, Bytes, Unit | Built-in types |
| List, Map, Set, Option, Result, tuples, function types | Built-in types |
| Duration, Timestamp, Url, Regex, Range, Iter | Types of tagged literals and ranges |
| Some, None, Ok, Err | Variants written bare |
| Eq, Ord, Ordering, Hash, Hasher, Display, Iterator, From, Encode, Decode, Add, Sub, Mul, Div, Rem, Neg | Interfaces and their helper types (the operator interfaces are in [Operators on user types](#operators-on-user-types)) |
| TaskScope, FailurePolicy, ScopeFailed | Types the background form names ([Concurrency](#long-lived-work-fire-and-forget)) |
| Never, DecodeError, InvalidFormat | Types the compiler produces or the prelude's own signatures name |
| assert, todo, unreachable, dbg | Functions |
| Net, Listen, Fs, Clock, Random, Env, Process, Console, State | Built-in effects ([Effects](#built-in-effects)) |

A name is in the prelude only if the language's syntax or the compiler needs it ([Prelude and special interfaces](#modules-and-imports)). r[effect.builtin-ops-no-import] Calling a built-in effect's operations never needs an import.

### Interfaces

```
pub interface Eq { fn eq(self, other: Self) -> Bool }
pub interface Ord { fn compare(self, other: Self) -> Ordering }
pub enum Ordering { Less, Equal, Greater }
impl Ordering {
  fn reversed(self) -> Ordering                 // for descending sorts
  fn then(self, next: Ordering) -> Ordering     // self unless Equal, for sorting by several keys
}
pub interface Hash { fn hash(self, hasher: Hasher) -> Hasher }
pub interface Display { fn display(self) -> String }
pub interface From<T> { fn from(value: T) -> Self }
pub interface Encode { fn encode(self) -> encoding.Data }
pub interface Decode { fn decode(data: encoding.Data) -> Result<Self, DecodeError> }
pub interface Iterator<uses E> {
  type Item
  fn next(var self) -> Option<Self.Item> uses E
}

impl Hasher {
  fn add<T: Hash>(self, value: T) -> Hasher
  fn add_bytes(self, bytes: Bytes) -> Hasher
}

pub struct DecodeError { pub path: String, pub expected: String, pub found: String }
```

- `Eq`, `Hash`, `Encode` and `Decode` are usually automatic ([Automatic equality and hashing](#automatic-equality-and-hashing)). `Ord` and `Display` are always written by hand.
- r[prelude.encode-data] `Encode` and `Decode` write to encoding.Data, the format-neutral data model in std.encoding. Code calls a format (`json.encode(x)`) rather than these methods, and implements them only for a custom wire form.
- r[prelude.display-impls] `String`, every number type, `Bool`, `Char`, `Duration`, `Timestamp` and `Url` implement `Display`, which string interpolation calls.
- r[prelude.eq-ord-impls] Every number type, `Bool`, `Char`, `String`, `Bytes`, `Duration` and `Timestamp` implement `Eq`, `Ord` and `Hash`; floats use their total order ([Types](#built-in-types)). r[prelude.unit-url-eq] `Unit` and `Url` implement `Eq` and `Hash`. r[prelude.collection-eq] `List`, `Map`, `Set`, `Option`, `Result` and tuples implement `Eq` and `Hash` when their element types do, and `List` and tuples implement `Ord` too, comparing element by element. r[prelude.no-other-ord] No other built-in type implements `Ord`.

### Functions

```
pub fn assert(condition: Bool, message: String = "")
pub fn todo(reason: String) -> Never
pub fn unreachable(reason: String) -> Never
pub fn dbg<T>(value: T) -> T
```

- r[prelude.assert] When the condition passed to `assert` is a comparison, a failure prints both sides.

### List

- r[expr.index.list] `xs[i]` is `xs.get(i)` that panics when `i` is out of range ([Errors and failure](#panics-bugs)).

### Map and Set

r[type.map.order] `Map` and `Set` iterate in insertion order, so a program's output never depends on hash values.

### Duration, Timestamp, Url and Regex

- r[prelude.tagged-values] The tagged literals `dur"30s"`, `ts"..."`, `url"..."` and `re"..."` build `Duration`, `Timestamp`, `Url` and `Regex` values at compile time. `parse` and `Regex.new` build them from text at runtime.

### Reference types and concurrency (std.sync)

```
pub struct TimedOut { pub after: Duration }
pub struct ChannelClosed
pub struct ScopeFailed { pub spawn_site: String, pub message: String }
pub enum FailurePolicy { LogAndContinue, Restart(max: Int), FailScope }
```

### Built-in effect operations

r[effect.builtin-ops] A built-in effect's operations are called through its name, like a package effect's: `Clock.now()`, `Fs.read_text(path)`, `Process.exit(1)`. These operations are what a handler for that effect implements, in tests and in a narrowing `with` ([Effects](#handlers-in-tests-and-production)). Standard library packages build higher-level APIs on them, such as `std.http` on `Net` and `Listen`.

```
effect Fs {
  fn read(path: String) -> Result<Bytes, FsError>
  fn read_text(path: String) -> Result<String, FsError>
  fn read_range(path: String, offset: Int, length: Int) -> Result<Bytes, FsError>
  fn read_lines(path: String) -> Result<Iter<Result<String, FsError>, uses Fs>, FsError>   // streams; each next() is an Fs call that may fail
  fn write(path: String, data: Bytes) -> Result<Unit, FsError>
  fn write_text(path: String, text: String) -> Result<Unit, FsError>
  fn append(path: String, data: Bytes) -> Result<Unit, FsError>
  fn exists(path: String) -> Result<Bool, FsError>
  fn list_dir(path: String) -> Result<List<String>, FsError>         // sorted by name
  fn walk(path: String) -> Result<List<String>, FsError>             // every file below path, sorted
  fn metadata(path: String) -> Result<Metadata, FsError>
  fn create_dir(path: String) -> Result<Unit, FsError>       // creates missing parents
  fn remove(path: String) -> Result<Unit, FsError>           // a file or an empty directory
  fn rename(path: String, to: String) -> Result<Unit, FsError>
  fn copy(path: String, to: String) -> Result<Unit, FsError>
  fn temp_dir() -> String
}

effect Net {
  fn connect(host: String, port: Int, tls: Bool = false) -> Result<Outbound, NetError>
  fn send(conn: Outbound, data: Bytes) -> Result<Unit, NetError>
  fn receive(conn: Outbound, max: Int) -> Result<Bytes, NetError>   // empty once the peer closes
  fn close(conn: Outbound)
}

effect Listen {
  fn bind(host: String, port: Int) -> Result<Listener, NetError>
  fn accept(listener: Listener) -> Result<Inbound, NetError>
  fn send(conn: Inbound, data: Bytes) -> Result<Unit, NetError>
  fn receive(conn: Inbound, max: Int) -> Result<Bytes, NetError>
  fn close(conn: Inbound)
}

effect Clock {
  fn now() -> Timestamp
  fn monotonic() -> Duration              // since a fixed point; never goes backwards
  fn sleep(d: Duration)
}

effect Random {
  fn bytes(count: Int) -> Bytes           // from the OS's secure generator
  fn int(range: Range<Int>) -> Int  requires !range.is_empty()   // Random.int(1..=6)
  fn float() -> Float                     // at least 0.0, below 1.0
  fn bool() -> Bool
}

effect Env {
  fn get(name: String) -> Option<String>
  fn vars() -> Map<String, String>
  fn args() -> List<String>               // without the program name
  fn program_name() -> String
  fn current_dir() -> String
}

effect Process {
  fn run(program: String, args: List<String>, input: Bytes = b"") -> Result<ProcessOutput, ProcessError>
  fn exit(code: Int) -> Never
}

effect Console {
  fn print(text: String)                  // standard output, then a newline
  fn write(text: String)                  // standard output, no newline
  fn eprint(text: String)                 // standard error, then a newline
  fn read_line() -> Option<String>        // without the newline; None at end of input
  fn read_all() -> Bytes
  fn is_terminal() -> Bool                // whether standard output is a terminal
}

pub enum FsError { NotFound(path: String), AlreadyExists(path: String), DirectoryNotEmpty(path: String), InvalidUtf8(path: String), PermissionDenied(path: String), Other(path: String, message: String) }
pub enum NetError { Refused, TimedOut, Reset, NameNotFound(host: String), PermissionDenied(host: String), Other(message: String) }
```

- r[effect.state-no-ops] `State` has no operations. It is the permission for the reference-type methods in [std.sync](../std/API.md#reference-types-and-concurrency-stdsync).
- A manifest limit that refuses an operation returns `FsError.PermissionDenied` or `NetError.PermissionDenied` ([Program execution](#capabilities-at-runtime)).
- r[exec.broken-pipe] Writes to a closed standard output end the program quietly with code 0, as Unix tools do on a broken pipe, so `Console` writes return nothing to check.
- Sleeping, accepting and receiving are cancellation points like every effectful call ([Cancellation](#cancellation)).
