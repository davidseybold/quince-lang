# Program execution

## Capabilities at runtime

r[exec.cap.main]
Each effect is backed by a capability that the runtime supplies only to `main`:

```quince
fn main() -> Result<Unit, AppError> uses * { ... }
```

r[exec.cap.static]
- A uses clause is a static permission.

r[exec.cap.context]
- At runtime, the capability behind each effect is implicit context that travels with each call. The compiler guarantees it is present wherever a uses clause says so, and nothing is passed by hand.

r[exec.cap.with]
- A with block replaces that context for the code inside it.

r[exec.cap.inherit]
- Tasks inherit this context: a spawned task keeps the handlers in effect where it was spawned ([Concurrency](concurrency.md)), and a C callback uses those of the call that registered it ([Interop with C](interop.md)).

r[exec.cap.no-capture]
- Nothing else captures handlers: a closure's effects go to the handlers in effect where it is called, and a lazy `Iter`'s to those in effect where it is pulled, even when it was created inside a different `with`.

r[exec.cap.manifest-limits]
- A program's entry in quince.toml (\[cmd.\<name>\]) can narrow its capabilities, e.g., `Net` limited to `api.example.com` or `Fs` limited to `./data`.

r[exec.cap.violation]
- A violation is not a panic: the operation returns Err(NetError.PermissionDenied) or Err(FsError.PermissionDenied), and every std error type built on Net or Fs carries that error, directly or inside the NetError or FsError it wraps, and its message names the manifest line.

r[exec.cap.dependency]
- A dependency can never use an effect its declared API doesn't list, so auditing a package means reading its signatures.

r[exec.cap.query-effects]
- quince query effects \<path> prints a function's effects with every package effect expanded to the built-in effects beneath it, which is the view to audit.

**Planned after v1: finer-grained limits.** Effects in signatures stay coarse (`uses Net`, not `uses Net("host")`): hosts are often dynamic, and per-host effects would churn signatures and turn new endpoints into breaking changes. Finer control will come later through one more runtime mechanism, which v1's runtime and manifest format must not rule out:

- Per-dependency limits in the manifest (e.g., an analytics package may reach only its own events host and no files), as a defense against compromised dependencies.

## The `main` function and process exit

r[exec.main.err-exit]
- If `main` returns `Err(e)`, the runtime prints `e`'s `Display` to standard error and exits with code 1.

r[exec.main.panic-exit]
- If the main task panics, the runtime prints the panic report to standard error and exits with code 101.

r[exec.main.process-exit]
- `Process.exit(code)` cancels every running scope, waits for cleanup, then exits with `code`. It flushes standard output and standard error first.

r[exec.main.signature]
- `main` is written `fn main()` or `fn main() -> Result<Unit, E>`, where `E` implements `Display`, with `uses *` on either.

r[exec.main.no-package-effects]
- `main` gets no package effects from `uses *`.

r[exec.main.handler-proof]
- The compiler proves that every package effect reachable from `main` is covered by a `with`; a missing one is a compile error that names the call path.

r[exec.main.uses-star]
- `uses *` is allowed only on `main`. Anywhere else it is a compile error, and `quince` `fix` writes the real `uses` clause. Tests opt into real effects with `test "name" uses Net`.

r[exec.broken-pipe]
- Writes to a closed standard output end the program quietly with code 0, as Unix tools do on a broken pipe, so `Console` writes return nothing to check.
