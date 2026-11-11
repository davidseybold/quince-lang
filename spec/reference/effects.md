# Effects

r[effect.pure-default]
Functions are pure by default: a function with no `uses` clause returns the same result for the same arguments and changes nothing its caller can observe.

r[effect.uses-required]
Any function that touches the outside world or shared state declares it with a `uses` clause, and the compiler rejects calls to effectful functions from a context that doesn't have that effect.

## Built-in effects

r[effect.net]
- `Net`: Outbound network connections

r[effect.listen]
- `Listen`: Accepting inbound connections

r[effect.fs]
- `Fs`: Reading and writing files

r[effect.clock]
- `Clock`: Reading the current time, sleeping

r[effect.random]
- `Random`: Nondeterministic randomness

r[effect.env]
- `Env`: Environment variables and command-line arguments

r[effect.process]
- `Process`: Spawning subprocesses and ending the program early

r[effect.console]
- `Console`: Reading standard input; writing standard output and standard error

r[effect.state]
- `State`: Reading or writing a reference type whose item in the [Reference types](types.md) list needs it

Each built-in effect's operations, such as Clock.now() and Fs.read\_text(path), are listed in [Built-in effect operations](#built-in-effect-operations).

r[effect.log-not-builtin]
Logging is not built in. The standard library declares it as a package effect, `pub effect Log uses Console, Clock { ... }`, so a function that logs says `uses Log`, and a test can capture its logs with `with Log = ...` without touching standard output.

```ebnf
effect_decl  = "effect" Name ( [ "uses" effects ] "{" { fn_sig NEWLINE } "}" | "=" type_path )
effects      = "*" | effect_ref { "," effect_ref }      // "*" only on main
effect_ref   = type_path | Name                          // an effect or an effect variable
```

r[effect.decl]
Packages declare their own effects with `effect`, naming the built-in effects each one is built on and listing its operations.

r[effect.handler.def]
A handler is any value whose type implements the operations with `impl Db for T`, and only those bodies may perform the underlying effects.

r[effect.handler.crossing]
A handler's type must be able to cross task boundaries ([Types](types.md)), since tasks inherit handlers ([Concurrency](concurrency.md)); that's why `PgDb` holds a `Pool` rather than a single connection:

`pg`, `PgConn` and `PgError`, here and in [Interop with C](interop.md), stand for a hypothetical third-party Postgres package. They are not part of std.

