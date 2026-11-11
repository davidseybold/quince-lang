# Quince standard library API

Nov 10, 2026 · @David

**Draft, scope approved.** This doc will hold the full API of Quince's standard library: the methods of every prelude type, the operations of every built-in effect, and every `std` package. The spec keeps only the prelude names, interfaces and effect operations the language depends on, and points here for the rest. David approved the package map and decisions 1 to 3 on 2026-11-10.

## About this doc

The spec decides what Quince code means; this doc decides the standard library's names and signatures. Where they disagree, the spec wins and this doc is fixed. It is versioned with the spec and currently tracks v0.3.

Each signature is written in one place. The spec holds the signatures of the prelude interfaces and every effect operation. The other prelude methods are in [Signatures from the spec](#signatures-from-the-spec) at the end of this doc, and the entries here add the rest. A fresh agent gets the spec and this doc. Every `std.*` signature lives only here.

### Entry format

Every entry is written for an agent that reads one entry and then writes a call. It starts with the signature exactly as `quince` `query signature` prints it (receiver, `var self`, `uses`, `requires` and `ensures`), followed by the same six fields every time:

| Field | Says | Why an agent needs it |
| --- | --- | --- |
| What | One sentence | Picks the right function without reading the others |
| Cost | O(1), O(log n), O(n), O(n log n) | Spec §6 has `quince` `check` warn when a contract calls a std function slower than constant time, so every function needs a stated cost |
| Fails | The error type and the variants it can return, or "never" | Write the `match` without guessing |
| Panics | The exact condition, or "never" | Know which calls are bugs on bad input |
| Waits | "Cancellation point" when the call can block | Reason about timeouts and cancellation (spec §9) |
| Example | A `>>>` doc example | Becomes a doc test once std is implemented, so the doc can't drift |

A sample entry:

```
fn update(var self, key: K, default: V, f: fn(V) -> V uses E) uses E
```

- What: replaces the value at `key` with `f(value)`, starting from `default` when the key is missing.
- Cost: O(1) average; in place when the map is unique, otherwise one copy (spec §3, Mutation).
- Fails: never. Panics: never. Waits: no.
- Example: on an empty `Map<String, Int>` named `m`, `>>> m.update("a", default: 0, fn(n) { n + 1 })` leaves `m` as `{"a": 1}`.

To keep the doc scannable, entries are grouped into one table per type or package, with the columns Signature, What, Cost and Notes. Notes lists Fails, Panics and Waits only when they apply, so a blank Notes cell means the call never fails, never panics and never waits. Examples follow a table when a call isn't obvious. Cost uses n for the length of the receiver and m for the length of the argument.

### Naming conventions

These apply to every package, so an agent can guess a name correctly before looking it up.

- `T.new(...)` builds a value and can't fail. A constructor that checks its inputs has another name and returns `Result`, as the spec's `User.create(email) -> Result<User, UserError>` does (spec §3). `T.parse(text)` reads text and returns `Result<T, InvalidFormat>`. `T.from(x)` always goes through a `From` impl and can't fail; a conversion that can fail is `T.try_from(x) -> Option<T>` (spec §3).
- `get` is a lookup that returns `Option`, with one exception: `Router.get(pattern, handler)` adds a route, because every router uses that name for the HTTP method.
- Lookups that may find nothing return `Option` and are called `get`, `first`, `last` or `find`. Only `xs[i]` panics on a missing index.
- Predicates start with `is_` or `has_`, or are `contains`.
- Conversions are `to_x()`: widening ones return the value, narrowing ones return `Option` (spec §3).
- The same operation has the same name on every type: `len`, `is_empty`, `contains`, `get`, `insert`, `remove`, `map`, `filter`, `fold`, `sorted`.
- A package's main error enum is named after it (`FsError`, `HttpError`). A second entry point with different failures gets its own enum (`ServeError` for `serve`, `ParseTimeError` for time parsing), so each `match` lists only what can happen. An error type for Net or Fs work carries `PermissionDenied` (spec §5).
- Every function that takes a closure declares an effect variable for it and passes it through, so pure and effectful closures both fit. The exception is `modify` and `update` on the reference types (`Ref`, `Atomic`, `ConcurrentMap`), which take pure closures on purpose (spec §9).
- A scoped helper that lends something to a closure is named for what it does (`pool.checkout`, `ffi.with_resource`, `time.with_timeout`). No std name, method or parameter is a keyword: `with`, `test` and the rest are reserved, so `Pool.checkout`, `replace(by:)`, `pred:` and `Headers.set` are used instead.
- A function is always called through its package: `http.get(url)`, `json.decode(text)`, `time.with_timeout(...)`, `testing.fixed_clock(...)`. Only types, interfaces, effects and constants are imported by name, as in `import std.http.{self, Request, Response}` (spec §7). The prelude's `assert`, `todo`, `unreachable` and `dbg` are the only functions called bare.

## Package map

The proposed v1 standard library is the prelude plus 22 packages, about 450 functions and types in all. It aims to be batteries-included, like Go's: an agent writing a service, a CLI or a pipeline should find one canonical package for each common job, and never have to pick between competing registry packages for JSON, HTTP, time or hashing.

| Package | Holds | Effects |
| --- | --- | --- |
| Prelude (no import) | Only what the language's syntax or compiler needs: the types literals produce and their methods, `Option`, `Result`, `Ordering`, `Iter`, `Range`, `Never`, `Hasher`, the 14 prelude interfaces, `TaskScope`, `FailurePolicy`, `ScopeFailed`, `DecodeError`, `InvalidFormat`, and `assert`, `todo`, `unreachable`, `dbg` | none |
| `std.sync` | `Ref`, `Atomic`, `ConcurrentMap`, `Channel`, `Pool`, `ChannelClosed` | State |
| `std.console` | A `Console` handler that discards output | Console |
| `std.env` | Config structs from environment variables | Env |
| `std.cli` | Parse command-line arguments into a struct through `Decode`, with generated `--help` | Env, Console, Process |
| `std.fs` | `FsError`, `Metadata`, `Path` helpers, narrowing handlers | Fs |
| `std.process` | `ProcessOutput`, `ProcessError` | Process |
| `std.net` | `Outbound`, `Inbound`, `Listener`, `NetError`, the `net.only` handler | Net, Listen |
| `std.http` | Client (`get`, `request`), server (`serve`, `Request`, `Response`, a router), status codes, headers | Net, Listen |
| `std.json` | `encode`, `decode`, `Value` | pure |
| `std.toml` | Same API as `std.json` | pure |
| `std.form` | Same API as `std.json`, for URL-encoded forms and query strings | pure |
| `std.encoding` | `Encoder` and `Decoder` over the format-neutral data model; base64, hex, percent-encoding | pure |
| `std.regex` | `Match`, `Captures` | pure |
| `std.time` | Calendar `Date` and `Time`, time zones, formatting, `with_timeout` and `TimedOut` | Clock for `with_timeout` |
| `std.random` | A seedable generator, `shuffle`, `choose` | Random |
| `std.uuid` | `Uuid`, versions 4 and 7 | Random, Clock |
| `std.crypto` | SHA-256 and SHA-512, HMAC, constant-time comparison, password hashing, secure tokens, encryption | Random for tokens and keys |
| `std.collections` | `Deque`, `SortedMap`, `SortedSet`, `Heap` | pure |
| `std.iter` | Builders for new iterators; the adapters live on the prelude `Iter` | pure, plus `E` |
| `std.log` | The `Log` effect, `Level`, JSON and text handlers | Console, Clock |
| `std.ffi` | C types (`CInt`, `CSize`, `CString`, `Ptr<T>`, `ConstPtr<T>`, `CBuf<T>`), `Resource<T>`, `with_resource`, `Callback<F>` | none of its own |
| `std.testing` | `fixed_clock`, fakes and capture handlers for every built-in effect, `Generate`, `Gen` | any |

Out of v1: database drivers and a `std.sql` interface (the spec's `Db` stays a user effect; drivers are third-party packages), templating, compression, XML, YAML and TLS configuration beyond what `std.http` does by default. Each is a candidate for a later minor version.

## Prelude

A name is in the prelude only if the language's syntax or the compiler needs it (David, 2026-11-10). Everything else is imported, so a name with no import is always a language name. That leaves about 40 names: the types that literals produce, `Option` and `Result` with their variants, `Ordering`, `Iter`, `Range`, `Never`, `Hasher`, the 14 prelude interfaces, `TaskScope`, `FailurePolicy`, the errors `ScopeFailed`, `DecodeError` and `InvalidFormat`, and four functions: `assert`, `todo`, `unreachable` and `dbg`.

[Signatures from the spec](#signatures-from-the-spec) is the single source for these signatures; this section adds cost, failure, panic and wait notes. Methods that come from an interface (`display`, `compare`, `eq`, `hash`) are listed once, under Prelude interfaces. Methods need no import, because the value's type already says where they come from.

### Integers

Every integer type (`Int8` to `Int64`, `UInt8` to `UInt64`, and `Int`, which is `Int64`) has the methods below, with `Self` meaning that type. Operators panic on overflow (spec §4); these methods are how code asks for another behavior.

| Signature | What | Cost | Notes |
| --- | --- | --- | --- |
| `Self.MIN`, `Self.MAX` | Smallest and largest value | O(1) | Associated constants, see open item 9 |
| `Self.parse(text: String, radix: Int = 10) -> Result<Self, InvalidFormat>` | Reads an optional sign and digits; no spaces, no `_` | O(m) | Fails: `InvalidFormat, also for out-of-range values`. Panics: radix outside 2 to 36 |
| `fn checked_add(self, other: Self) -> Option<Self>` | Sum, or `None` on overflow; also `checked_sub`, `checked_mul` | O(1) |  |
| `fn checked_div(self, other: Self) -> Option<Self>` | Quotient, or `None` for zero or overflow; also `checked_rem` | O(1) |  |
| `fn saturating_add(self, other: Self) -> Self` | Sum, clamped to `MIN` and `MAX`; also `saturating_sub`, `saturating_mul` | O(1) |  |
| `fn wrapping_add(self, other: Self) -> Self` | Sum modulo 2 to the bit width, for hashes and checksums; also `wrapping_sub`, `wrapping_mul` | O(1) |  |
| `fn div_euclid(self, other: Self) -> Self` | Quotient rounded so the remainder is never negative; also `rem_euclid` | O(1) | Panics: `other` is zero, or overflow |
| `fn abs(self) -> Self` | Absolute value | O(1) | Panics: `MIN` of a signed type |
| `fn pow(self, exponent: Int) -> Self` | `self` to the power `exponent` | O(log exp) | `requires exponent >= 0`. Panics: overflow |
| `fn min(self, other: Self) -> Self` | The smaller; also `max` | O(1) |  |
| `fn clamp(self, low: Self, high: Self) -> Self` | `self` limited to `low..=high` | O(1) | `requires low <= high` |
| `fn signum(self) -> Self` | -1, 0 or 1 | O(1) |  |
| `fn to_int8(self) -> Option<Int8>` and the other `to_` integer conversions | Widening conversions return the value, narrowing ones `Option` (spec §3) | O(1) |  |
| `fn to_float(self) -> Float` | Nearest `Float`; exact up to 2^53 | O(1) | Also `to_float32` |
| `fn to_string(self, radix: Int = 10) -> String` | Digits in the given base, lowercase letters above 9 | O(digits) | Panics: radix outside 2 to 36. With the default radix it equals `display()` |
| `fn to_char(self) -> Option<Char>` | The Unicode scalar with this code | O(1) | `None` for surrogates and values above 0x10FFFF |

### Float

`Float` (`Float64`) and `Float32` share these methods. Arithmetic follows IEEE 754; `Eq`, `Ord` and `Hash` use the total order from spec §3.

| Signature | What | Cost | Notes |
| --- | --- | --- | --- |
| `Float.PI`, `Float.E`, `Float.INFINITY`, `Float.NAN`, `Float.EPSILON` | Constants | O(1) | Associated constants, see open item 9 |
| `Float.parse(text: String) -> Result<Float, InvalidFormat>` | Decimal or exponent form; also `inf`, `-inf`, `nan` | O(m) | Fails: `InvalidFormat` |
| `fn is_nan(self) -> Bool` | Also `is_finite`, `is_infinite` | O(1) |  |
| `fn floor(self) -> Float` | Also `ceil`, `trunc`, and `round`, which rounds half away from zero | O(1) |  |
| `fn abs(self) -> Float` | Also `signum`, `min`, `max`, `clamp(low:, high:)` | O(1) |  |
| `fn sqrt(self) -> Float` | Also `cbrt`, `exp`, `ln`, `log2`, `log10`, `pow(exponent: Float)` | O(1) | NaN for invalid input, as IEEE says |
| `fn sin(self) -> Float` | Also `cos`, `tan`, `asin`, `acos`, `atan`, `atan2(x: Float)` | O(1) | Radians |
| `fn is_close(self, other: Float, tolerance: Float = 1e-9) -> Bool` | Relative comparison, for tests and contracts | O(1) |  |
| `fn to_int(self) -> Option<Int>` | Truncates toward zero | O(1) | `None` for NaN, infinities and out-of-range values (spec §3) |
| `fn to_string(self) -> String` | Shortest text that reads back to the same value | O(1) | Equals `display()` |
| `fn to_fixed(self, decimals: Int) -> String` | Exactly `decimals` digits after the point, rounded half to even | O(1) | `requires decimals >= 0 && decimals <= 20` |

```
>>> 3.14159.to_fixed(2)
"3.14"
>>> let price = total.to_fixed(2)   // then "Total: {price}"
```

### Bool and Char

`Bool` has no methods beyond `display()`; use `!`, `&&` and `||`.

| Signature | What | Cost | Notes |
| --- | --- | --- | --- |
| `fn is_alphabetic(self) -> Bool` | Also `is_numeric`, `is_alphanumeric`, `is_whitespace`, `is_uppercase`, `is_lowercase`, `is_ascii` | O(1) | Unicode properties |
| `fn to_uppercase(self) -> String` | Also `to_lowercase`. A `String`, because one char can map to several ("ß" to "SS") | O(1) |  |
| `fn to_digit(self, radix: Int = 10) -> Option<Int>` | Value of a digit character | O(1) | Panics: radix outside 2 to 36 |
| `fn code(self) -> UInt32` | The scalar value; `Int.to_char` goes back | O(1) |  |
| `fn utf8_len(self) -> Int` | Bytes in its UTF-8 form, 1 to 4 | O(1) |  |

### String

A `String` is UTF-8. There is no `len()`, because "length" could mean bytes or characters: `byte_len()` and `char_count()` make the caller choose. Offsets are byte offsets, and any method taking one returns `None` when it doesn't fall on a character boundary. Costs use n for the receiver's bytes and m for the argument's.

| Signature | What | Cost | Notes |
| --- | --- | --- | --- |
| `String.from_utf8(bytes: Bytes) -> Result<String, InvalidFormat>` | Checks and wraps the bytes | O(m) |  |
| `fn byte_len(self) -> Int` | Number of bytes | O(1) |  |
| `fn char_count(self) -> Int` | Number of `Char`s | O(n) |  |
| `fn is_empty(self) -> Bool` |  | O(1) |  |
| `fn contains(self, part: String) -> Bool` | Also `starts_with`, `ends_with` | O(n + m) |  |
| `fn find(self, part: String) -> Option<Int>` | Byte offset of the first match; also `find_last` | O(n + m) |  |
| `fn slice(self, range: Range<Int>) -> Option<String>` | The bytes in `range`, as in `s.slice(0..4)` | O(range) | `None` when out of range or off a boundary |
| `fn split(self, separator: String) -> List<String>` | Pieces between separators, empty pieces kept | O(n) |  |
| `fn split_once(self, separator: String) -> Option<(String, String)>` | Before and after the first separator | O(n) |  |
| `fn split_whitespace(self) -> List<String>` | Pieces between runs of Unicode whitespace, none empty | O(n) |  |
| `fn lines(self) -> List<String>` | Splits on `\n` and `\r\n`; no final empty line | O(n) |  |
| `fn trim(self) -> String` | Removes surrounding whitespace; also `trim_start`, `trim_end` | O(n) |  |
| `fn strip_prefix(self, prefix: String) -> Option<String>` | The rest, if it starts with `prefix`; also `strip_suffix` | O(n) |  |
| `fn replace(self, part: String, by: String) -> String` | Every occurrence; also `replace_first` | O(n) |  |
| `fn to_uppercase(self) -> String` | Also `to_lowercase`. Full Unicode mapping | O(n) |  |
| `fn repeat(self, times: Int) -> String` |  | O(n × times) | `requires times >= 0` |
| `fn pad_start(self, width: Int, fill: Char = ' ') -> String` | Adds `fill` until the string has `width` chars; also `pad_end` | O(n + width) | No change if already as wide |
| `fn concat(self, other: String) -> String` | Joins two strings; interpolation (`"{a}{b}"`) is the usual form | O(n + m) |  |
| `fn chars(self) -> Iter<Char>` | The characters | O(1), O(n) to consume |  |
| `fn bytes(self) -> Bytes` | The UTF-8 bytes | O(1) | Shares storage |
| `fn compare_ignore_case(self, other: String) -> Ordering` | Unicode case-folded comparison. Plain `String` `Ord` (`<`, `sort`) compares Unicode scalar values, which is the same as UTF-8 byte order, so `"B" < "a"`; use this for folded order. Also `fn case_fold(self) -> String`, the full Unicode case folding this comparison uses, O(n), for keys such as `names.sort_by_key(fn(s) { s.case_fold() })` | O(n + m) |  |

Parsing goes through the target type, never through `String`: `Int.parse(s)`, `Float.parse(s)`, `Url.parse(s)`. Strings are immutable (spec §3), so there is no push; build text in a List\<String> and join it with `parts.join(", ")` on `List<String>`.

### Bytes

| Signature | What | Cost | Notes |
| --- | --- | --- | --- |
| `Bytes.from_list(values: List<UInt8>) -> Bytes` |  | O(m) |  |
| `fn len(self) -> Int` | Also `is_empty` | O(1) |  |
| `fn get(self, index: Int) -> Option<UInt8>` | `b[i]` panics instead | O(1) |  |
| `fn slice(self, range: Range<Int>) -> Bytes` |  | O(1) | Shares storage. Panics: out of range |
| `fn concat(self, other: Bytes) -> Bytes` | Bytes are immutable, so this is how they grow | O(n + m) |  |
| `fn starts_with(self, prefix: Bytes) -> Bool` | Also `ends_with`, `find(part:) -> Option<Int>` | O(n) |  |
| `fn to_list(self) -> List<UInt8>` |  | O(n) |  |
| `fn iter(self) -> Iter<UInt8>` |  | O(1), O(n) to consume |  |

Hex and base64 live in `std.encoding`.

### List

`List<T>` is a growable array with value semantics. Every method that takes a closure is generic over an effect variable and passes it through, as in the first row; the rest of the table leaves the `<uses E>` and trailing `uses E` out to stay readable. There is one way to sort or reverse: the methods return a new list, and because a unique list is updated in place (spec §3), `xs = xs.sorted()` costs no copy.

| Signature | What | Cost | Notes |
| --- | --- | --- | --- |
| `fn map<U, uses E>(self, f: fn(T) -> U uses E) -> List<U> uses E` | Applies `f` to each item, in order | O(n) |  |
| `List.filled(value: T, count: Int) -> List<T>` | `count` copies of `value` | O(count) | `requires count >= 0` |
| `fn len(self) -> Int` | Also `is_empty` | O(1) |  |
| `fn get(self, index: Int) -> Option<T>` | `xs[i]` panics instead (spec §4) | O(1) |  |
| `fn first(self) -> Option<T>` | Also `last` | O(1) |  |
| `fn set(var self, index: Int, value: T)` | Replaces one item | O(1) when unique | `requires 0 <= index && index < self.len()` |
| `fn push(var self, item: T)` | Appends | O(1) amortized when unique |  |
| `fn pop(var self) -> Option<T>` | Removes the last item | O(1) when unique |  |
| `fn insert(var self, index: Int, item: T)` | Inserts before `index`; `index == len()` appends | O(n) | `requires 0 <= index && index <= self.len()` |
| `fn remove(var self, index: Int) -> T` | Removes and returns one item | O(n) | `requires 0 <= index && index < self.len()` |
| `fn extend(var self, items: List<T>)` | Adds every item of `items` | O(m) amortized |  |
| `fn concat(self, other: List<T>) -> List<T>` |  | O(n + m) |  |
| `fn slice(self, range: Range<Int>) -> List<T>` | The items in `range`, as in `xs.slice(1..3)` | O(range) | Panics: out of range, like `xs[i]` |
| `fn take(self, count: Int) -> List<T>` | At most `count` items from the front; also `skip`, `take_last` | O(count) | Never fails; clamps |
| `fn contains(self, item: T) -> Bool` where `T: Eq` | Also `index_of(item) -> Option<Int>` | O(n) |  |
| `fn find(self, pred: fn(T) -> Bool) -> Option<T>` | First match; also `find_index`, `find_last` | O(n) |  |
| `fn any(self, pred: fn(T) -> Bool) -> Bool` | Also `all`, `count(f) -> Int` | O(n) |  |
| `fn filter(self, keep: fn(T) -> Bool) -> List<T>` |  | O(n) |  |
| `fn filter_map<U>(self, f: fn(T) -> Option<U>) -> List<U>` | Maps and drops `None` | O(n) |  |
| `fn flat_map<U>(self, f: fn(T) -> List<U>) -> List<U>` |  | O(total) |  |
| `fn fold<A>(self, initial: A, f: fn(A, T) -> A) -> A` | Left to right | O(n) |  |
| `fn try_map<U, Err>(self, f: fn(T) -> Result<U, Err>) -> Result<List<U>, Err>` | Maps, stopping at the first `Err` | O(n) | The one `try_` name: it is the `Result` form of `map`, not a fallible twin |
| `fn partition(self, pred: fn(T) -> Bool) -> (List<T>, List<T>)` | Matching items, then the rest | O(n) |  |
| `fn group_by<K: Hash>(self, key: fn(T) -> K) -> Map<K, List<T>>` | Groups in first-seen order | O(n) |  |
| `fn zip<U>(self, other: List<U>) -> List<(T, U)>` | Pairs, as long as the shorter list | O(min(n, m)) |  |
| `fn enumerate(self) -> List<(Int, T)>` | Pairs each item with its index | O(n) |  |
| `fn chunks(self, size: Int) -> List<List<T>>` | Last chunk may be shorter; also `windows(size)` | O(n) | `requires size >= 1`, on both |
| `fn sorted(self) -> List<T>` where `T: Ord` | Stable sort | O(n log n) |  |
| `fn sorted_by(self, f: fn(T, T) -> Ordering) -> List<T>` | Also `sorted_by_key<K: Ord>(key: fn(T) -> K)` | O(n log n) | Stable |
| `fn reversed(self) -> List<T>` |  | O(n) |  |
| `fn unique(self) -> List<T>` where `T: Hash` | Drops repeats, keeping first occurrences | O(n) |  |
| `fn min(self) -> Option<T>` where `T: Ord` | Also `max`, `min_by_key`, `max_by_key`. On ties, `min_by_key` and `max_by_key` return the first of the equal items | O(n) |  |
| `fn sum(self) -> Int` on `List<Int>`, `fn sum(self) -> Float` on `List<Float>` | Zero for an empty list | O(n) | Panics: overflow, for `Int` |
| `fn join(self, separator: String) -> String` on `List<String>` |  | O(total) |  |
| `fn to_set(self) -> Set<T>` where `T: Hash` |  | O(n) |  |
| `fn to_map<K, V>(self) -> Map<K, V>` on `List<(K, V)>` | Later pairs win | O(n) |  |
| `fn iter(self) -> Iter<T>` | Lazy view, for chains (see Iter) | O(1) |  |

### Map

`Map<K, V>` needs `K: Hash`. It iterates in insertion order (decision 4), and replacing a key's value keeps its place. There is no `m[k]` (spec §4).

| Signature | What | Cost | Notes |
| --- | --- | --- | --- |
| `Map.from(pairs: List<(K, V)>) -> Map<K, V>` | Later pairs win | O(m) | `{:}` is the empty map |
| `fn len(self) -> Int` | Also `is_empty` | O(1) |  |
| `fn get(self, key: K) -> Option<V>` | Also `contains_key` | O(1) average |  |
| `fn get_or(self, key: K, default: V) -> V` |  | O(1) average |  |
| `fn insert(var self, key: K, value: V) -> Option<V>` | Sets the value and returns the old one | O(1) average when unique | Ignore the result with `let _ =` |
| `fn update(var self, key: K, default: V, f: fn(V) -> V)` | `f` of the current value, or of `default` | O(1) average when unique | See the sample entry above |
| `fn remove(var self, key: K) -> Option<V>` |  | O(n) worst, O(1) average | Later keys keep their order |
| `fn keys(self) -> List<K>` | Also `values`, `entries() -> List<(K, V)>` | O(n) | Insertion order |
| `fn iter(self) -> Iter<(K, V)>` |  | O(1) |  |
| `fn map_values<W>(self, f: fn(V) -> W) -> Map<K, W>` |  | O(n) |  |
| `fn filter(self, keep: fn(K, V) -> Bool) -> Map<K, V>` |  | O(n) |  |
| `fn merge(self, other: Map<K, V>) -> Map<K, V>` | `other` wins on shared keys | O(n + m) |  |

### Set

`Set<T>` needs `T: Hash` and iterates in insertion order. It has no literal: `Set.from([1, 2, 3])`.

| Signature | What | Cost | Notes |
| --- | --- | --- | --- |
| `Set.from(items: List<T>) -> Set<T>` |  | O(m) |  |
| `fn len(self) -> Int` | Also `is_empty` | O(1) |  |
| `fn contains(self, item: T) -> Bool` |  | O(1) average |  |
| `fn insert(var self, item: T) -> Bool` | `true` if it was new | O(1) average when unique |  |
| `fn remove(var self, item: T) -> Bool` | `true` if it was there | O(1) average |  |
| `fn union(self, other: Set<T>) -> Set<T>` | Also `intersection`, `difference` | O(n + m) |  |
| `fn is_subset(self, other: Set<T>) -> Bool` | Also `is_disjoint` | O(n) |  |
| `fn filter(self, keep: fn(T) -> Bool) -> Set<T>` |  | O(n) |  |
| `fn to_list(self) -> List<T>` | Also `iter() -> Iter<T>` | O(n) |  |

### Option

There is no `unwrap()` (spec §4). `expect` states why the value must be there; `unwrap_or` supplies a fallback and can't panic. Every method here is O(1) apart from the closure it calls.

| Signature | What | Notes |
| --- | --- | --- |
| `fn is_some(self) -> Bool` | Also `is_none`. For one case with a binding, prefer `if x is Some(v)` (spec §4) |  |
| `fn expect(self, reason: String) -> T` | The value | Panics: `None`, with `reason` in the message |
| `fn unwrap_or(self, default: T) -> T` | The value, or `default` |  |
| `fn unwrap_or_else<uses E>(self, f: fn() -> T uses E) -> T uses E` | The value, or `f()`, called only on `None` |  |
| `fn map<U, uses E>(self, f: fn(T) -> U uses E) -> Option<U> uses E` |  |  |
| `fn and_then<U, uses E>(self, f: fn(T) -> Option<U> uses E) -> Option<U> uses E` |  |  |
| `fn filter<uses E>(self, keep: fn(T) -> Bool uses E) -> Option<T> uses E` |  |  |
| `fn or(self, other: Option<T>) -> Option<T>` | `self` if `Some`, else `other`; also `or_else(f)` |  |
| `fn ok_or<Err>(self, error: Err) -> Result<T, Err>` | Turns `None` into `Err(error)` (spec §4) |  |
| `fn zip<U>(self, other: Option<U>) -> Option<(T, U)>` | `Some` when both are |  |

### Result

| Signature | What | Notes |
| --- | --- | --- |
| `fn is_ok(self) -> Bool` | Also `is_err` |  |
| `fn expect(self, reason: String) -> T` | The value | Panics: `Err`, with `reason` and the error's debug form |
| `fn unwrap_or(self, default: T) -> T` | Also `unwrap_or_else<uses E>(self, f: fn(Err) -> T uses E) -> T uses E` |  |
| `fn map<U, uses E>(self, f: fn(T) -> U uses E) -> Result<U, Err> uses E` |  |  |
| `fn map_err<F, uses E>(self, f: fn(Err) -> F uses E) -> Result<T, F> uses E` | The usual way to convert errors in a `spawn` (spec §9) |  |
| `fn and_then<U, uses E>(self, f: fn(T) -> Result<U, Err> uses E) -> Result<U, Err> uses E` |  |  |
| `fn or_else<F, uses E>(self, f: fn(Err) -> Result<T, F> uses E) -> Result<T, F> uses E` | Recovers from an error |  |
| `fn ok(self) -> Option<T>` | Also `err() -> Option<Err>` | Drops the other side on purpose |

### Ordering

`enum Ordering { Less, Equal, Greater }` is what `Ord.compare` returns. It is a prelude name, and these are its only methods (spec appendix).

| Signature | What |
| --- | --- |
| `fn reversed(self) -> Ordering` | Swaps `Less` and `Greater`, for descending sorts |
| `fn then(self, next: Ordering) -> Ordering` | `self` unless it is `Equal`, for sorting by several keys |

### Iter

`Iter<T, uses E>` is a concrete prelude type that implements `Iterator<uses E>` with `Item = T`, and carries every adapter as an ordinary method (decision 3). The spec forbids default method bodies on interfaces, so this is how lazy chains work: `xs.iter()`, `map.iter()`, `s.chars()` and `Iter.from(it)` give an `Iter`, and a chain ends in a consumer such as `to_list()`.

```
let names = users.iter()
  .filter(fn(u) { u.active })
  .map(fn(u) { u.name })
  .take(10)
  .to_list()
```

A line that starts with `.` continues the expression above (spec §2), so a long chain can break before each dot, the way Rust and TypeScript code usually does.

Adapters are lazy and O(1) to build; nothing runs until a consumer or a `for` loop pulls items. An adapter whose closure has effects `F` returns `Iter<U, uses E, F>`, so the chain's effects add up the way spec §5 describes, and the consumer `uses` all of them. Lists already have eager `map` and `filter`; use them for one step, and an `Iter` for a chain or for input that isn't a list.

| Signature | What | Cost | Notes |
| --- | --- | --- | --- |
| `Iter.from<I: Iterator<uses E>>(it: I) -> Iter<I.Item, uses E>` | Wraps any iterator | O(1) |  |
| `fn next(var self) -> Option<T> uses E` | From `Iterator` | O(1) plus the work of each adapter | Waits only if the source does |
| `fn map<U, uses F>(self, f: fn(T) -> U uses F) -> Iter<U, uses E, F>` |  | O(1) per item | Adapter |
| `fn filter<uses F>(self, keep: fn(T) -> Bool uses F) -> Iter<T, uses E, F>` | Also `filter_map`, `flat_map` | O(1) per item | Adapter |
| `fn take(self, count: Int) -> Iter<T, uses E>` | Also `skip`, `step_by` | O(1) per item | Adapter. `step_by(self, step: Int) -> Iter<T, uses E>  requires step >= 1` |
| `fn take_while<uses F>(self, pred: fn(T) -> Bool uses F) -> Iter<T, uses E, F>` | Also `skip_while` | O(1) per item | Adapter |
| `fn enumerate(self) -> Iter<(Int, T), uses E>` |  | O(1) per item | Adapter |
| `fn zip<U, uses F>(self, other: Iter<U, uses F>) -> Iter<(T, U), uses E, F>` | Stops at the shorter | O(1) per item | Adapter |
| `fn chain<uses F>(self, other: Iter<T, uses F>) -> Iter<T, uses E, F>` | `self`, then `other` | O(1) per item | Adapter |
| `fn chunks(self, size: Int) -> Iter<List<T>, uses E>` |  | O(size) per chunk | Adapter. `requires size >= 1` |
| `fn to_list(self) -> List<T> uses E` | Also `to_set`, and `to_map` on pairs | O(n) | Consumer |
| `fn fold<A, uses F>(self, initial: A, f: fn(A, T) -> A uses F) -> A uses E, F` |  | O(n) | Consumer |
| `fn count(self) -> Int uses E` |  | O(n) | Consumer |
| `fn find<uses F>(self, pred: fn(T) -> Bool uses F) -> Option<T> uses E, F` | Also `any`, `all`, `position` | O(n) worst; stops early | Consumer |
| `fn first(self) -> Option<T> uses E` | Also `last` | O(1) / O(n) | Consumer |
| `fn min(self) -> Option<T> uses E` where `T: Ord` | Also `max`, `min_by_key`, `max_by_key`. On ties, `min_by_key` and `max_by_key` return the first of the equal items | O(n) | Consumer |
| `fn sum(self) -> Int uses E` on `Iter<Int>`, `fn sum(self) -> Float uses E` on `Iter<Float>` |  | O(n) | Consumer. Panics: integer overflow |
| `fn join(self, separator: String) -> String uses E` on strings |  | O(total) | Consumer |

There is no `for_each`: a `for` loop does that. A range such as `0..n` is a `Range<Int>` (spec §2, Ranges), which implements `Iterator` and has `.iter()`.

### Duration and Timestamp

These are the values of the `dur` and `ts` literals. Both are pure; reading the current time is `Clock.now()`. Every method here is O(1). Arithmetic is done only with operators (spec appendix): `impl Add<Duration> for Duration`, `Sub<Duration>`, `Mul<Int>` and `Div<Int>` (which panics on 0) give a `Duration`; `impl Add<Duration> for Timestamp` and `Sub<Duration>` give a `Timestamp`, and `Sub<Timestamp>` gives a `Duration`. So `started + dur"5s"`, `timeout * 2` and `t - started` work, and there are no `plus`, `minus`, `times` or `div` methods. Overflow and out-of-range results panic. `since` stays as a method. A `Duration` displays in `dur` literal syntax, largest unit first, leaving out zero units: `30s`, `1h30m`, `1s500ms`, `250us`, with `0s` for zero and a leading `-` when negative. `Duration.parse` reads that text back.

| Signature | What | Notes |
| --- | --- | --- |
| `Duration.seconds(n: Int) -> Duration` | Also `nanoseconds`, `microseconds`, `milliseconds`, `minutes`, `hours`, `days` | Panics: overflow past about 292 years |
| `Duration.parse(text: String) -> Result<Duration, InvalidFormat>` | The `dur` literal's syntax at runtime, as in `"1h30m"` | For config files; literals are checked at compile time |
| `fn to_milliseconds(self) -> Int` | Whole milliseconds, truncated |  |
| `fn to_seconds(self) -> Float` |  |  |
| `fn is_zero(self) -> Bool` | Also `is_negative` |  |
| `Timestamp.from_unix_seconds(n: Int) -> Timestamp` | Also `from_unix_milliseconds` |  |
| `Timestamp.parse(text: String) -> Result<Timestamp, InvalidFormat>` | RFC 3339, as in `"2026-11-10T13:20:04Z"` | Other formats are in `std.time` |
| `fn to_unix_seconds(self) -> Int` | Also `to_unix_milliseconds` |  |
| `fn since(self, earlier: Timestamp) -> Duration` | Negative if `earlier` is later |  |
| `fn to_rfc3339(self) -> String` | UTC with a `Z`; equals `display()` | Also the wire form for `Encode` and `Decode` |

### Url

| Signature | What | Cost | Notes |
| --- | --- | --- | --- |
| `Url.parse(text: String) -> Result<Url, InvalidFormat>` | WHATWG URL parsing; `url"..."` checks at compile time. `display()` gives the WHATWG serialized form, so `url"https://a.example"` displays as `https://a.example/` and a trailing slash in the source is kept | O(m) | Fails: `InvalidFormat` |
| `fn scheme(self) -> String` | Also `path`, and `host`, `query`, `fragment`, which return `Option<String>` | O(1) |  |
| `fn port(self) -> Option<Int>` | The explicit port, if the URL has one | O(1) |  |
| `fn query_params(self) -> List<(String, String)>` | Decoded pairs in order | O(n) |  |
| `fn join(self, path: String) -> Result<Url, InvalidFormat>` | Resolves a relative reference | O(n + m) |  |
| `fn with_path(self, path: String) -> Url` | Also `with_query(params: List<(String, String)>)` | O(n) | Encodes as needed |

### Regex

`Regex` uses a linear-time engine, so matching is O(n × pattern size) for every pattern and input. There are no backreferences or lookaround, so no pattern can hang on hostile input.

`Regex` is in the prelude because `re"..."` produces it. `Match` and `Captures` are imported from `std.regex` when code names them; calling `find` or `captures` needs no import.

| Signature | What | Cost | Notes |
| --- | --- | --- | --- |
| `Regex.new(pattern: String) -> Result<Regex, InvalidFormat>` | For patterns built at runtime; `re"..."` checks at compile time | O(pattern) | Fails: `InvalidFormat` |
| `fn is_match(self, text: String) -> Bool` | Matches anywhere; anchor with `^` and `$` | O(n) |  |
| `fn find(self, text: String) -> Option<Match>` | Also `find_all -> List<Match>` | O(n) | `Match { start: Int, end: Int, text: String }` |
| `fn captures(self, text: String) -> Option<Captures>` | First match with its groups | O(n) | `Captures.get(index) -> Option<String>`, `Captures.named(name) -> Option<String>` |
| `fn replace_all(self, text: String, by: String) -> String` | `$1` and `${name}` refer to groups; also `replace_first` | O(n) |  |
| `fn split(self, text: String) -> List<String>` |  | O(n) |  |

### Prelude interfaces

```
pub interface Eq { fn eq(self, other: Self) -> Bool }
pub interface Ord { fn compare(self, other: Self) -> Ordering }      // implementors also implement Eq
pub interface Hash { fn hash(self, hasher: Hasher) -> Hasher }
pub interface Display { fn display(self) -> String }
pub interface From<T> { fn from(value: T) -> Self }
pub interface Iterator<uses E> {
  type Item
  fn next(var self) -> Option<Self.Item> uses E
}
pub interface Encode { fn encode(self) -> encoding.Data }
pub interface Decode { fn decode(data: encoding.Data) -> Result<Self, DecodeError> }
```

The operator interfaces `Add`, `Sub`, `Mul`, `Div`, `Rem` and `Neg` (spec §3) are prelude names too, because the compiler maps `+`, `-`, `*`, `/`, `%` and unary `-` onto them. That makes 14 prelude interfaces. Their signatures are in spec §3. `Duration` and `Timestamp` implement `Add` and `Sub` (see their section).

- `Eq`, `Hash`, `Encode` and `Decode` are automatic for most types (spec §3), so an impl is written only to replace the automatic one. `Ord` and `Display` are always written by hand for your own types. Built in: every number type, `Bool`, `Char`, `String`, `Bytes`, `Duration` and `Timestamp` implement `Eq`, `Ord` and `Hash`; `List` and tuples implement them when their elements do, and order element by element (spec appendix, Interfaces).
- `Hash` threads a `Hasher` value through, because a `var` parameter is the callee's own copy (spec §2). `Hasher` has two methods, `fn add<T: Hash>(self, value: T) -> Hasher` and `fn add_bytes(self, bytes: Bytes) -> Hasher` (`hasher.add(self.id)`), and is otherwise opaque. Hash seeds are fixed per build, and `Map` order never depends on them (decision 4).
- `From` is what `?` uses to convert errors (spec §4). Write one impl per source error type.
- `Encode` and `Decode` go through the data model in `std.encoding`; formats such as `std.json` sit on top. Most code never names `Data`.
- `Ord` needs `Eq` too, so a type that implements `Ord` writes both impls. Several bounds are joined with `+`, as in `T: Ord + Display` (spec §3, Generics).

### Core functions

| Signature | What | Notes |
| --- | --- | --- |
| `fn assert(condition: Bool, message: String = "")` | Panics when `condition` is false | A failed `assert(a == b)` prints both sides (spec §6), so there is no `assert_eq` |
| `fn todo(reason: String) -> Never` | Marks unfinished code | Panics. `quince publish` refuses a library that contains `todo` or `dbg` (spec §7) |
| `fn unreachable(reason: String) -> Never` | Marks a case the author has proved can't happen | Panics |
| `fn dbg<T>(value: T) -> T` | Prints the debug form and source location to standard error, returns `value` | Not an effect; rejected in release builds (spec §3) |
| `fn with_timeout<T, uses E>(limit: Duration, body: fn() -> T uses E) -> Result<T, TimedOut> uses E, Clock` | Cancels `body` after `limit` (spec §9) | Waits. Needs `Clock` because it reads a timer |
| `fn with_resource<T, R, uses E>(resource: Resource<T>, body: fn(Resource<T>) -> R uses E) -> R uses E` | Lends an open resource to `body`, then cleans it up, even on panic or cancellation (spec §11) |  |

The first four are the prelude's only free functions, and the only functions called bare. The other two are called through their packages: `time.with_timeout` (its error `TimedOut` is imported from `std.time`) and `ffi.with_resource` (with `Resource` from `std.ffi`); they are listed here because they pair with the spec's cancellation and cleanup rules. `expect` is a method on `Option` and `Result`. `Never` is the type of an expression that doesn't return (v0.3 plan item 6).

### Prelude error types

| Type | From | When | Fields |
| --- | --- | --- | --- |
| `InvalidFormat` | Prelude | Every prelude parse: `Int.parse`, `Float.parse`, `Duration.parse`, `Timestamp.parse`, `Url.parse`, `Regex.new`, `String.from_utf8` | `expected: String`, `found: String` |
| `DecodeError` | Prelude | Any `Decode` failure, in every format | `path: String`, `expected: String`, `found: String`. Syntax errors use the input position as `path`, as in `line 3, column 7` |
| `ScopeFailed` | Prelude | A `background` block failed (spec §9) | `spawn_site: String`, `message: String` |
| `TimedOut` | `std.time` | `with_timeout` ran out | `after: Duration` |
| `ChannelClosed` | `std.sync` | `send` on a closed channel | none |
| `FsError`, `NetError`, `ProcessError` | `std.fs`, `std.net`, `std.process` | Built-in effect operations | Listed in those packages |

Every error type implements `Display` with a one-line message an agent can show the user unchanged, so `"{error}"` always works. Examples:

- `InvalidFormat`: `invalid format: expected a digit at byte 3`
- `DecodeError`: `$.items[3].price: expected Int, found String`
- `FsError`: `not found: app.toml`, `permission denied: /etc/shadow`
- `NetError`: `connection refused`, `name not found: api.example.com`
- `ProcessError`: `program not found: git`
- `HttpError`: `timed out`, `network: connection refused`, `decode: $.id: expected Int, found String`
- `ServeError`: `can't bind: permission denied`
- `Status` (not an error, but often printed): `404 Not Found`

## Built-in effect operations

Each built-in effect's operations are called through the effect's name (decision 1), and a handler installed with `with` replaces exactly these operations. The spec's prelude appendix holds their exact signatures; where this section and the appendix ever differ, the appendix wins. The types they name (`FsError`, `NetError`, `ProcessError`, `ProcessOutput`, `Metadata`, `Outbound`, `Inbound`, `Listener`) live in the effect's std package. Calling an operation needs no import; naming one of these types, or matching an error's variants, imports it. Every operation is a cancellation point (spec §9).

A handler's methods may use `State` and the effect they handle, besides the effects that effect is built on (spec §3). A call to the handled effect from inside the handler goes to the handler that was in effect where the `with` installed it, never back to itself. That is how `net.only` and `fs.only` pass allowed calls through and how `testing.fixed_clock` and `testing.capture_log` keep their state, and a hand-written fake can do the same. Callers still see only the effect, such as `uses Clock`.

### Console

| Operation | What | Notes |
| --- | --- | --- |
| `print(text: String)` | Writes a line to standard output | Also `write`, without the newline. A write to a closed standard output ends the program quietly with code 0, as Unix tools do on a broken pipe (spec appendix) |
| `eprint(text: String)` | Writes a line to standard error |  |
| `read_line() -> Option<String>` | One line without its newline; `None` at end of input | Waits |
| `read_all() -> Bytes` | The rest of standard input | Waits |
| `is_terminal() -> Bool` | Whether standard output is a terminal, for colors and progress bars |  |

### Fs

Paths are plain `String`s, so `Fs.read_text("app.toml")` needs no import. `std.fs.Path` helps build and take apart paths, and its `to_string()` gives the `String` these operations take. Every operation that can fail returns `Result<_, FsError>`.

| Operation | What | Notes |
| --- | --- | --- |
| `read(path)` | Whole file as `Bytes`; also `read_text` | `read_text` fails with `FsError.InvalidUtf8` on bad UTF-8 |
| `read_range(path, offset: Int, length: Int)` | Some bytes of a file, without reading the rest |  |
| `read_lines(path)` | `Result<Iter<Result<String, FsError>, uses Fs>, FsError>`: opening can fail, and so can each line (a read error or invalid UTF-8 midway), so a loop writes `for r in lines { let line = r? ... }` | Each `next()` is an Fs call |
| `write(path, data: Bytes)` | Creates or replaces the file | Atomic: written to a temporary file, then renamed |
| `append(path, data: Bytes)` |  |  |
| `exists(path) -> Result<Bool, FsError>` |  | Fails on permission errors instead of guessing |
| `metadata(path) -> Result<Metadata, FsError>` | Size, kind, modified time |  |
| `list_dir(path)` | Entries sorted by name, so output is the same on every OS; also `walk` for every file below |  |
| `create_dir(path)` | Creates missing parents too | Succeeds if it already exists |
| `remove(path)` | A file or an empty directory |  |
| `rename(path, to: String)` | Also `copy(path, to:)` | `rename` is atomic within one file system |
| `temp_dir() -> String` |  | The current directory is `Env.current_dir()` |

### Net and Listen

| Effect | Operation | What | Notes |
| --- | --- | --- | --- |
| Net | `connect(host: String, port: Int, tls: Bool = false) -> Result<Outbound, NetError>` | Opens a connection; with `tls: true` it checks the host's certificate against the system roots | Waits |
| Net | `send(conn: Outbound, data: Bytes) -> Result<Unit, NetError>`, `receive(conn: Outbound, max: Int) -> Result<Bytes, NetError>`, `close(conn: Outbound)` | Traffic on an outbound connection; `receive` gives empty `Bytes` once the peer closes | `send` and `receive` wait |
| Listen | `bind(host: String, port: Int) -> Result<Listener, NetError>` | A listening socket; port 0 picks a free one |  |
| Listen | `accept(listener: Listener) -> Result<Inbound, NetError>` | The next connection | Waits |
| Listen | `send`, `receive` and `close` on an `Inbound` | Traffic on an accepted connection | So a server needs only `uses Listen` |

Most code uses `std.http` instead and never calls these directly. `Outbound`, `Inbound` and `Listener` are reference types (spec §3): copies share one connection, they can cross a task boundary, so a server can `accept` and then hand the connection to `spawn in bg handle(conn)`, and a connection closes with `close` or when its last reference is dropped. They are not `Resource<T>` values, which only `foreign` code makes. `std.testing` builds in-memory ones for fake handlers.

### Clock

| Operation | What | Notes |
| --- | --- | --- |
| `now() -> Timestamp` | Wall-clock time, UTC | Can jump; don't use it to measure |
| `monotonic() -> Duration` | Time since an arbitrary start that only moves forward | Measure with `Clock.monotonic() - started`, a `Duration` |
| `sleep(d: Duration)` | Waits | Under `quince` `test` the fake clock skips ahead instantly |

### Random

In production these draw from the operating system's secure generator, so they are safe for tokens and keys. Under `quince` `test` they are seeded (spec §6).

| Operation | What | Notes |
| --- | --- | --- |
| `int(range: Range<Int>) -> Int` | Uniform in `range`, as in `Random.int(1..7)` for a die | Panics: empty range |
| `float() -> Float` | Uniform in 0 up to but not including 1 |  |
| `bool() -> Bool` |  |  |
| `bytes(count: Int) -> Bytes` |  |  |

### Env

| Operation | What | Notes |
| --- | --- | --- |
| `get(name: String) -> Option<String>` | One environment variable |  |
| `vars() -> Map<String, String>` | All of them, sorted by name |  |
| `args() -> List<String>` | Command-line arguments, without the program name | Use `std.cli` to parse them |
| `program_name() -> String` |  |  |
| `current_dir() -> String` |  |  |

### Process

| Operation | What | Notes |
| --- | --- | --- |
| `run(program: String, args: List<String>, input: Bytes = b"") -> Result<ProcessOutput, ProcessError>` | Runs a program to completion and captures its output. Arguments go as a list, never through a shell | Waits. Fails: `ProcessError.NotFound(program:)`, `PermissionDenied(program:)` or `Other(message:)`. A nonzero exit is not an error: check `output.status` |
| `exit(code: Int) -> Never` | Cancels every scope, waits for cleanup, exits (spec §5) |  |

```
let out = Process.run("git", args: ["rev-parse", "HEAD"])?
```

### State

`State` has no operations of its own. It is the effect that the methods of reference types carry (next section).

## std.sync: reference types and concurrency

`Ref`, `Atomic`, `ConcurrentMap`, `Channel`, `Pool` and `ChannelClosed` are imported from `std.sync`, so a file that shares state says so in its imports: `import std.sync.{Atomic, Channel}`. They are the only types with shared identity (spec §3), apart from `TaskScope` (prelude, below) and `Resource` (`std.ffi`). Creating one is pure; reading or writing one needs `uses State`. Closures passed to `modify` and `update` must be pure, and the change is applied only after they return (spec §9).

### Ref, Atomic and ConcurrentMap

| Signature | What | Cost | Notes |
| --- | --- | --- | --- |
| `Ref.new(value: T) -> Ref<T>` |  | O(1) | Pure. Can't cross a task |
| `fn get(self) -> T uses State` | Also `set(value: T)` | O(1) |  |
| `fn modify(self, f: fn(T) -> T) -> T uses State` | Applies `f` and returns the new value | O(1) plus `f` |  |
| `Atomic.new(value: T) -> Atomic<T>` |  | O(1) | Pure. Crosses tasks |
| `fn get(self) -> T uses State` | Also `set(value: T)` | O(1) |  |
| `fn swap(self, value: T) -> T uses State` | Sets and returns the old value | O(1) |  |
| `fn modify(self, f: fn(T) -> T) -> T uses State` | Applies `f` atomically and returns the new value | O(1) plus `f`; may rerun `f` under contention | `f` must be pure, so rerunning it is safe |
| `fn compare_and_set(self, expected: T, new: T) -> Bool uses State` where `T: Eq` |  | O(1) |  |
| `ConcurrentMap.new() -> ConcurrentMap<K, V>` |  | O(1) | Pure. Crosses tasks |
| `fn get(self, key: K) -> Option<V> uses State` | Also `contains_key` | O(1) average |  |
| `fn insert(self, key: K, value: V) -> Option<V> uses State` | Also `remove(key)` | O(1) average | No `var self`: the map is shared |
| `fn update(self, key: K, default: V, f: fn(V) -> V) -> V uses State` | Atomic per key; returns the new value | O(1) average |  |
| `fn len(self) -> Int uses State` |  | O(1) |  |
| `fn snapshot(self) -> Map<K, V> uses State` | A consistent copy, to iterate or return | O(n) |  |

A counter shared by tasks is an `Atomic<Int>`: `hits.modify(fn(n) { n + 1 })`.

### Channel

| Signature | What | Cost | Notes |
| --- | --- | --- | --- |
| `Channel.new(capacity: Int) -> Channel<T>` | Capacity 0 makes every send wait for a receiver | O(1) | Pure. `requires capacity >= 0` |
| `fn send(self, value: T) -> Result<Unit, ChannelClosed> uses State` | Waits while full | O(1) | Waits |
| `fn receive(self) -> Option<T> uses State` | `None` once closed and empty | O(1) | Waits |
| `fn close(self) uses State` | Values already sent can still be received | O(1) | Closing twice is a no-op |
| `fn is_closed(self) -> Bool uses State` | Also `len` | O(1) |  |
| `fn iter(self) -> Iter<T, uses State>` | Receives until closed, so `for job in jobs.iter()` drains it | O(1) | Each `next()` waits |

`select` (spec §9) takes `receive()` and `send(v)` arms only.

### Pool

| Signature | What | Cost | Notes |
| --- | --- | --- | --- |
| `Pool.new<T, Err, uses E>(max: Int, healthy: fn(T) -> Bool uses E, open: fn() -> Result<T, Err> uses E) -> Pool<T, Err, uses E>  requires max >= 1` | Opens values lazily, up to `max`. Every time a value comes back from `checkout`, the pool calls `healthy` on it and closes it if that returns `false`, so a dead connection is never handed out again. `healthy` is required, so every pool states its policy; `fn(_) { true }` keeps every value | O(1) | Pure until used. Crosses tasks even when `T` can't (spec §3) |
| `fn checkout<R, uses E2>(self, body: fn(T) -> R uses E2) -> Result<R, Err> uses E, E2, State` | Waits for a free value, lends it, returns it | O(1) plus `body` | Waits. Fails only when `open` fails. If `body` panics or is cancelled the value is closed and replaced (spec §11). Closing a value, here or when `healthy` rejects it, means the pool drops it: any `Resource` inside it closes at that point, and v1 has no close hook |
| `fn size(self) -> Int uses State` | Also `idle` | O(1) |  |

`Pool` names the error type and effects of `open`, because `checkout` may open a new value: `PgDb { pool: Pool<PgConn, PgError, uses Net> }`. `checkout` returns `Err` only when opening fails.

### TaskScope and FailurePolicy (prelude)

`TaskScope` has no methods: it is created by `background` and used only by `spawn in bg` (spec §9). `FailurePolicy` is `enum FailurePolicy { LogAndContinue, Restart(max: Int), FailScope }`.

### Resource (std.ffi)

`fn keep<O: CMemory>(self, owned: O)` ties the lifetime of a value the C side still points at to the resource. `CMemory` is a `std.ffi` interface with no methods, implemented by `Callback<F>`, `CBuf<T>` and `CString`, so one method takes all three and nothing is overloaded. `keep` moves the value into the resource and returns `Unit`. It needs no effect, costs O(1), and may be called any number of times: every kept value stays alive until the resource is closed or released, and is released after the resource's own cleanup runs, in reverse order of keeping. Keeping a second value never frees the first, since C may still hold the old pointer. Panics: the resource is already closed. Like `adopt` and `ptr`, it is only available in the package that declares the foreign type (spec §11).

## System packages

The effect packages hold the types that effect operations name, their error enums, and handlers that narrow an effect. Test fakes for every effect are in `std.testing`.

### std.console

| Name | What |
| --- | --- |
| `fn discard() -> Discard` | A `Console` handler that drops all output and reads end of input, for running a noisy library quietly: `with Console = console.discard() { ... }` |

### std.env

| Signature | What | Notes |
| --- | --- | --- |
| `fn decode<T: Decode>(prefix: String) -> Result<T, DecodeError> uses Env` | Builds a config struct from variables named `PREFIX_FIELD`: the prefix, an underscore, then the field name in upper snake case. With `env.decode<Config>("APP")`, field `port` reads `APP_PORT` and `database_url` reads `APP_DATABASE_URL`. A nested struct adds its field's name the same way (`APP_DB_HOST`). A missing variable gives `None` for an `Option` field and an error for any other field; a variable set to the empty string is read as `""`. An enum is read by its exact wire name, case-sensitive, so `APP_LOG_LEVEL=info` gives `Level.Info` and `APP_LOG_LEVEL=INFO` fails | Values are read as their field types, so `APP_PORT=80x` fails with a path naming `APP_PORT` |

### std.cli

`std.cli` turns command-line arguments into a struct, through the same `Decode` the formats use. Fields become `--kebab-case` flags, `Bool` fields are switches, `Option` fields are optional, `List` fields repeat, and a field named `args: List<String>` collects positional arguments. That field is the only way to take positionals, so a command with one required file checks `args.len() == 1` in code and returns its own error otherwise. An enum of structs gives subcommands. Field doc comments become the `--help` text.

```
/// Fetches every URL in a file.
pub struct Args {
  /// File with one URL per line.
  pub input: String,
  /// Requests in flight at once.
  pub limit: Option<Int>,
  pub verbose: Bool,
}

fn main() -> Result<Unit, AppError> uses * {
  let args = cli.parse_or_exit<Args>()
  ...
}
```

| Signature | What | Notes |
| --- | --- | --- |
| `fn parse<T: Decode>() -> Result<T, CliError> uses Env` | Parses `Env.args()` | Fails: `CliError.Help(text:)` when `--help` was given, `CliError.Invalid(message:, usage:)` |
| `fn parse_or_exit<T: Decode>() -> T uses Env, Console, Process` | Prints help or the error and usage, then exits with code 0 or 2 | The usual call in `main` |
| `fn parse_from<T: Decode>(args: List<String>) -> Result<T, CliError>` | Pure form, for tests. `args` takes the same form as `Env.args()`, without the program name: `cli.parse_from<Args>(["--top", "5", "data.csv"])` |  |

Accepted syntax, the same on every OS:

- A value follows its flag as `--top 5` or `--top=5`. A word after a flag that takes a value is that value when it is `-` followed by a digit, so `--top -1` sets `top` to `-1`. There are no short flags in v1, so every flag in a script is self-describing.
- A `Bool` field is a switch: `--verbose` sets it, and leaving it out leaves it `false`.
- `--` ends the flags; everything after it goes to `args`.
- Field types: `String`, the integer types, `Float`, `Duration` (`--wait 30s`), `Url`, a dataless enum written by its wire name (so `@wire(case: kebab)` gives `--sort line-count`), and `Option` or `List` of any of these.
- `--help` prints a usage line, then one line per flag with its type, default and doc comment.

```
pub enum CliError {
  Help(text: String),                      // --help was given; text is the full help
  Invalid(message: String, usage: String), // unknown flag, missing value or a value that doesn't decode
}
```

Defaults and order:

- A struct field has no default value in `std.cli`. To make a flag optional, give it an `Option` type and choose the default in code, as in `args.top.unwrap_or(10)`. Help marks `Option` fields as optional and shows no default.
- Positionals and flags may come in any order, so `report --top 5 data.csv` and `report data.csv --top 5` parse the same. After `--`, everything is positional.

### std.fs

`FsError` and `Metadata` are what `Fs` operations return; import them to name a type or match a variant: `import std.fs.{FsError}`.

| Name | What | Cost | Notes |
| --- | --- | --- | --- |
| `Path.new(text: String) -> Path` | Forward slashes on every OS; converted at the system boundary | O(m) | Pure. No file is touched |
| `fn join(self, part: String) -> Path` |  | O(n + m) | An absolute `part` replaces the path, as in every OS |
| `fn parent(self) -> Option<Path>` | Also `file_name`, `stem` and `extension`, which return `Option<String>` | O(n) |  |
| `fn with_extension(self, ext: String) -> Path` |  | O(n) |  |
| `fn is_absolute(self) -> Bool` |  | O(1) |  |
| `fn to_string(self) -> String` | Equals `display()`; what `Fs` operations take | O(n) |  |
| `pub struct Metadata { pub size: Int, pub is_dir: Bool, pub modified: Timestamp }` | From `Fs.metadata` |  | Use `is_dir`; there is no separate kind |
| `pub enum FsError { NotFound(path: String), AlreadyExists(path: String), DirectoryNotEmpty(path: String), InvalidUtf8(path: String), PermissionDenied(path: String), Other(path: String, message: String) }` | Every Fs operation's error, as declared in the spec appendix |  | `PermissionDenied(path:)` holds the path as the program passed it, and covers manifest limits too (spec §5). A refusal from a `[cmd.<name>]` limit displays with the manifest line that set it: `permission denied: /home/me/secrets.txt (quince.toml:12)` |
| `fn only(dir: String) -> FsOnly` | Handler that limits `Fs` to one directory tree: `with Fs = fs.only("./data") { ... }` |  | Other paths fail with `PermissionDenied` |
| `fn read_only() -> FsReadOnly` | Handler that refuses every write |  |  |

### std.process

The types `Process.run` returns.

| Name | What |
| --- | --- |
| `pub struct ProcessOutput { pub status: Int, pub stdout: Bytes, pub stderr: Bytes }` | `status` 0 means success; read text with `String.from_utf8(out.stdout)` |
| `pub enum ProcessError { NotFound(program: String), PermissionDenied(program: String), Other(message: String) }` | A nonzero exit is not an error; check `status` |

### std.regex

| Name | What |
| --- | --- |
| `struct Match { pub start: Int, pub end: Int, pub text: String }` | From `Regex.find` and `find_all` |
| `Captures` | From `Regex.captures`: `get(index) -> Option<String>`, `named(name) -> Option<String>` |

## Network packages

### std.net

The types `Net` and `Listen` operations return, and the narrowing handler. `Outbound`, `Inbound` and `Listener` are reference types: copies share one connection, they cross task boundaries, and a connection closes with `close` or when its last reference is dropped (spec §3).

| Name | What | Notes |
| --- | --- | --- |
| `Outbound` | A connection from `Net.connect` | Used with `Net.send`, `Net.receive`, `Net.close` |
| `Listener`, `Inbound` | A socket from `Listen.bind`, and a connection from `Listen.accept` | Used with `Listen.send`, `Listen.receive`, `Listen.close` |
| `pub enum NetError { Refused, TimedOut, Reset, NameNotFound(host: String), PermissionDenied(host: String), Other(message: String) }` | Every Net and Listen error, as declared in the spec appendix | `PermissionDenied(host:)` holds the host without the port, and covers manifest limits (spec §5). A refusal from a `[cmd.<name>]` limit displays with the manifest line that set it: `permission denied: api.example.com (quince.toml:14)` |
| `fn only(host: String, also: List<String> = []) -> NetOnly` | Handler that limits `Net` to the listed hosts: `with Net = net.only("api.example.com") { ... }` | Other hosts fail with `NetError.PermissionDenied` |

### std.http

One package for the client and the server. Requests and responses are plain values, a non-2xx status is a normal `Response` and not an error, and every client call has a timeout by default.

**Client**

| Signature | What | Notes |
| --- | --- | --- |
| `fn get(url: Url, timeout: Duration = dur"30s") -> Result<Response, HttpError> uses Net` |  | Waits. Follows up to 10 redirects. `timeout` covers the whole call and fails with `HttpError.Timeout`. It is independent of an enclosing `with_timeout`: whichever timer ends first wins, and the outer one gives `Err(TimedOut)` and cancels the call |
| `fn request(req: Request, timeout: Duration = dur"30s") -> Result<Response, HttpError> uses Net` | Any method | Waits |

```
let resp = http.request(Request.post(url"https://api.example.com/orders").with_json(order))?
if !resp.is_success() { return Err(SyncError.Rejected(status: resp.status)) }
let created = resp.json<Order>()?
```

**Request and Response**

| Signature | What | Notes |
| --- | --- | --- |
| `Request.get(url: Url) -> Request` | Each method has its own constructor: `Request.post(url: Url) -> Request`, `Request.put(url: Url) -> Request`, `Request.patch(url: Url) -> Request` and `Request.delete(url: Url) -> Request`. `Request.new(method: Method, url: Url) -> Request` takes any method. | Pure |
| `fn with_header(self, name: String, value: String) -> Request` | Adds a header |  |
| `fn with_body(self, body: Bytes, content_type: String) -> Request` | Also `with_text(text:)` |  |
| `fn with_json<T: Encode>(self, value: T) -> Request` | Encodes `value` and sets `Content-Type` |  |
| `fn with_form<T: Encode>(self, value: T) -> Request` | URL-encoded form body |  |
| `pub struct Request { pub method: Method, pub url: Url, pub headers: Headers, pub body: Bytes }` | Also what a server handler receives |  |
| `fn text(self) -> Result<String, HttpError>` | On `Request` and `Response`: the body as UTF-8 |  |
| `fn json<T: Decode>(self) -> Result<T, HttpError>` | On both `Request` and `Response`: `fn json<T: Decode>(self) -> Result<T, HttpError>`. It decodes the body and does not check `Content-Type`; a handler that cares checks `req.headers.get("content-type")` itself. Also `fn form<T: Decode>(self) -> Result<T, HttpError>` for form-encoded bodies, the same way. | Fails: `HttpError.Decode(error:)` |
| `fn query<T: Decode>(self) -> Result<T, HttpError>` | On `Request`: the query string as a struct |  |
| `pub struct Response { pub status: Status, pub headers: Headers, pub body: Bytes }` |  |  |
| `fn is_success(self) -> Bool` | Status 200 to 299 |  |
| `Response.with_text(status: Status, body: String) -> Response` | Also `Response.with_json<T: Encode>(status: Status, value: T) -> Response` (sets `Content-Type`), `Response.empty(status: Status) -> Response` and `Response.redirect(to: Url) -> Response` (302). They are named `with_` so they never clash with the `text()` and `json<T>()` methods | Pure |
| `pub struct Status(pub Int)` | Constants: `OK`, `CREATED`, `ACCEPTED`, `NO_CONTENT`, `MOVED_PERMANENTLY`, `FOUND`, `SEE_OTHER`, `NOT_MODIFIED`, `TEMPORARY_REDIRECT`, `PERMANENT_REDIRECT`, `BAD_REQUEST`, `UNAUTHORIZED`, `PAYMENT_REQUIRED`, `FORBIDDEN`, `NOT_FOUND`, `METHOD_NOT_ALLOWED`, `CONFLICT`, `GONE`, `PAYLOAD_TOO_LARGE`, `UNSUPPORTED_MEDIA_TYPE`, `UNPROCESSABLE_CONTENT`, `TOO_MANY_REQUESTS`, `INTERNAL_SERVER_ERROR`, `NOT_IMPLEMENTED`, `BAD_GATEWAY`, `SERVICE_UNAVAILABLE`, `GATEWAY_TIMEOUT`. Any other code is `Status(n)`. Implements `Display` as code and reason, as in `404 Not Found` | Associated constants (spec §3) |
| `enum Method { Get, Head, Post, Put, Patch, Delete, Options }` | Implements `Display` as its upper-case name: `"GET"`, `"HEAD"`, `"POST"`, `"PUT"`, `"PATCH"`, `"DELETE"`, `"OPTIONS"` |  |
| `Headers` | Case-insensitive, ordered: `get(name) -> Option<String>`, `get_all(name) -> List<String>`, `set(name, value:) -> Headers` |  |
| `enum HttpError { Net(error: NetError), Timeout, TooManyRedirects, BodyTooLarge, InvalidUtf8, Decode(error: DecodeError) }` | Client and body errors. Implements `Display` |  |

**Server**

| Signature | What | Notes |
| --- | --- | --- |
| `fn serve<uses E>(port: Int, handler: fn(Request) -> Response uses E, host: String = "0.0.0.0", max_body: Int = 10_000_000) -> Result<Unit, ServeError> uses Listen, E` | Serves until cancelled; each request runs in its own task inside `serve`'s scope | Waits. A handler panic answers 500 and is reported on standard error, and the server keeps running (open item 14) |
| `enum ServeError { Bind(error: NetError) }` | `serve` only fails at startup. Implements `Display`, so `"{error}"` works |  |
| `Router.new() -> Router` | An empty router. Its effect argument is left out, which means no effects, as for `Iterator` (spec §5). Its methods live in `impl<uses E> Router<uses E>`, and each route added widens `E` by the effects of its handler. Effect arguments are sets, so `Router.new().get(.., uses Db).post(.., uses Log)` has the type `Router<uses Db, Log>`, the same type as `Router<uses Log, Db>`. A `Router` is an immutable value, so `serve`'s handler can capture it when its closures capture only values that can cross tasks (spec §3). | Pure |
| `fn get<uses F>(self, pattern: String, handler: fn(Request, Params) -> Response uses F) -> Router<uses E, F>` | The other methods have the same shape as `get`: `fn post<uses F>(self, pattern: String, handler: fn(Request, Params) -> Response uses F) -> Router<uses E, F>`, and likewise `put`, `patch` and `delete`. `fn route<uses F>(self, method: Method, pattern: String, handler: fn(Request, Params) -> Response uses F) -> Router<uses E, F>` takes any method. | Patterns name parameters with a colon and the rest of the path with a star: `"/users/:id"`, `"/files/*rest"`. They need no escaping, and since they hold no braces they can never interpolate a binding. Matching works on `req.url.path()` one `/`-separated segment at a time, after percent-decoding. A literal segment matches only itself, `:name` matches any one non-empty segment (so `/users/abc` reaches the handler, which decides whether `abc` is valid), and `*rest` may only come last and matches the remaining segments, possibly none. Trailing slashes are not normalized: `/users/` does not match `"/users"`. When several patterns match, a literal segment beats `:name`, which beats `*rest`, whatever order the routes were added in. Adding the same method and pattern twice panics. Pure |
| `fn handle(self, req: Request) -> Response uses E` | Dispatches on `req.method` and `req.url.path()`; answers 404 or 405 when nothing matches |  |
| `Params` | `get(name) -> Option<String>` |  |

```
import std.http.{self, Router}

fn main() -> Result<Unit, AppError> uses * {
  let router = Router.new()
    .get("/users/:id", fn(_, p) { show_user(p) })
    .post("/users", fn(req, _) { create_user(req) })
  http.serve(8080, handler: fn(req) { router.handle(req) })?
  Ok(())
}
```

## Data and formats

Every format has the same three functions, so learning one teaches all of them. Encoding is pure and never fails, except that a NaN or infinite `Float` panics (spec §3). Decoding takes a `String` (`decode_bytes` takes `Bytes`), checks shapes and every `requires` invariant, and reports a `DecodeError` with a path such as `$.items[3].price`. Malformed text is a `DecodeError` too, with the path where parsing stopped, so one error type covers both.

| Package | `encode<T: Encode>(value: T) -> String` | `decode<T: Decode>(text: String) -> Result<T, DecodeError>` | Also |
| --- | --- | --- | --- |
| `std.json` | Compact JSON | Strict JSON: no comments, no trailing commas | `encode_pretty<T: Encode>(value: T) -> String`: one field or item per line, 2-space indent, `": "` after each key, no trailing newline, and `[]` or `{}` for an empty list or object. Both `encode` and `encode_pretty` write struct fields in declaration order and map entries in key order, so output can be compared in tests. Also `decode_bytes<T: Decode>(data: Bytes) -> Result<T, DecodeError>`; `Value` |
| `std.toml` | TOML; a top-level value must be a struct or map | TOML 1.0 |  |
| `std.form` | `a=1&b=x%20y`; lists repeat the key | Same | Nested structs are a `DecodeError` |

Cost is O(n) in the size of the text for every format.

```
let cfg = toml.decode<Config>(Fs.read_text("app.toml")?)?
let body = json.encode(Report(total:, items:))
```

### std.json Value

`Value` holds JSON whose shape isn't known ahead of time. Decode into it like any type: `json.decode<json.Value>(text)`.

| Name | What |
| --- | --- |
| `enum Value { Null, Bool(value: Bool), Int(value: Int), Float(value: Float), String(value: String), Array(items: List<Value>), Object(fields: Map<String, Value>) }` | A number without a fraction or exponent that fits `Int` decodes as `Int` |
| `fn get(self, key: String) -> Option<Value>` | Field of an object; `None` for other variants |
| `fn at(self, index: Int) -> Option<Value>` | Item of an array |
| `fn as_string(self) -> Option<String>` | Also `as_int`, `as_float`, `as_bool`, `as_array`, `as_object` |
| `fn to<T: Decode>(self) -> Result<T, DecodeError>` | Decodes part of a document into a typed value |

### std.encoding

The data model every format shares, and the byte encodings.

| Signature | What | Cost | Notes |
| --- | --- | --- | --- |
| `enum Data { Null, Bool(value: Bool), Int(value: Int), Float(value: Float), String(value: String), Bytes(value: Bytes), List(items: List<Data>), Record(fields: List<(String, Data)>), Variant(name: String, fields: List<(String, Data)>) }` | What `Encode` produces and `Decode` reads |  | Wire names, tags and `Option` rules (spec §3) are already applied |
| `pub interface Encoder { fn write(self, data: Data) -> String }` | What a format implements to encode |  |  |
| `pub interface Decoder { fn read(self, text: String) -> Result<Data, DecodeError> }` | What a format implements to decode |  |  |
| `fn base64_encode(data: Bytes, url_safe: Bool = false) -> String` | Standard alphabet with padding, or URL-safe without | O(n) |  |
| `fn base64_decode(text: String, url_safe: Bool = false) -> Result<Bytes, EncodingError>` |  | O(n) | Fails: `EncodingError { at: Int }` |
| `fn hex_encode(data: Bytes) -> String` | Lowercase | O(n) |  |
| `fn hex_decode(text: String) -> Result<Bytes, EncodingError>` | Either case | O(n) |  |
| `fn percent_encode(text: String) -> String` | For one URL component | O(n) |  |
| `fn percent_decode(text: String) -> Result<String, EncodingError>` |  | O(n) | Fails on bad escapes or invalid UTF-8 |

A new format is a package that implements `Encoder` and `Decoder` and offers the same `encode` and `decode` functions; every type with `Encode` and `Decode` works with it at once. The tree form of `Data` is the API; an implementation may stream internally without changing it.

## Time, randomness and security

### std.time

`Timestamp` and `Duration` are in the prelude. This package adds calendar dates, times of day and zones. The time-zone database is built into the binary, so every function here is pure.

| Signature | What | Notes |
| --- | --- | --- |
| `Date.new(year: Int, month: Int, day: Int) -> Option<Date>` | `None` for a date that doesn't exist |  |
| `Date.parse(text: String) -> Result<Date, ParseTimeError>` | `YYYY-MM-DD` |  |
| `fn weekday(self) -> Weekday` | `enum Weekday { Monday, ..., Sunday }` |  |
| `fn plus_days(self, n: Int) -> Date` | Also `plus_months`, which clamps to the month's last day |  |
| `fn days_until(self, other: Date) -> Int` |  |  |
| `Time.new(hour: Int, minute: Int, second: Int = 0) -> Option<Time>` | A time of day |  |
| `Zone.named(name: String) -> Option<Zone>` | IANA names, as in `"Europe/Berlin"`; `Zone.UTC` |  |
| `fn in_zone(ts: Timestamp, zone: Zone) -> Local` | `Local { pub date: Date, pub time: Time, pub offset: Duration }` |  |
| `fn at(date: Date, time: Time, zone: Zone) -> Result<Timestamp, TimeError>` |  | Fails: `TimeError.Skipped` in a DST gap, `TimeError.Ambiguous(first:, second:)` in a DST overlap, so the caller decides |
| `fn format(ts: Timestamp, pattern: String, zone: Zone = Zone.UTC) -> String` | strftime patterns: `%Y-%m-%d %H:%M` | Panics: unknown `%` code. See open item 9 for `Zone.UTC` as a default |
| `fn parse(text: String, pattern: String, zone: Zone = Zone.UTC) -> Result<Timestamp, ParseTimeError>` | Same patterns |  |

`std.time` also exports `with_timeout` and its error `TimedOut`, whose entries are under Core functions in the prelude section: `import std.time.{self, TimedOut}`, then `time.with_timeout(dur"5s", fn() { ... })`.

### std.random

Functions that draw from the `Random` effect, and a seeded generator for pure code that needs repeatable randomness, such as simulations.

| Signature | What | Cost | Notes |
| --- | --- | --- | --- |
| `fn shuffle<T>(items: List<T>) -> List<T> uses Random` | Uniform permutation | O(n) |  |
| `fn choose<T>(items: List<T>) -> Option<T> uses Random` | One item; `None` if empty | O(1) |  |
| `fn sample<T>(items: List<T>, count: Int) -> List<T> uses Random` | Without replacement | O(n) | `requires count <= items.len()` |
| `Rng.new(seed: Int) -> Rng` | Seeded generator | O(1) | Pure. Not for secrets |
| `fn int(var self, range: Range<Int>) -> Int` | Also `float`, `bool`, `shuffle(items)` | O(1) | Pure; advances the generator |

### std.uuid

| Signature | What | Notes |
| --- | --- | --- |
| `Uuid.v4() -> Uuid uses Random` | Random |  |
| `Uuid.v7() -> Uuid uses Random, Clock` | Time-ordered, good for database keys |  |
| `Uuid.parse(text: String) -> Result<Uuid, UuidError>` | Hyphenated form, either case |  |
| `fn to_string(self) -> String` | Lowercase, hyphenated; equals `display()` and the wire form |  |

### std.crypto

Only modern, misuse-resistant defaults, with no algorithm parameters to get wrong.

| Signature | What | Cost | Notes |
| --- | --- | --- | --- |
| `fn sha256(data: Bytes) -> Bytes` | Also `sha512` | O(n) |  |
| `fn hmac_sha256(key: Bytes, data: Bytes) -> Bytes` |  | O(n) |  |
| `fn constant_time_eq(a: Bytes, b: Bytes) -> Bool` | Compare secrets without timing leaks | O(n) |  |
| `fn random_token(bytes: Int = 32) -> String uses Random` | URL-safe base64 token | O(bytes) |  |
| `fn hash_password(password: String) -> String uses Random` | Argon2id with a fresh salt, as a PHC string | Slow on purpose, about 50 ms | Never call it in a contract |
| `fn verify_password(password: String, hash: String) -> Bool` |  | Slow on purpose | Pure. `false` for a malformed hash |
| `Key.generate() -> Key uses Random` | 256-bit key; also `Key.from_bytes(data) -> Option<Key>` | O(1) | `@opaque` |
| `fn encrypt(key: Key, plaintext: Bytes) -> Bytes uses Random` | XChaCha20-Poly1305 with a random nonce | O(n) |  |
| `fn decrypt(key: Key, ciphertext: Bytes) -> Result<Bytes, CryptoError>` |  | O(n) | Fails: `CryptoError.Invalid`, with no detail on purpose |

## Collections and iteration

### std.collections

All four are values, like `List` and `Map`: mutating methods take `var self` and update in place when unique.

| Signature | What | Cost | Notes |
| --- | --- | --- | --- |
| `Deque.new() -> Deque<T>` | Double-ended queue; also `Deque.from(items: List<T>)` | O(1) |  |
| `fn push_back(var self, item: T)` | Also `push_front` | O(1) amortized |  |
| `fn pop_front(var self) -> Option<T>` | Also `pop_back` | O(1) |  |
| `fn front(self) -> Option<T>` | Also `back`, `len`, `is_empty`, `iter` | O(1) |  |
| `SortedMap.new() -> SortedMap<K, V>` | Map ordered by key, `K: Ord` | O(1) | Same methods as `Map`, in key order |
| `fn first(self) -> Option<(K, V)>` | Also `last` | O(log n) |  |
| `fn range(self, from: K, to: K) -> Iter<(K, V)>` | Keys from `from` up to but not including `to` | O(log n) to start |  |
| `SortedSet.new() -> SortedSet<T>` | Set ordered by value; `first`, `last`, `range` as above | O(1) | Same methods as `Set` |
| `Heap.new() -> Heap<T>` | Max-heap, `T: Ord`; wrap items in `Reverse(x)` for a min-heap | O(1) |  |
| `fn push(var self, item: T)` |  | O(log n) |  |
| `fn pop(var self) -> Option<T>` | Removes the largest | O(log n) |  |
| `fn peek(self) -> Option<T>` | Also `len`, `is_empty`, `to_sorted_list` | O(1) |  |
| `struct Reverse<T>(T)` | Inverts `Ord` |  |  |

### std.iter

`Iter` and its adapters are in the prelude (decision 3). This package only builds new iterators from scratch.

| Signature | What | Notes |
| --- | --- | --- |
| `fn from_fn<T, uses E>(f: fn() -> Option<T> uses E) -> Iter<T, uses E>` | Calls `f` until it returns `None` | `f` can't hold state of its own; pair it with a `Ref` (and `uses State`) or use `successors` |
| `fn successors<T, uses E>(first: T, next: fn(T) -> Option<T> uses E) -> Iter<T, uses E>` | `first`, `next(first)`, and so on | The pure way to write a generator |
| `fn repeat<T>(item: T) -> Iter<T>` | Endless; pair with `take` |  |
| `fn once<T>(item: T) -> Iter<T>` | Also `empty<T>()` |  |

## Services, interop and testing

### std.log

```
pub effect Log uses Console, Clock {
  fn debug(message: String, fields: Map<String, String> = {:})
  fn info(message: String, fields: Map<String, String> = {:})
  fn warn(message: String, fields: Map<String, String> = {:})
  fn error(message: String, fields: Map<String, String> = {:})
}
```

Calls look like `Log.info("order placed", fields: {"order_id": id.to_string()})`. Logging never fails and never waits for a slow reader.

| Name | What | Notes |
| --- | --- | --- |
| `enum Level { Debug, Info, Warn, Error }` | Has `Ord` and `Decode`, so a config file can set it. Declared with `@wire(case: snake)`, so its wire names are `"debug"`, `"info"`, `"warn"` and `"error"` |  |
| `fn json(level: Level) -> JsonLog` | Handler: one JSON object per line on standard error, with `time`, `level`, `message` and the fields | Takes each line's time from `Clock`, so a test under `fixed_clock` gets fixed timestamps |
| `fn text(level: Level) -> TextLog` | Handler: `2026-11-10T13:20:04Z INFO order placed order_id=42` | For terminals |
| `fn discard() -> DiscardLog` | Handler that drops everything |  |

### std.ffi

Types for `foreign` blocks (spec §11). The fixed-width C types are aliases (`CInt` is `Int32`), so they may appear in `pub` signatures like any number type. The target-width types, `CString`, `Ptr`, `ConstPtr`, `CBuf`, `Callback` and raw foreign declarations can't appear in a `pub` signature: the package that declares the `foreign` block wraps them.

| Name | What |
| --- | --- |
| `CSChar`, `CUChar`, `CShort`, `CUShort`, `CInt`, `CUInt`, `CLongLong`, `CULongLong`, `CFloat`, `CDouble` | Aliases for the fixed-width Quince types they always match: `CInt` is `Int32`, `CUChar` is `UInt8`, `CFloat` is `Float32`, `CDouble` is `Float`, and so on. No conversion needed |
| `CChar`, `CLong`, `CULong`, `CSize`, `CSSize`, `CIntPtr` | Their own types, because their width or sign depends on the target. Built with `T.try_from(n: Int) -> Option<T>` (`None` when out of range), accept integer literals that fit in 32 bits, and are read with `to_int64()` (signed) or `to_uint64()` (unsigned) |
| `CString` | A NUL-terminated copy: `CString.try_from(s: String) -> Option<CString>` (`None` if `s` contains NUL), `fn to_string(self) -> Result<String, InvalidFormat>` (fails when the C bytes aren't UTF-8). Both pure. A `String` is never lent to C; it is always copied this way. A foreign fn returning `CString` is marked `@borrowed` or `@owned(free_fn)` |
| `Ptr<T>` | A C pointer: `Ptr.null()`, `fn is_null(self) -> Bool`, `fn read(self) -> T`, `fn write(self, value: T)` for C types declared in the block. Panics: `read` or `write` on a null pointer. A foreign fn returning a `Ptr<T>` whose `T` has no `@cleanup` is marked `@borrowed` or `@owned(free_fn)` |
| `ConstPtr<T>` | A pointer C may only read; `quince` `bind` emits it for `const T*`, and only such a parameter accepts a lent pointer. `fn lend(self) -> ConstPtr<UInt8>` on `Bytes` lends the bytes in place for one call; for empty `Bytes` it passes a valid non-null pointer to zero bytes |
| `CBuf<T>` | A reference type for C-allocated memory, the only way to pass an out-parameter. One value: `CBuf.new<T>() -> CBuf<T>`, `fn get(self) -> T`, `fn set(self, value: T)`. An array: `CBuf.alloc<T>(count: Int) -> CBuf<T>`, whose `get` and `set` reach element 0. Bytes: `CBuf.from_bytes(data: Bytes) -> CBuf<UInt8>` copies `data` in, and `fn to_bytes(self, len: Int) -> Bytes`, only on `CBuf<UInt8>`, copies the first `len` bytes out. `fn ptr(self) -> Ptr<T>` points at element 0, only as a foreign-call argument. New memory is zeroed. All pure. Can't cross tasks. Panics: `len` past the allocated count |
| `Callback<F>` | `Callback.new(f: F) -> Callback<F>`: captures must be crossable, and each call runs as its own task. `Callback.local(f: F) -> Callback<F>`: may capture a `Ref`, valid only during the creating task's own foreign calls. `fn unregister(self)`. All pure. Runs with the handlers of the call that registered it (spec §11) |

A lent pointer (`bytes.lend()`), `res.ptr()` and `buf.ptr()` may be written only as an argument of a foreign call, never stored in a variable or returned. The entries in this package follow the C interop decisions accepted into the spec on 2026-11-10 (options doc: https://claude.ai/artifact/3Dvm7kZ2KDPCfvkgBZ4gUD).

`std.ffi` also exports `Resource<T>` (see std.sync's Resource subsection) and `with_resource` (under Core functions, called as `ffi.with_resource`), since only code built on a `foreign` block opens resources. Every name in this package works only inside the package that declares the `foreign` block (spec §11), so a C library's raw types and pointers never leak into its callers' code.

### std.testing

`quince` `test` already fakes every effect (spec §6). These handlers let a test set up the fake or read what the code did.

| Signature | What | Notes |
| --- | --- | --- |
| `fn fixed_clock(at: Timestamp) -> FixedClock` | `Clock.now()` returns `at`; `sleep` moves the time forward without waiting, and whenever every task in the test is blocked the time jumps to the earliest pending deadline (spec §6). `Clock.monotonic()` starts at `Duration` zero and moves forward by exactly as much as the time does, under this and every other clock fake | `with Clock = testing.fixed_clock(ts"2026-01-01T00:00:00Z") { ... }` |
| `fn seeded_random(seed: Int) -> SeededRandom` | Repeatable `Random` | The default under `quince` `test` uses the run's seed |
| `fn memory_fs(files: Map<String, String> = {:}) -> MemoryFs` | An in-memory `Fs` holding the given text files. Its root is `/` and its current directory is `/`, so a relative key or path resolves against `/`. Keys and paths are normalized the same way (`.` segments, `..` segments and repeated slashes), so `"words.txt"`, `"./words.txt"` and `"/words.txt"` name one file | `fn read(self, path: String) -> Option<String> uses State` to check what was written |
| `fn capture_console(input: String = "") -> CapturedConsole` | Feeds `input` to standard input and records output | `fn stdout(self) -> String uses State`, also `stderr`. Each returns every byte written, newlines included, so `Console.print("a")` then `Console.print("b")` gives `"a\nb\n"` |
| `fn capture_log() -> CapturedLog` | A `Log` handler that records entries | `fn entries(self) -> List<LogEntry> uses State`, where `pub struct LogEntry { pub level: Level, pub message: String, pub fields: Map<String, String> }` and `Level` is `std.log.Level` |
| `fn fake_http<uses E>(handler: fn(Request) -> Result<Response, NetError> uses E) -> FakeNet<uses E>` | A `Net` handler that answers every outbound request by calling `handler`, with no real network. The handler gets the full `Request`: the absolute `url` with scheme and host, the method, the headers and the body. It returns `Ok(response)`, or an `Err` such as `NetError.Refused` or `NetError.Reset` to simulate a failed connection, which `http.get` reports as `HttpError.Net(error:)`. | The handler runs in the calling task, under the caller's handlers, so its effect calls (such as `Clock.sleep`) are cancellation points of the caller. A `timeout` given to `http.get` or `http.request` runs on the test's `Clock`. Fake time follows spec §6: whenever every task in the test is blocked, it jumps to the earliest pending deadline, so a timeout fires without real waiting even if nothing calls `sleep`. |
| `fn fake_env(vars: Map<String, String> = {:}, args: List<String> = []) -> FakeEnv` |  |  |
| `pub interface Generate { fn generate(gen: Gen) -> (Self, Gen) }` | Replaces the derived generator for `forall` (spec §6) | `Gen` is threaded like `Hasher`: each call takes the current state and returns the value with the next state, which the caller passes on, as in `let (name, gen) = gen.string(max_len: 20)` then `let (age, gen) = gen.int(0..120)`. Reusing an old `gen` gives the same value again |
| `Gen` | `fn int(self, range: Range<Int>) -> (Int, Gen)`, and likewise `float`, `bool`, `string(max_len:)`, `list<T: Generate>(max_len:)` and `choose(items)`: each takes `self` and returns the value with the next `Gen` | Values shrink automatically on failure |

```
import std.http.{Response, Status}
import std.log.{Level}
import std.testing

test "a failed charge is logged" {
  let captured = testing.capture_log()
  with Log = captured, Net = testing.fake_http(fn(_) { Ok(Response.empty(Status.PAYMENT_REQUIRED)) }) {
    assert(charge(order).is_err())
  }
  assert(captured.entries().any(fn(e) { e.level == Level.Error }))
}
```

In-memory connections for hand-written `Net` and `Listen` fakes: `fn connection_pair() -> (Outbound, Inbound)` gives two ends of one in-memory connection, where bytes sent on one end are received on the other, and `fn memory_listener() -> Listener` gives a listener with no socket, for a fake's `bind` to return. A fake's `send`, `receive` and `close` pass these to the outer handler (spec §3), which under `quince` `test` moves the bytes in memory.

## Name index

Every top-level name in the standard library, by package. Lowercase names are functions and are called through the package (`import std.json`, then `json.decode(text)`); the rest are types, interfaces, effects and constants, which may also be imported by name (spec §7). Methods are listed under their type's section.

| Import | Names |
| --- | --- |
| Prelude | Types: `Int`, `Int8` to `Int64`, `UInt8` to `UInt64`, `Float`, `Float32`, `Bool`, `Char`, `String`, `Bytes`, `List`, `Map`, `Set`, `Option`, `Result`, `Unit`, `Ordering`, `Iter`, `Range`, `Duration`, `Timestamp`, `Url`, `Regex`, `Hasher`, `Never`, `TaskScope`. Enums and variants: `Some`, `None`, `Ok`, `Err`, `FailurePolicy`. Interfaces: `Eq`, `Ord`, `Hash`, `Display`, `From`, `Iterator`, `Encode`, `Decode`, Add, Sub, Mul, Div, Rem, Neg. Functions: `assert`, `todo`, `unreachable`, `dbg`. Errors: `InvalidFormat`, `DecodeError`, `ScopeFailed` |
| Effects (no import) | `Console`, `Fs`, `Net`, `Listen`, `Clock`, `Random`, `Env`, `Process`, `State` |
| `std.sync` | `Ref`, `Atomic`, `ConcurrentMap`, `Channel`, `Pool`, `ChannelClosed` |
| `std.console` | `discard` |
| `std.env` | `decode` |
| `std.cli` | `parse`, `parse_or_exit`, `parse_from`, `CliError` |
| `std.fs` | `FsError`, `Metadata`, `Path`, `only`, `read_only` |
| `std.process` | `ProcessOutput`, `ProcessError` |
| `std.net` | `Outbound`, `Inbound`, `Listener`, `NetError`, `only` |
| `std.http` | `get`, `request`, `serve`, `Request`, `Response`, `Status`, `Method`, `Headers`, `Router`, `Params`, `HttpError`, `ServeError` |
| `std.json` | `encode`, `encode_pretty`, `decode`, `decode_bytes`, `Value` |
| `std.toml` | `encode`, `decode` |
| `std.form` | `encode`, `decode` |
| `std.encoding` | `Data`, `Encoder`, `Decoder`, `EncodingError`, `base64_encode`, `base64_decode`, `hex_encode`, `hex_decode`, `percent_encode`, `percent_decode` |
| `std.regex` | `Match`, `Captures` |
| `std.time` | `Date`, `Time`, `Weekday`, `Zone`, `Local`, `TimeError`, `in_zone`, `at`, `format`, `parse`, `with_timeout`, `TimedOut` |
| `std.random` | `shuffle`, `choose`, `sample`, `Rng` |
| `std.uuid` | `Uuid`, `UuidError` |
| `std.crypto` | `sha256`, `sha512`, `hmac_sha256`, `constant_time_eq`, `random_token`, `hash_password`, `verify_password`, `Key`, `encrypt`, `decrypt`, `CryptoError` |
| `std.collections` | `Deque`, `SortedMap`, `SortedSet`, `Heap`, `Reverse` |
| `std.iter` | `from_fn`, `successors`, `repeat`, `once`, `empty` |
| `std.log` | `Log`, `Level`, `json`, `text`, `discard` |
| `std.ffi` | `CSChar`, `CUChar`, `CShort`, `CUShort`, `CInt`, `CUInt`, `CLongLong`, `CULongLong`, `CFloat`, `CDouble`, `CChar`, `CLong`, `CULong`, `CSize`, `CSSize`, `CIntPtr`, `CString`, `Ptr`, `ConstPtr`, `CBuf`, `Callback`, `Resource`, `with_resource` |
| `std.testing` | `fixed_clock`, `seeded_random`, `memory_fs`, `capture_console`, `capture_log`, `fake_http`, `fake_env`, `connection_pair`, `memory_listener`, `Generate`, `Gen`, `LogEntry` |

Every name the spec's examples use is here: `json.decode`, `json.encode`, `log.json`, `net.only`, `testing.fixed_clock`, `Pool.new` (`std.sync`), `time.with_timeout`, `ffi.with_resource`, `http.serve`, and `Process.exit`. `config.load` is the program's own code (decision 7).

## Decisions

David approved the recommendations for 1 to 3 on 2026-11-10, and the doc is written to them. Items 4 to 8 use the stated default. Where the spec later settled something differently, the spec wins and the item says so.

1. **How built-in effect operations are called.** Decided: through the effect's name, the same way as package effects: `Fs.read_text(path)`, `Clock.now()`, `Console.print("hi")`, `Process.exit(1)`. The std package of the same name (`std.fs`) holds only types, the error enum and handlers, not wrapper functions. That gives one way to call any effect, `uses Fs` matches `Fs.` at the call, and `with Fs = ...` replaces exactly the operations listed. The spec uses this form throughout.
2. **How much goes in std.** Recommended: the batteries-included map above, now 22 packages. The lean alternative ships only the prelude, the effect packages and `std.json`, and leaves HTTP, crypto, UUIDs and CLI parsing to third-party packages. Lean is less to build, but agents do best when each common job has exactly one obvious package.
3. **Iterator adapters.** Spec §3 forbids default method bodies that call other methods, so `map`, `filter` and `take` can't be methods on every `Iterator`. Recommended: a concrete prelude type `Iter<T, uses E>` that carries the adapters as ordinary methods, reached by `xs.iter()` or `Iter.from(it)`, so chains read the way they do in Rust and TypeScript: `xs.iter().filter(...).map(...).to_list()`. The alternative is free functions in `std.iter` (`iter.map(it, f)`), which keeps the rule with no new type but breaks chaining.
4. **Map and Set order.** Default: iteration follows insertion order, as in Python and JavaScript. Order is then the same on every run, whatever the hash seed, which deterministic tests (spec §6) need. Lookups stay O(1) on average.
5. **Number formatting.** Interpolation takes only names (spec §2), so `{price}` can't say "two decimals". Done as prelude methods `f.to_fixed(2)` and `s.pad_start(8, fill: ' ')`, used through a binding.
6. **Prelude gaps.** Superseded by the spec. The prelude rule (2026-11-10) keeps only what the syntax or compiler needs: `From`, `DecodeError` and `ScopeFailed` are prelude; the reference types, `ChannelClosed`, `TimedOut` and `Resource` are imported from `std.sync`, `std.time` and `std.ffi`; `Generate` is in `std.testing`; `Encoder` and `Decoder` are in `std.encoding`. The spec appendix lists the prelude.
7. **`config.load` in spec §5.** Default: it is the program's own code, three lines over `toml.decode`, not a std function.
8. **Out of v1.** Database drivers, templating, compression, XML and YAML stay out (see the package map). Say if any of them should be in.

### Open items found while writing

All of these are now settled: items 9 to 11 in the spec's own rounds, and items 12 to 15 by David on 2026-11-10.

9. **Associated constants.** Settled in the spec (2026-11-10): `pub const NAME: T = ...` inside the type's `impl` block, read as `Int.MAX`, `Status.OK` or `Self.MAX` (never bare), and usable as a default argument such as `zone: Zone = Zone.UTC`. Interfaces can't declare constants in v1.
10. **Prelude names the spec's list leaves out.** Settled by the prelude rule (2026-11-10): `Ordering`, `Iter`, `Hasher` and `Never` are prelude; `Match` and `Captures` are imported from `std.regex`.
11. **Method chains across lines.** Settled by the grammar round (2026-11-10): a line that starts with `.` continues the expression above, as in Swift and Kotlin.
12. **Writing to a closed standard output.** Settled by the spec appendix, which wins over the earlier default here: `Console.write` returns `Unit`, and a write to a closed standard output ends the program quietly with code 0, as Unix tools do on a broken pipe.
13. **`Pool` type parameters.** Decided (David, 2026-11-10): `Pool<T, Err, uses E>`, carrying the error type and effects of `open`. The spec already uses this form.
14. **HTTP handler panics.** Decided (David, 2026-11-10): a panic in one request answers 500 and prints the usual panic report on standard error, as the runtime does for any task, without the server needing `uses Console`; the server keeps running.
15. **`Log` needs `Clock`.** Decided (David, 2026-11-10): `pub effect Log uses Console, Clock`, as spec §5 now declares.

## Signatures from the spec

Until the spec restructure (2026-11-10), these signatures and rules sat in the spec's Prelude appendix, which kept the signature of every prelude method and std type the spec used. The spec now keeps only what the language depends on, and these sections are the source for the rest until M9 merges them into the entries above.

### Conventions

Every function in this appendix that takes a closure declares an effect variable for it, except `modify` and `update` on reference types, which take pure closures (section 9), so passing an effectful closure is always allowed and the caller takes on its effects (section 5).

Each such section names its package, and code that names one of those types imports it, as in `import std.sync.{Ref}`.

- `expect` is a method on `Option` and `Result` (below), not a free function.
- `unwrap()` is not in the standard library. Use `expect("why this can't fail")`, which forces the author to state the assumption.

### Timeouts and resources (std.time, std.ffi)

```
pub fn with_timeout<T, uses E>(limit: Duration, body: fn() -> T uses E)
  -> Result<T, TimedOut>
  uses E, Clock
pub fn with_resource<T, R, uses E>(resource: Resource<T>, body: fn(Resource<T>) -> R uses E)
  -> R
  uses E
```

They are called through their packages: `time.with_timeout` from std.time, which needs `uses Clock` (section 9), and `ffi.with_resource` from std.ffi (section 11). Neither is in the prelude.

### Numbers, Bool and Char

For each integer type `I` (`Int8`…`Int64`, `UInt8`…`UInt64`):

```
impl I {
  fn parse(text: String, radix: Int = 10) -> Result<I, InvalidFormat>  requires radix >= 2 && radix <= 36
  fn checked_add(self, other: I) -> Option<I>     // also checked_sub, checked_mul, checked_div, checked_rem
  fn wrapping_add(self, other: I) -> I            // also wrapping_sub, wrapping_mul
  fn saturating_add(self, other: I) -> I          // clamps to MIN and MAX; also saturating_sub, saturating_mul
  fn div_euclid(self, other: I) -> I              // remainder never negative; also rem_euclid
  fn signum(self) -> I                            // -1, 0 or 1; signed types only
  fn pow(self, exponent: Int) -> I  requires exponent >= 0
  fn abs(self) -> I                               // signed types only; panics on I.MIN
  fn min(self, other: I) -> I
  fn max(self, other: I) -> I
  fn clamp(self, low: I, high: I) -> I  requires low <= high
  fn to_float(self) -> Float
  fn to_float32(self) -> Float32
  fn to_char(self) -> Option<Char>                // None for surrogates and values above 0x10FFFF
  fn to_string(self, radix: Int = 10) -> String  requires radix >= 2 && radix <= 36
}
```

- Every integer type has `to_<type>()` for every other integer type, such as `n.to_int64()` and `n.to_uint8()`. It returns the target type when every value fits, and `Option` when some don't.
- Associated constants: `I.MIN` and `I.MAX`.

```
impl Float {
  fn parse(text: String) -> Result<Float, InvalidFormat>
  fn abs(self) -> Float
  fn floor(self) -> Float
  fn ceil(self) -> Float
  fn round(self) -> Float                         // halves round away from zero
  fn trunc(self) -> Float
  fn sqrt(self) -> Float
  fn pow(self, exponent: Float) -> Float
  fn exp(self) -> Float
  fn ln(self) -> Float
  fn min(self, other: Float) -> Float             // total order (section 3)
  fn max(self, other: Float) -> Float
  fn clamp(self, low: Float, high: Float) -> Float  requires low <= high
  fn is_nan(self) -> Bool
  fn is_finite(self) -> Bool
  fn to_int(self) -> Option<Int>                  // truncates; None for NaN, infinities, out of range
  fn to_float32(self) -> Float32                  // rounds to nearest
  fn to_string(self) -> String                    // shortest text that reads back as the same value
  fn to_fixed(self, decimals: Int) -> String  requires decimals >= 0 && decimals <= 20   // rounds half to even
  fn is_infinite(self) -> Bool
  fn signum(self) -> Float
  fn is_close(self, other: Float, tolerance: Float = 1e-9) -> Bool   // relative, for tests and contracts
  fn atan2(self, x: Float) -> Float
  // also cbrt, log2, log10, sin, cos, tan, asin, acos, atan: each fn(self) -> Float, in radians
}
```

- Associated constants: `Float.PI`, `Float.E`, `Float.INFINITY`, `Float.NAN`, Float.EPSILON. `Float32` has the same methods.
- `Bool` has no methods; the operators cover it.

```
impl Char {
  fn from_code(code: UInt32) -> Option<Char>
  fn code(self) -> UInt32
  fn is_alphabetic(self) -> Bool
  fn is_numeric(self) -> Bool
  fn is_whitespace(self) -> Bool
  fn is_uppercase(self) -> Bool
  fn is_lowercase(self) -> Bool
  fn is_alphanumeric(self) -> Bool
  fn is_ascii(self) -> Bool
  fn to_digit(self, radix: Int = 10) -> Option<Int>  requires radix >= 2 && radix <= 36
  fn utf8_len(self) -> Int                        // 1 to 4
  fn to_uppercase(self) -> String                 // some characters uppercase to several
  fn to_lowercase(self) -> String
}
```

### Ranges

`a..b` and `a..=b` build a `Range<T>` over one integer type (section 2).

```
impl<T> Range<T> {                       // T is an integer type
  fn contains(self, value: T) -> Bool
  fn len(self) -> Int
  fn is_empty(self) -> Bool
  fn first(self) -> Option<T>
  fn last(self) -> Option<T>
  fn rev(self) -> Range<T>                // the same values, last to first
  fn step_by(self, step: Int) -> Range<T>  requires step >= 1
  fn iter(self) -> Iter<T>
  fn to_list(self) -> List<T>
}

impl<T> Iterator for Range<T> { type Item = T }
```

- A range is a start, an end and a step, so `rev` and `step_by` return another `Range` rather than a new type: `for i in (0..10).step_by(2).rev()` counts 8, 6, 4, 2, 0. `contains`, `len`, `first` and `last` take the step into account.
- `first` and `last` return `Option` because a range can be empty.

### String and Bytes

```
impl String {
  fn from_utf8(bytes: Bytes) -> Result<String, InvalidFormat>
  fn byte_len(self) -> Int
  fn char_count(self) -> Int
  fn is_empty(self) -> Bool
  fn contains(self, part: String) -> Bool
  fn starts_with(self, prefix: String) -> Bool
  fn ends_with(self, suffix: String) -> Bool
  fn find(self, part: String) -> Option<Int>      // byte offset of the first match
  fn find_last(self, part: String) -> Option<Int>
  fn split_once(self, separator: String) -> Option<(String, String)>
  fn slice(self, range: Range<Int>) -> Option<String>  // byte offsets; None off a char boundary or out of range
  fn strip_prefix(self, prefix: String) -> Option<String>
  fn strip_suffix(self, suffix: String) -> Option<String>
  fn split(self, separator: String) -> List<String>
  fn split_whitespace(self) -> List<String>
  fn lines(self) -> List<String>                 // splits on \n or \r\n; a trailing newline adds no empty line
  fn trim(self) -> String
  fn trim_start(self) -> String
  fn trim_end(self) -> String
  fn to_uppercase(self) -> String
  fn to_lowercase(self) -> String
  fn replace(self, part: String, by: String) -> String   // every match
  fn replace_first(self, part: String, by: String) -> String
  fn pad_start(self, width: Int, fill: Char = ' ') -> String   // pads to width chars; no change if already that wide
  fn pad_end(self, width: Int, fill: Char = ' ') -> String
  fn concat(self, other: String) -> String
  fn compare_ignore_case(self, other: String) -> Ordering
  fn repeat(self, times: Int) -> String  requires times >= 0
  fn chars(self) -> Iter<Char>
  fn bytes(self) -> Bytes
}
```

- `String` has no `len`. `s.len()` is a compile error whose suggested fixes are `byte_len()` and `char_count()`, so a length always says which unit it counts.
- `String` is immutable, so none of its methods take `var self`. Joining is `parts.join(", ")` on a `List<String>`.

```
impl Bytes {
  fn from_list(values: List<UInt8>) -> Bytes
  fn lend(self) -> ConstPtr<UInt8>    // std.ffi; only as a foreign-call argument (section 11)
  fn len(self) -> Int
  fn is_empty(self) -> Bool
  fn get(self, index: Int) -> Option<UInt8>
  fn slice(self, range: Range<Int>) -> Bytes      // panics out of bounds, like xs[i]
  fn concat(self, other: Bytes) -> Bytes
  fn starts_with(self, prefix: Bytes) -> Bool
  fn ends_with(self, suffix: Bytes) -> Bool
  fn find(self, part: Bytes) -> Option<Int>
  fn iter(self) -> Iter<UInt8>
  fn to_list(self) -> List<UInt8>
}
```

### List

```
impl<T> List<T> {
  fn filled(value: T, count: Int) -> List<T>  requires count >= 0
  fn len(self) -> Int
  fn is_empty(self) -> Bool
  fn get(self, index: Int) -> Option<T>
  fn first(self) -> Option<T>
  fn last(self) -> Option<T>
  fn slice(self, range: Range<Int>) -> List<T>    // panics out of bounds, like xs[i]
  fn take(self, count: Int) -> List<T>           // at most count items
  fn skip(self, count: Int) -> List<T>
  fn concat(self, other: List<T>) -> List<T>
  fn reversed(self) -> List<T>
  fn chunks(self, size: Int) -> List<List<T>>  requires size >= 1
  fn zip<U>(self, other: List<U>) -> List<(T, U)>  // stops at the shorter list
  fn enumerate(self) -> List<(Int, T)>
  fn iter(self) -> Iter<T>                       // lazy, for chains (see Iter)
  fn take_last(self, count: Int) -> List<T>
  fn windows(self, size: Int) -> List<List<T>>  requires size >= 1

  fn map<U, uses E>(self, f: fn(T) -> U uses E) -> List<U> uses E
  fn filter<uses E>(self, keep: fn(T) -> Bool uses E) -> List<T> uses E
  fn filter_map<U, uses E>(self, f: fn(T) -> Option<U> uses E) -> List<U> uses E
  fn flat_map<U, uses E>(self, f: fn(T) -> List<U> uses E) -> List<U> uses E
  fn try_map<U, Err, uses E>(self, f: fn(T) -> Result<U, Err> uses E) -> Result<List<U>, Err> uses E   // stops at the first Err
  fn partition<uses E>(self, pred: fn(T) -> Bool uses E) -> (List<T>, List<T>) uses E
  fn fold<A, uses E>(self, initial: A, f: fn(A, T) -> A uses E) -> A uses E
  fn find<uses E>(self, pred: fn(T) -> Bool uses E) -> Option<T> uses E
  fn find_index<uses E>(self, pred: fn(T) -> Bool uses E) -> Option<Int> uses E
  fn find_last<uses E>(self, pred: fn(T) -> Bool uses E) -> Option<T> uses E
  fn any<uses E>(self, pred: fn(T) -> Bool uses E) -> Bool uses E
  fn all<uses E>(self, pred: fn(T) -> Bool uses E) -> Bool uses E
  fn count<uses E>(self, pred: fn(T) -> Bool uses E) -> Int uses E
  fn sorted_by<uses E>(self, compare: fn(T, T) -> Ordering uses E) -> List<T> uses E
  fn sorted_by_key<K: Ord, uses E>(self, key: fn(T) -> K uses E) -> List<T> uses E
  fn group_by<K: Hash, uses E>(self, key: fn(T) -> K uses E) -> Map<K, List<T>> uses E
  fn min_by_key<K: Ord, uses E>(self, key: fn(T) -> K uses E) -> Option<T> uses E
  fn max_by_key<K: Ord, uses E>(self, key: fn(T) -> K uses E) -> Option<T> uses E

  fn push(var self, item: T)
  fn pop(var self) -> Option<T>
  fn insert(var self, index: Int, item: T)  requires 0 <= index && index <= self.len()
  fn remove(var self, index: Int) -> T  requires 0 <= index && index < self.len()
  fn set(var self, index: Int, value: T)  requires 0 <= index && index < self.len()
  fn extend(var self, items: List<T>)
  fn clear(var self)
}

impl<T: Eq> List<T> {
  fn contains(self, item: T) -> Bool
  fn index_of(self, item: T) -> Option<Int>
}

impl<T: Ord> List<T> {
  fn sorted(self) -> List<T>                      // stable
  fn min(self) -> Option<T>
  fn max(self) -> Option<T>
}

impl<T: Hash> List<T> {
  fn unique(self) -> List<T>                      // keeps the first of each
  fn to_set(self) -> Set<T>
}

impl List<Int> { fn sum(self) -> Int }
impl List<Float> { fn sum(self) -> Float }
impl List<String> { fn join(self, separator: String) -> String }
impl<K: Hash, V> List<(K, V)> { fn to_map(self) -> Map<K, V> }   // later pairs win
```

- Sorting, reversing and filtering return a new list and have no in-place twin, so there is one way to do each: xs = xs.sorted() updates in place when xs is unique (section 3).
- There is no `for_each`. A `for` loop is the one way to run code for each item.

### Iter

`Iter<T, uses E>` is a concrete type that implements `Iterator<uses E>` with `Item = T` and carries every adapter as an ordinary method, since interfaces have no default method bodies (section 3). `xs.iter()`, `m.iter()`, `s.chars()`, `(0..n).iter()` and `Iter.from(it)` give one. Adapters are lazy; nothing runs until a consumer or a `for` loop pulls items.

```
impl<T, uses E> Iter<T, uses E> {
  fn from<I: Iterator<uses E>>(it: I) -> Iter<I.Item, uses E>
  fn next(var self) -> Option<T> uses E

  fn map<U, uses F>(self, f: fn(T) -> U uses F) -> Iter<U, uses E, F>
  fn filter<uses F>(self, keep: fn(T) -> Bool uses F) -> Iter<T, uses E, F>
  fn filter_map<U, uses F>(self, f: fn(T) -> Option<U> uses F) -> Iter<U, uses E, F>
  fn flat_map<U, uses F>(self, f: fn(T) -> Iter<U, uses F> uses F) -> Iter<U, uses E, F>
  fn take(self, count: Int) -> Iter<T, uses E>
  fn skip(self, count: Int) -> Iter<T, uses E>
  fn step_by(self, step: Int) -> Iter<T, uses E>  requires step >= 1
  fn take_while<uses F>(self, pred: fn(T) -> Bool uses F) -> Iter<T, uses E, F>
  fn skip_while<uses F>(self, pred: fn(T) -> Bool uses F) -> Iter<T, uses E, F>
  fn enumerate(self) -> Iter<(Int, T), uses E>
  fn zip<U, uses F>(self, other: Iter<U, uses F>) -> Iter<(T, U), uses E, F>   // stops at the shorter
  fn chain<uses F>(self, other: Iter<T, uses F>) -> Iter<T, uses E, F>
  fn chunks(self, size: Int) -> Iter<List<T>, uses E>  requires size >= 1

  fn to_list(self) -> List<T> uses E
  fn fold<A, uses F>(self, initial: A, f: fn(A, T) -> A uses F) -> A uses E, F
  fn count(self) -> Int uses E
  fn find<uses F>(self, pred: fn(T) -> Bool uses F) -> Option<T> uses E, F
  fn position<uses F>(self, pred: fn(T) -> Bool uses F) -> Option<Int> uses E, F
  fn any<uses F>(self, pred: fn(T) -> Bool uses F) -> Bool uses E, F
  fn all<uses F>(self, pred: fn(T) -> Bool uses F) -> Bool uses E, F
  fn first(self) -> Option<T> uses E
  fn last(self) -> Option<T> uses E
  fn min_by_key<K: Ord, uses F>(self, key: fn(T) -> K uses F) -> Option<T> uses E, F
  fn max_by_key<K: Ord, uses F>(self, key: fn(T) -> K uses F) -> Option<T> uses E, F
}

impl<T: Ord, uses E> Iter<T, uses E> {
  fn min(self) -> Option<T> uses E
  fn max(self) -> Option<T> uses E
}
impl<T: Hash, uses E> Iter<T, uses E> { fn to_set(self) -> Set<T> uses E }
impl<K: Hash, V, uses E> Iter<(K, V), uses E> { fn to_map(self) -> Map<K, V> uses E }
impl<uses E> Iter<Int, uses E> { fn sum(self) -> Int uses E }
impl<uses E> Iter<Float, uses E> { fn sum(self) -> Float uses E }
impl<uses E> Iter<String, uses E> { fn join(self, separator: String) -> String uses E }
impl<T, uses E> Iterator<uses E> for Iter<T, uses E> { type Item = T }
```

- A chain's effects add up: each adapter adds its closure's effects, and the consumer that pulls the items has all of them.
- `List` keeps eager `map` and `filter` for one step. Use an `Iter` for a chain, or for input that isn't a list, such as `Fs.read_lines(path)`.
- There is no `for_each`; a `for` loop pulls the items.

### Map and Set

Both convert from a list with `From`: `Map.from([("a", 1)])`, `Set.from([1, 2, 3])`. A conversion that can fail is named `try_from` and returns `Option`, as in `CString.try_from(s)`, so `T.from(x)` always goes through `From` and never fails.

```
impl<K: Hash, V> Map<K, V> {
  fn len(self) -> Int
  fn is_empty(self) -> Bool
  fn get(self, key: K) -> Option<V>
  fn contains_key(self, key: K) -> Bool
  fn keys(self) -> List<K>
  fn values(self) -> List<V>
  fn entries(self) -> List<(K, V)>
  fn map_values<U, uses E>(self, f: fn(V) -> U uses E) -> Map<K, U> uses E
  fn filter<uses E>(self, keep: fn(K, V) -> Bool uses E) -> Map<K, V> uses E
  fn get_or(self, key: K, default: V) -> V
  fn merge(self, other: Map<K, V>) -> Map<K, V>   // other wins on shared keys
  fn iter(self) -> Iter<(K, V)>

  fn insert(var self, key: K, value: V) -> Option<V>   // the value it replaced
  fn remove(var self, key: K) -> Option<V>
  fn update<uses E>(var self, key: K, default: V, f: fn(V) -> V uses E) uses E
  fn clear(var self)
}

impl<T: Hash> Set<T> {
  fn len(self) -> Int
  fn is_empty(self) -> Bool
  fn contains(self, item: T) -> Bool
  fn union(self, other: Set<T>) -> Set<T>
  fn intersection(self, other: Set<T>) -> Set<T>
  fn difference(self, other: Set<T>) -> Set<T>
  fn is_subset(self, other: Set<T>) -> Bool
  fn is_disjoint(self, other: Set<T>) -> Bool
  fn iter(self) -> Iter<T>
  fn to_list(self) -> List<T>
  fn filter<uses E>(self, keep: fn(T) -> Bool uses E) -> Set<T> uses E

  fn insert(var self, item: T) -> Bool            // false if it was already there
  fn remove(var self, item: T) -> Bool
  fn clear(var self)
}
```

- `for (k, v) in m` iterates a map's entries and `for x in s` a set's items.
- `m.update(key, default: d, f)` applies `f` to the current value, or to `d` when the key is missing, and stores the result.

### Option and Result

```
impl<T> Option<T> {
  fn is_some(self) -> Bool
  fn is_none(self) -> Bool
  fn expect(self, reason: String) -> T            // panics with reason on None
  fn unwrap_or(self, default: T) -> T
  fn unwrap_or_else<uses E>(self, f: fn() -> T uses E) -> T uses E
  fn ok_or<Err>(self, error: Err) -> Result<T, Err>
  fn or(self, other: Option<T>) -> Option<T>
  fn or_else<uses E>(self, f: fn() -> Option<T> uses E) -> Option<T> uses E
  fn zip<U>(self, other: Option<U>) -> Option<(T, U)>
  fn map<U, uses E>(self, f: fn(T) -> U uses E) -> Option<U> uses E
  fn and_then<U, uses E>(self, f: fn(T) -> Option<U> uses E) -> Option<U> uses E
  fn filter<uses E>(self, keep: fn(T) -> Bool uses E) -> Option<T> uses E
}

impl<T, Err> Result<T, Err> {
  fn is_ok(self) -> Bool
  fn is_err(self) -> Bool
  fn expect(self, reason: String) -> T            // panics with reason and the error's debug form
  fn unwrap_or(self, default: T) -> T
  fn unwrap_or_else<uses E>(self, f: fn(Err) -> T uses E) -> T uses E
  fn ok(self) -> Option<T>
  fn err(self) -> Option<Err>
  fn map<U, uses E>(self, f: fn(T) -> U uses E) -> Result<U, Err> uses E
  fn map_err<F, uses E>(self, f: fn(Err) -> F uses E) -> Result<T, F> uses E
  fn and_then<U, uses E>(self, f: fn(T) -> Result<U, Err> uses E) -> Result<U, Err> uses E
  fn or_else<F, uses E>(self, f: fn(Err) -> Result<T, F> uses E) -> Result<T, F> uses E
}
```

- There is no `unwrap()` (section 4). `unwrap_or` is kept because it can't panic.

### Duration, Timestamp, Url and Regex

```
impl Duration {
  fn milliseconds(n: Int) -> Duration
  fn seconds(n: Int) -> Duration
  fn minutes(n: Int) -> Duration
  fn hours(n: Int) -> Duration
  fn days(n: Int) -> Duration
  fn microseconds(n: Int) -> Duration
  fn nanoseconds(n: Int) -> Duration
  fn parse(text: String) -> Result<Duration, InvalidFormat>   // the dur literal's syntax, as in "1h30m"
  fn is_zero(self) -> Bool
  fn is_negative(self) -> Bool
  fn to_milliseconds(self) -> Int
  fn to_seconds(self) -> Float
}

impl Add<Duration> for Duration { type Output = Duration }     // also Sub<Duration>; Mul<Int> and Div<Int>, which panics on 0

impl Timestamp {
  fn parse(text: String) -> Result<Timestamp, InvalidFormat>   // RFC 3339
  fn from_unix_seconds(seconds: Int) -> Timestamp
  fn to_unix_seconds(self) -> Int
  fn from_unix_milliseconds(ms: Int) -> Timestamp
  fn to_unix_milliseconds(self) -> Int
  fn to_rfc3339(self) -> String                    // UTC with Z; equals display()
  fn since(self, earlier: Timestamp) -> Duration
}

impl Add<Duration> for Timestamp { type Output = Timestamp }   // also Sub<Duration>, and Sub<Timestamp> with Output = Duration

impl Url {
  fn parse(text: String) -> Result<Url, InvalidFormat>
  fn scheme(self) -> String
  fn host(self) -> Option<String>
  fn port(self) -> Option<Int>
  fn path(self) -> String
  fn query(self) -> Option<String>
  fn fragment(self) -> Option<String>
  fn query_params(self) -> List<(String, String)>   // decoded, in order
  fn with_path(self, path: String) -> Url
  fn with_query(self, params: List<(String, String)>) -> Url
  fn join(self, path: String) -> Result<Url, InvalidFormat>
}

impl Regex {
  fn new(pattern: String) -> Result<Regex, InvalidFormat>
  fn is_match(self, text: String) -> Bool
  fn find(self, text: String) -> Option<Match>
  fn find_all(self, text: String) -> List<Match>
  fn captures(self, text: String) -> Option<Captures>
  fn replace_all(self, text: String, by: String) -> String   // $1 and ${name} refer to groups
  fn replace_first(self, text: String, by: String) -> String
  fn split(self, text: String) -> List<String>
}

pub struct InvalidFormat { pub expected: String, pub found: String }
pub struct Match { pub start: Int, pub end: Int, pub text: String }   // byte offsets
impl Captures {
  fn get(self, index: Int) -> Option<String>      // 0 is the whole match
  fn named(self, name: String) -> Option<String>
}
```

- `Timestamp` and `Duration` display as RFC 3339 text and as `30s`-style text, and encode the same way. Calendar and time-zone work is in `std.time`. Match and Captures are imported from std.regex; Regex itself is in the prelude because re"..." produces it.
- Every `parse` in the prelude fails with the same `InvalidFormat`, so code that parses several kinds of text handles one error type.

### Reference types and concurrency (std.sync)

Ref, Atomic, ConcurrentMap, Channel, Pool and ChannelClosed are imported from std.sync, TimedOut from std.time, and Resource from std.ffi. TaskScope, FailurePolicy and ScopeFailed are in the prelude. Every method on a reference type takes `self`, never `var self`, because it changes shared state through the reference's identity. A `let` binding is enough: ` let ch =  ``Channel<Job>.new(100)`.

```
impl<T> Ref<T> {
  fn new(value: T) -> Ref<T>
  fn get(self) -> T uses State
  fn set(self, value: T) uses State
  fn modify(self, f: fn(T) -> T) -> T uses State           // returns the new value
}

impl<T> Atomic<T> {                                       // T must be able to cross a task boundary
  fn new(value: T) -> Atomic<T>
  fn get(self) -> T uses State
  fn set(self, value: T) uses State
  fn swap(self, value: T) -> T uses State                  // returns the old value
  fn modify(self, f: fn(T) -> T) -> T uses State           // f may run more than once; returns the new value
}

impl<T: Eq> Atomic<T> {
  fn compare_and_set(self, expected: T, new: T) -> Bool uses State
}

impl<K: Hash, V> ConcurrentMap<K, V> {
  fn new() -> ConcurrentMap<K, V>
  fn get(self, key: K) -> Option<V> uses State
  fn contains_key(self, key: K) -> Bool uses State
  fn insert(self, key: K, value: V) -> Option<V> uses State
  fn remove(self, key: K) -> Option<V> uses State
  fn update(self, key: K, default: V, f: fn(V) -> V) -> V uses State
  fn len(self) -> Int uses State
  fn snapshot(self) -> Map<K, V> uses State
}

impl<T> Channel<T> {
  fn new(capacity: Int) -> Channel<T>  requires capacity >= 0   // 0: every send waits for a receiver
  fn send(self, value: T) -> Result<Unit, ChannelClosed> uses State
  fn receive(self) -> Option<T> uses State
  fn close(self) uses State                                  // closing twice does nothing
  fn is_closed(self) -> Bool uses State
  fn len(self) -> Int uses State
  fn iter(self) -> Iter<T, uses State>             // receives until closed: for job in jobs.iter()
}

impl<T, Err, uses E> Pool<T, Err, uses E> {
  fn new(max: Int, healthy: fn(T) -> Bool uses E, open: fn() -> Result<T, Err> uses E) -> Pool<T, Err, uses E>  requires max >= 1
  fn checkout<R, uses E2>(self, body: fn(T) -> R uses E2) -> Result<R, Err> uses E, E2, State
  fn size(self) -> Int uses State                  // values open now
  fn idle(self) -> Int uses State
}
```

- `modify` and `update` take pure closures (section 9).
- There is no unbounded channel and no non-blocking `try_send` or `try_receive`, matching `select`, which has no default arm.
- `Pool` names the error and effects of its `open` function, because `checkout` may open a new value: `PgDb { pool: Pool<PgConn, PgError, uses Net> }`. `checkout` returns `Err` only when opening fails.
- `TaskScope` has no methods; tasks start with `spawn in` (section 9). `Resource` is built with `Resource.adopt` lent with res.ptr(), closed with res.close(), checked with res.is\_closed() and tied to a callback with res.keep(cb), all only in the package that declares the foreign type (section 11).

### Types the built-in effects use

Calling the operations needs no import, but naming the types they use does: FsError and Metadata come from std.fs; NetError, Outbound, Inbound and Listener from std.net; ProcessError and ProcessOutput from std.process.

```
pub enum ProcessError { NotFound(program: String), PermissionDenied(program: String), Other(message: String) }
pub struct ProcessOutput { pub status: Int, pub stdout: Bytes, pub stderr: Bytes }
pub struct Metadata { pub size: Int, pub is_dir: Bool, pub modified: Timestamp }
```

- `Outbound`, `Inbound` and `Listener` are reference types (section 3): copies share one connection, they can cross a task boundary, so a server can hand an accepted connection to a task, and a connection closes with `close` or when its last reference is dropped. std.testing builds in-memory ones for fake handlers. `Inbound` connections take `Listen` operations, so a server that only accepts connections needs `uses Listen` and not `Net`.

### Standard library index

Every standard library name this spec uses that isn't in the prelude. Each needs an import (section 7), and its full signature is in the [Quince](https://claude.ai/artifact/MRcpqw3BqGotK6yE1psFvC) [standard library API](https://claude.ai/artifact/MRcpqw3BqGotK6yE1psFvC) doc.

| Name | Package | Used in |
| --- | --- | --- |
| Log effect, Log.info, `log.json` | std.log | Sections 3, 5, 9 |
| fixed\_clock, Generate | std.testing | Sections 5, 6 |
| Encoder, Decoder, Encode and Decode methods | std.encoding | Section 3 |
| json.decode, json.encode, json.Value | std.json | Sections 3, 7 |
| toml, form formats | std.toml, std.form | Section 3 |
| get, serve, Request, Response, ServeError | std.http | Sections 5, 7, 9 |
| net.only(host) | std.net | Section 5 |
| CInt and the other C types, CString, Ptr, ConstPtr, CBuf, Callback | std.ffi | Sections 3, 11 |
| Ref, Atomic, ConcurrentMap, Channel, Pool, ChannelClosed | std.sync | Sections 3, 9, 11 |
| with\_timeout, TimedOut | std.time | Section 9 |
| Resource, with\_resource | std.ffi | Sections 3, 11 |
| FsError, Metadata | std.fs | Section 5, Appendix: Prelude |
| NetError, Outbound, Inbound, Listener | std.net | Section 5, Appendix: Prelude |
| ProcessError, ProcessOutput | std.process | Appendix: Prelude |
| Match, Captures | std.regex | Appendix: Prelude |

Names in examples that are not std: `retry` in section 5 is an illustrative signature, and `load_config` and the database types (`Db`, `PgDb`, `Rows`, `DbError`) belong to the example program.
