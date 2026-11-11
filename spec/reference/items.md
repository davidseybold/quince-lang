# Items

```ebnf
decl         = { doc_comment } { attribute NEWLINE } [ "pub" ] item
item         = fn_decl | struct_decl | newtype_decl | enum_decl | alias_decl | interface_decl
             | impl_decl | effect_decl | const_decl | foreign_decl | test_decl
fn_decl      = fn_sig { contract } block
fn_sig       = "fn" name [ generics ] "(" [ params ] ")" [ "->" type ] [ "uses" effects ]
params       = param { "," param } [ "," ]
param        = [ "var" ] "self" | [ "var" ] ( name | "_" ) ":" type [ "=" expr ]   // closures: "_" too
```

## Constants

```quince
const MAX_RETRIES: Int = 3
const EMAIL_PATTERN: Regex = re"^[^@]+@[^@]+$"
const DEFAULT_CONFIG: Config = Config.parse(DEFAULT_CONFIG_TEXT).expect("built-in config is valid")
```

```ebnf
const_decl   = "const" NAME ":" type "=" expr
```

r[item.const.decl]
- `const` declares a top-level constant.

r[item.const.typed-pure]
- Its type is always written, and its initializer must be pure ([Effects](effects.md)).

r[item.const.eval.literal]
- A literal, including a tagged literal, is evaluated at compile time.

r[item.const.eval.lazy]
- Any other initializer runs once, on first use, and the result is safe to share across tasks. Because initializers are pure, the order in which constants are first used never matters.

r[item.const.no-ref]
- A const's type can't contain a reference type ([Types](types.md)) anywhere inside it, in fields, payloads or closure captures. The compiler derives this the same way it derives whether a type can cross a task boundary, and the error names the field path that holds the reference. State the whole program shares, such as a request counter, a cache or a registry, belongs in an effect handler installed in `main` ([Effects](effects.md)), so every function that touches it says so in its `uses` clause.

r[item.const.panic.site]
- A panic in an initializer is reported at the first use site.

r[item.const.panic.warn]
- `quince` `check` warns about any initializer that can panic.

r[item.const.assoc]
- A type's own constants are declared in its `impl` block, in the type's package: `impl Int { pub const MAX: Int = 9_223_372_036_854_775_807 }`. They follow every rule above.

r[item.const.assoc.qualified]
- They are always read through the type's name, as `Int.MAX`, `Float.PI` or `Status.OK`, and inside the impl as `Self.MAX`, never as a bare `MAX`.

r[item.const.assoc.no-interface]
- Interfaces can't declare constants in v1.

## Structs

```quince
pub struct User {
  id: UserId,
  email: Email,
  name: String,
  created_at: Timestamp,
}
```

```ebnf
struct_decl  = "struct" Name [ generics ] [ "{" [ field { "," field } [ "," ] ] "}" [ "requires" expr ] ]   // no braces: no fields
field        = { doc_comment } { attribute NEWLINE } [ "pub" ] name ":" type
struct_lit  = type_path [ type_args ] "(" [ ".." expr "," ] field_init { "," field_init } [ "," ] ")"
field_init  = name ":" [ expr ]
```

r[item.struct.init-all]
- All fields must be initialized at construction. No default constructors, no partially built values.

r[item.struct.unit]
- A struct with no fields is declared without braces, as in `pub struct ChannelClosed`, and its one value is written as its name, `ChannelClosed`.

r[item.struct.literal]
- A struct is built with its name and every field named, in declaration order: `User(id: UserId(1), email:, name: "Ada", created_at: now)`.

r[item.struct.shorthand]
- A field name with nothing after the colon takes the binding of the same name, as in patterns, and `quince` `fmt` rewrites `email: email` to that form.

r[item.struct.update]
- `..base`, written first, copies every field not written after it: `User(..user, name: "New")`.

r[item.enum.construct]
- A variant with a payload is built the same way, shorthand included: `SqliteError.Open(code:)`.

