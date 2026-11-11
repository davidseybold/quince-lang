# Quince Language Spec

## 1. Overview and design principles

### Target domain

Quince is built for services and tools: web servers and APIs, command-line tools, data pipelines, and automation. It relies on compiler-inserted reference counting and a green-thread runtime, and accepts their small runtime costs in exchange for never making anyone manage memory by hand.

- **Non-goals:** operating systems, drivers, and embedded devices, which can't host a runtime; and latency-critical work such as game engines and audio, where every allocation and free must be under the programmer's control.
- **Future option:** arenas or regions (memory allocated and freed as a group) for hot paths that can't afford per-object reference counting. The runtime and Quince IR should not rule this out, but it is not part of version 1.
- **Ruled out:** an ownership system like Rust's. It would reach every domain but reintroduces the complexity this language exists to avoid.

### Core principles

1. **Locality.** Everything that affects a function's behavior is visible in its signature, its body, and the declarations of the types and effects it names: its types, its effects, its failure modes, and its contracts.
2. **One way to do it.** Where two constructs would overlap, the language keeps one, and where it keeps two (such as is and match) the spec says when to use each. Fewer choices means fewer wrong choices.
3. **The compiler is a collaborator.** Diagnostics are precise, machine-readable, and come with suggested fixes. The compiler can be queried, not just run.
4. **Correctness is stated, then checked.** Types, exhaustive matching, contracts, and executable examples turn intent into something the toolchain verifies.
5. **Nothing changes meaning silently.** Published code never changes behavior under a dependent. Changes to the shape of a public API (types, signatures, effects and contracts) are detected mechanically.
6. **Familiar surface.** Syntax stays close to TypeScript, Rust, and Go so knowledge transfers. Novelty goes into semantics and tooling, not punctuation.

## 2. Lexical structure and syntax

### Formatting rules (enforced by `quince` `fmt`)

- r[tool.fmt.one-per-line] One declaration, statement, or match arm per line.
- r[tool.fmt.trailing-comma] Trailing commas required in all multi-line lists, arguments, fields, and arms.
- r[tool.fmt.width] 100-column limit; the formatter decides all line breaks.
- r[tool.fmt.imports] Imports sorted and grouped (standard library, then packages, then local).
- r[tool.fmt.check] Unformatted code compiles but fails `quince` `check`, which CI runs by default.

## 3. Type system

### Methods

- A struct literal may write a field whose value is a binding of the same name once: `Dashboard(profile:, orders:)`. r[tool.fmt.field-shorthand] `quince` `fmt` rewrites `profile: profile` to that form, so each literal has one canonical layout.

## 5. Effects and capabilities

### Capabilities at runtime

r[exec.cap.main] Each effect is backed by a capability that the runtime supplies only to `main`:

```
fn main() -> Result<Unit, AppError> uses * { ... }
```

