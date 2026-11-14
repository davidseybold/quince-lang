//! Runs the `.qnc` conformance fixtures in `tests/conformance/testdata/` and reports spec
//! coverage by rule ID.
//!
//! The fixture format and the coverage report are in `tests/conformance/testdata/README.md`.
//! `cargo xtask conformance` drives this crate. The compiler plugs in through [`Backend`].

pub mod coverage;
pub mod fixture;

use fixture::{Fixture, Mode};
use quince_diagnostics::{Applicability, Diagnostic, Fix, LineIndex, Severity};
use quince_spec::{GRAMMAR_PATH, MOVED_PATH, README_PATH, RETIRED_PATH, Spec};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

pub const FIXTURE_DIR: &str = "tests/conformance/testdata";

/// The extensions of the files that sit beside a fixture: its expected standard output, and
/// its text once its fixes are applied.
const SIBLING_EXTENSIONS: [&str; 2] = ["stdout", "fixed"];

/// The section that grammar-production rule IDs are reported under.
pub const GRAMMAR_SECTION: &str = "Grammar productions";

/// What a compiler must provide to run fixtures. A method returns `None` when that mode isn't
/// implemented yet, and the fixture then fails rather than being skipped. `path` is the fixture's
/// path relative to the repo root, and `source` its bytes as read from disk, which need not be
/// valid UTF-8.
pub trait Backend {
    /// The parser's diagnostics for one file, and nothing from later stages.
    fn parse(&self, path: &str, source: &[u8]) -> Option<Vec<Diagnostic>>;
    /// Every diagnostic for one file, from every stage that exists.
    fn check(&self, path: &str, source: &[u8]) -> Option<Vec<Diagnostic>>;
    /// Builds and runs one file as a program. `Err` holds the build's error diagnostics.
    fn run(&self, path: &str, source: &[u8]) -> Option<Result<RunOutput, Vec<Diagnostic>>>;
}

pub struct RunOutput {
    pub exit: i32,
    pub stdout: String,
}

/// The compiler as far as it exists: `parse` runs the lexer and the parser. `check` waits for
/// M2, which decides how to wire the type checker, and `run` for M6.
pub struct SyntaxBackend;

impl Backend for SyntaxBackend {
    fn parse(&self, path: &str, source: &[u8]) -> Option<Vec<Diagnostic>> {
        // The runner rejects a fixture too large to decode before it calls a backend.
        let decoded = quince_syntax::decode_source(source).ok()?;
        Some(quince_syntax::parse(&decoded).diagnostics(&decoded.text, path))
    }

    fn check(&self, _path: &str, _source: &[u8]) -> Option<Vec<Diagnostic>> {
        None
    }

    fn run(&self, _path: &str, _source: &[u8]) -> Option<Result<RunOutput, Vec<Diagnostic>>> {
        None
    }
}

/// A backend that can't do anything, for testing the runner.
pub struct NoBackend;

impl Backend for NoBackend {
    fn parse(&self, _path: &str, _source: &[u8]) -> Option<Vec<Diagnostic>> {
        None
    }

    fn check(&self, _path: &str, _source: &[u8]) -> Option<Vec<Diagnostic>> {
        None
    }

    fn run(&self, _path: &str, _source: &[u8]) -> Option<Result<RunOutput, Vec<Diagnostic>>> {
        None
    }
}

/// Every rule ID, with the coverage section it belongs to: its chapter's title, or
/// [`GRAMMAR_SECTION`].
pub type RuleSet = BTreeMap<String, String>;

/// What fixtures are checked against: every live rule ID, the section order, and the retired
/// and moved IDs, so citing one gets its own message.
pub struct Rules {
    pub ids: RuleSet,
    /// Chapter titles in manifest order, ending with [`GRAMMAR_SECTION`].
    pub sections: Vec<String>,
    pub retired: HashSet<String>,
    /// Each ID that moved chapter, with the ID it moved to.
    pub moved: HashMap<String, String>,
    /// The draft version in the header of `spec/README.md`, such as `0.5`.
    pub spec_version: String,
}

/// The version in the `Draft vX.Y` header line, which must be within the first 10 lines.
pub fn spec_version(spec: &str) -> Option<&str> {
    spec.lines()
        .take(10)
        .find_map(|line| line.split("Draft v").nth(1))
        .map(|rest| rest.split_whitespace().next().unwrap_or(rest))
        .filter(|version| !version.is_empty())
}

/// Loads every rule ID: the tags in every chapter file plus `grammar.<production>` for each
/// production.
pub fn load_rules(root: &Path) -> Result<Rules, Vec<String>> {
    let read = |path: &str| {
        std::fs::read_to_string(root.join(path))
            .map_err(|e| vec![format!("can't read {path}: {e}")])
    };
    let located = |path: &str, errors: Vec<quince_spec::SpecError>| {
        errors
            .into_iter()
            .map(|e| quince_spec::FileError::new(path, e).to_string())
            .collect::<Vec<_>>()
    };
    let spec = Spec::load(root)
        .map_err(|errors| errors.iter().map(ToString::to_string).collect::<Vec<_>>())?;
    let (retired, retired_errors) = quince_spec::rules::parse_retired(&read(RETIRED_PATH)?);
    let (moved, moved_errors) = quince_spec::rules::parse_moved(&read(MOVED_PATH)?);
    let mut errors = located(RETIRED_PATH, retired_errors);
    errors.extend(located(MOVED_PATH, moved_errors));
    let version = spec_version(&spec.readme).map(str::to_string);
    if version.is_none() {
        errors.push(format!(
            "{README_PATH}: the header has no `Draft vX.Y` version line in its first 10 lines"
        ));
    }
    let (rules, spec_errors) = quince_spec::rules::check(&spec.files, &retired, &spec.chapters);
    errors.extend(spec_errors.iter().map(ToString::to_string));
    let moved_errors = quince_spec::rules::check_moved(&moved, &rules, &retired);
    errors.extend(moved_errors.iter().map(ToString::to_string));
    if !errors.is_empty() {
        return Err(errors);
    }
    let mut sections: Vec<String> = spec.files.iter().map(|f| f.title.clone()).collect();
    sections.push(GRAMMAR_SECTION.to_string());
    let mut ids: RuleSet = rules.into_iter().map(|r| (r.id, r.chapter)).collect();
    let grammar_file = read(GRAMMAR_PATH)?;
    let (_, grammar) = quince_spec::grammar::split_header(&grammar_file);
    for production in quince_spec::grammar::productions(grammar) {
        ids.insert(
            format!("grammar.{}", production.name),
            GRAMMAR_SECTION.to_string(),
        );
    }
    Ok(Rules {
        ids,
        sections,
        retired,
        moved: moved
            .into_iter()
            .map(|(old, (new, _))| (old, new))
            .collect(),
        spec_version: version.expect("checked above"),
    })
}

