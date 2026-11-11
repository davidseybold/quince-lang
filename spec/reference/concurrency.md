# Concurrency

r[conc.structured]
Quince uses structured concurrency on green threads: every task belongs to a scope, and a scope doesn't exit until all its tasks have finished or been cancelled.

r[conc.no-async]
There is no `async` or `await`; blocking calls look like ordinary calls, and the effect system already shows which ones do I/O.

## Concurrent blocks

```ebnf
concurrent   = "concurrent" [ "(" arg { "," arg } ")" ]
               ( "{" spawn { "," spawn } [ "," ] "}" | "for" pattern "in" expr block )
spawn        = "spawn" expr
```

```quince
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

r[conc.block]
- A `concurrent` block runs its `spawn`ed tasks in parallel and evaluates to `Result<(A, B, ...), E>`: `Ok` holds each task's value, in the order written. Write `concurrent { ... }?` to unwrap it like any other `Result`.

r[conc.error-type]
- Every task that can fail returns `Result` with the same error type `E`. When the error types differ, convert them in the spawn expression, for example `spawn fetch_orders(user).map_err(LoadError.Orders)`; there is no automatic widening ([Errors and failure](errors.md)).

r[conc.plain]
- A task that returns a plain value can't fail, and a block whose tasks all return plain values evaluates to a plain tuple.

r[conc.mixed]
- When some tasks return `Result` and others plain values, the block evaluates to `Result<(A, B, ...), E>`. `Ok` holds each `Result` task's value unwrapped and each plain value as it is, and only the `Result` tasks need the same `E`.

r[conc.static-type]
- Whether a task can fail is decided by its static type at the spawn site: a value of a type parameter is a plain value even when a caller instantiates it with a `Result`, so a generic function behaves the same for every type. The same holds for failure policies.

r[conc.err-cancel]
- If any task returns `Err`, the block cancels its siblings, waits for them to stop, and evaluates to that `Err`.

r[conc.panic]
- If a task panics, the block cancels its siblings the same way and then panics the enclosing task.

r[conc.cancelled-silent]
- Siblings the block cancelled report nothing (see [Cancellation](#cancellation)).

r[conc.settle]
- `concurrent(settle: true) { ... }` runs every task to the end instead. It never cancels on an `Err`, and it evaluates to a tuple of `Result`s in the order written, so each task may have its own error type.

r[conc.settle-panic]
- A panic still cancels the siblings and panics the enclosing task, because a panic is a bug.

r[conc.no-outlive]
- A task can never outlive its block, so when a function returns, all work it started is finished.

r[conc.not-effect]
- Starting tasks is not an effect. Because nothing escapes the scope, a function's signature already describes everything it can do.

## Fan-out over a collection

To run one task per item of a list, put `for` after `concurrent`:

```quince
fn fetch_all(urls: List<Url>) -> Result<List<Page>, FetchError>
  uses Net
{
  concurrent(limit: 10) for url in urls {
    spawn fetch(url)
  }
}
```

r[conc.fanout.result]
- The block evaluates to `Result<List<T>, E>`, with the results in the same order as the input. It follows the same rules as a `concurrent` block: one shared error type, and the first `Err` cancels the remaining tasks.

r[conc.fanout.plain]
- When every task returns a plain value, the block can't fail and evaluates to a plain `List<T>`, as a `concurrent { }` block gives a plain tuple.

r[conc.fanout.limit]
- `limit` caps how many tasks run at once.

r[conc.fanout.limit-required]
- It is required, so every fan-out states how many tasks run at once.

r[conc.fanout.limit-type]
- It takes any `Int` expression, with `requires limit >= 1`.

r[conc.fanout.settle]
- `settle: true` works here too. `concurrent(limit: 10, settle: true) for ...` never cancels on an `Err` and evaluates to `List<Result<T, E>>`.

r[conc.fanout.one-spawn]
- The body holds exactly one `spawn`. It can bind values before it, like any block, but it can't spawn twice or return early.

## Long-lived work (fire and forget)

```ebnf
background   = "background" "(" name ":" "TaskScope" [ "," arg ] ")" block
```

Work that must outlive the current function is spawned into a longer-lived scope passed in as a `TaskScope` value:

```quince
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

r[conc.background.taskscope-param]
- Only a function that takes a `TaskScope` parameter can start work that outlives it, so this is visible in the signature.