- r[exec.cap.static] A uses clause is a static permission. r[exec.cap.context] At runtime, the capability behind each effect is implicit context that travels with each call. The compiler guarantees it is present wherever a uses clause says so, and nothing is passed by hand. r[exec.cap.with] A with block replaces that context for the code inside it.
- r[exec.cap.inherit] Tasks inherit this context: a spawned task keeps the handlers in effect where it was spawned ([Concurrency](reference/concurrency.md)), and a C callback uses those of the call that registered it ([Interop with C](#callbacks)). r[exec.cap.no-capture] Nothing else captures handlers: a closure's effects go to the handlers in effect where it is called, and a lazy `Iter`'s to those in effect where it is pulled, even when it was created inside a different `with`.
- r[exec.cap.manifest-limits] A program's entry in quince.toml (\[cmd.\<name>\]) can narrow its capabilities, e.g., `Net` limited to `api.example.com` or `Fs` limited to `./data`. r[exec.cap.violation] A violation is not a panic: the operation returns Err(NetError.PermissionDenied) or Err(FsError.PermissionDenied), and every std error type built on Net or Fs carries that error, directly or inside the NetError or FsError it wraps, and its message names the manifest line.
- r[exec.cap.dependency] A dependency can never use an effect its declared API doesn't list, so auditing a package means reading its signatures. r[exec.cap.query-effects] quince query effects \<path> prints a function's effects with every package effect expanded to the built-in effects beneath it, which is the view to audit.
- r[exec.main.err-exit] If `main` returns `Err(e)`, the runtime prints `e`'s `Display` to standard error and exits with code 1. r[exec.main.panic-exit] If the main task panics, the runtime prints the panic report to standard error and exits with code 101. r[exec.main.process-exit] `Process.exit(code)` cancels every running scope, waits for cleanup, then exits with `code`. It flushes standard output and standard error first. r[exec.main.signature] `main` is written `fn main()` or `fn main() -> Result<Unit, E>`, where `E` implements `Display`, with `uses *` on either.

**Planned after v1: finer-grained limits.** Effects in signatures stay coarse (`uses Net`, not `uses Net("host")`): hosts are often dynamic, and per-host effects would churn signatures and turn new endpoints into breaking changes. Finer control will come later through one more runtime mechanism, which v1's runtime and manifest format must not rule out:

- Per-dependency limits in the manifest (e.g., an analytics package may reach only its own events host and no files), as a defense against compromised dependencies.

### Handlers in tests and production

- r[exec.main.no-package-effects] `main` gets no package effects from `uses *`. r[exec.main.handler-proof] The compiler proves that every package effect reachable from `main` is covered by a `with`; a missing one is a compile error that names the call path.
- r[exec.main.uses-star] `uses *` is allowed only on `main`. Anywhere else it is a compile error, and `quince` `fix` writes the real `uses` clause. Tests opt into real effects with `test "name" uses Net`.
- r[test.package-effect-proof] A test that reaches a package effect with no `with` around it is a compile error that names the call path, the same proof as for `main`. The suggested fix wraps the call in `with Db = ...`. Built-in effects keep their default fakes ([Testing](#unit-tests)); package effects have none, because no fake database can be guessed. r[test.default-handler] An effect's own package may name one with `@test_default(f)` on the effect declaration, where `f` takes no arguments and returns a handler. std.log names `capture_log`, so a test of code that logs needs no `with Log`.

## 6. Contracts and testing

### Executable examples in doc comments

```
/// Splits a full name into first and last.
///   >>> split_name("Ada Lovelace")
///   Some(("Ada", "Lovelace"))
///   >>> split_name("")
///   None
pub fn split_name(full: String) -> Option<(String, String)> { ... }
```

r[test.doc.run] `quince` `test` runs every example. A doc example that fails is a test failure, so documentation can't drift from behavior. r[test.doc.compare] The line after a `>>>` line is a Quince expression, compared with the result using `==`, and examples run under the same effect fakes as tests.

### Unit tests

- r[test.unit-location] `test "description" { ... }` blocks live in the same file as the code they test, after the code. r[test.name] A test's name is plain text: it doesn't interpolate, so braces in it need no escape. r[test.attr] A top-level declaration marked `@test`, such as a fake handler, is compiled only by `quince` `test`. r[test.attr-pub] A `pub` `@test` declaration in a library is also visible to the test code of packages that depend on it, so a library can ship a fake for its own effect. r[test.attr-use] Naming it from code that isn't a test is a compile error. r[test.cmd] Program packages are no exception: `quince` `test` runs the tests in every `cmd/<name>/` folder too.
- r[test.fakes] Tests may use any effect, but `Net`, `Clock`, and `Random` are replaced with deterministic fakes unless a test opts in, written test "name" uses Net { ... }. r[test.uses] A test needs no `uses` for any other effect: its body may use every effect, and `uses` on a test only swaps a fake for the real one. r[test.clock-sleep] Under every fake clock, the default one included, `Clock.sleep` moves fake time forward instead of waiting, so a `with_timeout` in a test fires without real waiting. r[test.clock-virtual] A test has one fake clock, shared by all its tasks, and its time is virtual: a sleep blocks its task until fake time reaches the sleep's deadline, and fake time jumps to the earliest pending deadline only when every task in the test is blocked. So a task that can still run never times out because another task slept. r[test.clock-timeout] A timeout that fires during a sleep moves fake time only to the timeout's own deadline, which is what Clock.monotonic() then reads. r[test.default-fakes] The default fake clock starts at `ts"2000-01-01T00:00:00Z"`, the default `Net` refuses every connection with `NetError.Refused`, and the default `Random` is seeded from the run's seed.
- r[test.fakes-fs-console] `Fs` and `Console` are faked by default too: `Fs` is an empty in-memory file system and `Console` output is captured. r[test.fakes-other] Env is empty, Listen accepts only in-memory connections from the same test, and Process is refused unless the test opts in. r[test.deterministic] A test with no opt-ins is fully deterministic, so the same seed gives the same run. r[test.opt-in-report] Opting into a real effect (`uses Net`, `uses Fs`) gives that up, and the test report says which tests did. r[test.foreign-opt-in] Foreign code ignores fakes, so a test that reaches a foreign block must opt in to each faked effect the block declares ([Interop with C](#testing-foreign-code)).
- r[test.assert-values] Assertions show the full values on both sides of a failed comparison.

r[diag.query.without-tests] Unit tests make files longer, so `quince` `query source <file> --without-tests` returns the file with its test blocks folded away. An agent reading code to change one function doesn't pay for every test in its context.

### Integration tests

Integration tests check how a package behaves from the outside, through its public API only. They live in a top-level `tests/` directory instead of beside the code they test.

- r[test.itest.packages] Each file directly in `tests/`, and each folder tests/\<name>/, is compiled as a separate test package that depends on the library ([Packages](reference/packages.md#packages)), so it can use only `pub` names, exactly as an outside user would.
- They don't break when internals are rewritten, which makes them the main safety net for refactoring.
- r[test.itest.same-syntax] Same `test` block syntax, effect fakes, and property tests as unit tests. r[test.itest.flags] `quince` `test` runs both; `quince` `test --unit` or `--integration` runs one kind.

### Property tests

```
test "parse inverts display" forall (n: Int) {
  assert(Int.parse(n.display()) == Ok(n))
}
```

- r[test.prop.forall] `forall` generates inputs from the types; every type has a derived generator unless it is marked @opaque or has private fields. r[test.prop.filter] Generated values are filtered against the type's requires invariants with a retry limit, and when the limit is reached the test reports which invariant it couldn't satisfy instead of passing.
- r[test.prop.custom] `impl Generate for T` replaces the derived generator, or provides one for an `@opaque` type or a type with private fields, usually by calling the type's own constructor. r[test.prop.discard] A generated case that breaks a `requires` of the function under test is discarded, not counted as a failure, and the report says how many were discarded.
- r[test.prop.contracts] Contracts on the function under test act as extra assertions on every generated case.
- r[test.prop.shrink] Failing cases are shrunk to a minimal input and saved to `.``quince``/regressions`, which reruns them on every later test run.

## 7. Modules, packages, and versioning

### Modules and imports

- r[diag.query.signature-location] Since an import doesn't name a file, `quince` `query signature <name>` and the language server return the file and line where any name is defined.

### Packages

r[dep.lock.contents] The lockfile records the exact version and content hash of every dependency, direct or indirect.

### The manifest

r[dep.manifest.strict] `quince.toml` is TOML with the tables below and nothing else. r[dep.manifest.unknown] An unknown table or key is a compile error that names its line, so a misspelled key never silently does nothing. r[dep.manifest.fmt] `quince fmt` also formats `quince.toml`: tables in the order of the example below, several `[cmd.*]` tables and the keys within a table as written.

```
[project]
name = "users"              # required: the library's import name, lowercase snake_case
edition = "2027"            # required: the only edition in v1
allow_native = ["sqlite"]   # optional: native dependencies this project trusts (Interop with C)

[dependencies]
pg = { repo = "github.com/acme/quince-pg", version = "1.4" }
http = { repo = "github.com/acme/quince-http", version = "2.1" }
shared = { path = "../shared" }
sqlite = { repo = "github.com/acme/quince-sqlite", version = "0.5" }

[test-dependencies]
fake_smtp = { repo = "github.com/acme/fake-smtp", version = "0.3" }

[native]                    # optional: C libraries (Interop with C, Linking)
link = ["minz"]

[cmd.users]                 # optional: capability limits for cmd/users/ (Program execution)
net = ["db.internal:5432", "*.example.com"]
fs = ["./data"]
```

- r[dep.manifest.project] `[project]` is required, and so are its name and edition. r[dep.manifest.no-version] The manifest holds no version of its own: a project's versions are its git tags. Every other table is optional.
- r[dep.no-registry] There is no central registry. A dependency comes from a git repository or a folder. r[dep.repo] `repo` is the repository's path without a scheme, fetched over HTTPS, and `version` is the minimum version needed: `"MAJOR.MINOR"` or `"MAJOR.MINOR.PATCH"`, where a missing part is 0. r[dep.min-select] A minimum selects the lowest tag at or above it in its major, and `quince update` and `quince add` always write the full `MAJOR.MINOR.PATCH`. r[dep.tag] A version is the repository's git tag `vMAJOR.MINOR.PATCH`, and the lockfile pins each one by content hash ([Content addressing](#content-addressing)). r[dep.dir] A project in a subfolder of its repository adds `dir`: `textutil = { repo = "github.com/acme/tools", dir = "textutil", version = "0.3" }`. r[dep.dir-tags] Its versions are the tags `textutil/vMAJOR.MINOR.PATCH`, as in Go, while tags without a prefix belong to the project at the repository's root, so every project in a repository has its own versions.
- r[dep.key] The key is the name that code imports, and it must be the dependency's own name, the `name` in its `[project]` table, so a package is called the same in every project that uses it. r[dep.rename] Any other key is a compile error, except where two dependencies would share a name: then one must be renamed, and only to the form the compiler suggests and `quince fix` writes, `<name>_v<N>` for another major version of the same repository (`pg_v2`) and `<owner>_<name>` for packages from different repositories that share a name, all of them renamed so that no name depends on which was added first (`acme_json` and `zeta_json` for github.com/acme/json and github.com/zeta/json; the owner is lowercased, with `-` and `.` turned into `_`). r[dep.key-major] A key's major version is the major of its `version`. r[dep.identity] Within a build, a package is identified by its repository, folder and major version, so every manifest that lists it shares one package.
- r[dep.path] `path` names another project on this machine, relative to this manifest. It is built from the folder as it stands and isn't hashed into the lockfile. r[dep.path-override] An entry may hold `path` beside `repo`, `dir` and `version`, as in Cargo: builds of this project use the folder, while dependents, the lockfile and `publish` see only the repository. r[dep.path-publish] A project with an entry that has only `path` can't be published. r[dep.lock.file] The lockfile is `quince.lock` beside `quince.toml`. r[dep.lock.commit] The toolchain writes it, and every project commits it, libraries included. r[dep.lock.format] It is TOML with one `[[package]]` table per dependency in the build, holding `repo`, `dir`, `major`, `tag` and one `hash` over the package's definition hashes (Content addressing). r[dep.lock.path] A dependency with only `path` is listed with `path` alone.
- r[dep.test-only] Names under `[test-dependencies]` may be imported by test packages, and by a library or program file only when every use is inside a `test` block or an `@test` declaration. Any other use is a compile error. r[dep.test-unused] Such an import is never reported as unused in a non-test build.
- r[dep.manifest.cmd-folder] Each `[cmd.<name>]` table must name an existing `cmd/<name>/` folder, and a program without one has no limits. r[dep.manifest.cmd-net] `net` lists the hosts it may connect to, as `host`, `host:port` or `*.domain`. r[dep.manifest.cmd-fs] `fs` lists the paths it may read or write, relative to the working directory at start, and each entry covers everything below it. r[dep.manifest.path-normalize] Entries, and every path the program opens, are made absolute against that directory, normalized (`.` and `..` removed) and resolved through symlinks before they are compared, so an absolute path inside an allowed folder is allowed and a symlink can't lead out of one. r[dep.manifest.limit-default] A missing key means no limit, and `[]` allows nothing.
- r[dep.manifest.future-room] The format leaves room for a later `[workspace]` table, a caching proxy or checksum database for repositories, and per-dependency limits ([Program execution](#capabilities-at-runtime)).

### Version resolution

r[dep.mvs.min] Quince uses minimal version selection, as Go does, so the same manifest always builds the same dependency graph without a solver.

- A manifest states the minimum version it needs of each dependency (`version = "1.4"`).
- r[dep.mvs.one-per-major] A build uses exactly one version per major version of each package: the highest minimum that anything in the graph asks for. A newer release is never picked up unless some manifest asks for it.
- r[dep.mvs.majors-distinct] Different major versions are different packages. `http` and `http_v2` (major 2 of the same repository) can both be in one build, and `http.Response` and `http_v2.Response` are distinct types; a mismatch error names each with its repository and major version.
- r[dep.update.raise] `quince` `update` raises the version of each direct dependency within its major, rewriting that dependency's `version` in `quince.toml` and the lockfile, and reports what changed in each dependency's public API. r[dep.update.indirect] Indirect versions follow from the manifests that ask for them, so `update` and `add` change no other file, and both list every version that moved, including indirect ones. r[dep.update.skip-breaking] A tag that the API check refuses (Enforced semantic versioning) is skipped: they take the newest tag in the major that passes, and name each skipped tag and the change that broke it. r[dep.update.major-explicit] Moving to a new major version is always an explicit edit to the manifest.

### Content addressing

- r[dep.hash.definition] Each published definition is identified by a hash of its normalized syntax tree plus the hashes of everything it depends on. r[dep.hash.recursive] Mutually recursive definitions are hashed together as one group.
- r[dep.hash.verify] Versions are chosen by name and version ([Version resolution](#version-resolution)), then verified against the lockfile's hashes, so a republished or tampered package can't change what an existing lockfile builds. Hashes also key the build cache ([Toolchain](#incremental-compilation)). r[dep.lock.root-only] Only the root project's lockfile is read; a dependency's own lockfile is ignored. r[dep.lock.build] `quince build` writes a missing lockfile and stops on one that no longer matches the manifests, naming `quince update`.
- r[dep.hash.text-source] Source stays in ordinary text files; hashes are computed by the toolchain and stored in the lockfile.

### Enforced semantic versioning

- r[dep.semver.compare] `quince` `publish` compares the new public API with the previous version: types, signatures, effects, and contracts.
- r[dep.semver.breaking] Removing or changing anything public, adding an effect, or strengthening a `requires` clause is breaking and needs a major version bump. Publishing refuses otherwise. r[dep.publish.version] Publishing picks the version from that comparison, the smallest bump it allows over the latest tag (major, minor or patch), or a larger one the author asks for, then creates and pushes that git tag. r[dep.publish.first] The first tag is v0.1.0 unless the author names another. r[dep.semver.bump] The bump follows from the comparison: a public API identical to the latest tag's allows a patch; any addition, such as a new public name, method, impl, optional field, parameter with a default, or variant of an `@extensible` enum, needs a minor; and a breaking change needs a major. r[dep.semver.pre-1] Below v1.0.0 the same rules hold, so a breaking change after v0.3.2 is published as v1.0.0. r[dep.publish.needs-review] A [needs-review finding](#enforced-semantic-versioning) is reported and doesn't stop the publish. r[dep.publish.prefix] In a subfolder project, `publish` creates the prefixed tag (The manifest). r[dep.publish.git] `publish` tags the current commit, refuses uncommitted changes in the project's folder, and pushes the tag to the git remote `origin`. r[dep.publish.refuse] It refuses a project with no library, since nothing can depend on it (its programs are released with ordinary git tags), and a library that contains `dbg` or `todo`. r[dep.semver.dep-check] Adding or updating a dependency repeats the same comparison for every tag it picks, against the newest earlier tag in that major version that passed this check, and refuses a tag that breaks it, naming the change. So a tag pushed by hand can't bring a breaking change into a minor version.
- r[dep.semver.effvar] Removing an effect variable, or adding a named effect beside one (`uses E` to `uses E, Log`), is breaking. r[dep.semver.effvar-add] Adding an effect variable to a closure parameter that was pure is not, since every existing caller passes a pure closure and still type-checks.
- r[dep.semver.wire] For a public type that implements `Encode` or `Decode`, any change to its wire shape is breaking: renaming a field or variant without `@wire`, changing a `@wire` name, case or tag, adding a required field, or removing a field. Adding an `Option` field is not.
- r[dep.semver.receiver] Changing a public method's receiver between `self` and `var self` is breaking in either direction.
- r[dep.semver.variant] Adding an enum variant is breaking unless the enum is marked `@extensible`, which in turn forces callers to include a wildcard arm.
- r[dep.semver.contract] Any change to a contract's normalized syntax tree counts as breaking (reformatting doesn't), because whether a new `requires` is stronger than the old one can't be decided in general. r[dep.semver.iface-method] Adding a method to a public interface is breaking too. `quince` `publish` can't see a behavior change behind an unchanged signature; doc examples and integration tests are the guard there.
- r[dep.semver.derived] `quince` `publish` also compares the properties the compiler derives for each public type: automatic `Eq`, `Hash`, `Encode` and `Decode`, whether it can cross a task boundary, and whether it can be held in a `const`. Losing any of them is breaking, for example by adding a field whose type has no `Hash`. r[dep.semver.pub-field] Adding a field to a struct whose fields are all `pub` is breaking, because it breaks struct literals outside the package. r[dep.semver.impl-review] Adding an impl to a public type is reported as needs-review, because it can create a method-name clash ([Items](reference/items.md)) in a dependent. So is adding a `pub` method, for the same reason.

## 8. Toolchain

r[tool.cli.single-binary] The toolchain is a single binary, `quince`, whose every command can emit structured JSON. It is designed to be driven by an agent as much as by a person.

### Commands

| Command | Does |
| --- | --- |
| `quince` `check` | r[tool.cli.check] Type-checks, lints, and verifies formatting; no code generation; warnings don't fail it unless --deny warnings is passed, which CI does by default |
| `quince` `build` | r[tool.cli.build] Compiles a binary or library |
| `quince` `run` | r[tool.cli.run] Builds and runs `main` |
| `quince` `test` | r[tool.cli.test] Runs unit tests, integration tests, doc examples, property tests, and saved regressions |
| `quince` `fmt` | r[tool.cli.fmt] Applies the canonical format |
| `quince` `fix` | r[tool.cli.fix] Applies every machine-applicable suggested fix |
| `quince` `query` | r[tool.cli.query] Answers questions about the codebase ([Query API](#query-api)) |
| `quince` `verify` | r[tool.cli.verify] Attempts static proof of `@verify` contracts |
| `quince` `publish` | r[tool.cli.publish] Checks semver against the previous version, then creates and pushes the version's git tag |
| `quince` `migrate` | r[tool.cli.migrate] Moves a package to a newer edition |
| `quince` `bind` | r[tool.cli.bind] Generates raw foreign declarations from a C header ([Interop with C](#generated-bindings)) |
| `quince` `update` | r[tool.cli.update] Raises dependency minimums and reports API changes ([Dependencies and publishing](#version-resolution)) |
| `quince` `explain` | r[tool.cli.explain] Prints the full explanation of an error code |
| `quince` `add` | r[tool.cli.add] Adds a dependency at its newest version and lists every version it raised ([Dependencies and publishing](#version-resolution)) |

r[tool.perf.targets] Performance targets: `quince` `check` on a single changed file in a 500,000-line project should finish in under 1 second, and an incremental build in under 5 seconds.

### Structured diagnostics

r[diag.json] With `--format json`, every diagnostic has a stable code, an exact span, a plain-language message, and zero or more fixes expressed as text edits:

```
{
  "code": "E0412",
  "severity": "error",
  "message": "match on `FetchError` is missing variant `Timeout`",
  "span": { "file": "src/fetch.qnc", "start": [42, 3], "end": [47, 4] },
  "fixes": [
    {
      "description": "add an arm for `Timeout`",
      "applicability": "needs-review",
      "edits": [{ "start": [46, 5], "end": [46, 5], "text": "FetchError.Timeout(after:) => todo(\"handle timeout\"),\n" }]
    }
  ],
  "docs": "quince explain E0412"
}
```

- r[diag.applicability] `applicability` is `safe` (apply automatically), `needs-review`, or `placeholder` (inserts a `todo`).
- r[diag.edits] An edit replaces the text from `start` up to `end` with `text`: when `start` equals `end` it inserts, and an empty `text` deletes.
- r[diag.edit-file] An edit may name a `file`, a path relative to the project root; without one it applies to the diagnostic's span file. An edit at `[1, 1]` to a file that doesn't exist creates it.
- r[diag.code-stable] Error codes are stable across editions; r[diag.explain] `quince` `explain <code>` prints a full explanation with examples.
- r[diag.all-errors] The compiler reports all independent errors in one run, ordered so fixing the first rarely invalidates the rest.

### Query API

`quince` `query` exposes what the compiler knows, so an agent can look things up instead of guessing:

| Query | Returns |
| --- | --- |
| `members <Type>` | r[diag.query.members] Fields, methods, and implemented interfaces, with signatures |
| `signature <path>` | r[diag.query.signature] Full signature, effects, contracts, and doc comment |
| `callers <path>` / `callees <path>` | r[diag.query.callers] The call graph around a function |
| `impls <Interface>` | r[diag.query.impls] Every type implementing an interface |
| `type-at <file:line:col>` | r[diag.query.type-at] The inferred type of an expression |
| `effects <path>` | r[diag.query.effects] Each of a function's effects, with the call chain that brings it in and every package effect expanded to the built-in effects beneath it |
| `search <type pattern>` | r[diag.query.search] Functions matching a type shape, e.g., `String -> Result<Int, _>` |
| `dependents <path>` | r[diag.query.dependents] Everything that would break if a definition changed |
| `wire <Type>` | r[diag.query.wire] The JSON shape a type encodes to and decodes from, with wire names and which fields are optional |
| source \<file> \[--without-tests\] | r[diag.query.source] A file's source, optionally with test blocks folded away ([Testing](#unit-tests)) |
| `handlers <fn>` | r[diag.query.handlers] Which handler each of a function's effects resolves to, from each program's main ([Effects](reference/effects.md)) |

r[diag.query.lsp] The same queries are served over a long-running language server for editors and agents that keep a session open.

## 9. Concurrency

### Deterministic testing

r[test.dtest.seeded] Under `quince` `test`, the scheduler runs tasks in a seeded, reproducible order and explores several orderings per test. r[test.dtest.replay] A failure prints its seed, and `quince` `test --seed <n>` replays it exactly. r[test.dtest.real-one] A test that opts into a real effect runs one ordering, so its real I/O happens once. How many orderings the other tests explore is set by the toolchain.

### Calling C

Green threads complicate foreign calls ([Interop with C](#11-interop)), so the runtime handles them specially:

- r[ffi.thread-pool] The runtime runs foreign calls on a separate thread pool, so a blocking C call can't stall other tasks.
- r[ffi.pinned] `@pinned` marks a foreign library that must always be called from the same OS thread (e.g., GUI toolkits).
- r[ffi.cancel-after] A task blocked in a foreign call is cancelled only after the call returns.
- r[ffi.callback-cancel] Cancellation never interrupts a callback that C calls into Quince ([Interop with C](#callbacks)). It is delivered once the foreign call returns. r[ffi.callback-panic] A panic that reaches a callback's boundary aborts the process, with a message naming the callback, because unwinding through C frames is undefined behavior.

## 10. Compilation

r[tool.build.native] Quince compiles ahead of time to native code through LLVM, producing a single self-contained binary that includes the runtime (scheduler and cycle collector). r[tool.build.static] On Linux the binary is fully static, unless a native library is available only as a shared library, which quince build reports; on macOS and Windows it links only the operating system's own libraries. r[tool.build.no-jit] There is no interpreter or JIT.

### Backend decision

r[tool.build.llvm-only] LLVM is the only backend, for debug and release builds alike. A second, faster debug backend (as Zig and Rust have pursued) is not planned: it is years of work for modest gains, since type checking, monomorphization, and linking take much of the build time too. Fast feedback comes instead from `quince` `check` (no code generation), per-definition caching, and debug builds that skip optimization. Quince IR is kept backend-independent so a Cranelift debug backend can be added later if real-world build times require it.

### Pipeline

1. **Parse** source into a syntax tree.
2. **Check** types, effects, exhaustiveness, and formatting. `quince` `check` stops here.
3. **Lower** to Quince IR, a mid-level representation where generics are monomorphized, contracts become explicit checks, reference-count operations are inserted and optimized, and `concurrent` blocks become runtime calls.
4. **Generate** LLVM IR, with reference counting already explicit, so no GC stack maps or LLVM statepoints are needed.
5. **Optimize and emit** native object code, then link with the runtime.

### Memory management

r[mem.rc] Quince manages memory with reference counting that the compiler inserts in Quince IR, as Koka (Perceus) and Lean 4 do. There is no tracing collector for ordinary values.

- **Counts the compiler removes.** r[mem.borrow] A parameter the callee doesn't store is borrowed and touches no count, and a value is moved, not counted, at its last use. Most calls touch no counts at all. r[mem.c-scope] A value that owns C memory is never released before the end of its scope, because C may still hold its pointer ([Interop with C](#memory-across-the-boundary)).
- **Freeing and reuse.** r[mem.free] A value is freed as soon as its last reference goes away, at a point the code determines. r[mem.reuse] When a unique value is consumed by an update that builds a value of the same shape, its memory is reused in place.
- **Across tasks.** r[mem.atomic-shared] Counts are non-atomic by default. A value becomes shared when it crosses a task boundary (captured by `spawn`, sent on a channel, or stored in an `Atomic<T>` or `ConcurrentMap<K, V>`), and only shared values use atomic count operations. r[mem.mark-shared] The first crossing marks everything reachable from the value as shared, which takes time proportional to its size, so sending a large map on a channel is not free. r[mem.const-shared] Constants are marked shared when they are initialized.
- **Cycles.** r[mem.no-value-cycles] Plain values can't form cycles, because a value can't be changed through an alias. Only reference types (`Ref<T>`, `Atomic<T>`, `ConcurrentMap<K, V>`, Channel\<T>, TaskScope, Pool\<T, Err, uses E> and Callback\<F>) can, by holding a value that refers back to them. r[mem.cycle-collector] The runtime runs an incremental cycle collector over reference-type objects only. r[mem.cycle-per-task] It collects each task's own Refs inside that task, and the types that cross tasks in a background task, so it never stops all tasks at once.
- r[mem.copy-report] **Hidden copies are reported.** `quince` `check` and the query API ([Query API](#query-api)) list every update that may copy instead of updating in place, and name the alias that causes it.
- **Large frees.** Freeing a large structure takes time proportional to its size. r[mem.large-free] Above a size threshold, the runtime hands the rest of the free to a background task, so a request handler isn't stalled. r[mem.resource-not-deferred] Resource cleanup is never deferred this way: any Resource inside the structure is cleaned up at the drop point, before the rest is handed off.

### Build profiles

| Profile | LLVM optimization | Overflow checks | Contracts | Debug info |
| --- | --- | --- | --- | --- |
| r[tool.profile.debug] `debug` (default) | O0 | On | On | Full |
| r[tool.profile.release] `release` | O2, ThinLTO | On | On, except `@unchecked` | Line tables |

r[tool.profile.overflow] Overflow checks stay on in release builds because overflow is defined as a panic ([Errors and failure](reference/errors.md)), not an optimization opportunity.

### Targets

| Tier | Targets | Guarantee |
| --- | --- | --- |
| r[tool.target.tier1] 1 | x86-64 and ARM64 on Linux, macOS, Windows | Built and fully tested on every release |
| r[tool.target.tier2] 2 | WebAssembly (`wasm32`) | Builds; concurrency limited until WebAssembly stack switching is widely available |

r[tool.target.cross] Cross-compilation is built in: `quince` `build --target linux-arm64` works from any tier 1 host with no separate toolchain install, for packages with no foreign dependencies. r[tool.target.cross-foreign] Cross-compiling foreign code needs the target's C libraries, and macOS targets need Apple's SDK.

### Incremental compilation

r[tool.incr.cache] Compiled output is cached per definition, keyed by a hash of the definition's own body plus the signature hashes of everything it calls. r[tool.incr.body] Changing a function's body recompiles only that function and what it was inlined into; changing a signature also recompiles its callers. The full content hash from [Content addressing](#content-addressing), which covers every dependency, identifies definitions in the lockfile.

### Debugging

r[tool.debug.dwarf] Binaries emit DWARF (or PDB on Windows), so standard debuggers work. r[tool.debug.locations] Panic traces and the deterministic test scheduler ([Testing](#deterministic-testing)) report Quince source locations, never generated code.

## 11. Interop

r[ffi.c-abi] Quince interoperates with other languages only through the C ABI. Any library that exposes a C interface (including ones written in Rust, Zig, Swift, or C++) can be called; direct interop with Rust generics or JavaScript is out of scope.

### Generated bindings

r[ffi.cbind.generate] `quince` `bind <header.h>` reads a C header and generates raw declarations. Bindings are generated rather than hand-written, because a mistyped foreign declaration corrupts memory silently.

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

- r[ffi.block] A `foreign` block names the library and the effects its code performs. r[ffi.effects] Callers of anything built on it see those effects like any others. r[ffi.pure-block] A block without uses declares pure functions. r[ffi.cbind.all-effects] Like every foreign effect, that is a promise the compiler can't check, so quince bind writes every built-in effect on the block and the author removes the ones the library doesn't use.
- r[ffi.multi-block] A package may have several `foreign` blocks for one library, each with its own effects, for example a pure block for one-shot functions and a `uses State` block for a stream. r[ffi.namespace] They share one namespace, so a type declared in one is visible in the others. r[ffi.attr-consistent] A library attribute such as `@pinned` must appear on every block for that library or on none.
- r[ffi.not-pub] Raw declarations use C types (`CInt`, `CString`, `Ptr<T>`) and cannot be `pub`. The package that declares them must wrap them in a safe API using `Result` and ordinary Quince types. r[ffi.resource-not-pub] A Resource\<T> whose T is a foreign type can't appear in a pub signature either; hold it in a field of a struct, as Db does.
- r[ffi.type] `type name` inside a `foreign` block declares an opaque C type. r[ffi.cleanup] `@cleanup(fn, limit: dur)` above it names the C function that releases it and the cleanup time limit, 10 seconds when left out. r[ffi.cleanup-signature] The cleanup function must be declared in a foreign block of the same package and take exactly one parameter, Ptr\<T>. r[ffi.cleanup-return] It may return any C type, and res.close() returns that value; a wrong signature is a compile error on the @cleanup line.
- r[ffi.cresource.adopt] `Resource.adopt(p)` turns a `Ptr<T>` into a `Resource<T>`, and works only for a type with `@cleanup`. r[ffi.cresource.ptr] `res.ptr()` lends the raw pointer for one call. r[ffi.cresource.ptr-arg-only] Like buf.ptr() and bytes.lend(), it may only be written as an argument of a foreign call; binding it to a name is a compile error. r[ffi.cresource.close] res.close() runs the cleanup now and returns the cleanup function's result, res.is\_closed() tells whether cleanup has run, and res.keep(x) moves a Callback, CBuf or CString into the resource, which releases it right after the C cleanup runs, so the end of the local scope no longer releases it.
- r[ffi.cbuf.def] `CBuf<T>` is C-allocated memory with identity (a reference type, [Types](reference/types.md)), and is the only way to pass an out-parameter, so Quince memory never escapes to C. r[ffi.cbuf.api] CBuf.new\<T>() holds one value and CBuf.alloc\<T>(n) an array; buf.get(), buf.set(v) and buf.ptr() reach it, and CBuf.from\_bytes(b) and buf.to\_bytes(n) copy UInt8 buffers. r[ffi.cstring.convert] `CString.try_from(s)` converts text for C (`None` when it holds a NUL byte), and `cstr.to_string()` converts it back as a `Result`, since C strings may not be UTF-8.
- r[ffi.lend] `bytes.lend()` lends a `Bytes` value in place for one call, as a `ConstPtr<UInt8>`. r[ffi.lend-const] `quince` `bind` emits `ConstPtr<T>` for a `const T*` parameter, and only such a parameter accepts a lent pointer, so C can't write into Quince memory. r[ffi.no-string-lend] A `String` is never lent, since C strings need a trailing NUL byte; `CString.try_from` copies it.
- r[ffi.ownership] A foreign function that returns a `CString` must say who owns it. r[ffi.borrowed] `@borrowed` means C keeps it: Quince never frees it, and it must be converted with `to_string()` before the next call into that library. r[ffi.owned] `@owned(free_fn)` means Quince frees it with `free_fn` when it is dropped. r[ffi.ownership-required] Leaving both out is a compile error, and `quince` `bind` leaves them out because a header can't say. r[ffi.ownership-ptr] The same rule covers a returned `Ptr<T>` whose `T` has no `@cleanup`. r[ffi.cstring.owned] A `CString` made by `CString.try_from` is always owned by Quince.
- r[ffi.ptr-ops] The pointer operations are `Ptr.null()`, `p.is_null()`, and `p.read()` and `p.write(v)` for C types declared in the block.
- r[ffi.package-only] Everything in this list works only in the package that declares the `foreign` block. r[ffi.no-unsafe] There is no `unsafe` keyword: the native mark and [`allow_native`](#trust) are the audit surface, and `quince` `query` lists every raw operation in a package.

### C types

r[ffi.ctype.fixed-alias] A C type whose width is the same on every target is another name for a Quince number type, so literals, constants and conversions work as usual: `const MINZ_BUF_ERROR: CInt = -5`, `n.to_int32()`, `c.to_int64()`. r[ffi.ctype.target-width] A C type whose width depends on the target is a type of its own, so code that compiles on one target compiles on all of them. r[ffi.ctype.pub] The fixed-width names may appear in `pub` signatures, since each is only another name for a Quince type. Target-width C types, raw foreign declarations and `Resource<foreign T>` may not.

| C type | Quince type |
| --- | --- |
| signed char, unsigned char | r[ffi.ctype.char] CSChar = Int8, CUChar = UInt8 |
| short, unsigned short | r[ffi.ctype.short] CShort = Int16, CUShort = UInt16 |
| int, unsigned int | r[ffi.ctype.int] CInt = Int32, CUInt = UInt32 |
| long long, unsigned long long | r[ffi.ctype.longlong] CLongLong = Int64, CULongLong = UInt64 |
| float, double | r[ffi.ctype.float] CFloat = Float32, CDouble = Float |
| int8\_t to uint64\_t | r[ffi.ctype.stdint] Int8 to UInt64 |
| char, long, unsigned long, size\_t, ssize\_t, intptr\_t | r[ffi.ctype.own] Own types: CChar, CLong, CULong, CSize, CSSize, CIntPtr |

r[ffi.ctype.target-convert] Each target-width type is built with a checked conversion such as `CSize.try_from(n) -> Option<CSize>`, accepts integer literals that fit in 32 bits, and converts back with `to_int64()` (signed) or `to_uint64()` (unsigned), which never fails on any target.

r[ffi.cbind.params] `quince` `bind` writes pointer and callback parameters this way:

| C parameter | Quince parameter |
| --- | --- |
| `T*` | r[ffi.cbind.ptr] `Ptr<T>` |
| `const T*` | r[ffi.cbind.const-ptr] `ConstPtr<T>` |
| `T**` (an out-parameter) | r[ffi.cbind.out-param] `Ptr<Ptr<T>>`, passed as a `CBuf<Ptr<T>>`'s `ptr()` |
| `const char*` | r[ffi.cbind.cstring] `CString`, lent for the call |
| A function pointer followed by its `void*` context | r[ffi.cbind.callback] One `Callback<fn(...)>` parameter, its arguments in C types |

r[ffi.cbind.param-names] Parameter names come from the header, since every argument after the first is named at the call. r[ffi.cbind.keyword-param] A C parameter whose name is a keyword gets `_` appended (`fn` becomes `fn_`). r[ffi.cbind.callback-name] The one `Callback` parameter made from a function pointer and its context takes the function pointer's name. r[ffi.cbind.unnamed] A parameter the header leaves unnamed becomes `arg1`, `arg2`, and so on by position. r[ffi.cbind.returns] Return types follow the same table: a returned `T*` becomes `Ptr<T>`, and a returned `const char*` or `char*` becomes `CString`. r[ffi.cbind.ownership-left] Both need `@borrowed` or `@owned`, which `quince bind` leaves for the author (Generated bindings).

### Trust

r[ffi.trust.native] A package containing a `foreign` block is native, which the toolchain sees in its source. r[ffi.trust.allow] A project must allow each native dependency by name in its manifest:

```
[project]
allow_native = ["sqlite"]
```

r[ffi.trust.unlisted] Building with an unlisted native dependency is an error. The effect declared on a foreign block can't be verified, so this list is the audit surface: everything else is checked by the compiler.

### Linking

r[ffi.link.table] The `[native]` table in `quince``.toml` names every C library the project links and any C sources it builds:

```
[native]
link = ["minz"]            # found with pkg-config, then the system linker's search path
sources = ["native/*.c"]   # optional: C files built with the toolchain's C compiler
include = ["native"]       # header folders for sources and quince bind
```

- r[ffi.link.match] Every library named by a `foreign` block must match a `link` entry or the library built from `sources`, which takes the project's name. Otherwise it is a compile error that names the missing manifest line. r[ffi.link.own-only] `[native]` covers only the project's own foreign blocks: a native dependency's link entries and sources carry into the final binary without being listed again.
- r[ffi.trust.transitive] `allow_native` lists every native package in the build by its own name, whether it is a direct, indirect or test-only dependency; a project never lists itself.
- r[ffi.link.ship-sources] A published package ships its `sources` and builds them on the user's machine. r[ffi.link.missing] A package that only links needs the library installed, and `quince` `build` names the missing library and the pkg-config name it tried.

### Memory across the boundary

- r[ffi.cmem.no-escape] **Quince** **memory never escapes to C.** Data is copied into C-allocated buffers, or lent in place for the duration of one call (bytes.lend(), above) (Quince never moves memory, so this needs no pinning). C code can never hold a pointer to Quince memory after the call returns, because reference counts can't see it.
- **C-owned memory is a `Resource<T>`.** r[ffi.cmem.resource-once] A resource declares its cleanup function and is released exactly once: by res.close(), at the end of a with\_resource call, or when its last reference is dropped, whichever comes first. r[ffi.cmem.with-resource] with\_resource hands its body the resource itself and closes it when the body ends, even on error, panic, or cancellation:

```
let changes = ffi.with_resource(Resource.adopt(out.get()), fn(db) {
  sqlite3_changes(db.ptr())
})
```

- r[ffi.cmem.drop-close] Dropping the last reference to an open resource closes it then and there, a moment reference counting makes exact ([Memory](#memory-management)). So a wrapper such as `Db` is correct without any close call, and its `close` method exists for callers that want the C error code. r[ffi.cmem.pinned-cleanup] For a `@pinned` library the cleanup is queued onto that library's thread. r[ffi.cmem.cycle-warn] Only a Resource freed when the cycle collector breaks its cycle ([Memory](#memory-management)) logs a warning, since the moment of cleanup is then unpredictable, and `quince` `check` warns about any reference type that can hold a Resource in a cycle.
- r[ffi.cmem.use-after-close] Using a resource after it is closed is a bug: `res.ptr()` or `res.close()` on a closed resource panics, naming the foreign type and the line that closed it. r[ffi.cmem.escape-warn] `quince` `check` warns when a Resource, or a value holding one, is returned from or escapes a `with_resource` body.
- r[ffi.cmem.scope-release] A value that owns C memory (a `Resource`, `CBuf`, `Callback` or `CString`, or a value holding one) is never released before the end of the scope that holds it, even after its last use, because C may still hold its pointer. r[ffi.cmem.drop-order] On every exit (normal, `?`, panic, cancellation), locals are released in reverse order of creation, and the fields of a dropped struct in declaration order. r[ffi.cmem.keep] To tie a callback to the handle C stores it in, use `res.keep(cb)`. The same is how to hand C memory it keeps, such as a string it stores: a `CString` passed directly is lent for that call only, so keep it on the resource with `res.keep(cstr)`.
- r[ffi.cmem.cleanup-limit] Cleanup is shielded from cancellation and has a time limit, 10 seconds unless the resource's declaration sets another. Past the limit the cleanup is cancelled, and the C-level release still runs ([Cancellation](reference/concurrency.md#cancellation)).

**Sharing resources between tasks.** r[ffi.cresource.crossing] A `Resource<T>` can't cross a task boundary unless its foreign type is marked `@shareable`, a promise that the C handle is safe to use from several threads. r[ffi.pool.def] For the usual case, a handle that isn't, the standard library's `Pool<T, Err, uses E>` holds up to `max` of them and hands each one to a single task at a time:

```
let pool = Pool.new(20, healthy: fn(conn) { conn.is_open() }, fn() { pg.connect(url) })
let rows = pool.checkout(fn(conn) { conn.query("SELECT 1", params: []) })??
```

- r[ffi.pool.checkout] `pool.checkout` waits for a free value (a cancellation point), lends it to the closure, and returns it afterwards. r[ffi.pool.closure-crossing] The closure is checked like a spawned task: what it captures and what it returns must be able to cross a task boundary. r[ffi.pool.lent] The lent value can't, so it can't be returned, stored or kept after the closure ends, and two tasks never hold it at once. r[ffi.pool.panic] If the closure panics or is cancelled, the value is closed and replaced rather than returned. r[ffi.pool.close-drop] Here and below, the pool closes a value by dropping it, so a Resource inside it closes then; v1 has no close hook. r[ffi.pool.err] An `Err` that the closure returns is an ordinary value, since the pool can't see inside `R`, so the value goes back to the pool. r[ffi.pool.healthy] On every return the pool first calls `healthy` on the value, and closes a value it rejects instead of keeping it, so a connection that died during the closure is never handed out again. r[ffi.pool.healthy-required] `healthy` is required, so every pool states its policy; `fn(_) { true }` keeps every value.
- r[ffi.pool.crossing] The pool itself can cross task boundaries, so a handler such as `PgDb` holds one and every task that inherits the handler shares it.

### Callbacks

r[ffi.callback.def] C code may call back into Quince functions passed to it as a Callback\<F>: a resource that keeps the closure alive until it is released, so C can store it beyond one call, as event loops and sqlite hooks do. r[ffi.callback.effects] `F` carries the callback's effects like any function type, as in `Callback<fn(CInt, CInt) uses State>`, and they must be among those declared on its `foreign` block. r[ffi.callback.no-poly] So a `pub` wrapper fixes a callback's effects, and can't take an effect-polymorphic callback. r[ffi.callback.handlers] A callback runs with the effect handlers that were in effect at the call that registered it, so a test that registers one under a fake gets the fake on every later call. r[ffi.callback.param] A foreign parameter typed `Callback<fn(...)>` receives a C function pointer plus its context, which `quince` `bind` sets up from the header. r[ffi.callback.release] `cb.unregister()` or the end of its scope releases it, unless `res.keep(cb)` has moved it into a resource, whose cleanup then releases it.

- r[ffi.callback.new] `Callback.new(f)` is for C that may call at any time, from any thread. r[ffi.callback.new-crossing] The closure's captures must be able to cross a task boundary, as for `spawn`, and each call runs as a short task in the scope that registered it. r[ffi.callback.new-threads] The runtime registers each calling thread on first use.
- r[ffi.callback.local] `Callback.local(f)` is for C that calls back only during a call the task itself is making, such as progress reports, sort comparators and row callbacks. r[ffi.callback.local-ref] The closure may capture a `Ref`, and it runs as part of the task that created it. r[ffi.callback.local-abort] A call from anywhere else aborts the process with a message naming the callback.

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

### Testing foreign code

C code does real I/O whatever handler is installed, so a test fake can't stand in for it. r[ffi.ffitest.opt-in] A test that reaches a `foreign` block must opt in to each effect the block declares that tests fake by default: a test that calls into `foreign "sqlite3" uses Fs` needs `test "..." uses Fs`, and the compile error names the block and the opt-in to add. r[ffi.ffitest.pure] Pure blocks and `uses State` blocks need nothing. r[ffi.ffitest.step] The scheduler counts each foreign call as one step. A test that should not touch real I/O replaces the wrapper's own effect with a fake ([Effects](reference/effects.md)).

## 12. Open questions

Only the name remains open. Every design question raised in the v0.1 and v0.2 reviews has been decided in this draft, and the reviews record each decision.

- [x] **Name: Quince.** Decided 2026-11-10, before the first public release. "Lucid" was the working title, dropped because it collides with an active Princeton research language for network data planes (with a verification-focused successor, VeriLucid) and an old 1970s dataflow language. A web search found no language or developer tool named Quince. The command is quince, the manifest is quince.toml, and source files use .qnc.

## Appendix: Expression grammar

### Declarations, types and patterns

```ebnf
foreign_decl = "foreign" string [ "uses" effects ] "{" { ( { attribute NEWLINE } ( c_fn | "type" c_name ) ) NEWLINE } "}"
c_fn         = "fn" c_name "(" [ c_name ":" type { "," c_name ":" type } [ "," ] ] ")" [ "->" type ]
test_decl    = "test" string [ "forall" "(" params ")" ] [ "uses" effects ] block
```

## Appendix: Prelude

### Built-in effect operations

- r[exec.broken-pipe] Writes to a closed standard output end the program quietly with code 0, as Unix tools do on a broken pipe, so `Console` writes return nothing to check.