/// One fixture file found on disk, before parsing.
#[derive(Debug)]
pub struct FixtureFile {
    pub name: String,
    /// Relative to the repo root.
    pub path: String,
    pub source: Vec<u8>,
    pub stdout: Option<String>,
    pub fixed: Option<String>,
}

/// Every `.qnc` file under `testdata/`, in sorted path order, with its sibling `.stdout` and
/// `.fixed`. A missing folder means no fixtures. A sibling with no `.qnc` file is an error.
pub fn discover(root: &Path) -> Result<Vec<FixtureFile>, Vec<String>> {
    discover_in(&root.join(FIXTURE_DIR), FIXTURE_DIR)
}

/// `discover` for the fixture folder `dir`, whose repo-relative path is `relative_dir`.
fn discover_in(dir: &Path, relative_dir: &str) -> Result<Vec<FixtureFile>, Vec<String>> {
    let mut paths = Vec::new();
    if dir.exists() {
        collect(dir, &mut paths).map_err(|e| vec![e])?;
    }
    paths.sort();
    let name_of = |path: &Path| {
        path.strip_prefix(dir)
            .expect("found below the fixture folder")
            .with_extension("")
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/")
    };
    let has_ext = |path: &Path, ext: &str| path.extension().is_some_and(|e| e == ext);
    let fixtures: HashSet<PathBuf> = paths
        .iter()
        .filter(|p| has_ext(p, "qnc"))
        .map(|p| p.with_extension(""))
        .collect();
    let mut errors = Vec::new();
    let mut files = Vec::new();
    for path in &paths {
        if let Some(ext) = SIBLING_EXTENSIONS.iter().find(|ext| has_ext(path, ext)) {
            if !fixtures.contains(&path.with_extension("")) {
                errors.push(format!(
                    "{relative_dir}/{}.{ext} has no matching .qnc fixture; rename or remove it",
                    name_of(path)
                ));
            }
            continue;
        }
        let name = name_of(path);
        let source = match std::fs::read(path) {
            Ok(source) => source,
            Err(e) => {
                errors.push(format!("can't read {}: {e}", path.display()));
                continue;
            }
        };
        let read_sibling = |ext: &str| match std::fs::read_to_string(path.with_extension(ext)) {
            Ok(text) => Ok(Some(text)),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("can't read {relative_dir}/{name}.{ext}: {e}")),
        };
        let (stdout, fixed) = match (read_sibling("stdout"), read_sibling("fixed")) {
            (Ok(stdout), Ok(fixed)) => (stdout, fixed),
            (stdout, fixed) => {
                errors.extend(stdout.err().into_iter().chain(fixed.err()));
                continue;
            }
        };
        files.push(FixtureFile {
            path: format!("{relative_dir}/{name}.qnc"),
            name,
            source,
            stdout,
            fixed,
        });
    }
    if errors.is_empty() {
        Ok(files)
    } else {
        Err(errors)
    }
}

/// Every `.qnc` file and every sibling file below `dir`.
fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("can't read {}: {e}", dir.display()))?;
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_dir() {
            collect(&path, out)?;
        } else if path
            .extension()
            .is_some_and(|ext| ext == "qnc" || SIBLING_EXTENSIONS.iter().any(|s| ext == *s))
        {
            out.push(path);
        }
    }
    Ok(())
}

/// The verdict on one fixture.
#[derive(Debug)]
pub struct FixtureResult {
    pub name: String,
    pub rules: Vec<String>,
    /// Empty when the fixture passed.
    pub failures: Vec<String>,
}

impl FixtureResult {
    pub fn passed(&self) -> bool {
        self.failures.is_empty()
    }
}

/// Parses and runs one fixture file against `backend`.
pub fn run_file(file: &FixtureFile, rules: &Rules, backend: &dyn Backend) -> FixtureResult {
    match fixture::parse(&file.name, &file.source, file.stdout.clone()) {
        Ok(mut fixture) => {
            fixture.fixed = file.fixed.clone();
            run_fixture(&fixture, &file.path, rules, backend)
        }
        Err(failures) => FixtureResult {
            name: file.name.clone(),
            rules: Vec::new(),
            failures,
        },
    }
}

