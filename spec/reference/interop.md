# Interop with C

r[ffi.c-abi]
Quince interoperates with other languages only through the C ABI. Any library that exposes a C interface (including ones written in Rust, Zig, Swift, or C++) can be called; direct interop with Rust generics or JavaScript is out of scope.

## Generated bindings

```ebnf
foreign_decl = "foreign" string [ "uses" effects ] "{" { ( { attribute NEWLINE } ( c_fn | "type" c_name ) ) NEWLINE } "}"
c_fn         = "fn" c_name "(" [ c_name ":" type { "," c_name ":" type } [ "," ] ] ")" [ "->" type ]
```

r[ffi.cbind.generate]
`quince` `bind <header.h>` reads a C header and generates raw declarations. Bindings are generated rather than hand-written, because a mistyped foreign declaration corrupts memory silently.

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

r[ffi.block]
- A `foreign` block names the library and the effects its code performs.

r[ffi.effects]
- Callers of anything built on it see those effects like any others.

r[ffi.pure-block]
- A block without uses declares pure functions.

r[ffi.cbind.all-effects]
- Like every foreign effect, that is a promise the compiler can't check, so quince bind writes every built-in effect on the block and the author removes the ones the library doesn't use.

r[ffi.multi-block]
- A package may have several `foreign` blocks for one library, each with its own effects, for example a pure block for one-shot functions and a `uses State` block for a stream.

r[ffi.namespace]
- They share one namespace, so a type declared in one is visible in the others.

r[ffi.attr-consistent]
- A library attribute such as `@pinned` must appear on every block for that library or on none.

r[ffi.not-pub]
- Raw declarations use C types (`CInt`, `CString`, `Ptr<T>`) and cannot be `pub`. The package that declares them must wrap them in a safe API using `Result` and ordinary Quince types.

r[ffi.resource-not-pub]
- A Resource\<T> whose T is a foreign type can't appear in a pub signature either; hold it in a field of a struct, as Db does.

r[ffi.type]
- `type name` inside a `foreign` block declares an opaque C type.

r[ffi.cleanup]
- `@cleanup(fn, limit: dur)` above it names the C function that releases it and the cleanup time limit, 10 seconds when left out.

r[ffi.cleanup-signature]
- The cleanup function must be declared in a foreign block of the same package and take exactly one parameter, Ptr\<T>.

r[ffi.cleanup-return]
- It may return any C type, and res.close() returns that value; a wrong signature is a compile error on the @cleanup line.

r[ffi.cresource.adopt]
- `Resource.adopt(p)` turns a `Ptr<T>` into a `Resource<T>`, and works only for a type with `@cleanup`.

r[ffi.cresource.ptr]
- `res.ptr()` lends the raw pointer for one call.

r[ffi.cresource.ptr-arg-only]
- Like buf.ptr() and bytes.lend(), it may only be written as an argument of a foreign call; binding it to a name is a compile error.

r[ffi.cresource.close]
- res.close() runs the cleanup now and returns the cleanup function's result, res.is\_closed() tells whether cleanup has run, and res.keep(x) moves a Callback, CBuf or CString into the resource, which releases it right after the C cleanup runs, so the end of the local scope no longer releases it.

r[ffi.cbuf.def]
- `CBuf<T>` is C-allocated memory with identity (a reference type, [Types](types.md)), and is the only way to pass an out-parameter, so Quince memory never escapes to C.

r[ffi.cbuf.api]
- CBuf.new\<T>() holds one value and CBuf.alloc\<T>(n) an array; buf.get(), buf.set(v) and buf.ptr() reach it, and CBuf.from\_bytes(b) and buf.to\_bytes(n) copy UInt8 buffers.

r[ffi.cstring.convert]
- `CString.try_from(s)` converts text for C (`None` when it holds a NUL byte), and `cstr.to_string()` converts it back as a `Result`, since C strings may not be UTF-8.

r[ffi.lend]
- `bytes.lend()` lends a `Bytes` value in place for one call, as a `ConstPtr<UInt8>`.

r[ffi.lend-const]
- `quince` `bind` emits `ConstPtr<T>` for a `const T*` parameter, and only such a parameter accepts a lent pointer, so C can't write into Quince memory.

r[ffi.no-string-lend]
- A `String` is never lent, since C strings need a trailing NUL byte; `CString.try_from` copies it.

r[ffi.ownership]
- A foreign function that returns a `CString` must say who owns it.

