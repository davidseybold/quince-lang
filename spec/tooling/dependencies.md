# Dependencies and publishing

r[dep.lock.contents]
The lockfile records the exact version and content hash of every dependency, direct or indirect.

## The manifest

r[dep.manifest.strict]
`quince.toml` is TOML with the tables below and nothing else.

r[dep.manifest.unknown]
An unknown table or key is a compile error that names its line, so a misspelled key never silently does nothing.

r[dep.manifest.fmt]
`quince fmt` also formats `quince.toml`: tables in the order of the example below, several `[cmd.*]` tables and the keys within a table as written.

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

r[dep.manifest.project]
- `[project]` is required, and so are its name and edition.

r[dep.manifest.no-version]
- The manifest holds no version of its own: a project's versions are its git tags. Every other table is optional.

r[dep.no-registry]
- There is no central registry. A dependency comes from a git repository or a folder.

r[dep.repo]
- `repo` is the repository's path without a scheme, fetched over HTTPS, and `version` is the minimum version needed: `"MAJOR.MINOR"` or `"MAJOR.MINOR.PATCH"`, where a missing part is 0.

r[dep.min-select]
- A minimum selects the lowest tag at or above it in its major, and `quince update` and `quince add` always write the full `MAJOR.MINOR.PATCH`.

r[dep.tag]
- A version is the repository's git tag `vMAJOR.MINOR.PATCH`, and the lockfile pins each one by content hash ([Content addressing](#content-addressing)).

r[dep.dir]
- A project in a subfolder of its repository adds `dir`: `textutil = { repo = "github.com/acme/tools", dir = "textutil", version = "0.3" }`.

r[dep.dir-tags]
- Its versions are the tags `textutil/vMAJOR.MINOR.PATCH`, as in Go, while tags without a prefix belong to the project at the repository's root, so every project in a repository has its own versions.

r[dep.key]
- The key is the name that code imports, and it must be the dependency's own name, the `name` in its `[project]` table, so a package is called the same in every project that uses it.

r[dep.rename]
- Any other key is a compile error, except where two dependencies would share a name: then one must be renamed, and only to the form the compiler suggests and `quince fix` writes, `<name>_v<N>` for another major version of the same repository (`pg_v2`) and `<owner>_<name>` for packages from different repositories that share a name, all of them renamed so that no name depends on which was added first (`acme_json` and `zeta_json` for github.com/acme/json and github.com/zeta/json; the owner is lowercased, with `-` and `.` turned into `_`).

r[dep.key-major]
- A key's major version is the major of its `version`.

r[dep.identity]
- Within a build, a package is identified by its repository, folder and major version, so every manifest that lists it shares one package.

r[dep.path]
- `path` names another project on this machine, relative to this manifest. It is built from the folder as it stands and isn't hashed into the lockfile.

r[dep.path-override]
- An entry may hold `path` beside `repo`, `dir` and `version`, as in Cargo: builds of this project use the folder, while dependents, the lockfile and `publish` see only the repository.

r[dep.path-publish]
- A project with an entry that has only `path` can't be published.

r[dep.lock.file]
- The lockfile is `quince.lock` beside `quince.toml`.

r[dep.lock.commit]
- The toolchain writes it, and every project commits it, libraries included.

r[dep.lock.format]
- It is TOML with one `[[package]]` table per dependency in the build, holding `repo`, `dir`, `major`, `tag` and one `hash` over the package's definition hashes (Content addressing).

r[dep.lock.path]
- A dependency with only `path` is listed with `path` alone.

r[dep.test-only]
- Names under `[test-dependencies]` may be imported by test packages, and by a library or program file only when every use is inside a `test` block or an `@test` declaration. Any other use is a compile error.

r[dep.test-unused]
- Such an import is never reported as unused in a non-test build.

r[dep.manifest.cmd-folder]
- Each `[cmd.<name>]` table must name an existing `cmd/<name>/` folder, and a program without one has no limits.

r[dep.manifest.cmd-net]
- `net` lists the hosts it may connect to, as `host`, `host:port` or `*.domain`.

r[dep.manifest.cmd-fs]
- `fs` lists the paths it may read or write, relative to the working directory at start, and each entry covers everything below it.

r[dep.manifest.path-normalize]
- Entries, and every path the program opens, are made absolute against that directory, normalized (`.` and `..` removed) and resolved through symlinks before they are compared, so an absolute path inside an allowed folder is allowed and a symlink can't lead out of one.

r[dep.manifest.limit-default]
- A missing key means no limit, and `[]` allows nothing.

r[dep.manifest.future-room]
- The format leaves room for a later `[workspace]` table, a caching proxy or checksum database for repositories, and per-dependency limits ([Program execution](../reference/execution.md)).

## Version resolution

r[dep.mvs.min]
Quince uses minimal version selection, as Go does, so the same manifest always builds the same dependency graph without a solver.

- A manifest states the minimum version it needs of each dependency (`version = "1.4"`).

r[dep.mvs.one-per-major]
- A build uses exactly one version per major version of each package: the highest minimum that anything in the graph asks for. A newer release is never picked up unless some manifest asks for it.

