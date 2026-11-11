# Statements

```ebnf
stmt        = "let" pattern [ ":" type ] "=" expr
            | "var" name [ ":" type ] "=" expr
            | place assign_op expr
            | "for" pattern "in" expr block
            | "while" expr block
            | expr
place       = ( name | "self" ) { "." name | "[" expr "]" }   // self only under var self
assign_op   = "=" | "+=" | "-=" | "*=" | "/=" | "%="
```

## Bindings

- r[stmt.bind.let-var] `let` declares an immutable binding; `var` declares a mutable one.
- r[stmt.bind.shadow.same-scope] Shadowing a binding in the same scope is a compile error. That rules out let x = x?, so the suggested fix gives the unwrapped value its own name (let user = user\_result?). r[stmt.bind.shadow.nested] Shadowing in a nested scope is allowed but flagged by the linter.
- r[stmt.bind.var-param] A parameter can be marked `var`, as in `fn sum<I: Iterator<Item = Int>>(var it: I)` `-> Int`. The function gets its own mutable copy, and the caller's value never changes. This is not an inout parameter, and it avoids `var it = it`, which the shadowing rule forbids.
- r[stmt.bind.underscore] \_ is not a binding, so it can be written any number of times in one scope.

## Assignment

- r[stmt.assign.stmt] Assignment is a statement: `place = expr`, or `place op= expr` with `+=` `-=` `*=` `/=` `%=`, meaning `place = place op expr`. r[stmt.assign.place] A place is a `var` binding, a field path from one, or an index into one (`xs[i] = v`). r[stmt.assign.no-chain] There is no `x = y = z`, no assignment inside a condition, and no `++` or `--`.

## Loops

r[stmt.loop.forms] There is exactly one loop over collections (`for x in xs`) and one conditional loop (`while`). No `do-while`, no C-style `for`, no `loop`. r[stmt.loop.no-labels] There are no loop labels: to leave nested loops at once, move them into a function and `return`.

- r[stmt.loop.stmt] `for` and `while` are statements, and `break` takes no value. r[stmt.loop.for-sources] `for` iterates any `Iterator`, and a `List`, `Set` or `Map` directly (a map's entries as `(key, value)` pairs). r[stmt.loop.for-string] A `String` or `Bytes` value needs `.chars()`, `.bytes()` or `.iter()`, so the unit is always written. r[stmt.loop.fanout-sources] `concurrent(limit:) for` takes the same things.
