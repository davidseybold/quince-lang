# Errors and failure

Quince separates expected failures, which are values in the return type, from bugs, which panic and stop the program. r[err.panic.no-catch] There are no exceptions and no way to catch a panic in ordinary code: a panic ends its task, and only the scope that owns the task decides what happens next ([Concurrency](concurrency.md)). Cancellation is a third case, separate from both: it unwinds a task but is neither an error value nor a bug ([Cancellation](concurrency.md#cancellation)).

## Expected failures: `Result` and `Option`

- r[err.result-option] A function that can fail returns `Result<T, E>`. A function that may have no answer returns `Option<T>`.
- Error types are usually enums, so callers can match on every failure mode:

```
pub enum ParseError {
  Empty,
  InvalidChar(at: Int, found: Char),
  TooLong(max: Int),
}
```

- r[err.unused-result] An unused `Result` is a compile error. r[err.discard] To discard one deliberately, write `let _ = save(file)`. r[err.unused-value] An unused value of any other type except `Unit` is a warning, which `let _ =` also silences. r[err.var-self-value] A value returned by a `var self` method, such as the `Option` from `Map.insert`, raises no warning.

## The `?` operator

- r[err.try.unwrap] `expr?` unwraps `Ok`/`Some` or returns early with the `Err`/`None`.
- r[err.try.from] The error must convert to the enclosing function's error type through an explicit `impl From<A> for B`. There is no automatic widening, so every error path can be traced by searching for its `From` impl.
- `.map_err(f)` and `.ok_or(err)` cover cases with no `From` impl. r[err.try.same-type] When the error type already equals the function's error type, `?` needs no `From` impl. r[err.try.closure] Inside a closure, `?` returns from the closure, not from the enclosing function. r[err.try.option] `?` on an `Option` works only in a function that returns `Option`; in one that returns `Result`, the suggested fix is `.ok_or(e)?`.

## Panics (bugs)

A panic means the program has a bug, not that something in the world went wrong. These panic:

- r[err.panic.arith] Integer overflow, division by zero, and a shift count outside the bit width
- r[err.panic.index] Out-of-bounds indexing with `xs[i]` (use `xs.get(i)` for an `Option`)
- r[err.panic.contract] A failed `requires` or `ensures` contract ([Contracts](contracts.md))
- r[err.panic.explicit] An explicit `unreachable("reason")` or `todo("reason")`. Both have type Never ([Types](types.md)), so either can stand in for any value while code is still being written

r[err.panic.message] Every panic message includes the source location and, for contracts, the values that violated the condition. r[err.panic.task] A panic stops the current task; the scope that owns the task may log the panic or restart it, as its FailurePolicy says ([Concurrency](concurrency.md)), but cannot inspect or resume it. That scope boundary is the only place a panic is handled.
