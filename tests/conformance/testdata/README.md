# Conformance fixtures

Each `.qnc` file below this folder is one conformance test, run by `cargo xtask conformance`. Folders group fixtures by topic, such as `bindings/shadow_same_scope.qnc`. This is a `testdata/` folder, so nothing here is compiled as part of a Quince project.

A fixture starts with a header: comment lines (and blank lines) before any code.

```
// rule: bind.shadow.same-scope
// Shadowing a binding in the same scope is an error.
fn f() {
  let x = 1
  let x = 2 //~ E0100
}
```

Header lines of the form `// <word>:` with a lowercase word are directives. The directives are `rule`, `mode` and `exit`. Any other lowercase word before a colon is an error, and so is a case variant such as `// Mode:`. Other comments are free text, including `// Note: ...`. A directive after the first line of code is an error too.

- `// rule: <id>` cites the spec rule the fixture tests. At least one is required, each ID once. Every ID must exist in `spec/quince-spec.md` (or be `grammar.<production>`) and not be retired.
- `// mode: <mode>` is set at most once:
  - `check` (the default) runs the whole checker as far as it exists.
  - `parse` runs only the parser, so a syntax fixture keeps passing when later stages add diagnostics such as unused bindings.
  - `run` builds and runs the file. Its program must build without errors. `// exit: <n>` sets the expected exit code (default 0). Standard output must equal the sibling `<name>.stdout` file, and with no such file it must be empty. Standard error isn't compared yet.
- `//~ CODE` expects a diagnostic with that code starting on the same line. `//~^ CODE` expects one on the line above, and each extra `^` moves one more line up.
  - A comment may list several codes, `//~ E0001 E0001`, and each code expects one diagnostic. Nothing but codes may follow `//~`.
  - A diagnostic at the very end of a file that ends with a newline counts as being on the last line.
  - Every error and warning the compiler reports must be expected.

Write the fixture from the spec text alone, before the code that makes it pass. Until a compiler backend is wired into the runner, every fixture fails with "no backend", so a fixture lands in the same PR as the code that makes it pass.