pub fn run_fixture(
    fixture: &Fixture,
    path: &str,
    rules: &Rules,
    backend: &dyn Backend,
) -> FixtureResult {
    let mut failures: Vec<String> = fixture
        .rules
        .iter()
        .filter(|id| !rules.ids.contains_key(*id))
        .map(|id| {
            if let Some(new) = rules.moved.get(id) {
                if rules.retired.contains(new) {
                    format!(
                        "rule `{id}` moved to `{new}`, which is retired ({RETIRED_PATH}); cite the rule that replaced it"
                    )
                } else {
                    format!("rule `{id}` moved: cite `{new}` ({MOVED_PATH})")
                }
            } else if rules.retired.contains(id) {
                format!(
                    "cites retired rule ID `{id}` ({RETIRED_PATH}); cite the rule that replaced it"
                )
            } else {
                format!("cites unknown rule ID `{id}`")
            }
        })
        .collect();
    let no_backend = |what: &str| {
        format!(
            "no backend: the compiler can't {what} fixtures yet (only `parse` is wired; `check` comes in M2 and `run` in M6)"
        )
    };
    match fixture.mode {
        Mode::Check | Mode::Parse => {
            let (diagnostics, what) = if fixture.mode == Mode::Check {
                (backend.check(path, &fixture.source), "check")
            } else {
                (backend.parse(path, &fixture.source), "parse")
            };
            match diagnostics {
                Some(diagnostics) => {
                    failures.extend(match_diagnostics(fixture, path, &diagnostics));
                    if let Some(expected) = &fixture.fixed {
                        failures.extend(compare_fixed(fixture, path, &diagnostics, expected));
                    }
                }
                None => failures.push(no_backend(what)),
            }
        }
        Mode::Run if fixture.fixed.is_some() => failures.push(format!(
            "{}.fixed exists, but only `// mode: check` and `// mode: parse` fixtures apply fixes",
            fixture.name
        )),
        Mode::Run => match backend.run(path, &fixture.source) {
            Some(Ok(output)) => {
                if output.exit != fixture.exit {
                    failures.push(format!(
                        "exited with {}, expected {}",
                        output.exit, fixture.exit
                    ));
                }
                let expected = fixture.stdout.as_deref().unwrap_or("");
                if output.stdout != expected {
                    let source = if fixture.stdout.is_some() {
                        format!("{}.stdout", fixture.name)
                    } else {
                        format!("no output (there is no {}.stdout)", fixture.name)
                    };
                    failures.push(format!(
                        "stdout differs from {source}:\n--- expected\n{expected}--- actual\n{}",
                        output.stdout
                    ));
                }
            }
            Some(Err(errors)) => {
                failures.push("the program didn't build".to_string());
                failures.extend(errors.iter().map(|d| describe("error", d)));
            }
            None => failures.push(no_backend("build and run")),
        },
    }
    FixtureResult {
        name: fixture.name.clone(),
        rules: fixture.rules.clone(),
        failures,
    }
}

/// Pairs each expectation with a diagnostic of the same code starting on its line. Unmatched
/// expectations and unexpected errors or warnings are failures, and so is a malformed fix. A
/// diagnostic that starts at the end of input after a trailing line break counts as starting on
/// the last line. `path` is the fixture's file, the only one that exists for a fix to edit.
fn match_diagnostics(fixture: &Fixture, path: &str, diagnostics: &[Diagnostic]) -> Vec<String> {
    let index = LineIndex::new(&fixture.text);
    // After a trailing line break, the last line is the empty one at the end of input.
    let end_line = index.line_count();
    let ends_with_line_break = fixture.text.ends_with(['\n', '\r']);
    let line_of = |d: &Diagnostic| {
        let line = d.span.start.line();
        if line == end_line && ends_with_line_break {
            line - 1
        } else {
            line
        }
    };
    let mut used = vec![false; diagnostics.len()];
    let mut failures = Vec::new();
    // A fix's edits default to the span's file, so a wrong one would fail as a missing file.
    for d in diagnostics.iter().filter(|d| d.span.file != path) {
        failures.push(format!(
            "{} on line {} names the file `{}`, but the fixture is `{path}`",
            d.code,
            d.span.start.line(),
            d.span.file
        ));
    }
    for expectation in &fixture.expectations {
        let found = diagnostics.iter().enumerate().position(|(i, d)| {
            !used[i] && d.code == expectation.code && line_of(d) == expectation.line
        });
        match found {
            Some(i) => used[i] = true,
            None => failures.push(format!(
                "expected {} on line {}, but no such diagnostic was reported",
                expectation.code, expectation.line
            )),
        }
    }
    for (d, _) in diagnostics.iter().zip(&used).filter(|(_, used)| !**used) {
        if matches!(d.severity, Severity::Error | Severity::Warning) {
            failures.push(describe("unexpected", d));
        }
    }
    for d in diagnostics {
        for fix in &d.fixes {
            let checked = fix.check(&d.span.file).and_then(|()| {
                fix.check_positions(&d.span.file, |file| (file == path).then_some(&index))
            });
            if let Err(why) = checked {
                failures.push(format!(
                    "the fix \"{}\" on {} at line {} is malformed: {why}",
                    fix.description,
                    d.code,
                    d.span.start.line()
                ));
            }
        }
    }
    failures
}

/// Compares the fixture's text, once every fix in `diagnostics` is applied, with its `.fixed`
/// file, `expected`.
fn compare_fixed(
    fixture: &Fixture,
    path: &str,
    diagnostics: &[Diagnostic],
    expected: &str,
) -> Vec<String> {
    let name = &fixture.name;
    match apply_fixes(&fixture.text, path, diagnostics) {
        Ok(actual) if actual == expected => Vec::new(),
        Ok(actual) => {
            let line_break = if expected.ends_with(['\n', '\r']) {
                ""
            } else {
                "\n"
            };
            vec![format!(
                "applying every fix gives text that differs from {name}.fixed {}:\n--- expected\n{expected}{line_break}--- actual\n{actual}",
                first_difference(expected, &actual)
            )]
        }
        Err(why) => vec![format!("can't check {name}.fixed: {why}")],
    }
}

/// Where `actual` first differs from `expected`, as `at line 3: expected "a\r\n", got "a\n"`,
/// quoted so that a different line break shows.
fn first_difference(expected: &str, actual: &str) -> String {
    let (mut expected_lines, mut actual_lines) = (lines(expected), lines(actual));
    let mut line = 1;
    loop {
        let (e, a) = (expected_lines.next(), actual_lines.next());
        if e != a || e.is_none() {
            let show =
                |l: Option<&str>| l.map_or("the end of the text".to_string(), |l| format!("{l:?}"));
            return format!("at line {line}: expected {}, got {}", show(e), show(a));
        }
        line += 1;
    }
}