r[dep.mvs.majors-distinct]
- Different major versions are different packages. `http` and `http_v2` (major 2 of the same repository) can both be in one build, and `http.Response` and `http_v2.Response` are distinct types; a mismatch error names each with its repository and major version.

r[dep.update.raise]
- `quince` `update` raises the version of each direct dependency within its major, rewriting that dependency's `version` in `quince.toml` and the lockfile, and reports what changed in each dependency's public API.

r[dep.update.indirect]
- Indirect versions follow from the manifests that ask for them, so `update` and `add` change no other file, and both list every version that moved, including indirect ones.

r[dep.update.skip-breaking]
- A tag that the API check refuses (Enforced semantic versioning) is skipped: they take the newest tag in the major that passes, and name each skipped tag and the change that broke it.

r[dep.update.major-explicit]
- Moving to a new major version is always an explicit edit to the manifest.

## Content addressing

r[dep.hash.definition]
- Each published definition is identified by a hash of its normalized syntax tree plus the hashes of everything it depends on.

r[dep.hash.recursive]
- Mutually recursive definitions are hashed together as one group.

r[dep.hash.verify]
- Versions are chosen by name and version ([Version resolution](#version-resolution)), then verified against the lockfile's hashes, so a republished or tampered package can't change what an existing lockfile builds. Hashes also key the build cache ([Toolchain](toolchain.md)).

r[dep.lock.root-only]
- Only the root project's lockfile is read; a dependency's own lockfile is ignored.

r[dep.lock.build]
- `quince build` writes a missing lockfile and stops on one that no longer matches the manifests, naming `quince update`.

r[dep.hash.text-source]
- Source stays in ordinary text files; hashes are computed by the toolchain and stored in the lockfile.

## Enforced semantic versioning

r[dep.semver.compare]
- `quince` `publish` compares the new public API with the previous version: types, signatures, effects, and contracts.

r[dep.semver.breaking]
- Removing or changing anything public, adding an effect, or strengthening a `requires` clause is breaking and needs a major version bump. Publishing refuses otherwise.

r[dep.publish.version]
- Publishing picks the version from that comparison, the smallest bump it allows over the latest tag (major, minor or patch), or a larger one the author asks for, then creates and pushes that git tag.

r[dep.publish.first]
- The first tag is v0.1.0 unless the author names another.

r[dep.semver.bump]
- The bump follows from the comparison: a public API identical to the latest tag's allows a patch; any addition, such as a new public name, method, impl, optional field, parameter with a default, or variant of an `@extensible` enum, needs a minor; and a breaking change needs a major.

r[dep.semver.pre-1]
- Below v1.0.0 the same rules hold, so a breaking change after v0.3.2 is published as v1.0.0.

r[dep.publish.needs-review]
- A [needs-review finding](#enforced-semantic-versioning) is reported and doesn't stop the publish.

r[dep.publish.prefix]
- In a subfolder project, `publish` creates the prefixed tag (The manifest).

r[dep.publish.git]
- `publish` tags the current commit, refuses uncommitted changes in the project's folder, and pushes the tag to the git remote `origin`.

r[dep.publish.refuse]
- It refuses a project with no library, since nothing can depend on it (its programs are released with ordinary git tags), and a library that contains `dbg` or `todo`.

r[dep.semver.dep-check]
- Adding or updating a dependency repeats the same comparison for every tag it picks, against the newest earlier tag in that major version that passed this check, and refuses a tag that breaks it, naming the change. So a tag pushed by hand can't bring a breaking change into a minor version.

r[dep.semver.effvar]
- Removing an effect variable, or adding a named effect beside one (`uses E` to `uses E, Log`), is breaking.

r[dep.semver.effvar-add]
- Adding an effect variable to a closure parameter that was pure is not, since every existing caller passes a pure closure and still type-checks.

r[dep.semver.wire]
- For a public type that implements `Encode` or `Decode`, any change to its wire shape is breaking: renaming a field or variant without `@wire`, changing a `@wire` name, case or tag, adding a required field, or removing a field. Adding an `Option` field is not.

r[dep.semver.receiver]
- Changing a public method's receiver between `self` and `var self` is breaking in either direction.

r[dep.semver.variant]
- Adding an enum variant is breaking unless the enum is marked `@extensible`, which in turn forces callers to include a wildcard arm.

r[dep.semver.contract]
- Any change to a contract's normalized syntax tree counts as breaking (reformatting doesn't), because whether a new `requires` is stronger than the old one can't be decided in general.

r[dep.semver.iface-method]
- Adding a method to a public interface is breaking too. `quince` `publish` can't see a behavior change behind an unchanged signature; doc examples and integration tests are the guard there.

r[dep.semver.derived]
- `quince` `publish` also compares the properties the compiler derives for each public type: automatic `Eq`, `Hash`, `Encode` and `Decode`, whether it can cross a task boundary, and whether it can be held in a `const`. Losing any of them is breaking, for example by adding a field whose type has no `Hash`.

r[dep.semver.pub-field]
- Adding a field to a struct whose fields are all `pub` is breaking, because it breaks struct literals outside the package.

r[dep.semver.impl-review]
- Adding an impl to a public type is reported as needs-review, because it can create a method-name clash ([Items](../reference/items.md)) in a dependent. So is adding a `pub` method, for the same reason.
