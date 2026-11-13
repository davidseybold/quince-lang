# Expressions

```ebnf
expr        = jump | logic | range | bits
primary     = literal | path [ type_args ] | type_path [ type_args ] | NAME | "self" | "(" expr ")" | tuple
            | list_lit | map_lit | struct_lit | closure
            | if_expr | match_expr | concurrent | background | select | with_expr
tuple       = "(" ")" | "(" expr "," expr { "," expr } [ "," ] ")"
list_lit    = "[" [ expr { "," expr } [ "," ] ] "]"
map_lit     = "{" ":" "}" | "{" entry { "," entry } [ "," ] "}"
entry       = expr ":" expr
```

## Blocks, statements and expressions

```ebnf
block       = "{" { stmt NEWLINE } [ stmt ] "}"   // a last stmt that is an expr is the value
if_expr     = "if" expr block [ "else" ( if_expr | block ) ]
```

r[expr.block.expr-stmt]
`if`, `match`, closures and the keyword forms (`concurrent`, `background`, `select`, `with`) are expressions. Declarations, loops and assignments are statements.

r[expr.block.value]
- A block's value is its last line when that line is an expression, and `()` otherwise.

r[expr.block.last-newline]
- The last statement needs no newline before `}`, so a one-line block may hold a single statement: `Some(m) => { total = total + m },`.

r[expr.block.fn-value]
- A function returns its body's value, as `word_counts` does in [Mutation](types.md#mutation).

r[expr.if.type]
- `if` with an `else` has its branches' common type.

r[expr.if.no-else]
- Without an `else`, its type is `Unit` and its branch must be `Unit`: `let sign = if n < 0 { -1 } else { 1 }`.

r[expr.block.unit-context]
- Where the expected type is `Unit` (an `if` without `else`, a loop body, the body of a function that returns `Unit`, or an `if` or `match` used as a statement), a last expression is a statement: its value is discarded under the unused-value rules ([Errors and failure](errors.md)). So `if ok { seen.insert(x) }` compiles, an unused `Result` is still an error, and the arms of a `match` used as a statement need not share a type.

r[expr.block.no-bare]
- There are no bare blocks.

r[expr.block.cond-brace]
- In the condition of an `if` or `while`, the subject of a `match`, and the iterable of a `for`, `{` always starts the block. A map literal there is written in parentheses.

r[expr.block.brace-map]
- Anywhere else, `{` where an expression is expected starts a map literal.

r[expr.block.positions]
- Blocks appear only as the bodies of functions, closures, `if`, `else`, match arms, `for`, `while`, `test` and the keyword forms.

r[expr.block.keyword-bodies]
- The bodies of `with` and `background`, like `select` arms, are ordinary blocks of the enclosing function: `return`, `?`, `break` and `continue` leave them, and a `background` block first cancels its tasks and waits for them.

r[expr.block.concurrent-tasks]
- A `concurrent` block's spawned expressions are tasks, not blocks of the function.

## Operators and precedence

```ebnf
logic       = compare { "&&" compare }
            | compare "||" compare { "||" compare }   // never mixed with &&
compare     = arith [ cmp_op arith | "is" pattern ]    // never chained
cmp_op      = "==" | "!=" | "<" | "<=" | ">" | ">="
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
```

r[expr.op.levels]
Operators bind in seven levels, tightest first.

r[expr.op.parens-required]
Where an item in the list below says an expression must be parenthesized, it can't be an operand of a different operator without parentheses, and the compiler's fix adds the parentheses that give the usual reading. So every expression that compiles reads one way.

- 1 Postfix: `.name  .method()  f()  xs[i]  ?`. Grouping: Left to right. `-x?` is `-(x?)`
- 2 Prefix: `-  !  ~`. Grouping: Right to left. `-x.abs()` is `-(x.abs())`
- 3 Multiplicative: `*  /  %`. Grouping: Left to right
- 4 Additive: `+  -`. Grouping: Left to right

r[expr.op.range.no-chain]
- 5 Range: `..  ..=`. Grouping: Doesn't chain

