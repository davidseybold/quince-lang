# Testing

## Executable examples in doc comments

```quince
/// Splits a full name into first and last.
///   >>> split_name("Ada Lovelace")
///   Some(("Ada", "Lovelace"))
///   >>> split_name("")
///   None
pub fn split_name(full: String) -> Option<(String, String)> { ... }
```

r[test.doc.run]
`quince` `test` runs every example. A doc example that fails is a test failure, so documentation can't drift from behavior.

r[test.doc.compare]
The line after a `>>>` line is a Quince expression, compared with the result using `==`, and examples run under the same effect fakes as tests.

## Unit tests

```ebnf
test_decl    = "test" string [ "forall" "(" params ")" ] [ "uses" effects ] block
```

r[test.unit-location]
- `test "description" { ... }` blocks live in the same file as the code they test, after the code.

r[test.name]
- A test's name is plain text: it doesn't interpolate, so braces in it need no escape.

r[test.name-escapes]
- A test's name takes the same escapes as a string, but `{` is plain text.

r[test.attr]
- A top-level declaration marked `@test`, such as a fake handler, is compiled only by `quince` `test`.

r[test.attr-pub]
- A `pub` `@test` declaration in a library is also visible to the test code of packages that depend on it, so a library can ship a fake for its own effect.

r[test.attr-use]
- Naming it from code that isn't a test is a compile error.

r[test.cmd]
- Program packages are no exception: `quince` `test` runs the tests in every `cmd/<name>/` folder too.

r[test.fakes]
- Tests may use any effect, but `Net`, `Clock`, and `Random` are replaced with deterministic fakes unless a test opts in, written test "name" uses Net { ... }.

r[test.uses]
- A test needs no `uses` for any other effect: its body may use every effect, and `uses` on a test only swaps a fake for the real one.

r[test.clock-sleep]
- Under every fake clock, the default one included, `Clock.sleep` moves fake time forward instead of waiting, so a `with_timeout` in a test fires without real waiting.

r[test.clock-virtual]
- A test has one fake clock, shared by all its tasks, and its time is virtual: a sleep blocks its task until fake time reaches the sleep's deadline, and fake time jumps to the earliest pending deadline only when every task in the test is blocked. So a task that can still run never times out because another task slept.

r[test.clock-timeout]
- A timeout that fires during a sleep moves fake time only to the timeout's own deadline, which is what Clock.monotonic() then reads.

r[test.default-fakes]
- The default fake clock starts at `ts"2000-01-01T00:00:00Z"`, the default `Net` refuses every connection with `NetError.Refused`, and the default `Random` is seeded from the run's seed.

r[test.fakes-fs-console]
- `Fs` and `Console` are faked by default too: `Fs` is an empty in-memory file system and `Console` output is captured.

r[test.fakes-other]
- Env is empty, Listen accepts only in-memory connections from the same test, and Process is refused unless the test opts in.

r[test.deterministic]
- A test with no opt-ins is fully deterministic, so the same seed gives the same run.

r[test.opt-in-report]
- Opting into a real effect (`uses Net`, `uses Fs`) gives that up, and the test report says which tests did.

r[test.foreign-opt-in]
- Foreign code ignores fakes, so a test that reaches a foreign block must opt in to each faked effect the block declares ([Interop with C](interop.md)).

r[test.package-effect-proof]
- A test that reaches a package effect with no `with` around it is a compile error that names the call path, the same proof as for `main`. The suggested fix wraps the call in `with Db = ...`. Built-in effects keep their default fakes ([Testing](#unit-tests)); package effects have none, because no fake database can be guessed.

r[test.default-handler]
- An effect's own package may name one with `@test_default(f)` on the effect declaration, where `f` takes no arguments and returns a handler. std.log names `capture_log`, so a test of code that logs needs no `with Log`.

r[test.assert-values]
- Assertions show the full values on both sides of a failed comparison.

## Integration tests

Integration tests check how a package behaves from the outside, through its public API only. They live in a top-level `tests/` directory instead of beside the code they test.

r[test.itest.packages]
- Each file directly in `tests/`, and each folder tests/\<name>/, is compiled as a separate test package that depends on the library ([Packages](packages.md#packages)), so it can use only `pub` names, exactly as an outside user would.

- They don't break when internals are rewritten, which makes them the main safety net for refactoring.

r[test.itest.same-syntax]
- Same `test` block syntax, effect fakes, and property tests as unit tests.

r[test.itest.flags]
- `quince` `test` runs both; `quince` `test --unit` or `--integration` runs one kind.

## Property tests

```quince
test "parse inverts display" forall (n: Int) {
  assert(Int.parse(n.display()) == Ok(n))
}
```

r[test.prop.forall]
- `forall` generates inputs from the types; every type has a derived generator unless it is marked @opaque or has private fields.

r[test.prop.filter]
- Generated values are filtered against the type's requires invariants with a retry limit, and when the limit is reached the test reports which invariant it couldn't satisfy instead of passing.

r[test.prop.custom]
- `impl Generate for T` replaces the derived generator, or provides one for an `@opaque` type or a type with private fields, usually by calling the type's own constructor.

r[test.prop.discard]
- A generated case that breaks a `requires` of the function under test is discarded, not counted as a failure, and the report says how many were discarded.

r[test.prop.contracts]
- Contracts on the function under test act as extra assertions on every generated case.

r[test.prop.shrink]
- Failing cases are shrunk to a minimal input and saved to `.``quince``/regressions`, which reruns them on every later test run.

## Deterministic testing

r[test.dtest.seeded]
Under `quince` `test`, the scheduler runs tasks in a seeded, reproducible order and explores several orderings per test.

r[test.dtest.replay]
A failure prints its seed, and `quince` `test --seed <n>` replays it exactly.

r[test.dtest.real-one]
A test that opts into a real effect runs one ordering, so its real I/O happens once. How many orderings the other tests explore is set by the toolchain.