r[conc.background.spawn-in]
- `spawn in bg expr` starts `expr` as a task in the scope `bg`. It follows the same rule as `spawn` inside `concurrent`: the whole expression is evaluated in the new task.

r[conc.background.spawn-type]
- The form has type `Unit`, and `expr` must be `Unit` or `Result<Unit, E>` where `E` implements `Display`, so no task's result is silently thrown away.

r[conc.background.scope-value]
- The scope can be any `TaskScope` value, such as `self.bg`. `spawn` is a keyword, so a `TaskScope` has no `spawn` method.

r[conc.background.policy]
- The scope's owner sets the failure policy with `on_error`, which takes a `FailurePolicy`: `FailurePolicy.LogAndContinue`, `FailurePolicy.Restart(max: n)`, or `FailurePolicy.FailScope` (the default).

r[conc.background.log-policy]
- `LogAndContinue` writes through the `Log` effect, so the enclosing function needs `uses Log`.

r[conc.background.log-import]
- `Log` comes from std.log, and this is the one place the language names a std effect, so code that uses this policy imports it: `import std.log.{Log}`.

r[conc.background.failure]
- A failure is a task that panics or returns `Err`.

r[conc.background.policy-semantics]
- Under `LogAndContinue` the failure is logged and the scope carries on. Under `Restart(max: n)` the task is started again, up to n times. The count is per spawned task. Under `FailScope`, or once the restarts run out, the scope cancels its remaining tasks and fails.

r[conc.background.result]
- A `background` block evaluates to `Result<T, ScopeFailed>`, where `T` is the body's value.