r[ffi.borrowed]
- `@borrowed` means C keeps it: Quince never frees it, and it must be converted with `to_string()` before the next call into that library.

r[ffi.owned]
- `@owned(free_fn)` means Quince frees it with `free_fn` when it is dropped.

r[ffi.ownership-required]
- Leaving both out is a compile error, and `quince` `bind` leaves them out because a header can't say.

r[ffi.ownership-ptr]
- The same rule covers a returned `Ptr<T>` whose `T` has no `@cleanup`.

r[ffi.cstring.owned]
- A `CString` made by `CString.try_from` is always owned by Quince.

r[ffi.ptr-ops]
- The pointer operations are `Ptr.null()`, `p.is_null()`, and `p.read()` and `p.write(v)` for C types declared in the block.

r[ffi.package-only]
- Everything in this list works only in the package that declares the `foreign` block.

r[ffi.no-unsafe]
- There is no `unsafe` keyword: the native mark and [`allow_native`](#trust) are the audit surface, and `quince` `query` lists every raw operation in a package.

## C types

r[ffi.ctype.fixed-alias]
A C type whose width is the same on every target is another name for a Quince number type, so literals, constants and conversions work as usual: `const MINZ_BUF_ERROR: CInt = -5`, `n.to_int32()`, `c.to_int64()`.

r[ffi.ctype.target-width]
A C type whose width depends on the target is a type of its own, so code that compiles on one target compiles on all of them.

r[ffi.ctype.pub]
The fixed-width names may appear in `pub` signatures, since each is only another name for a Quince type. Target-width C types, raw foreign declarations and `Resource<foreign T>` may not.

r[ffi.ctype.char]
- signed char, unsigned char: CSChar = Int8, CUChar = UInt8

r[ffi.ctype.short]
- short, unsigned short: CShort = Int16, CUShort = UInt16

r[ffi.ctype.int]
- int, unsigned int: CInt = Int32, CUInt = UInt32

r[ffi.ctype.longlong]
- long long, unsigned long long: CLongLong = Int64, CULongLong = UInt64

r[ffi.ctype.float]
- float, double: CFloat = Float32, CDouble = Float

r[ffi.ctype.stdint]
- int8\_t to uint64\_t: Int8 to UInt64

r[ffi.ctype.own]
- char, long, unsigned long, size\_t, ssize\_t, intptr\_t: Own types: CChar, CLong, CULong, CSize, CSSize, CIntPtr

r[ffi.ctype.target-convert]
Each target-width type is built with a checked conversion such as `CSize.try_from(n) -> Option<CSize>`, accepts integer literals that fit in 32 bits, and converts back with `to_int64()` (signed) or `to_uint64()` (unsigned), which never fails on any target.

r[ffi.cbind.params]
`quince` `bind` writes pointer and callback parameters this way:

r[ffi.cbind.ptr]
- `T*`: `Ptr<T>`

r[ffi.cbind.const-ptr]
- `const T*`: `ConstPtr<T>`

r[ffi.cbind.out-param]
- `T**` (an out-parameter): `Ptr<Ptr<T>>`, passed as a `CBuf<Ptr<T>>`'s `ptr()`

r[ffi.cbind.cstring]
- `const char*`: `CString`, lent for the call

r[ffi.cbind.callback]
- A function pointer followed by its `void*` context: One `Callback<fn(...)>` parameter, its arguments in C types

r[ffi.cbind.param-names]
Parameter names come from the header, since every argument after the first is named at the call.

r[ffi.cbind.keyword-param]
A C parameter whose name is a keyword gets `_` appended (`fn` becomes `fn_`).

r[ffi.cbind.callback-name]
The one `Callback` parameter made from a function pointer and its context takes the function pointer's name.

r[ffi.cbind.unnamed]
A parameter the header leaves unnamed becomes `arg1`, `arg2`, and so on by position.

r[ffi.cbind.returns]
Return types follow the same mapping: a returned `T*` becomes `Ptr<T>`, and a returned `const char*` or `char*` becomes `CString`.

r[ffi.cbind.ownership-left]
Both need `@borrowed` or `@owned`, which `quince bind` leaves for the author (Generated bindings).

## Trust

r[ffi.trust.native]
A package containing a `foreign` block is native, which the toolchain sees in its source.

r[ffi.trust.allow]
A project must allow each native dependency by name in its manifest:

```
[project]
allow_native = ["sqlite"]
```

r[ffi.trust.unlisted]
Building with an unlisted native dependency is an error. The effect declared on a foreign block can't be verified, so this list is the audit surface: everything else is checked by the compiler.

## Linking

r[ffi.link.table]
The `[native]` table in `quince``.toml` names every C library the project links and any C sources it builds:

```
[native]
link = ["minz"]            # found with pkg-config, then the system linker's search path
sources = ["native/*.c"]   # optional: C files built with the toolchain's C compiler
include = ["native"]       # header folders for sources and quince bind
```

r[ffi.link.match]
- Every library named by a `foreign` block must match a `link` entry or the library built from `sources`, which takes the project's name. Otherwise it is a compile error that names the missing manifest line.

r[ffi.link.own-only]
- `[native]` covers only the project's own foreign blocks: a native dependency's link entries and sources carry into the final binary without being listed again.

r[ffi.trust.transitive]
- `allow_native` lists every native package in the build by its own name, whether it is a direct, indirect or test-only dependency; a project never lists itself.

r[ffi.link.ship-sources]
- A published package ships its `sources` and builds them on the user's machine.

r[ffi.link.missing]
- A package that only links needs the library installed, and `quince` `build` names the missing library and the pkg-config name it tried.

## Memory across the boundary

r[ffi.cmem.no-escape]
- **Quince** **memory never escapes to C.** Data is copied into C-allocated buffers, or lent in place for the duration of one call (bytes.lend(), above) (Quince never moves memory, so this needs no pinning). C code can never hold a pointer to Quince memory after the call returns, because reference counts can't see it.

r[ffi.cmem.resource-once]
- **C-owned memory is a `Resource<T>`.** A resource declares its cleanup function and is released exactly once: by res.close(), at the end of a with\_resource call, or when its last reference is dropped, whichever comes first.

r[ffi.cmem.with-resource]
- with\_resource hands its body the resource itself and closes it when the body ends, even on error, panic, or cancellation:

```
let changes = ffi.with_resource(Resource.adopt(out.get()), fn(db) {
  sqlite3_changes(db.ptr())
})
```

r[ffi.cmem.drop-close]
- Dropping the last reference to an open resource closes it then and there, a moment reference counting makes exact ([Memory](memory.md)). So a wrapper such as `Db` is correct without any close call, and its `close` method exists for callers that want the C error code.

r[ffi.cmem.pinned-cleanup]
- For a `@pinned` library the cleanup is queued onto that library's thread.

r[ffi.cmem.cycle-warn]
- Only a Resource freed when the cycle collector breaks its cycle ([Memory](memory.md)) logs a warning, since the moment of cleanup is then unpredictable, and `quince` `check` warns about any reference type that can hold a Resource in a cycle.

r[ffi.cmem.use-after-close]
- Using a resource after it is closed is a bug: `res.ptr()` or `res.close()` on a closed resource panics, naming the foreign type and the line that closed it.

r[ffi.cmem.escape-warn]
- `quince` `check` warns when a Resource, or a value holding one, is returned from or escapes a `with_resource` body.

r[ffi.cmem.scope-release]
- A value that owns C memory (a `Resource`, `CBuf`, `Callback` or `CString`, or a value holding one) is never released before the end of the scope that holds it, even after its last use, because C may still hold its pointer.

r[ffi.cmem.drop-order]
- On every exit (normal, `?`, panic, cancellation), locals are released in reverse order of creation, and the fields of a dropped struct in declaration order.

r[ffi.cmem.keep]
- To tie a callback to the handle C stores it in, use `res.keep(cb)`. The same is how to hand C memory it keeps, such as a string it stores: a `CString` passed directly is lent for that call only, so keep it on the resource with `res.keep(cstr)`.

r[ffi.cmem.cleanup-limit]
- Cleanup is shielded from cancellation and has a time limit, 10 seconds unless the resource's declaration sets another. Past the limit the cleanup is cancelled, and the C-level release still runs ([Cancellation](concurrency.md#cancellation)).

**Sharing resources between tasks.**

r[ffi.cresource.crossing]
A `Resource<T>` can't cross a task boundary unless its foreign type is marked `@shareable`, a promise that the C handle is safe to use from several threads.

r[ffi.pool.def]
For the usual case, a handle that isn't, the standard library's `Pool<T, Err, uses E>` holds up to `max` of them and hands each one to a single task at a time:

```
let pool = Pool.new(20, healthy: fn(conn) { conn.is_open() }, fn() { pg.connect(url) })
let rows = pool.checkout(fn(conn) { conn.query("SELECT 1", params: []) })??
```

r[ffi.pool.checkout]
- `pool.checkout` waits for a free value (a cancellation point), lends it to the closure, and returns it afterwards.

r[ffi.pool.closure-crossing]
- The closure is checked like a spawned task: what it captures and what it returns must be able to cross a task boundary.

r[ffi.pool.lent]
- The lent value can't, so it can't be returned, stored or kept after the closure ends, and two tasks never hold it at once.

r[ffi.pool.panic]
- If the closure panics or is cancelled, the value is closed and replaced rather than returned.

r[ffi.pool.close-drop]
- Here and below, the pool closes a value by dropping it, so a Resource inside it closes then; v1 has no close hook.

r[ffi.pool.err]
- An `Err` that the closure returns is an ordinary value, since the pool can't see inside `R`, so the value goes back to the pool.

r[ffi.pool.healthy]
- On every return the pool first calls `healthy` on the value, and closes a value it rejects instead of keeping it, so a connection that died during the closure is never handed out again.

r[ffi.pool.healthy-required]
- `healthy` is required, so every pool states its policy; `fn(_) { true }` keeps every value.

r[ffi.pool.crossing]
- The pool itself can cross task boundaries, so a handler such as `PgDb` holds one and every task that inherits the handler shares it.

## Callbacks

r[ffi.callback.def]
C code may call back into Quince functions passed to it as a Callback\<F>: a resource that keeps the closure alive until it is released, so C can store it beyond one call, as event loops and sqlite hooks do.

r[ffi.callback.effects]
`F` carries the callback's effects like any function type, as in `Callback<fn(CInt, CInt) uses State>`, and they must be among those declared on its `foreign` block.

r[ffi.callback.no-poly]
So a `pub` wrapper fixes a callback's effects, and can't take an effect-polymorphic callback.

r[ffi.callback.handlers]
A callback runs with the effect handlers that were in effect at the call that registered it, so a test that registers one under a fake gets the fake on every later call.

r[ffi.callback.param]
A foreign parameter typed `Callback<fn(...)>` receives a C function pointer plus its context, which `quince` `bind` sets up from the header.

r[ffi.callback.release]
`cb.unregister()` or the end of its scope releases it, unless `res.keep(cb)` has moved it into a resource, whose cleanup then releases it.

r[ffi.callback.new]
- `Callback.new(f)` is for C that may call at any time, from any thread.

r[ffi.callback.new-crossing]
- The closure's captures must be able to cross a task boundary, as for `spawn`, and each call runs as a short task in the scope that registered it.

r[ffi.callback.new-threads]
- The runtime registers each calling thread on first use.

r[ffi.callback.local]
- `Callback.local(f)` is for C that calls back only during a call the task itself is making, such as progress reports, sort comparators and row callbacks.

r[ffi.callback.local-ref]
- The closure may capture a `Ref`, and it runs as part of the task that created it.

r[ffi.callback.local-abort]
- A call from anywhere else aborts the process with a message naming the callback.

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

## Calling C

Green threads complicate foreign calls ([Interop with C](interop.md)), so the runtime handles them specially:

r[ffi.thread-pool]
- The runtime runs foreign calls on a separate thread pool, so a blocking C call can't stall other tasks.

r[ffi.pinned]
- `@pinned` marks a foreign library that must always be called from the same OS thread (e.g., GUI toolkits).

r[ffi.cancel-after]
- A task blocked in a foreign call is cancelled only after the call returns.

r[ffi.callback-cancel]
- Cancellation never interrupts a callback that C calls into Quince ([Interop with C](#callbacks)). It is delivered once the foreign call returns.

r[ffi.callback-panic]
- A panic that reaches a callback's boundary aborts the process, with a message naming the callback, because unwinding through C frames is undefined behavior.

## Testing foreign code

C code does real I/O whatever handler is installed, so a test fake can't stand in for it.

r[ffi.ffitest.opt-in]
A test that reaches a `foreign` block must opt in to each effect the block declares that tests fake by default: a test that calls into `foreign "sqlite3" uses Fs` needs `test "..." uses Fs`, and the compile error names the block and the opt-in to add.

r[ffi.ffitest.pure]
Pure blocks and `uses State` blocks need nothing.

r[ffi.ffitest.step]
The scheduler counts each foreign call as one step. A test that should not touch real I/O replaces the wrapper's own effect with a fake ([Effects](effects.md)).
