# Memory

r[mem.rc]
Quince manages memory with reference counting that the compiler inserts in Quince IR, as Koka (Perceus) and Lean 4 do. There is no tracing collector for ordinary values.

r[mem.borrow]
- **Counts the compiler removes.** A parameter the callee doesn't store is borrowed and touches no count, and a value is moved, not counted, at its last use. Most calls touch no counts at all.

r[mem.c-scope]
- A value that owns C memory is never released before the end of its scope, because C may still hold its pointer ([Interop with C](interop.md)).

r[mem.free]
- **Freeing and reuse.** A value is freed as soon as its last reference goes away, at a point the code determines.

r[mem.reuse]
- When a unique value is consumed by an update that builds a value of the same shape, its memory is reused in place.

r[mem.atomic-shared]
- **Across tasks.** Counts are non-atomic by default. A value becomes shared when it crosses a task boundary (captured by `spawn`, sent on a channel, or stored in an `Atomic<T>` or `ConcurrentMap<K, V>`), and only shared values use atomic count operations.

r[mem.mark-shared]
- The first crossing marks everything reachable from the value as shared, which takes time proportional to its size, so sending a large map on a channel is not free.

r[mem.const-shared]
- Constants are marked shared when they are initialized.

r[mem.no-value-cycles]
- **Cycles.** Plain values can't form cycles, because a value can't be changed through an alias. Only reference types (`Ref<T>`, `Atomic<T>`, `ConcurrentMap<K, V>`, Channel\<T>, TaskScope, Pool\<T, Err, uses E> and Callback\<F>) can, by holding a value that refers back to them.

r[mem.cycle-collector]
- The runtime runs an incremental cycle collector over reference-type objects only.

r[mem.cycle-per-task]
- It collects each task's own Refs inside that task, and the types that cross tasks in a background task, so it never stops all tasks at once.

r[mem.copy-report]
- **Hidden copies are reported.** `quince` `check` and the query API ([Query API](../tooling/diagnostics.md#query-api)) list every update that may copy instead of updating in place, and name the alias that causes it.

- **Large frees.** Freeing a large structure takes time proportional to its size.

r[mem.large-free]
- Above a size threshold, the runtime hands the rest of the free to a background task, so a request handler isn't stalled.

r[mem.resource-not-deferred]
- Resource cleanup is never deferred this way: any Resource inside the structure is cleaned up at the drop point, before the rest is handed off.