r[conc.background.scope-failed]
- `ScopeFailed` names the spawn site of the task that failed and holds its panic message or the `Display` text of its error. Cancellation is never a failure (see [Cancellation](#cancellation)).

r[conc.background.exit]
- When a `background` block exits, it cancels remaining tasks and waits for them, giving clean shutdown.

r[conc.background.owned]
- A task with no owning scope is impossible by construction.

r[conc.background.no-escape]
- A `background` block's value can't contain a `TaskScope` (a compile error that names the field path), and spawning into a scope whose block has already exited panics, naming the spawn site and the `background` line.

r[conc.background.store]
- Storing a scope in a handler or a struct for the block's lifetime is fine.

r[conc.background.handlers]
- A spawned task keeps the handlers that were in effect where it was spawned, even after it outlives that `with` block. If a test runs ` with Net = fake {  ``handle(req, bg:)` `}` and `handle` spawns analytics work into `bg`, that work uses the fake. A handler therefore lives as long as the longest task that captured it.

## Cancellation

r[conc.cancel.cooperative]
- Cancellation is cooperative.

r[conc.cancel.stop]
- A cancelled task stops at its next cancellation point (listed in [Cancellation](#cancellation)), and execution doesn't continue past it.

r[conc.cancel.unwind]
- Instead the task unwinds to the scope that cancelled it, running `Resource` cleanup and closing any scopes it opened on the way ([Interop with C](interop.md)).

r[conc.cancel.not-error]
- Cancellation is never an error value: `?` never sees it, and no `Result` type has to carry it.

r[conc.cancel.points]
- The cancellation points are every effectful call, including package effects, since they are built on the others; every operation that waits (`receive`, `send` on a full channel, and `select`); and the checks the compiler inserts at the top of every loop iteration and at every function entry.

r[conc.cancel.state]
- A `State` operation that doesn't wait is never a cancellation point (this means a reference-type method; a package effect's operation is always a cancellation point, even when the effect is built only on State), so an update to a `Ref` or `Atomic` is never stopped halfway. Waiting changes nothing, so stopping a task while it waits is always safe.

r[conc.cancel.no-catch]
- Code can't catch or suppress cancellation.

r[conc.cancel.scope-sees]
- Only the scope that cancelled a task sees it: a `concurrent` block discards the siblings it cancelled, a `background` block treats cancelled tasks as shut down, and `with_timeout` reports it as a value ([Cancellation](#cancellation)).

r[conc.cancel.not-failure]
- Cancellation is not a failure, so an `on_error` policy never sees it. This keeps cancellation out of every error type, and there is no catch-all for an agent to write that would swallow it by accident.

r[conc.cancel.pure]
- Pure code needs no explicit check: the compiler-inserted checks stop a long pure computation too, so every task can be cancelled. A pure function stays pure ([Effects](effects.md)), because a cancelled computation's result is never observed.

r[conc.cancel.preempt]
- The same checks let the scheduler preempt a long-running task, so it can't starve the others.

r[conc.timeout.scope]
- `time.with_timeout(dur``"5s", fn() { ... })` is a scope that cancels its contents when the time runs out.

r[conc.timeout.result]
- It evaluates to `Result<T, TimedOut>`, where `T` is the body's value, so the caller must handle the timeout.

r[conc.timeout.own-timer]
- It reports only its own timer: if an enclosing scope cancels the task while the body runs, the cancellation keeps unwinding past `with_timeout` and never becomes `Err(TimedOut)`.

r[conc.timeout.clock]
- It needs `uses Clock`, because it reads the time, so under `quince` `test` the timer runs on the fake clock and fires at the same point on every run.

r[conc.cancel.cleanup-shield]
- Cleanup that runs while a task unwinds is shielded from cancellation, so a graceful close (flushing a buffer, sending a goodbye) finishes.

r[conc.cancel.cleanup-limit]
- It has a time limit, 10 seconds unless the resource declares another ([Interop with C](interop.md)). Past the limit the cleanup itself is cancelled, and the resource's C-level release still runs.

r[conc.cancel.cleanup-panic]
- A panic in cleanup doesn't stop the unwinding. The remaining cleanups still run, and the task ends as panicked, whether it was unwinding from a cancellation or from an earlier panic. The owning scope handles it like any other panic, and the report lists every panic that occurred.

## Shared state

r[conc.share.values]
- Values are immutable by default and can be shared freely between tasks.

- A `Ref<T>` cannot cross a task boundary, and neither can any value that contains one ([Reference types](types.md)).

r[conc.share.spawn-reject]
- The compiler rejects capturing such a value in a `spawn`, and the error names the field that holds the reference.

r[conc.share.channels]
- Tasks communicate through `Channel<T>` or explicitly concurrent types such as `Atomic<T>` and `ConcurrentMap<K, V>`.

r[conc.share.state]
- Every operation on them requires `uses State`.

r[conc.share.no-races]
- Data races are impossible in safe code.

r[conc.chan.capacity]
- A `Channel<T>` has a fixed capacity, set when it is created.

r[conc.chan.send]
- `ch.send(v)` waits while the channel is full and returns `Err(ChannelClosed)` once it has been closed.

r[conc.chan.receive]
- `ch.receive()` waits for a value and returns `Option<T>`, which is `None` once the channel is closed and empty.

r[conc.chan.close]
- `ch.close()` closes it, and values already sent can still be received.

r[conc.share.modify-pure]
- The closures passed to `Ref.modify`, `Atomic.modify` and `ConcurrentMap.update` must be pure.

r[conc.share.modify-atomic]
- The update is applied only after the closure returns, so a cancellation inside the closure leaves the reference unchanged, and an update is never applied halfway.

## Waiting on several channels

```ebnf
select       = "select" "{" { pattern "=" expr "=>" ( block | expr ) "," } "}"
```

r[conc.select.wait]
`select` waits on several channel operations at once and runs the arm of the first one that is ready:

```quince
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

r[conc.select.arm]
- Each arm is `pattern = operation => expression`, where the operation is a `receive()` or a `send(v)` on a channel. Nothing else can be an arm.

r[conc.select.irrefutable]
- An arm's pattern must be irrefutable, usually a plain name. A refutable one is a compile error, and its fix binds a name and matches in the body.

r[conc.select.closed]
- A `receive()` on a closed, empty channel is ready at once and yields `None`, so a loop that keeps selecting on it spins. On `None`, leave the loop and drain the other channel with `.iter()`.

r[conc.select.body]
- An arm's body is an ordinary block of the enclosing function, not a closure, so it may assign the function's `var`s and `return` from the function.

r[conc.select.value]
- The `select` evaluates to the value of the arm that ran, so every arm has the same type.

r[conc.select.random]
- When several arms are ready at once, one is picked at random.

r[conc.select.seeded]
- Under `quince` `test` the pick is part of the seeded schedule, so a failure replays exactly.

r[conc.select.no-default]
- There is no timeout arm and no default arm. For a deadline, wrap the `select` in `with_timeout`; for shutdown, cancel the task. A `select` is a cancellation point like any other wait.

## Failure, timeout and channel types

```quince
pub struct TimedOut { pub after: Duration }
pub struct ChannelClosed
pub struct ScopeFailed { pub spawn_site: String, pub message: String }
pub enum FailurePolicy { LogAndContinue, Restart(max: Int), FailScope }
```