```quince
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

r[effect.handler.receiver]
- Unlike an interface's impl, a handler implements each operation of an `effect` as a method with a `self` receiver.

r[effect.handler.uses]
- That method's `uses` may name the effects the effect is built on ([Effects](#declaring-and-inferring)), `State`, and the effect it handles.

r[effect.handler.outer-call]
- A call to that effect from inside the handler goes to the handler that was in effect where the `with` installed it, never back to the handler itself. So a narrowing handler such as `net.only` passes allowed calls through, and a fake such as `fixed_clock` keeps its time in a reference type.

r[effect.handler.uses-subset]
- It may name fewer of them, down to none.

r[effect.op-call]
- Effect operations are called through the effect's name: `Log.info("started")`, `Db.query(sql)`. The handler that runs is the one `with` installed ([Effects](#handlers-in-tests-and-production)).

r[effect.op-uses]
- Calling a `Db` operation needs `uses Db`.

r[effect.no-expand]
- A caller with `uses Db` doesn't also list `Net`; `quince` `query signature` shows what `Db` is built on.

r[effect.interface]
- The operations are the effect's whole interface: code outside the package reaches `Db` only through them, and they are what `with Db = ...` installs, in tests and in `main`.

r[effect.keyword-decl-only]
- `effect` appears only in declarations. [Effect variables](#effect-variables) are written `uses E`, so the keyword never appears in a generic parameter list.

r[effect.no-generics]
- Effects and their operations take no type parameters in v1. Handlers are installed at runtime and there is no `dyn`, so a generic operation such as `Cache.get<T>(key)` would have no implementation the compiler could pick. A cache that holds many types stores `Bytes` or `json.Value`, and an ordinary generic function gives it a typed wrapper.

r[effect.copy]
- `effect UsersDb = Db` declares a distinct effect with the same operations. A program with two databases declares one copy per database, writes `uses UsersDb, OrdersDb`, calls `UsersDb.query(sql)`, and installs each with its own `with`. The type checker keeps the two apart, and a signature says which database a function touches.

r[effect.copy-handler]
- A handler for `Db` (such as `PgDb`) handles every copy. When the set is known only at runtime (shards, one database per tenant), use one effect whose operations take the key, such as `TenantDb.query(tenant, sql)`.

r[effect.no-index]
- Effects can't be indexed by a value.

## Declaring and inferring

```quince
pub fn sync_users(api: ApiClient, store: UserStore) -> Result<Int, SyncError>
  uses Net, Db, Log
{ ... }
```

r[effect.declare-all]
- Every function declares its effects, public or private, so a reader never has to open a callee to learn what a function can do.

r[effect.fix-uses]
- `quince` `fix` writes in a missing or out-of-date `uses` clause, so nobody works it out by hand.

r[effect.unused-warn]
- Declaring an effect a function doesn't use is a warning, so signatures stay honest in both directions.

r[effect.unused-pub]
- A `pub` function is exempt, because adding an effect later is a breaking change ([Dependencies and publishing](../tooling/dependencies.md)).

r[effect.fix-pub-review]
- `quince` `fix` marks adding an effect to a `pub` signature `needs-review`.

r[effect.closure-type]
- Closures carry effects in their type: `fn(Int) -> Int uses Log`.

r[effect.pure-param]
- A parameter type with no `uses` accepts only pure closures. A function that should accept effectful ones declares an [effect variable](#effect-variables).

## Effect variables

r[effect.effvar.decl]
A function that takes a closure and calls it declares an effect variable among its generic parameters with `uses`, and puts that variable in its own `uses` clause. The function then has exactly the effects of the closure it was given:

```quince
impl<T> List<T> {
  fn map<U, uses E>(self, f: fn(T) -> U uses E) -> List<U> uses E
}

pub fn retry<T, Err, uses E>(attempts: Int, f: fn() -> Result<T, Err> uses E)
  -> Result<T, Err>
  uses E, Clock