r[item.struct.value]
- Structs are values, like collections: passing or assigning one copies it (see [Mutation](types.md#mutation)).

r[item.struct.let-immutable]
- On a `let` binding a struct never changes, and updates build a new value with `User(..user, name: "New")`.

r[item.struct.field-assign]
- On a `var` binding, `user.name = "New"` assigns one field, which is shorthand for `user = User(..user, name: "New")`.

r[item.struct.field-private]
- Each field is private unless marked `pub`, as in `pub struct Account { pub id: AccountId, balance: Money }`.

r[item.struct.literal-visibility]
- A struct literal or a `Type(..x)` update can be written only where every field is visible, so a type with private fields can only be built through its own functions. Invariants are stated once, with `requires` on the struct ([Contracts](contracts.md)).

## Enums (algebraic data types)

```quince
pub enum Shape {
  Circle(radius: Float),
  Rect(width: Float, height: Float),
  Point,
}
```

```ebnf
enum_decl    = "enum" Name [ generics ] "{" [ evariant { "," evariant } [ "," ] ] "}"
evariant     = { doc_comment } { attribute NEWLINE } variant
variant      = Name [ "(" vfield { "," vfield } [ "," ] ")" ]
vfield       = { doc_comment } { attribute NEWLINE } name ":" type   // no pub: the enum's visibility
```

r[item.enum.named-fields]
Variant payloads use named fields so call sites read unambiguously: `Shape.Rect(width: 2.0, height: 3.0)`.

r[item.enum.field-visibility]
A variant's fields always have its enum's visibility, so other packages can build and match a `pub` enum's variants, and a variant field is never marked `pub`: writing it is a compile error.

r[item.enum.recursive]
An enum may contain itself, directly or inside a collection: `enum Tree { Leaf, Node(left: Tree, value: Int, right: Tree) }`. Enums are values, so no box or pointer is written.

## Newtypes

```ebnf
newtype_decl = "struct" Name [ generics ] "(" [ "pub" ] type ")"
alias_decl   = "type" Name [ generics ] "=" type
```

r[item.newtype.distinct]
`struct UserId(Int)` creates a distinct type that does not unify with `Int`.

r[item.alias.lint]
Plain aliases (`type Name = String`) are allowed but flagged by the linter in public APIs, since they hide meaning without adding safety.

r[item.newtype.access]
`UserId(1)` builds a newtype and `id.0` reads its inner value.

r[item.newtype.visibility]
The inner value follows `pub` like a field: `pub struct UserId(pub Int)` lets other packages build and read it, while `pub struct Email(String)` leaves both to the package's own functions, which is how a newtype guards an invariant.

## Methods

```ebnf
impl_decl    = "impl" [ generics ] [ type_path [ type_args ] "for" ] type "{" { impl_item NEWLINE } "}"
impl_item    = [ "pub" ] fn_decl | "type" Name "=" type | [ "pub" ] const_decl
```

r[item.method.impl]
- Methods and associated functions are declared in an `impl` block for the type, in the type's own package: ` impl User {  ``fn create(email: Email) -> Result<User, UserError>` `{ ... } }`.

r[item.method.call]
- A function with a `self` or `var self` receiver is called as a method (`u.display_name()`); one without is called on the type (`User.create(email)`, `Config.parse(text)`).

r[item.method.visibility]
- An inherent method or associated function is visible outside the package only when marked `pub`.

r[item.method.impl-no-pub]
- Methods in an interface impl or a handler take no `pub`: they are visible wherever both the type and the interface or effect are.

## Interfaces

```quince
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

```ebnf
interface_decl = "interface" Name [ generics ] "{" { ( "type" Name | fn_sig { contract } ) NEWLINE } "}"
```

r[item.iface.explicit]
- Implementation is always explicit with `impl`. A type never satisfies an interface by accident.

r[item.iface.receiver]
- An impl's method takes the same receiver as the interface declares, `self` or `var self`.

r[item.iface.in-scope]
- A method that comes from an interface can be called only when the interface is in scope, either defined in the same package or imported.

r[item.iface.ambiguous]
- If two interfaces in scope define the same method name for a type, the call is a compile error, and the suggested fix names the interface: `Display.display(u)`.

r[item.iface.ambiguous-inherent]
- The same holds when a type's own method and an interface method in scope share a name: the call is an error until it is written `User.display(u)` or `Display.display(u)`, so a new method in a dependency can never silently change which body runs. Picking an `impl` by argument type, as `Money.from(x)` does across several `From<T>` impls, is ordinary generic dispatch, not overloading.

r[item.iface.orphan]
- An `impl` must live in the package that defines the type or the package that defines the interface (the orphan rule), so every implementation can be found in one of two places.

r[item.iface.no-default]
- Interface methods have no bodies, default or otherwise. Shared behavior goes in a regular generic function.

r[item.iface.no-dyn]
- There are no interface-typed values (no `dyn Interface`). Interfaces are used only as generic bounds, so the implementation that runs is always known from the types at the call site. To hold handlers of different types in one list, use a struct of closures or an enum.

## Generics

```ebnf
generics     = "<" generic { "," generic } ">"
generic      = Name [ ":" bounds ] | "uses" Name
bounds       = bound { "+" bound }
bound        = type_path [ "<" bound_arg { "," bound_arg } ">" ]
bound_arg    = type_arg | Name "=" type   // Item = Int, Output = T
```

r[item.generic.bounds]
- Type parameters with interface bounds: `fn max<T: Ord>(a: T, b: T) -> T`.

r[item.generic.multi-bounds]
- Several bounds are joined with `+`: `fn show_sorted<T: Ord + Display>(xs: List<T>) -> String`.

r[item.generic.assoc-binding]
- A bound can fix an interface's associated type, as in Rust: `fn sum<I: Iterator<Item = Int>>(var it: I) -> Int` or `fn total<T: Add<T, Output = T>>(xs: List<T>) -> T`.

r[item.iface.type-params]
- Interfaces may take type parameters: `interface From<T> { fn from(value: T) -> Self }`, and effect parameters ([Effects](effects.md)): `interface Iterator<uses E>`.

r[item.iface.assoc-types]
- Interfaces may declare associated types, which each implementation fixes:

```quince
pub interface Iterator<uses E> {
  type Item
  fn next(var self) -> Option<Self.Item> uses E
}
```

r[item.generic.excluded]
- Deliberately excluded: higher-kinded types (keeps type errors readable), variadic generics (`concurrent` and tuples are handled by the compiler instead), and specialization (which implementation runs must never depend on code elsewhere).

- Type arguments can be written at a call site, as in `json.decode<Order>(body)` or `Channel<Job>.``new(16)`, and are optional wherever inference finds them ([Type arguments and construction](types.md#type-arguments-and-construction)).

## Arguments

```quince
counts.update(word, default: 0, fn(n) { n + 1 })
http.serve(8080, handler: fn(req) { handle(req, bg:) })
```

r[item.arg.first-positional]
- The first argument may be positional.

r[item.arg.named]
- Every later argument is written with its parameter name, except the argument for the last declared parameter when that parameter has a function type: it is written without its name, whether it is a closure or a named function, and quince fmt rewrites either form into this one.

r[item.arg.shorthand]
- As in struct literals, `name:` alone passes the binding of the same name, so `handle(req, bg:)` means `handle(req, bg: bg)`.

r[item.arg.fmt-positional]
- When the first argument may be positional, `quince` `fmt` writes it positionally, so `http.serve(8080``, handler: h)` is the one form. That holds even when the first parameter's name carries meaning: `Channel<Job>.new(16)`, `log.json(cfg.log_level)`.

r[item.arg.receiver]
- On a method call the receiver doesn't count, so the first argument after it may be positional: `counts.update(word, default: 0, ...)`.

r[item.arg.fn-value-positional]
- A call through a function value, such as `handler(req)` or `f(a, b)`, is always positional, because function types have no parameter names.

r[item.arg.order]
- Named arguments are passed in declaration order, so each call has one canonical form.

r[item.arg.default]
- A parameter may have a default value, which must be a literal, an empty list or map literal (`[]` or `{:}`), including a tagged literal such as dur"30s", or the name of a const, including a type's constant such as Int.MAX: `fn retry(times: Int = 3)`. A caller can leave it out.

r[item.arg.semver]
- Adding a parameter with a default is not a breaking change ([Dependencies and publishing](../tooling/dependencies.md)). Renaming any parameter is.

## Attributes

```ebnf
attribute    = "@" name [ "(" arg { "," arg } ")" ]
```

r[item.attr.fixed]
Attributes are a fixed set defined by the language. Packages cannot define their own, so attributes can never become a form of metaprogramming.

r[item.attr.unknown]
An unknown attribute is a compile error, and new attributes arrive only through editions ([Packages and modules](packages.md)).

r[item.attr.unchecked]
- `@unchecked` (Functions): Skip contract checks in release builds ([Contracts](contracts.md))

r[item.attr.verify]
- `@verify` (Functions): Attempt static proof of contracts with `quince` `verify` ([Contracts](contracts.md))

r[item.attr.extensible]
- `@extensible` (Enums): Variants may be added without a major version; callers must include a wildcard arm ([Dependencies and publishing](../tooling/dependencies.md))

r[item.attr.pinned]
- `@pinned` (Foreign libraries): Always call from the same OS thread ([Interop with C](interop.md))

r[item.attr.shareable]
- `@shareable` (Foreign resource types): The C handle is safe to use from several threads, so its Resource may cross task boundaries. Unverifiable, so it is listed with the package's native dependencies ([Interop with C](interop.md))

r[item.attr.cleanup]
- `@cleanup(fn, limit: dur)` (Foreign types): The C function that releases the type, and the cleanup time limit (10 seconds when left out), so it can be held in a Resource ([Interop with C](interop.md))

r[item.attr.ownership]
- `@borrowed`, `@owned(fn)` (Foreign functions that return a CString or a Ptr\<T> whose T has no @cleanup): Who owns the returned memory: C, or Quince, which frees it with fn when it is dropped. One of the two is required ([Interop with C](interop.md))

r[item.attr.deprecated]
- `@deprecated("why", use: path)` (Any public declaration): Warn on use and name the replacement; `quince` `fix` rewrites calls automatically when the replacement takes the same arguments

r[item.attr.opaque]
- `@opaque` (Structs and enums): No automatic Eq, Hash, debug form, test generator, Encode or Decode, for handle-like types ([Prelude and special interfaces](prelude.md))

r[item.attr.test]
- `@test` (Top-level declarations): Compiled only by `quince` `test`; naming it from code that isn't a test is a compile error. Impls of a `@test` type are test-only too, without their own `@test` ([Testing](testing.md))

r[item.attr.wire]
- `@wire("name")`, `@wire(case: camel)`, `@wire(tag: "kind")` (Fields and variants (name); structs and enums (case, tag)): Set the wire name, the naming case, or the enum tag field used by Encode and Decode ([Serialization](prelude.md#serialization))

r[item.attr.test-default]
- `@test_default(f)` (Package effects): The handler a test gets when it reaches this effect with no `with` around it; `f` takes no arguments and returns a handler ([Effects](effects.md))

r[item.attr.placement]
An attribute goes on its own line directly above the declaration it applies to.
