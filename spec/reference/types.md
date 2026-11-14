# Types

r[type.nominal]
Quince's type system is static, sound, and nominal, with full type annotations required at every declaration boundary and inference allowed only inside function bodies.

r[type.no-any]
There is no `any` type and no unchecked cast.

```ebnf
type         = type_path [ type_args ] | "(" type "," type { "," type } [ "," ] ")"
             | fn_type | "(" fn_type ")"
fn_type      = "fn" "(" [ type { "," type } [ "," ] ] ")" [ "->" type ] [ "uses" effects ]   // a trailing uses after a bare fn_type return belongs to the outer fn
type_args    = "<" type_arg { "," type_arg } [ "," ] ">"
type_arg     = type | "uses" effects   // uses comes last and takes the rest of the list
type_path    = [ name "." ] Name { "." Name } | "Self" { "." Name } | c_name   // a type declared in a foreign block
```

## Built-in types

r[type.int]
- `Int`, `Int8`...`Int64`, `UInt8`...`UInt64`: Integers; overflow is a panic (see [Errors and failure](errors.md)), and checked\_add etc. return Option.

r[type.int-alias]
- Int is another name for Int64, not a separate type.

r[type.no-uint]
- There is no plain UInt; use UInt64

r[type.float]
- `Float`, `Float32`: IEEE 754 floats; Float is another name for Float64.

r[type.float.total-order]
- Float arithmetic follows IEEE, but Eq, Ord and Hash use a total order (NaN equals NaN, and -0.0 sorts before 0.0), so floats can be map keys and always sort the same way

r[type.bool]
- `Bool`: `true` / `false`

r[type.string]
- `String`: Immutable UTF-8 text.

r[type.string.no-index]
- There is no s\[i\]; .bytes() and .chars() say which unit you mean

r[type.bytes]
- `Bytes`: Immutable byte sequence