/// `text`'s lines, each with its line break: LF, CR LF or a lone CR, as positions count them.
fn lines(text: &str) -> impl Iterator<Item = &str> {
    let mut rest = text;
    std::iter::from_fn(move || {
        if rest.is_empty() {
            return None;
        }
        let end = rest.find(['\n', '\r']).map_or(rest.len(), |i| {
            if rest[i..].starts_with("\r\n") {
                i + 2
            } else {
                i + 1
            }
        });
        let (line, after) = rest.split_at(end);
        rest = after;
        Some(line)
    })
}

/// `text`, the fixture `path`, after every fix in `diagnostics` is applied, as if all at once.
/// Each diagnostic may offer at most one fix, since there is no choosing between them, and a fix
/// may edit only the fixture. No two fixes may overlap or meet at an insertion, the rule
/// `Fix::check` applies within one fix. A malformed fix is an error too, and
/// `match_diagnostics` says why.
fn apply_fixes(text: &str, path: &str, diagnostics: &[Diagnostic]) -> Result<String, String> {
    let index = LineIndex::new(text);
    let mut fixes = Vec::new();
    for d in diagnostics {
        let at = format!("{} at line {}", d.code, d.span.start.line());
        match d.fixes.as_slice() {
            [] => {}
            [fix] => fixes.push((format!("the fix \"{}\" on {at}", fix.description), d, fix)),
            many => {
                return Err(format!(
                    "{at} offers {} fixes, and only one can be applied",
                    many.len()
                ));
            }
        }
    }
    for (which, d, fix) in &fixes {
        let span_file = d.span.file.as_str();
        let other = fix
            .edits
            .iter()
            .map(|e| e.file.as_deref().unwrap_or(span_file))
            .find(|file| *file != path);
        if let Some(file) = other {
            return Err(format!(
                "{which} edits `{file}`, and a .fixed file can only show edits to the fixture"
            ));
        }
        let checked = fix
            .check(span_file)
            .and_then(|()| fix.check_positions(span_file, |_| Some(&index)));
        if checked.is_err() {
            return Err(format!("{which} is malformed, as reported above"));
        }
    }
    for (i, (which, _, fix)) in fixes.iter().enumerate() {
        for (other_which, _, other) in &fixes[i + 1..] {
            for a in &fix.edits {
                for b in &other.edits {
                    let pair = Fix {
                        description: String::new(),
                        applicability: Applicability::NeedsReview,
                        edits: vec![a.clone(), b.clone()],
                    };
                    if pair.check(path).is_err() {
                        return Err(format!(
                            "{which} edits {:?}-{:?} and {other_which} edits {:?}-{:?}, which \
                             overlap or meet at an insertion, so they can't both be applied",
                            a.start, a.end, b.start, b.end
                        ));
                    }
                }
            }
        }
    }
    let offset = |position| {
        index
            .offset(position)
            .expect("check_positions found every position in the fixture")
    };
    let mut replacements: Vec<_> = fixes
        .iter()
        .flat_map(|(_, _, fix)| &fix.edits)
        .map(|edit| (offset(edit.start), offset(edit.end), edit.text.as_str()))
        .collect();
    replacements.sort_by_key(|&(start, end, _)| (start, end));
    let mut fixed = String::new();
    let mut at = 0;
    for (start, end, with) in replacements {
        fixed.push_str(&text[at..start]);
        fixed.push_str(with);
        at = end;
    }
    fixed.push_str(&text[at..]);
    Ok(fixed)
}

