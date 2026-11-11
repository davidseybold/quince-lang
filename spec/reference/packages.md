# Packages and modules

r[mod.namespace] A package is one namespace, and a project holds one library package plus its programs and tests ([Packages](#packages)): files organize code but never change what a name is called or how it is imported. r[mod.name-sources] Every name used in a file is either defined in the package, listed in an import, or in the prelude ([Prelude and special interfaces](../quince-spec.md#modules-and-imports)). r[mod.pkg.one-version] Each dependency, meaning one repository folder at one major version, resolves to one exact version, pinned in the lockfile by content hash, so nothing a dependent relies on changes without a change to its lockfile.

## Modules and imports

```
import std.http.{Response}
import std.json
import acme_billing.{Invoice, InvoiceError}
```

```ebnf
file         = { import NEWLINE } { decl NEWLINE }
import       = "import" path ( "." names | [ "as" name ] )   // as: packages only
names        = "{" name_ref { "," name_ref } [ "," ] "}"
name_ref     = "self" | name | Name | NAME
```

- r[mod.import.explicit] Imports list names explicitly; `import std.json` brings in the package name only, used as `json.decode(...)`. r[mod.import.fn-qualified] A function is always called through its package, as `json.decode(body)` or `http.get(url)`, so every call says where it comes from. r[mod.import.by-name-kinds] Only types, interfaces, effects and constants are imported by name; importing a function by name is a compile error whose fix imports its package instead. r[mod.import.prelude-fns] The prelude's free functions (`assert`, `todo`, `unreachable` and `dbg`) are called bare. r[mod.import.self] To use both the package name and some of its names, write one line, `import std.log.{self, Log}`. r[mod.import.one-line] A package appears in at most one import line, and `quince` `fmt` merges two lines into that form. r[mod.import.as] When two packages in one file end in the same name, one is renamed for the file with `as`, which only a package import takes: `import json as acme_json` beside `import std.json`. r[mod.import.major] A second major version is imported by its manifest key, as `import http_v2` ([The manifest](../quince-spec.md#the-manifest)); no import or type name contains `@`. r[mod.import.test-only] An import used only by `test` blocks is allowed at the top of the file and raises no unused-import warning.
- r[mod.extension] Source files use the `.qnc` extension.
- r[mod.unique-names] Every top-level name is unique within its package, and its import path is the package, never the file. r[mod.no-reexport] There are no re-exports, so a name has exactly one import path, and moving a definition to another file never changes it.
- r[mod.pkg.program] A program is a package of its own in `cmd/<name>/`, holding the one `main` for that binary and depending on the library package beside it, as in Go. r[mod.pkg.main-location] `main` may appear only in such a package, and `quince` `build` builds every program in the project.
- r[mod.pkg.program-path] A package is a program exactly when its folder is `cmd/<name>/`. The path decides; no manifest field makes or unmakes a program. This holds for a project with only one program too, so adding a second program never moves code.
- r[mod.pkg.main-outside] A `main` outside `cmd/<name>/` is a compile error with its own error code. r[mod.pkg.main-fix] `quince` `fix` moves it to `cmd/<package>/`, using the package name from `quince``.toml`.
- r[mod.pkg.binary-name] A program's binary is named `<name>`, after its folder. Build flags may choose where the binary is written, never what it is called, so every build of a commit produces the same names.
- r[mod.intra-package] Files in the same package may refer to each other freely. r[mod.no-cycles] Circular dependencies between packages are a compile error.
- r[mod.private-default] Everything is private unless marked `pub`.

## Packages

r[mod.pkg.project] A project is a folder with a `quince``.toml` manifest at its root, naming the project, its edition, its dependencies, the capability limits of its programs ([Program execution](../quince-spec.md#capabilities-at-runtime)), and the native libraries it links ([Interop with C](../quince-spec.md#linking)). Its format is under [The manifest](../quince-spec.md#the-manifest). r[mod.pkg.one-manifest] One manifest and one lockfile cover every package in the project, so all of them build with the same edition and the same dependency versions.

- **Library.** r[mod.pkg.library-files] Every source file in the project except those under `cmd/`, `tests/` and folders whose names start with a dot, and except any `testdata/` folder, whose files are never compiled, so a tool can keep `.qnc` fixtures there. r[mod.pkg.library-name] Its import name is the `name` in `quince``.toml`. r[mod.pkg.program-import] A program imports it by that name, `import <name>.{...}`, and a program folder may share the library's name, since folders are not namespaces. r[mod.pkg.one-library] A project has at most one library, and it has none when every file is in a program.

* **Programs.** Each `cmd/<name>/` ([Modules and imports](#modules-and-imports)). r[mod.pkg.program-visibility] A program sees only the library's `pub` names, exactly as a dependent does. r[mod.pkg.program-no-import] Nothing imports a program, and programs never import each other, so code that two programs share goes in the library.

- **Test packages.** r[mod.pkg.test-packages] Each file directly in `tests/`, and each folder `tests/<name>/`, is one test package ([Testing](../quince-spec.md#integration-tests)). A folder lets several test files share helpers. r[mod.pkg.test-visibility] A test package sees only the library's `pub` names.

* r[mod.pkg.dependents] **Dependents** import only the library. Programs and test packages are never part of a project's public API, and `quince` `publish` compares only the library's.

- r[mod.pkg.test-deps] Dependencies used only by tests (test packages, `test` blocks and `@test` declarations) are listed in their own section of the manifest. They never become dependencies of anyone who depends on the project.

* r[mod.pkg.nested-manifest] A `quince``.toml` anywhere below the project root is a compile error. r[mod.pkg.multi-project] A repository with several libraries holds several projects, each in its own folder with its own tags, and a dependent names one with `dir` ([The manifest](../quince-spec.md#the-manifest)). r[mod.pkg.workspace-room] The manifest format must leave room for a later workspace that groups them.

## Editions

- r[mod.edition.per-package] Language changes ship in editions (e.g., `edition = "2027"`), chosen per package.
- r[mod.edition.interop] Packages on different editions link together freely.
- r[mod.edition.migrate] `quince` `migrate` rewrites code to a new edition mechanically; any change it can't make automatically is a compile error with a suggested fix.
