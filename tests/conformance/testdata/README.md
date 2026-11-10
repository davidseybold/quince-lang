# Conformance fixtures

Each `.qnc` file below this folder is one conformance test, run by `cargo xtask conformance`. Folders group fixtures by topic, such as `bindings/shadow_same_scope.qnc`. This is a `testdata/` folder, so nothing here is compiled as part of a Quince project.

A fixture starts with header comments, before any code:

```
// rule: bind.shadow.same-scope
// Shadowing a binding in the same scope is an error.
fn f() {
  let x = 1
  let x = 2 //~ E0100
}
```

- `// rule: <id>` cites the spec rule the fixture tests. At least one is required, and every ID must exist in `spec/quince-spec.md` (or be `grammar.<production>`).
- `// mode: check` is the default. `// mode: run` builds and runs the file instead. A run fixture can set `// exit: <n>` (default 0), and compares standard output with a sibling `<name>.stdout` file when one exists.
- `//~ CODE` expects a diagnostic with that code starting on the same line. `//~^ CODE` expects one on the line above, and each extra `^` moves one more line up. Every error and warning the compiler reports must be expected.

Write the fixture from the spec text alone, before the code that makes it pass. Until a compiler backend is wired into the runner, every fixture fails with "no backend", so a fixture lands in the same PR as the code that makes it pass.