```

r[effect.effvar.inferred]
- **Inferred at every call, never written there.** `xs.map(fn(x) { x * 2 })` is pure. `xs.map(fn(u) { describe(u) })` calls `describe`, which uses `Log`, so the caller needs `Log`. `keys.map(fn(k) { cache.get(k) })` reads a `Ref`, so it uses `State`; reference types need no extra rule.

r[effect.effvar.opaque]
- **Opaque inside the function.** The body can call `f` but can't perform an effect from `E` itself or test what `E` holds. A higher-order function's behavior can't depend on which effects its caller passed in, so its signature tells a reader everything it can do.

r[effect.effvar.combine]
- **Combining.** A `uses` clause can mix variables and named effects (`uses E, Clock`). Two closure parameters can share one variable or take one each (`<uses E1, uses E2>` and `uses E1, E2`). An effect variable stands for a set of effects; there is no syntax for subtracting effects from one.

r[effect.effvar.subsumption]
- Subsumption. A closure whose effects are a subset of the expected ones is accepted, so a pure closure fits wherever an effectful one is expected.

r[effect.effvar.union]
- When several closures meet in one type, inference takes the union of their effects: a list holding a route that uses `Db` and one that uses `Net` has type `List<Route<uses Db, Net>>`.

r[effect.effvar.types]
- **Structs and enums** that store a closure take effect parameters the same way: `struct Route<uses E> { path: String, handler: fn(Request) -> Response uses E }`. So does `serve`: `pub fn serve<uses E>(port: Int, handler: fn(Request) -> Response uses E) -> Result<Unit, ServeError> uses Listen, E`. This is abridged: std.http's `serve` also takes `host:` and `max_body:`, which have defaults.

r[effect.effvar.iface]
- **Interfaces** may take effect parameters, like `Iterator<uses E>` in [Prelude and special interfaces](prelude.md), so `impl Iterator<uses Fs> for FileLines` can read a file one line at a time.

r[effect.effvar.default-pure]
- Leaving out an effect argument means no effects: a `T: Iterator` bound accepts only pure iterators.

r[effect.effvar.pure-ifaces]
- `Eq`, `Ord`, `Hash` and `Display` take no effect parameter, and neither do the operator interfaces ([Prelude and special interfaces](prelude.md)), so comparing, hashing, printing and arithmetic never have effects.

r[effect.effvar.contracts]
- **Contracts** must be pure, so a `requires` or `ensures` can't call a closure whose type includes an effect variable.

r[effect.effvar.diagnostic]
- **Diagnostics.** When a closure is rejected because its parameter type has no `uses`, the error names the closure's effects and the call in it that brings in each one. If the function is in the caller's own package, the suggested fix adds an effect variable to it.

## Handlers in tests and production

```ebnf
with_expr    = "with" handler { "," handler } block
handler      = type_path "=" expr
```

Tests can replace an effect's implementation for a block:

```quince
test "expired sessions are removed" {
  with Clock = testing.fixed_clock(ts"2026-01-01T00:00:00Z") {
    assert(purge_sessions(store).len() == 3)
  }
}
```

A handler is an ordinary value whose type implements an effect's operations (see [package effects](#declaring-and-inferring)).

r[effect.with.install]
`with` installs handlers for the code inside it, in tests and in production. This is how a program configures its database, logger and other services:

```quince
fn main() -> Result<Unit, AppError> uses * {
  let cfg = load_config("app.toml")?
  with Db = PgDb.connect(cfg.db, max: 20)?,
       Log = log.json(cfg.log_level) {
    http.serve(cfg.port, handler: route).map_err(AppError.Serve)
  }
}
```

r[effect.with.package-main-only]
- Outside test code, a `with` for a package effect may appear only in `main`. A package that builds several programs, such as a server and a worker, gives each its own small package with one main ([Packages](packages.md#packages)), so a program's production wiring always sits in one function an agent can read, and "which `Db` is this?" is always answered there.

r[effect.with.nested-warn]
- Outside tests, a `with` nested inside another `with` for the same effect is a warning, because an override deep in the call stack hides which handler runs.

r[effect.with.query-handlers]
- `quince` `query handlers <fn>` prints which handler each of a function's effects resolves to from each program's main.

r[effect.with.builtin-anywhere]
- A `with` for a built-in effect can narrow it anywhere, in any function: `with Net = net.only("api.example.com") { run_plugin(input) }`.

r[effect.handler.no-continuation]
- Handlers are a handler type's value, not a continuation: they can't resume, abort or capture the code that called them. `with` is the only form of effect handler in the language.

## Built-in effect operations

r[effect.builtin-ops]
A built-in effect's operations are called through its name, like a package effect's: `Clock.now()`, `Fs.read_text(path)`, `Process.exit(1)`. These operations are what a handler for that effect implements, in tests and in a narrowing `with` ([Effects](#handlers-in-tests-and-production)). Standard library packages build higher-level APIs on them, such as `std.http` on `Net` and `Listen`.

r[effect.builtin-ops-no-import]
Calling a built-in effect's operations never needs an import.

```quince
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

r[effect.state-no-ops]
- `State` has no operations. It is the permission for the reference-type methods in [std.sync](../../std/API.md#reference-types-and-concurrency-stdsync).

- A manifest limit that refuses an operation returns `FsError.PermissionDenied` or `NetError.PermissionDenied` ([Program execution](execution.md)).
- Sleeping, accepting and receiving are cancellation points like every effectful call ([Cancellation](concurrency.md#cancellation)).