r[expr.op.range.parens]
- A range must be parenthesized as an operand (see [Ranges](#ranges))

r[expr.op.is-pattern]
- 6 Comparison: `==  !=  <  <=  >  >=  is`. Grouping: Doesn't chain. `is` takes a pattern on its right ([Patterns](patterns.md))

r[expr.op.logic.short-circuit]
- 7 Logical: `&&  ||`. Grouping: Left to right, short-circuit

r[expr.op.logic.no-mix]
- `&&` and `||` can't be mixed without parentheses

r[expr.op.bits.chain]
- Apart: `&  |  ^  <<  >>`. Grouping: `&`, `|` and `^` chain only with themselves

r[expr.op.bits.parens]
- `&`, `|`, `^`, `<<` and `>>` have no level: each must be parenthesized as an operand of any other operator, and its own operands are prefix or postfix expressions

```
flags & MASK == 0        // error: write (flags & MASK) == 0
a || b && c              // error: write a || (b && c)
0 <= i < n               // error: write 0 <= i && i < n
1 << n - 1               // error: write 1 << (n - 1)
```

r[expr.op.compare.no-chain]
- Comparison operators don't chain: `a < b > c` is a syntax error, and the suggested fix is `a < b && b < c`. So `json.decode<Order>(body)` has only one reading.

r[expr.op.type-args]
- In an expression, `<` after a name or path starts type arguments when what follows parses as type arguments closed by `>`, and the token after the `>` is `(`, `.`, `?`, `,`, `)`, `]`, `}` or a line break, and not a `=` directly after the `>`. Otherwise `<` is less-than.

r[expr.op.keyword-forms]
`return`, `break`, `continue` and `spawn` are keyword forms, not operators; `return expr` and `spawn expr` take everything to their right.

Operand types.

r[expr.op.no-conversion]
No operator converts its operands, so `Int + Float` and `Int32 + Int64` are errors whose fix names the conversion method ([Types](types.md)). User types take part only through the interfaces in [Operators on user types](prelude.md#operators-on-user-types).

r[expr.op.arith.operands]
- Operands of `+  -  *  /  %`: Two values of the same integer or float type

r[expr.op.arith.panic]
- Integers panic on overflow, on division by zero and on Int.MIN / -1, and / rounds toward zero.

r[expr.op.arith.float]
- Floats follow IEEE

r[expr.op.neg.operands]
- Operands of Prefix `-`: Signed integers and floats, not unsigned ones

r[expr.op.neg.panic]
- -Int.MIN panics

r[expr.op.compare.operands]
- Operands of `==  !=  <  <=  >  >=`: Two values of the same type, with Eq or Ord. Failure: Never fails

r[expr.op.logic.operands]
- Operands of `&&  ||  !`: Bool only. Failure: Never fails

r[expr.op.bits.operands]
- Operands of `&  |  ^  ~`: Values of the same integer type. Failure: Never fails

r[expr.op.shift.operands]
- Operands of `<<  >>`: An integer, shifted by a count of any integer type

r[expr.op.shift.panic]
- A shift panics when the count is negative or not below the bit width.

r[expr.op.shift.semantics]
- Bits shifted out are dropped, and >> keeps the sign of a signed value

r[expr.op.plus-numeric]
`+` is numeric only.

r[expr.op.plus-suggest]
Strings join with interpolation (`"{first}{last}"`) and lists with `.concat()`, and the compiler suggests both.

r[expr.op.float-total-order]
Because comparisons use `Eq` and `Ord`, floats compare in their total order ([Types](types.md)): `NaN == NaN` is true and `-0.0 == 0.0` is false.

r[expr.op.float-eq-warn]
`quince` `check` warns when a float is compared with `==` against a literal.

r[expr.op.excluded]
Not in Quince: the ternary `c ? a : b` (use `if`), `++` and `--` (`+= 1`), `**` (`x.pow(n)`), `??` and `?.` (`unwrap_or`, `?`, `map`), a pipe operator, and casts with `as`.

## Ranges

```ebnf
range       = arith ( ".." | "..=" ) arith
```

r[expr.range.semantics]
`a..b` runs from `a` up to but not including `b`, and `a..=b` includes `b`.

r[expr.range.type]
Both are values of the prelude type `Range<T>`, where `T` is one integer type for both ends.

r[expr.range.empty]
- When `a >= b` (or `a > b` for `..=`), the range is empty; it never panics.

r[expr.range.no-overflow]
- `0..=Int.MAX` iterates to the end without overflowing.

- Ranges drive loops (`for i in 0..n`) and are passed as values, as in `xs.slice(1..3)`. Their methods are listed in [the standard library API](../../std/API.md#ranges).

r[expr.range.pattern]
- Both forms are patterns too: `1..=9 => "digit"`.

r[expr.range.pattern.char]
- A pattern range may also be over `Char`, as in `'a'..='z' => ...`, and exhaustiveness counts its scalar values; a `Range<T>` value is always over an integer type.

r[expr.range.pattern.exhaustive]
- Exhaustiveness is checked over integer ranges, so `0..=127` and `128..=255` cover a `UInt8`.

r[expr.range.no-open]
- There are no open ranges (`a..`, `..b`, `..`), because `..` already means the remaining fields in a pattern and the copied fields in `User(..user, name: "New")`.

r[expr.range.precedence]
- `0..n + 1` is `0..(n + 1)`, and a range must be parenthesized as an operand: `r == (0..3)`.

## Indexing and slicing

r[expr.index.int]
- `xs[i]` takes only an `Int`; another integer type is converted first, as in `xs[n.to_int64()]`.

r[expr.index.slice]
- A slice is the method call `xs.slice(r)`, so `[]` has one meaning.

r[expr.index.list]
- `xs[i]` is `xs.get(i)` that panics when `i` is out of range ([Errors and failure](errors.md)).

## Exhaustive `match`

```ebnf
match_expr  = "match" expr "{" { arm "," } "}"
arm         = pattern [ "if" expr ] "=>" ( block | expr )
```

r[expr.match.arm-syntax]
- A `match` arm is `pattern => expr,` or `pattern => { block },`.

r[expr.match.arm-block]
- In an arm, `{` starts a block.

r[expr.match.arm-comma]
- Every arm ends with a comma, a block arm included.

r[expr.match.exhaustive]
- Every `match` must cover every variant. A missing case is a compile error that lists the missing variants.

r[expr.match.wildcard-foreign]
- A wildcard arm (`_`) on a non-extensible enum from another package triggers a warning, because a later major version could add variants, and the wildcard would silently swallow them when you upgrade.

r[expr.match.guard]
- Guards are allowed: `n if n > 0 => ...`.

r[expr.match.guard-exhaustive]
- Arms with guards don't count toward exhaustiveness.

r[expr.match.empty-type]
- A case whose type has no values needs no arm. A `Result<T, Never>` can't be `Err`, so matching it needs only `Ok(v)`, and code generic over an error type can be given `Never` to say it can't fail.

## Pattern tests with `is`

r[expr.is.bool]
`expr is Pattern` evaluates to `Bool` and can be used anywhere an expression can.

r[expr.is.scope]
Inside an `if` condition, variables bound by the pattern are in scope in the true branch, in the right side of a following && and in the body of a while the test heads (never through || or !):

```
if response is Ok(page) {
  render(page)
}
```

`is` complements `match`: use it to check or extract one case, and `match` when every case needs handling.

## `return`, `break` and `continue`

```ebnf
jump        = "return" [ expr ] | "break" | "continue"
            | "spawn" [ "in" postfix ] expr   // a TaskScope value: bg, self.bg
```

r[expr.ret.value]
`return expr` ends the function with `expr`.

r[expr.ret.bare]
A bare `return` is allowed only in a function that returns `Unit`, where it means `return ()`.

r[expr.jump.expr]
- `return`, `break` and `continue` are expressions, so they can stand in a match arm (`None => return,`).

## Closures and function values

```ebnf
closure     = "fn" "(" [ cparam { "," cparam } [ "," ] ] ")" [ "->" type ] [ "uses" effects ] block
cparam      = ( name | "_" ) [ ":" type ]
```

r[expr.closure.syntax]
- A closure is `fn(params) -> T uses E { body }`.

r[expr.closure.infer]
- Parameter types, the return type and `uses` can be left out when the expected type gives them: `fn(n) { n + 1 }`.

r[expr.closure.no-expected]
- With no expected type, the parameter types are written, and the return type and the effects are inferred from the body: `Callback.local(fn(sent: CInt, total: CInt) { done.set(sent.to_int64()) })` needs no `uses`.

r[expr.closure.underscore]
- An unused parameter of a closure or function may be named `_`.

r[expr.fn-value.named]
- A named function, or an associated function such as `Url.parse`, can be passed where a function type is expected: `lines.map(parse_user)`. Its type carries its effects.

r[expr.fn-value.no-method]
- A method is passed through a closure, `fn(u) { u.display() }`, so a function value never hides which receiver it holds.

r[expr.fn-value.variant]
- An enum variant with a payload is a function from its fields, in declaration order, so `map_err(AppError.Serve)` passes the variant as a function.

r[expr.closure.capture-value]
- Closures capture values, never `var` bindings by reference. So a closure can't call a `var self` method on a value it captured; it copies the value into a local `var` first.

r[expr.closure.no-assign-capture]
- Assigning to a captured binding is a compile error too, and the suggested fix uses `fold` or a `for` loop.