/// `<prefix> error E0100 on line 4: message`.
fn describe(prefix: &str, d: &Diagnostic) -> String {
    let severity = match d.severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    };
    let prefix = if prefix == severity {
        prefix.to_string()
    } else {
        format!("{prefix} {severity}")
    };
    format!(
        "{prefix} {} on line {}: {}",
        d.code,
        d.span.start.line(),
        d.message
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use quince_diagnostics::{Code, Edit, Position, Span};

    /// Returns fixed diagnostics, or fixed run output.
    #[derive(Default)]
    struct Fake {
        /// `(code, line, column)` reported by both `parse` and `check`.
        diagnostics: Vec<(&'static str, u32, u32)>,
        /// Only reported by `check`, as a later stage would.
        check_only: Vec<(&'static str, u32, u32)>,
        run: Option<Result<(i32, &'static str), &'static str>>,
    }

    fn diagnostic(path: &str, (code, line, column): (&str, u32, u32)) -> Diagnostic {
        let code = Code::parse(code).unwrap();
        let severity = code.default_severity();
        let span = Span {
            file: path.to_string(),
            start: Position(line, column),
            end: Position(line, column + 1),
        };
        Diagnostic::new(code, severity, "from the fake", span)
    }

    impl Backend for Fake {
        fn parse(&self, path: &str, _source: &[u8]) -> Option<Vec<Diagnostic>> {
            Some(
                self.diagnostics
                    .iter()
                    .map(|&d| diagnostic(path, d))
                    .collect(),
            )
        }

        fn check(&self, path: &str, _source: &[u8]) -> Option<Vec<Diagnostic>> {
            let all = self.diagnostics.iter().chain(&self.check_only);
            Some(all.map(|&d| diagnostic(path, d)).collect())
        }

        fn run(&self, path: &str, _source: &[u8]) -> Option<Result<RunOutput, Vec<Diagnostic>>> {
            self.run.map(|result| match result {
                Ok((exit, stdout)) => Ok(RunOutput {
                    exit,
                    stdout: stdout.to_string(),
                }),
                Err(code) => Err(vec![diagnostic(path, (code, 1, 1))]),
            })
        }
    }

    fn rules() -> Rules {
        Rules {
            ids: [
                ("bind.let-var", "2. Syntax"),
                ("main.signature", "5. Effects"),
            ]
            .into_iter()
            .map(|(id, section)| (id.to_string(), section.to_string()))
            .collect(),
            sections: Vec::new(),
            retired: ["bind.old".to_string(), "type.old".to_string()].into(),
            moved: [("bind.moved", "main.signature"), ("bind.gone", "type.old")]
                .into_iter()
                .map(|(old, new)| (old.to_string(), new.to_string()))
                .collect(),
            spec_version: "0.5".into(),
        }
    }

    fn run(source: &str, stdout: Option<&str>, backend: &dyn Backend) -> FixtureResult {
        run_with(source, stdout, None, backend)
    }

    fn run_with(
        source: &str,
        stdout: Option<&str>,
        fixed: Option<&str>,
        backend: &dyn Backend,
    ) -> FixtureResult {
        let file = FixtureFile {
            name: "t".into(),
            path: "tests/conformance/testdata/t.qnc".into(),
            source: source.into(),
            stdout: stdout.map(str::to_string),
            fixed: fixed.map(str::to_string),
        };
        run_file(&file, &rules(), backend)
    }

    fn fake(diagnostics: Vec<(&'static str, u32)>) -> Fake {
        Fake {
            diagnostics: diagnostics.into_iter().map(|(c, l)| (c, l, 1)).collect(),
            ..Fake::default()
        }
    }

    fn ran(exit: i32, stdout: &'static str) -> Fake {
        Fake {
            run: Some(Ok((exit, stdout))),
            ..Fake::default()
        }
    }

    #[test]
    fn matched_expectation_passes() {
        let r = run(
            "// rule: bind.let-var\nlet x = 1 //~ E0100\n",
            None,
            &fake(vec![("E0100", 2)]),
        );
        assert!(r.passed(), "{:?}", r.failures);
    }

    #[test]
    fn caret_expectation_matches_the_line_above() {
        let source = "// rule: bind.let-var\nx\ny\nz //~^^ E0100\n";
        assert!(run(source, None, &fake(vec![("E0100", 2)])).passed());
    }

    #[test]
    fn missing_diagnostic_fails() {
        let r = run("// rule: bind.let-var\nx //~ E0100\n", None, &fake(vec![]));
        assert_eq!(
            r.failures,
            ["expected E0100 on line 2, but no such diagnostic was reported"]
        );
    }

    #[test]
    fn unexpected_diagnostic_fails() {
        let r = run(
            "// rule: bind.let-var\nx\n",
            None,
            &fake(vec![("W0003", 2)]),
        );
        assert_eq!(
            r.failures,
            ["unexpected warning W0003 on line 2: from the fake"]
        );
    }

    #[test]
    fn wrong_line_fails() {
        let r = run(
            "// rule: bind.let-var\nx //~ E0100\ny\n",
            None,
            &fake(vec![("E0100", 3)]),
        );
        assert_eq!(r.failures.len(), 2, "{:?}", r.failures);
    }

    #[test]
    fn each_code_expects_one_diagnostic() {
        let source = "// rule: bind.let-var\nx //~ E0001 E0001\n";
        let once = run(source, None, &fake(vec![("E0001", 2)]));
        assert_eq!(
            once.failures,
            ["expected E0001 on line 2, but no such diagnostic was reported"]
        );
        assert!(run(source, None, &fake(vec![("E0001", 2), ("E0001", 2)])).passed());
    }

    #[test]
    fn end_of_input_matches_the_last_line() {
        let source = "// rule: bind.let-var\nfn f() {\n}\nx\ny //~ E0002\n";
        assert!(run(source, None, &fake(vec![("E0002", 6)])).passed());
        assert!(!run(source, None, &fake(vec![("E0002", 7)])).passed());
        for ending in ["\r\n", "\r"] {
            let source = source.replace('\n', ending);
            assert!(
                run(&source, None, &fake(vec![("E0002", 6)])).passed(),
                "{ending:?}"
            );
            assert!(
                !run(&source, None, &fake(vec![("E0002", 7)])).passed(),
                "{ending:?}"
            );
        }
        let no_newline = "// rule: bind.let-var\nx //~ E0002";
        assert!(run(no_newline, None, &fake(vec![("E0002", 2)])).passed());
    }

    #[test]
    fn parse_mode_ignores_later_stages() {
        let backend = Fake {
            check_only: vec![("W0003", 2, 1)],
            ..Fake::default()
        };
        let parse = "// rule: bind.let-var\n// mode: parse\nlet x = 1\n";
        assert!(run(parse, None, &backend).passed());
        let check = "// rule: bind.let-var\nlet x = 1\n";
        assert!(!run(check, None, &backend).passed());
    }

    #[test]
    fn unknown_and_retired_rules_fail() {
        let r = run("// rule: bind.nonexistent\nx\n", None, &fake(vec![]));
        assert_eq!(r.failures, ["cites unknown rule ID `bind.nonexistent`"]);
        let r = run("// rule: bind.old\nx\n", None, &fake(vec![]));
        assert_eq!(
            r.failures,
            [
                "cites retired rule ID `bind.old` (spec/retired-rules.txt); cite the rule that replaced it"
            ]
        );
    }

    #[test]
    fn moved_rules_fail_with_the_new_id() {
        let r = run("// rule: bind.moved\nx\n", None, &fake(vec![]));
        assert_eq!(
            r.failures,
            ["rule `bind.moved` moved: cite `main.signature` (spec/moved-rules.txt)"]
        );
        let r = run("// rule: bind.gone\nx\n", None, &fake(vec![]));
        assert_eq!(
            r.failures,
            [
                "rule `bind.gone` moved to `type.old`, which is retired (spec/retired-rules.txt); cite the rule that replaced it"
            ]
        );
    }

    #[test]
    fn no_backend_fails_loudly() {
        for source in [
            "// rule: bind.let-var\nx\n",
            "// rule: bind.let-var\n// mode: parse\nx\n",
            "// rule: main.signature\n// mode: run\nx\n",
        ] {
            let r = run(source, None, &NoBackend);
            assert!(r.failures[0].starts_with("no backend:"), "{:?}", r.failures);
        }
    }

    #[test]
    fn syntax_backend_lexes_parse_fixtures_only() {
        let source = "// rule: bind.let-var\n// mode: parse\nfn f() {\n  let x = 1; //~ E0002\n}\n";
        let r = run(source, None, &SyntaxBackend);
        assert!(r.passed(), "{:?}", r.failures);
        let r = run("// rule: bind.let-var\nx\n", None, &SyntaxBackend);
        assert!(r.failures[0].starts_with("no backend:"), "{:?}", r.failures);
    }

    /// Reports one E0100 on line 2, in another file.
    struct WrongFile;

    impl Backend for WrongFile {
        fn parse(&self, _path: &str, _source: &[u8]) -> Option<Vec<Diagnostic>> {
            Some(vec![diagnostic("src/elsewhere.qnc", ("E0100", 2, 1))])
        }

        fn check(&self, path: &str, source: &[u8]) -> Option<Vec<Diagnostic>> {
            self.parse(path, source)
        }

        fn run(&self, _path: &str, _source: &[u8]) -> Option<Result<RunOutput, Vec<Diagnostic>>> {
            None
        }
    }

    #[test]
    fn a_diagnostic_in_another_file_fails() {
        let r = run("// rule: bind.let-var\nx //~ E0100\n", None, &WrongFile);
        assert_eq!(
            r.failures,
            [
                "E0100 on line 2 names the file `src/elsewhere.qnc`, but the fixture is \
                 `tests/conformance/testdata/t.qnc`"
            ]
        );
    }

    /// A parse fixture with a `.fixed` file: the parser's real fixes, applied together.
    #[test]
    fn fixed_file_holds_the_text_with_every_fix_applied() {
        let source =
            "// rule: bind.let-var\n// mode: parse\nstruct S {} //~ E0024\nstruct T {} //~ E0024\n";
        let fixed =
            "// rule: bind.let-var\n// mode: parse\nstruct S //~ E0024\nstruct T //~ E0024\n";
        let r = run_with(source, None, Some(fixed), &SyntaxBackend);
        assert!(r.passed(), "{:?}", r.failures);
        let r = run_with(source, None, Some(source), &SyntaxBackend);
        assert_eq!(r.failures.len(), 1, "{:?}", r.failures);
        assert!(
            r.failures[0].starts_with(
                "applying every fix gives text that differs from t.fixed at line 3: \
                 expected \"struct S {} //~ E0024\\n\", got \"struct S //~ E0024\\n\":"
            ),
            "{:?}",
            r.failures
        );
        // With no fix offered, the `.fixed` file is the fixture itself.
        let source = "// rule: bind.let-var\n// mode: parse\nfn f(a: List<Int) {} //~ E0017\n";
        let r = run_with(source, None, Some(source), &SyntaxBackend);
        assert!(r.passed(), "{:?}", r.failures);
        assert!(!run_with(source, None, Some(""), &SyntaxBackend).passed());
    }

    #[test]
    fn a_run_fixture_cant_have_a_fixed_file() {
        let source = "// rule: main.signature\n// mode: run\nfn main() {}\n";
        let r = run_with(source, None, Some(source), &ran(0, ""));
        assert_eq!(
            r.failures,
            ["t.fixed exists, but only `// mode: check` and `// mode: parse` fixtures apply fixes"]
        );
    }

    /// Reports `diagnostics` for every file, whatever it holds.
    struct Canned(Vec<Diagnostic>);

    impl Backend for Canned {
        fn parse(&self, _path: &str, _source: &[u8]) -> Option<Vec<Diagnostic>> {
            Some(self.0.clone())
        }

        fn check(&self, path: &str, source: &[u8]) -> Option<Vec<Diagnostic>> {
            self.parse(path, source)
        }

        fn run(&self, _path: &str, _source: &[u8]) -> Option<Result<RunOutput, Vec<Diagnostic>>> {
            None
        }
    }

    /// An E0100 on line 2 of the fixture, with one fix per list of edits, each replacing
    /// `(file, start column, end column)` on line 2 with `x`.
    fn with_fixes(fixes: &[&[(Option<&str>, u32, u32)]]) -> Diagnostic {
        let mut d = diagnostic("tests/conformance/testdata/t.qnc", ("E0100", 2, 1));
        d.fixes = fixes
            .iter()
            .map(|edits| Fix {
                description: "a fix".to_string(),
                applicability: Applicability::Safe,
                edits: edits
                    .iter()
                    .map(|&(file, start, end)| Edit {
                        file: file.map(str::to_string),
                        start: Position(2, start),
                        end: Position(2, end),
                        text: "x".to_string(),
                    })
                    .collect(),
            })
            .collect();
        d
    }

    #[test]
    fn fixes_that_cant_all_be_applied_fail_a_fixed_file() {
        let source = "// rule: bind.let-var\nab //~ E0100\n";
        let failure = |diagnostics: Vec<Diagnostic>| {
            let r = run_with(source, None, Some(source), &Canned(diagnostics));
            r.failures.last().cloned().unwrap_or_default()
        };
        assert_eq!(
            failure(vec![with_fixes(&[&[(None, 1, 2)], &[(None, 2, 3)]])]),
            "can't check t.fixed: E0100 at line 2 offers 2 fixes, and only one can be applied"
        );
        assert_eq!(
            failure(vec![with_fixes(&[&[(Some("src/other.qnc"), 1, 1)]])]),
            "can't check t.fixed: the fix \"a fix\" on E0100 at line 2 edits `src/other.qnc`, \
             and a .fixed file can only show edits to the fixture"
        );
        let overlapping = vec![
            with_fixes(&[&[(None, 1, 3)]]),
            with_fixes(&[&[(None, 2, 3)]]),
        ];
        assert_eq!(
            failure(overlapping),
            "can't check t.fixed: the fix \"a fix\" on E0100 at line 2 edits \
             Position(2, 1)-Position(2, 3) and the fix \"a fix\" on E0100 at line 2 edits \
             Position(2, 2)-Position(2, 3), which overlap or meet at an insertion, so they \
             can't both be applied"
        );
        // A fix that is malformed by itself is reported once, by `match_diagnostics`.
        let r = run_with(
            source,
            None,
            Some(source),
            &Canned(vec![with_fixes(&[&[(None, 3, 1)]])]),
        );
        assert_eq!(r.failures.len(), 2, "{:?}", r.failures);
        assert!(
            r.failures[0].contains("is malformed: edit in"),
            "{:?}",
            r.failures
        );
        assert_eq!(
            r.failures[1],
            "can't check t.fixed: the fix \"a fix\" on E0100 at line 2 is malformed, as reported above"
        );
    }

    #[test]
    fn first_difference_quotes_line_breaks() {
        let cases = [
            (
                "a\r\nb\r\n",
                "a\r\nb\n",
                r#"at line 2: expected "b\r\n", got "b\n""#,
            ),
            ("a\rb", "a\rb\r", r#"at line 2: expected "b", got "b\r""#),
            (
                "a\n",
                "a\nb\n",
                r#"at line 2: expected the end of the text, got "b\n""#,
            ),
        ];
        for (expected, actual, difference) in cases {
            assert_eq!(first_difference(expected, actual), difference);
        }
    }

    #[test]
    fn run_mode_compares_exit_and_stdout() {
        let source = "// rule: main.signature\n// mode: run\nfn main() {}\n";
        assert!(run(source, Some("hi\n"), &ran(0, "hi\n")).passed());
        assert_eq!(
            run(source, Some("hi\n"), &ran(1, "bye\n")).failures.len(),
            2
        );
    }

    #[test]
    fn run_without_stdout_file_expects_no_output() {
        let source = "// rule: main.signature\n// mode: run\nfn main() {}\n";
        assert!(run(source, None, &ran(0, "")).passed());
        let r = run(source, None, &ran(0, "hi\n"));
        assert!(
            r.failures[0].starts_with("stdout differs from no output"),
            "{:?}",
            r.failures
        );
    }

    #[test]
    fn run_fixture_that_doesnt_build_fails() {
        let source = "// rule: main.signature\n// mode: run\nfn main() {}\n";
        let backend = Fake {
            run: Some(Err("E0100")),
            ..Fake::default()
        };
        assert_eq!(
            run(source, None, &backend).failures,
            [
                "the program didn't build",
                "error E0100 on line 1: from the fake"
            ]
        );
    }

    /// An edit's start, end and file, as `(line, column)` pairs.
    type FakeEdit = ((u32, u32), (u32, u32), Option<&'static str>);

    /// Reports one E0100 on line 2 with one fix made of the given edits.
    struct FixBackend(Vec<FakeEdit>);

    impl Backend for FixBackend {
        fn parse(&self, path: &str, source: &[u8]) -> Option<Vec<Diagnostic>> {
            self.check(path, source)
        }

        fn check(&self, path: &str, _source: &[u8]) -> Option<Vec<Diagnostic>> {
            let mut d = diagnostic(path, ("E0100", 2, 1));
            d.fixes.push(quince_diagnostics::Fix {
                description: "rename".into(),
                applicability: quince_diagnostics::Applicability::Safe,
                edits: self
                    .0
                    .iter()
                    .map(|&(start, end, file)| quince_diagnostics::Edit {
                        file: file.map(str::to_string),
                        start: Position(start.0, start.1),
                        end: Position(end.0, end.1),
                        text: "y".into(),
                    })
                    .collect(),
            });
            Some(vec![d])
        }

        fn run(&self, _path: &str, _source: &[u8]) -> Option<Result<RunOutput, Vec<Diagnostic>>> {
            None
        }
    }

    #[test]
    fn reported_fixes_are_checked() {
        let source = "// rule: bind.let-var
let x = 1 //~ E0100
";
        let good = FixBackend(vec![((2, 5), (2, 6), None)]);
        assert!(run(source, None, &good).passed());
        let overlapping = FixBackend(vec![((2, 1), (2, 6), None), ((2, 3), (2, 9), None)]);
        let r = run(source, None, &overlapping);
        assert_eq!(r.failures.len(), 1, "{:?}", r.failures);
        assert!(
            r.failures[0]
                .starts_with("the fix \"rename\" on E0100 at line 2 is malformed: edits in"),
            "{:?}",
            r.failures
        );
    }

    #[test]
    fn fix_positions_must_be_in_the_fixture() {
        let source = "// rule: bind.let-var\r\nlet x = 1 //~ E0100\r\n";
        let line_end = FixBackend(vec![((2, 20), (2, 20), None)]);
        assert!(run(source, None, &line_end).passed());
        let past_line_end = FixBackend(vec![((2, 5), (2, 21), None)]);
        assert_eq!(
            run(source, None, &past_line_end).failures,
            [
                "the fix \"rename\" on E0100 at line 2 is malformed: edit in \
                 tests/conformance/testdata/t.qnc ends at Position(2, 21), which isn't in the \
                 file: line 2 has columns 1 to 20"
            ]
        );
        let new_file = FixBackend(vec![((1, 1), (1, 1), Some("src/new.qnc"))]);
        assert!(run(source, None, &new_file).passed());
        let into_missing_file = FixBackend(vec![((2, 1), (2, 3), Some("src/new.qnc"))]);
        assert_eq!(run(source, None, &into_missing_file).failures.len(), 1);
    }

    #[test]
    fn reads_the_spec_version() {
        assert_eq!(spec_version("# Spec\n\nNov 10 · Draft v0.5\n"), Some("0.5"));
        assert_eq!(spec_version("# Spec\n"), None);
        let late = format!("# Spec\n{}Draft v0.5\n", "\n".repeat(9));
        assert_eq!(spec_version(&late), None);
    }

    /// `load_rules` over a small spec the test writes: each chapter's tags map to its title,
    /// grammar productions to [`GRAMMAR_SECTION`], which comes last, and the version is read.
    #[test]
    fn loads_rules_from_a_small_spec() {
        let root = std::env::temp_dir()
            .join(format!("quince_conformance-tests-{}", std::process::id()))
            .join("small-spec");
        let _ = std::fs::remove_dir_all(&root);
        let files = [
            (
                "spec/chapters.txt",
                "lex  reference/lexical.md  Lexical structure\n\
                 stmt  reference/statements.md  Statements\n",
            ),
            ("spec/README.md", "# Spec\n\nNov 11 · David · Draft v1.2\n"),
            (
                "spec/reference/lexical.md",
                "# Lexical structure\n\nr[lex.braces]\nBraces balance.\n",
            ),
            (
                "spec/reference/statements.md",
                "# Statements\n\nr[stmt.order]\nStatements run in order.\n",
            ),
            ("spec/retired-rules.txt", ""),
            ("spec/moved-rules.txt", ""),
            ("spec/grammar.ebnf", "// terminals: name\n\nfile = name\n"),
        ];
        for (path, text) in files {
            let path = root.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        let rules = load_rules(&root).unwrap();
        let section = |id: &str| rules.ids.get(id).map(String::as_str);
        assert_eq!(section("lex.braces"), Some("Lexical structure"));
        assert_eq!(section("stmt.order"), Some("Statements"));
        assert_eq!(section("grammar.file"), Some(GRAMMAR_SECTION));
        assert_eq!(rules.ids.len(), 3);
        assert_eq!(
            rules.sections,
            ["Lexical structure", "Statements", GRAMMAR_SECTION]
        );
        assert_eq!(rules.spec_version, "1.2");
    }

    #[test]
    fn malformed_fixture_fails() {
        let r = run("fn main() {}\n", None, &NoBackend);
        assert!(r.failures[0].contains("must cite at least one rule"));
    }

    fn selftest(dir: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/selftest/testdata")
            .join(dir)
    }

    #[test]
    fn discovers_nested_fixtures_with_their_output() {
        let files = discover_in(&selftest("ok"), "selftest").unwrap();
        let found: Vec<_> = files
            .iter()
            .map(|f| {
                let siblings = (f.stdout.as_deref(), f.fixed.as_deref());
                (f.name.as_str(), f.path.as_str(), siblings)
            })
            .collect();
        assert_eq!(
            found,
            [
                // Discovery only: these files are never run, so the `.fixed` text is a placeholder.
                (
                    "bindings/shadow_same_scope",
                    "selftest/bindings/shadow_same_scope.qnc",
                    (None, Some("// the fixed text\n"))
                ),
                ("hello", "selftest/hello.qnc", (Some("hi\n"), None)),
            ]
        );
    }

    /// Reports E0001 at every `$`, at the position `LineIndex` gives, as a lexer would, with a
    /// fix that deletes it. The diagnostics come last `$` first, so applying the fixes has to
    /// sort them.
    struct DollarBackend;

    impl Backend for DollarBackend {
        fn parse(&self, path: &str, source: &[u8]) -> Option<Vec<Diagnostic>> {
            let text = fixture::decode(source).ok()?;
            let index = LineIndex::new(&text);
            let dollar = |(offset, _)| {
                let span = Span {
                    file: path.to_string(),
                    start: index.position(offset),
                    end: index.position(offset + 1),
                };
                let code = Code::parse("E0001").unwrap();
                let mut d = Diagnostic::new(code, Severity::Error, "a `$`", span.clone());
                d.fixes.push(Fix {
                    description: "delete the `$`".to_string(),
                    applicability: Applicability::Safe,
                    edits: vec![Edit {
                        file: None,
                        start: span.start,
                        end: span.end,
                        text: String::new(),
                    }],
                });
                d
            };
            Some(text.match_indices('$').map(dollar).rev().collect())
        }

        fn check(&self, path: &str, source: &[u8]) -> Option<Vec<Diagnostic>> {
            self.parse(path, source)
        }

        fn run(&self, _path: &str, _source: &[u8]) -> Option<Result<RunOutput, Vec<Diagnostic>>> {
            None
        }
    }

    /// The `lines/` fixtures hold a byte order mark, each line ending, no final line break and
    /// invalid UTF-8. The runner's line numbers must agree with the positions a backend reports,
    /// and its fixes must apply at the offsets those positions name.
    #[test]
    fn line_numbers_agree_with_positions() {
        let files = discover_in(&selftest("lines"), "selftest").unwrap();
        let names: Vec<_> = files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(
            names,
            ["bom", "crlf", "invalid_utf8", "lone_cr", "no_final_newline"]
        );
        for file in &files {
            let expected = fixture::parse(&file.name, &file.source, None)
                .unwrap()
                .expectations;
            assert!(expected.len() >= 2, "{}: {expected:?}", file.name);
            assert!(file.fixed.is_some(), "{} has no .fixed file", file.name);
            let r = run_file(file, &rules(), &DollarBackend);
            assert!(r.passed(), "{}: {:?}", file.name, r.failures);
        }
    }

    #[test]
    fn orphan_siblings_are_errors() {
        let errors = discover_in(&selftest("orphan"), "selftest").unwrap_err();
        assert_eq!(
            errors,
            [
                "selftest/renamed.fixed has no matching .qnc fixture; rename or remove it",
                "selftest/renamed.stdout has no matching .qnc fixture; rename or remove it",
            ]
        );
    }

    #[test]
    fn missing_fixture_folder_means_no_fixtures() {
        assert!(
            discover_in(&selftest("absent"), "selftest")
                .unwrap()
                .is_empty()
        );
    }
}
