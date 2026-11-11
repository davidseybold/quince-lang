# Prelude and special interfaces

r[prelude.only]
- The prelude is the only set of names a file can use without importing them, and the [table of prelude names](#prelude-names) lists every one.

r[prelude.contents]
- It holds `Option`, `Result` and their variants, the number, text, time and collection types, the core and operator interfaces, the built-in effects, `TaskScope`, `FailurePolicy`, `ScopeFailed`, `assert`, `todo`, `unreachable` and `dbg`.

r[prelude.not-ref-types]
- The other reference types of [Types](types.md) are not in it: `Ref`, `Atomic`, `ConcurrentMap`, `Channel` and `Pool` are imported from std.sync, and `Resource`, `Callback` and the C types from std.ffi.

r[prelude.fixed]
- The language fixes the prelude, and it changes only in an edition.

r[prelude.criterion]
- A name is in the prelude only if the language's syntax or the compiler needs it, so any name that isn't imported is a language name. Everything else, including test helpers such as `fixed_clock` from std.testing, is imported.

The prelude is every name a file can use without an import ([Prelude and special interfaces](prelude.md)). Signatures are written the way `quince` `query signature` prints them.

r[prelude.hash-eq]
A type parameter written `T: Hash` on a key type also requires `Eq`, which every type with `Hash` has.

r[prelude.ord-eq]
Likewise `T: Ord` implies `T: Eq`, and a type can implement `Ord` only when it has `Eq`. A `fn` with no body, a `{ ... }` body, or a one-line `{ fn ... }` interface or impl, here and in examples elsewhere in the spec, is a signature listing, not compilable code.

## Prelude names

| Name | Kind |
| --- | --- |
| Int, Int8…Int64, UInt8…UInt64, Float, Float32, Bool, Char, String, Bytes, Unit | Built-in types |
| List, Map, Set, Option, Result, tuples, function types | Built-in types |
| Duration, Timestamp, Url, Regex, Range, Iter | Types of tagged literals and ranges |
| Some, None, Ok, Err | Variants written bare |
| Eq, Ord, Ordering, Hash, Hasher, Display, Iterator, From, Encode, Decode, Add, Sub, Mul, Div, Rem, Neg | Interfaces and their helper types (the operator interfaces are in [Operators on user types](#operators-on-user-types)) |
| TaskScope, FailurePolicy, ScopeFailed | Types the background form names ([Concurrency](concurrency.md)) |
| Never, DecodeError, InvalidFormat | Types the compiler produces or the prelude's own signatures name |
| assert, todo, unreachable, dbg | Functions |
| Net, Listen, Fs, Clock, Random, Env, Process, Console, State | Built-in effects ([Effects](effects.md)) |

A name is in the prelude only if the language's syntax or the compiler needs it ([Prelude and special interfaces](prelude.md)).

## Interfaces

```quince
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

r[prelude.encode-data]
- `Encode` and `Decode` write to encoding.Data, the format-neutral data model in std.encoding. Code calls a format (`json.encode(x)`) rather than these methods, and implements them only for a custom wire form.

r[prelude.display-impls]
- `String`, every number type, `Bool`, `Char`, `Duration`, `Timestamp` and `Url` implement `Display`, which string interpolation calls.

r[prelude.eq-ord-impls]
- Every number type, `Bool`, `Char`, `String`, `Bytes`, `Duration` and `Timestamp` implement `Eq`, `Ord` and `Hash`; floats use their total order ([Types](types.md)).

r[prelude.unit-url-eq]
- `Unit` and `Url` implement `Eq` and `Hash`.

r[prelude.collection-eq]
- `List`, `Map`, `Set`, `Option`, `Result` and tuples implement `Eq` and `Hash` when their element types do, and `List` and tuples implement `Ord` too, comparing element by element.

r[prelude.no-other-ord]
- No other built-in type implements `Ord`.

## Functions

```quince
pub fn assert(condition: Bool, message: String = "")
pub fn todo(reason: String) -> Never
pub fn unreachable(reason: String) -> Never
pub fn dbg<T>(value: T) -> T
```

r[prelude.assert]
- When the condition passed to `assert` is a comparison, a failure prints both sides.

## Operators on user types

r[prelude.user-op.interfaces]
Arithmetic operators on user types come from six prelude interfaces, and each operator always calls the same one: `+` calls `Add`, `-` calls `Sub`, `*` calls `Mul`, `/` calls `Div`, `%` calls `Rem`, and unary `-` calls `Neg`.

r[prelude.user-op.builtin-impls]
The built-in numeric types implement them, and a user type opts in with an ordinary `impl`:

```quince
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

r[prelude.user-op.pure]
- **Pure.** The operator interfaces take no effect parameter, like `Eq` and `Ord`, so an operator can't do I/O or touch shared state. It can still panic, as `Int` overflow does.

r[prelude.user-op.compare]
- **Comparison.** `==` and `!=` always come from `Eq`, and `<`, `<=`, `>` and `>=` from `Ord`.

r[prelude.user-op.ne]
- There are no separate comparison interfaces, and `a != b` always means `!(a == b)`.

r[prelude.user-op.left-picks]
- **The left operand picks the impl.** `R` is the right operand's type, so `impl Mul<Int> for Money` allows `amount * 3` and nothing else.

r[prelude.user-op.swap-fix]
- `3 * amount` and `amount * amount` are compile errors, and for the first one the suggested fix swaps the operands.

r[prelude.user-op.output]
- `Output` lets an operator return another type, as `Timestamp - Timestamp` returns `Duration`.

r[prelude.user-op.fixed]
- **A fixed set.** No other operator can be overloaded. `&&`, `||`, `!`, `?`, the bitwise operators, ranges, field access and calls keep their built-in meaning, and a program can't define new operators.

r[prelude.user-op.compound]
- **Compound assignment.** `a += b` always means `a = a + b`, and the same holds for every other operator, so there is nothing more to implement.

r[prelude.user-op.query]
- **Tooling.** The query API reports which impl an operator expression calls, so a tool can go from `a - b` straight to its body.

## Iterators

r[prelude.iter.next-var-self]
- `next` takes `var self`, so it advances the iterator it is called on (see [Mutation](types.md#mutation)).

r[prelude.iter.for-copy]
- `for x in it` iterates over a copy of `it`, so `it` doesn't need to be `var` and is unchanged after the loop. That holds when the iterator's source is a value, such as a list or a range.

r[prelude.iter.shared-source]
- An iterator over an effect or a reference type, such as `Fs.read_lines` or `Channel.iter()`, shares its source, so every copy pulls from the same stream.

## Automatic equality and hashing

r[prelude.auto.eq-hash]
- Every struct and enum implements `Eq` and `Hash` automatically, field by field, when all of its fields do.

r[prelude.auto.builtin-eq-hash]
- The built-in `List`, `Map`, `Set`, `Option`, `Result` and tuples have `Eq` and `Hash` whenever their element types do.

r[prelude.auto.ref-no-eq]
- Reference types have neither, so a struct that holds one has no automatic `Eq` or `Hash`. There is nothing to write and nothing to forget.

r[prelude.auto.manual-eq]
- Writing `impl Eq` by hand turns off the automatic `Hash`, because the two must agree for maps and sets to work: the type has no `Hash` until it writes one, and the error on a missing `Hash` says why.

r[prelude.auto.encode]
- Every struct and enum whose fields are all `pub` also implements `Encode` and `Decode` automatically, when all of its field types do (see [Serialization](#serialization)).

r[prelude.auto.debug]
- Every value also has a built-in debug form, which assertions, panic messages and `quince` `test` print. It is not `Display` and can't be overridden.

r[prelude.dbg.behavior]
- `dbg(x)` writes `x`'s debug form and its source location to standard error and returns `x`, so it can wrap any expression.

r[prelude.dbg.pure]
- It is not an effect and is allowed in any function, pure ones included, so adding a print while debugging never changes a signature.

r[prelude.dbg.release]
- `quince` `check` rejects `dbg` in release builds, and `quince` `publish` refuses a package that contains one.

r[prelude.auto.no-ord-display]
- `Ord` and `Display` are never automatic, because field order and formatting are real choices. Write an `impl` for them.

r[prelude.auto.opaque]
- `@opaque` turns off the automatic `Eq`, `Hash`, debug form, test generator, `Encode` and `Decode`, for types whose fields shouldn't be compared or shown, such as handles and secrets.

## Serialization

A struct or enum whose fields are all `pub` implements the std interfaces `Encode` and `Decode` automatically when all of its field types do, the same way it gets `Eq` and `Hash`.

r[prelude.wire.field-visibility]
Only the fields' visibility counts, not the type's own, so a private struct whose fields are all `pub` gets them too.

r[prelude.wire.data-model]
The std formats (`json`, `toml`, `form`) all read and write through one format-neutral data model, so a type that encodes to one of them encodes to all of them. There is no derive to write and no hand-written codec to drift out of date.

```quince
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

r[prelude.wire.name.default]
- **Wire names.** A field or variant appears on the wire under its Quince name.

r[prelude.wire.name.explicit]
- `@wire("name")` on a field or variant sets its wire name exactly, which is also how keyword names such as `type`, `in` and `match` are written.

r[prelude.wire.case]
- `@wire(case: camel)` on a type renames every field and variant that has no `@wire` of its own; the cases are `camel`, `snake`, `kebab`, `pascal` and `upper_snake`.

r[prelude.wire.shape.struct]
- **Shapes.** A struct is an object.

r[prelude.wire.shape.collections]
- `List` and `Set` are arrays, tuples are fixed-length arrays, and `Bytes` is a base64 string.

r[prelude.wire.shape.map]
- A `Map` is an object when its key is `String` or a newtype over `String`, and an array of `[key, value]` pairs otherwise.

r[prelude.wire.shape.newtype]
- Newtypes are transparent: `UserId(Int)` is a plain number.

r[prelude.wire.option]
- **Optional fields.** An `Option` field decodes to `None` when it is missing or `null`, and `None` is left out on encode.

r[prelude.wire.required]
- Every other field must be present.

r[prelude.wire.enum.unit]
- **Enums.** An enum whose variants carry no data is a string.

r[prelude.wire.enum.tagged]
- Any other enum is an object with a `"type"` tag naming the variant, with the variant's fields beside it; `@wire(tag: "kind")` renames the tag.

r[prelude.wire.enum.tag-clash]
- A variant field with the same wire name as the tag is a compile error.

r[prelude.wire.number.int]
- **Numbers.** `Int` decodes only from a whole number within the `Int64` range.

r[prelude.wire.number.float]
- `Float` decodes from any number.

r[prelude.wire.number.nan]
- Encoding a `NaN` or infinite `Float` panics, because JSON can't represent it.

r[prelude.wire.unknown-fields]
- **Decoding is checked.** Unknown fields are ignored.

r[prelude.wire.invariants]
- Every `requires` invariant on a decoded struct is checked.

r[prelude.wire.decode-error]
- A failure is a `DecodeError(path:, expected:, found:)`, where the path points into the input, as in `$.items[3].price: expected Int, found "12"`.

r[prelude.wire.custom]
- **Writing your own.** An `impl Encode` or `impl Decode` replaces the automatic one, for example to write a `Timestamp` as an RFC 3339 string.

r[prelude.wire.private-fields]
- A type with private fields gets no automatic impl, because decoding would build it without going through its own functions; its package writes an `impl Decode` that calls its constructor.

r[prelude.wire.opaque]
- `@opaque` turns both off.

r[prelude.wire.formats]
- Other formats implement std's `Encoder` and `Decoder` interfaces against the same data model.

r[prelude.wire.pure]
- Encoding and decoding are pure.

- **Dynamic data.** `json.Value` holds JSON whose shape isn't known ahead of time.

r[prelude.wire.query]
- **Tooling.** `quince` `query wire <Type>` prints the type's wire shape.

r[prelude.wire.semver]
- `quince` `publish` treats any change to the wire shape of a public type as breaking ([Dependencies and publishing](../tooling/dependencies.md)), and when the change is a renamed field, its suggested fix adds `@wire("old_name")` to keep the old wire name.

## Duration, Timestamp, Url and Regex

r[prelude.tagged-values]
- The tagged literals `dur"30s"`, `ts"..."`, `url"..."` and `re"..."` build `Duration`, `Timestamp`, `Url` and `Regex` values at compile time. `parse` and `Regex.new` build them from text at runtime.
