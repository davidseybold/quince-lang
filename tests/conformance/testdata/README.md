# Conformance fixtures

Each `.qnc` file below this folder is one conformance test, run by `cargo xtask conformance`. Folders group fixtures by topic, and a fixture is named by its path without the extension, such as `bindings/shadow_same_scope`. Fixtures run in sorted path order. This is a `testdata/` folder, so nothing here is compiled as part of a Quince project.

A fixture starts with a header: comment lines (and blank lines) before any code.

```
// rule: stmt.bind.shadow.same-scope
// Shadowing a binding in the same scope is an error.
fn f() {
  let x = 1
  let x = 2 //~ E0100
}
```

Header lines of the form `// <word>:`, where the word is lowercase ASCII letters and hyphens, are directives. The directives are `rule`, `mode` and `exit`. Any other such word before a colon is an error, and so is a case variant such as `// Mode:`. Other comments are free text, including `// Note: ...`. After the first line of code, a comment whose word is `rule`, `mode` or `exit` in any letter case is an error that says to move it into the header.

- `// rule: <id>` cites the spec rule the fixture tests. At least one is required, each ID once. Every ID must be tagged in a spec chapter listed in `spec/chapters.txt` (or be `grammar.<production>`) and not be retired.
- `// mode: <mode>` is set at most once. Modes are an open set: later milestones add more (`fmt` in M1), and an unknown mode is an error that lists the known ones.
  - `check` (the default) runs the whole checker as far as it exists. A check fixture must be a valid program under the whole spec apart from its expected diagnostics, so it keeps passing when later stages arrive. No tool can tell until those stages exist, so review enforces this.
  - `parse` runs only the parser, even after later stages exist, so a syntax fixture keeps passing when stages such as name resolution or lints add diagnostics (an unused binding, for example).
  - `run` builds and runs the file. Its program must build without errors, and warnings aren't judged (test those with a check fixture). `// exit: <n>`, set at most once, gives the expected exit code (default 0). Standard output must equal the sibling `<name>.stdout` file, and with no such file it must be empty. Standard error isn't compared yet.
- `//~ CODE` expects a diagnostic with that code starting on the same line. `//~^ CODE` expects one on the line above, and each extra `^` moves one more line up. Expectations apply only to `check` and `parse` fixtures, and `exit` and a `.stdout` file only to `run` fixtures. Anything else is an error.
  - A comment may list several codes, `//~ E0001 E0001`, and each code expects one diagnostic. Each diagnostic satisfies at most one expectation. Nothing but codes may follow `//~`.
  - A diagnostic at the very end of a file that ends with a newline counts as being on the last line.
  - Every error and warning the compiler reports must be expected.
- Every fix a backend reports is checked with `Fix::check` (`compiler/diagnostics/README.md`), and a malformed fix fails the fixture.

A `.stdout` file with no `.qnc` file of the same name is an error, because it's usually left behind by a rename. So is any failure to read a `.stdout` file other than "not found".

Write the fixture from the spec text alone, before the code that makes it pass. Until a compiler backend is wired into the runner, every fixture fails with "no backend", never a skip, so a fixture lands in the same PR as the code that makes it pass.

## Coverage report

Each run writes `target/conformance/coverage.json` and prints the total and covered counts per spec chapter. A rule is covered only when at least one passing fixture cites it, so with no fixtures every rule is uncovered and the run still succeeds. The report has this shape:

```
{ "spec_version", "total", "covered",
  "sections": [{ "section", "total", "covered",
                 "rules": [{ "id", "covered_by": [<fixture name>] }] }] }
```

- `spec_version` is the spec's draft version, such as `"0.5"`, read from the header of `spec/README.md`.
- Sections are chapters in the order of `spec/chapters.txt`, each named by its title there. A chapter with no rule IDs is left out. Grammar IDs come last, under `Grammar productions`.
- An empty `covered_by` means the rule is uncovered.