r[type.collections]
- `List<T>`, `Map<K, V>`, `Set<T>`: Values: passing or assigning never shares a mutable alias (see [Mutation](#mutation))

r[type.option]
- `Option<T>`: `Some(value)` or `None`

r[type.result]
- `Result<T, E>`: `Ok(value)` or `Err(error)`

r[type.unit]
- `Unit`: The type with exactly one value, `()`. The type is written `Unit`, never `()`.

r[type.never]
- `Never`: The type of an expression that never produces a value: return, break, continue, unreachable, todo and Process.exit. It has no values and fits wherever any type is expected (see [Inference rules](#inference-rules))

r[type.char]
- Char: One Unicode scalar value; .chars() yields them

r[type.tuple]
- Tuples: Fixed-size groups such as (Int, String), written (1, "a") and taken apart with let (n, s) = t or match, or read by position with t.0 and t.1

r[type.fn]
- Function types: fn(A) -> B, with uses E for effects ([Effects](effects.md))

r[type.fn-parens]
- A function type may be written in parentheses, which end it: `fn make() -> (fn(Int) -> Int uses Io) uses Net`. Only a function type takes them: `(Int)` is an error, since a single type needs no grouping.

r[type.tagged]
- Duration, Timestamp, Url, Regex: The values of the tagged literals dur, ts, url and re ([Lexical structure](lexical.md))

r[type.range]
- Range\<T>: The value of a..b or a..=b, over one integer type ([Ranges](expressions.md#ranges))

r[type.num.div-round]
Integer division `/` and remainder `%` round toward zero, so `-7 / 2` is `-3` and `-7 % 2` is `-1`.

r[type.num.conversions]
Conversions between numeric types are explicit methods: widening ones (`n.to_int64()`) always succeed, narrowing ones (`n.to_int32()`) return `Option`, and `f.to_int()` returns `None` for NaN, infinities and out-of-range values.

r[type.tuple.derived]
- The compiler provides Eq, Ord, Hash, Encode, Decode and test generators for tuples of any size, so the standard library needs no per-size implementations.

## Map and Set

r[type.map.order]
`Map` and `Set` iterate in insertion order, so a program's output never depends on hash values.

r[type.map.no-index]
- Maps have no `m[k]` at all, because a missing key is an expected case, not a bug. `m.get(k)` returns an `Option`.

## Type arguments and construction

```
let order = json.decode<Order>(body)?
let jobs = Channel<Job>.new(16)
let user = User(id: UserId(1), email:, name: "Ada", created_at: now)
```

r[type.type-arg.angle]
- After a name or path in an expression, `<` starts type arguments when the tokens up to its matching `>` parse as types and that `>` is directly followed by `(` or `.`. Otherwise `<` is less-than. Type names are always `PascalCase`, so in `f(a < b, c > (d))` neither `b` nor `c` can be a type, and the call has two comparison arguments.

r[type.type-arg.optional]
- Type arguments are optional wherever inference finds them ([Inference rules](#inference-rules)).

r[type.type-arg.construct]
- A `PascalCase` name followed by `(` builds a value: a struct (`User(id:, name: "Ada")`), a newtype (`UserId(1)`) or, through its enum's name, a variant (`Shape.Rect(width: 2.0, height: 3.0)`). Functions are `snake_case`, so this is never a function call.

r[type.type-arg.no-brace-construct]
- Braces never build a value of a user type: `{` starts a block or a map literal. So a struct value can appear in an `if`, `while`, `for`, `match` or `with` head without parentheses: `if p == Point(x: 0, y: 0) { ... }`.

## Mutation

r[type.mut.value-semantics]
All built-in collections and structs have value semantics.

r[type.mut.shared-in-ref]
Mutable state shared between functions lives only in a reference type (listed under [Reference types](#mutation)), and reading or writing one requires the `State` effect ([Effects](effects.md)), except where its item in the Reference types list below says otherwise. A signature therefore shows which state a function can reach, through its parameter types, and whether it touches that state, through `uses State`. A constant can't hold a reference type ([Items](items.md)), so there is no global mutable state to reach any other way.

r[type.mut.local]
**Local mutation.** On a `var` binding, collections and struct fields can be changed in place:

```
fn word_counts(text: String) -> Map<String, Int> {
  var counts: Map<String, Int> = {:}
  for word in text.split_whitespace() {
    counts.update(word, default: 0, fn(n) { n + 1 })
  }
  counts
}
```

r[type.mut.no-alias]
- Passing or assigning a value never creates a shared alias. Changing `counts` can't affect any other variable, so a function that mutates locally is still pure from the outside.

r[type.mut.rc-cow]
- Every heap value carries a reference count. A mutating method updates in place when the count is one, and copies first otherwise; the copy is unique, so later changes to it are in place again.

r[type.mut.move-last-use]
- The compiler moves a value at its last use, so building a collection in a loop never copies.

r[type.mut.c-memory-scope]
- Values that own C memory are the one exception: they are released at the end of their scope, never at last use ([Interop with C](interop.md)).

r[type.mut.copy-report]
- `quince` `check` reports every update that may copy, and names the alias that causes it ([Memory](memory.md)).

r[type.mut.var-self]
- A method that changes its receiver declares `var self`, as in `fn push(var self, item: T)`.

r[type.mut.var-self.call]
- It can be called only on a `var` binding or on a field path that starts at one (`state.items.push(x)`). On a `let` binding the call is a compile error, and the suggested fix makes the binding `var`.

r[type.mut.var-self.rebind]
- A call computes the updated value and rebinds it, the same shorthand as `user.name = ...`, so it updates in place when the value is unique. The built-in `push`, `insert` and `update` are declared this way, and the query API prints `var self` in every signature, so whether a method mutates is always visible.

**Shared state.** State that must outlive a call or be seen by several functions lives in a `Ref<T>`:

```
fn record_hit(stats: Ref<Stats>, path: String) -> Stats uses State {
  stats.modify(fn(s) { Stats(..s, hits: s.hits + 1) })
}
```

r[type.ref.identity]
- **Reference types.** A reference type has identity: copying one shares the same underlying state. These are the only types with reference semantics:

r[type.ref.ref]
- `Ref<T>`: One mutable value, within a single task. Needs `uses State`: `get`, `set`, `modify`. Crosses a task boundary: No

r[type.ref.atomic]
- `Atomic<T>`: One value updated atomically. Needs `uses State`: Every read and write. Crosses a task boundary: Yes

r[type.ref.concurrent-map]
- `ConcurrentMap<K, V>`: A map that tasks can update concurrently. Needs `uses State`: Every read and write. Crosses a task boundary: Yes

r[type.ref.channel]
- `Channel<T>`: A queue between tasks. Needs `uses State`: Every operation. Crosses a task boundary: Yes

r[type.ref.task-scope]
- `TaskScope`: A scope that owns long-lived tasks ([Concurrency](concurrency.md)). Needs `uses State`: No: `spawn` is not an effect. Crosses a task boundary: Yes

r[type.ref.resource]
- `Resource<T>`: C-owned memory with guaranteed cleanup ([Interop with C](interop.md)). Needs `uses State`: No: close() carries its foreign block's effects, and ptr, keep and is\_closed need none. Crosses a task boundary: Only if its foreign type is `@shareable`

r[type.ref.pool]
- `Pool<T, Err, uses E>`: A bounded set of values, such as connections, that tasks check out one at a time ([Interop with C](interop.md)). Needs `uses State`: Yes: checking out carries the open function's effects plus State, and waiting is a cancellation point. Crosses a task boundary: Yes, even when T can't

r[type.ref.callback]
- Callback\<F>: A Quince closure that C code may call later ([Interop with C](interop.md)). Needs `uses State`: No: its calls carry its foreign block's effects. Crosses a task boundary: No

r[type.ref.cbuf]
- `CBuf<T>`: C-allocated memory that C writes through and Quince reads ([Interop with C](interop.md)). Needs `uses State`: No: creating, reading and writing it are pure. Crosses a task boundary: No

r[type.ref.net]
- `Outbound`, `Inbound`, `Listener`: A network connection or listening socket ([Types the built-in effects use](../../std/API.md#types-the-built-in-effects-use)). Needs `uses State`: No: their operations are `Net` and `Listen` operations. Crosses a task boundary: Yes

r[type.ref.pure-pass]
Passing, storing or capturing a reference is pure.

r[type.ref.pure-create]
So is creating one, as in `Ref.new(0)`, since a fresh reference can't be observed without `uses State`.

r[type.ref.state-access]
Only reading or writing through it requires `uses State`, so a function with no `uses` clause always returns the same result for the same arguments.

r[type.ref.no-merge]
Same means equal under `Eq`: two calls that each create a `Ref` return different references, so the compiler never merges such calls.

r[type.ref.release-pure]
Releasing memory or a `Resource` is not an effect either, wherever the last reference drops: a resource's C cleanup runs without its foreign block's effects being required there ([Interop with C](interop.md)).

r[type.ref.in-fields]
Reference types may appear in struct fields, enum payloads and closure captures.

r[type.ref.crossing]
Whether a type can cross a task boundary is a property the compiler derives: a type can cross unless it contains a `Ref<T>`, `Resource<T>`, `Callback<F>` or `CBuf<T>` (other than a `@shareable` one, or one held inside a `Pool`) anywhere inside it.

r[type.ref.crossing.report]
The query API reports this property for every type, and a rejected `spawn` names the field path that holds the reference.

r[type.ref.no-cross]
- A `Ref<T>`, or any value that contains one, cannot cross a task boundary ([Concurrency](concurrency.md)). State shared between tasks uses `Atomic<T>`, `ConcurrentMap<K, V>` or `Channel<T>`.

## Inference rules

r[type.infer.required]
- Required annotations: function parameters, return types (leaving one out means Unit, and quince fmt removes a written -> Unit, so there is one form; the same holds in a function type such as `fn(Int) uses Log`), struct fields, constants.

r[type.infer.inferred]
- Inferred: local `let`/`var` bindings, closure parameters when passed directly to a typed parameter, generic arguments at call sites.

r[type.infer.locality]
- If inference would need information from more than one statement away, the compiler asks for an annotation instead of guessing.

r[type.never.type]
- **Never.** `return`, `break`, `continue`, `unreachable(...)`, `todo(...)` and `Process.exit(...)` have type `Never`, so each fits in any branch or match arm: `None => return Err(e)` sits beside an arm that yields a value.

r[type.never.fn]
- A function can declare `-> Never`, as in `fn fatal(msg: String) -> Never uses Console, Process`, and the compiler rejects it if the end of its body is reachable.

r[type.never.while-true]
- A `while true` with no `break` aimed at it can't complete ([Statements](statements.md)), so a server loop can be typed `-> Never`.

r[type.never.block]
- A block whose last statement or expression can't complete normally can't complete normally itself, and neither can a `while true` with no `break` aimed at it. Such a block has type `Never`, so a function returning `Summary` may end in a `while true` loop that it leaves only through `return`.
