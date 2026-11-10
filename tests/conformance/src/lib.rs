//! Runs the `.qnc` conformance fixtures in `tests/conformance/testdata/` and reports spec
//! coverage by rule ID.
//!
//! The fixture format and the coverage report are in `tests/conformance/testdata/README.md`.
//! `cargo xtask conformance` drives this crate. The compiler plugs in through [`Backend`].

pub mod coverage;
pub mod fixture;

use fixture::{Fixture, Mode};
use quince_diagnostics::{Diagnostic, Severity};
use quince_spec::{GRAMMAR_PATH, RETIRED_PATH, SPEC_PATH, TOPICS_PATH};
use std::collections::{BTreeMap, HashSet};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

pub const FIXTURE_DIR: &str = "tests/conformance/testdata";

/// The section that grammar-production rule IDs are reported under.
pub const GRAMMAR_SECTION: &str = "Grammar productions";

/// What a compiler must provide to run fixtures. A method returns `None` when that mode isn't
/// implemented yet, and the fixture then fails rather than being skipped.
pub trait Backend {
    /// The parser's diagnostics for one file, and nothing from later stages. `path` is the
    /// fixture's path relative to the repo root.
    fn parse(&self, path: &str, source: &str) -> Option<Vec<Diagnostic>>;
    /// Every diagnostic for one file, from every stage that exists.
    fn check(&self, path: &str, source: &str) -> Option<Vec<Diagnostic>>;
    /// Builds and runs one file as a program. `Err` holds the build's error diagnostics.
    fn run(&self, path: &str, source: &str) -> Option<Result<RunOutput, Vec<Diagnostic>>>;
}

pub struct RunOutput {
    pub exit: i32,
    pub stdout: String,
}

/// The backend until M1 wires in the parser: it can't do anything.
pub struct NoBackend;

impl Backend for NoBackend {
    fn parse(&self, _path: &str, _source: &str) -> Option<Vec<Diagnostic>> {
        None
    }

    fn check(&self, _path: &str, _source: &str) -> Option<Vec<Diagnostic>> {
        None
    }

    fn run(&self, _path: &str, _source: &str) -> Option<Result<RunOutput, Vec<Diagnostic>>> {
        None
    }
}

/// Every rule ID, with the spec section it belongs to.
pub type RuleSet = BTreeMap<String, String>;

/// What fixtures are checked against: every live rule ID, the section order, and the retired
/// IDs, so citing one gets its own message.
pub struct Rules {
    pub ids: RuleSet,
    /// Section names in spec order, ending with [`GRAMMAR_SECTION`].
    pub sections: Vec<String>,
    pub retired: HashSet<String>,
    /// The draft version in the spec's header, such as `0.5`.
    pub spec_version: String,
}

/// The version in the spec's `Draft vX.Y` header line.
pub fn spec_version(spec: &str) -> Option<&str> {
    spec.lines()
        .take(10)
        .find_map(|line| line.split("Draft v").nth(1))
        .map(|rest| rest.split_whitespace().next().unwrap_or(rest))
        .filter(|version| !version.is_empty())
}

/// Loads every rule ID: the spec's tags plus `grammar.<production>` for each production.
pub fn load_rules(root: &Path) -> Result<Rules, Vec<String>> {
    let read = |path: &str| {
        std::fs::read_to_string(root.join(path))
            .map_err(|e| vec![format!("can't read {path}: {e}")])
    };
    let located = |path: &str, errors: Vec<quince_spec::SpecError>| {
        errors
            .into_iter()
            .map(|e| format!("{path}:{}: {}", e.line, e.message))
            .collect::<Vec<_>>()
    };
    let (retired, retired_errors) = quince_spec::rules::parse_retired(&read(RETIRED_PATH)?);
    let (topics, topic_errors) = quince_spec::rules::parse_topics(&read(TOPICS_PATH)?);
    let mut errors = located(RETIRED_PATH, retired_errors);
    errors.extend(located(TOPICS_PATH, topic_errors));
    let spec = read(SPEC_PATH)?;
    let version = spec_version(&spec).map(str::to_string);
    if version.is_none() {
        errors.push(format!(
            "{SPEC_PATH}: the header has no `Draft vX.Y` version line"
        ));
    }
    let rules = match quince_spec::rules::check(&spec, &retired, &topics) {
        Ok(rules) => rules,
        Err(spec_errors) => {
            errors.extend(located(SPEC_PATH, spec_errors));
            Vec::new()
        }
    };
    if !errors.is_empty() {
        return Err(errors);
    }
    let mut sections: Vec<String> = Vec::new();
    for rule in &rules {
        if !sections.contains(&rule.section) {
            sections.push(rule.section.clone());
        }
    }
    sections.push(GRAMMAR_SECTION.to_string());
    let mut ids: RuleSet = rules.into_iter().map(|r| (r.id, r.section)).collect();
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
        spec_version: version.expect("checked above"),
    })
}

/// One fixture file found on disk, before parsing.
#[derive(Debug)]
pub struct FixtureFile {
    pub name: String,
    /// Relative to the repo root.
    pub path: String,
    pub source: String,
    pub stdout: Option<String>,
}

/// Every `.qnc` file under `testdata/`, in sorted path order, with its sibling `.stdout`. A
/// missing folder means no fixtures. A `.stdout` file with no `.qnc` file is an error.
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
        if has_ext(path, "stdout") {
            if !fixtures.contains(&path.with_extension("")) {
                errors.push(format!(
                    "{relative_dir}/{}.stdout has no matching .qnc fixture; rename or remove it",
                    name_of(path)
                ));
            }
            continue;
        }
        let name = name_of(path);
        let source = match std::fs::read_to_string(path) {
            Ok(source) => source,
            Err(e) => {
                errors.push(format!("can't read {}: {e}", path.display()));
                continue;
            }
        };
        let stdout = match std::fs::read_to_string(path.with_extension("stdout")) {
            Ok(text) => Some(text),
            Err(e) if e.kind() == ErrorKind::NotFound => None,
            Err(e) => {
                errors.push(format!("can't read {relative_dir}/{name}.stdout: {e}"));
                continue;
            }
        };
        files.push(FixtureFile {
            path: format!("{relative_dir}/{name}.qnc"),
            name,
            source,
            stdout,
        });
    }
    if errors.is_empty() {
        Ok(files)
    } else {
        Err(errors)
    }
}

/// Every `.qnc` and `.stdout` file below `dir`.
fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("can't read {}: {e}", dir.display()))?;
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_dir() {
            collect(&path, out)?;
        } else if path
            .extension()
            .is_some_and(|ext| ext == "qnc" || ext == "stdout")
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
        Ok(fixture) => run_fixture(&fixture, &file.path, rules, backend),
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
            if rules.retired.contains(id) {
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
            "no backend: the compiler can't {what} fixtures yet (nothing is wired into the runner)"
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
                Some(diagnostics) => failures.extend(match_diagnostics(fixture, &diagnostics)),
                None => failures.push(no_backend(what)),
            }
        }
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
/// expectations and unexpected errors or warnings are failures. A diagnostic that starts at the
/// end of input after a trailing newline counts as starting on the last line.
fn match_diagnostics(fixture: &Fixture, diagnostics: &[Diagnostic]) -> Vec<String> {
    let last_line = u32::try_from(fixture.source.lines().count()).expect("lines fit in u32");
    let line_of = |d: &Diagnostic| {
        let line = d.span.start.line();
        if line == last_line + 1 && fixture.source.ends_with('\n') {
            last_line
        } else {
            line
        }
    };
    let mut used = vec![false; diagnostics.len()];
    let mut failures = Vec::new();
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
            if let Err(why) = fix.check(&d.span.file) {
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
    use quince_diagnostics::{Code, Position, Span};

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
        fn parse(&self, path: &str, _source: &str) -> Option<Vec<Diagnostic>> {
            Some(
                self.diagnostics
                    .iter()
                    .map(|&d| diagnostic(path, d))
                    .collect(),
            )
        }

        fn check(&self, path: &str, _source: &str) -> Option<Vec<Diagnostic>> {
            let all = self.diagnostics.iter().chain(&self.check_only);
            Some(all.map(|&d| diagnostic(path, d)).collect())
        }

        fn run(&self, path: &str, _source: &str) -> Option<Result<RunOutput, Vec<Diagnostic>>> {
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
            retired: ["bind.old".to_string()].into(),
            spec_version: "0.5".into(),
        }
    }

    fn run(source: &str, stdout: Option<&str>, backend: &dyn Backend) -> FixtureResult {
        let file = FixtureFile {
            name: "t".into(),
            path: "tests/conformance/testdata/t.qnc".into(),
            source: source.into(),
            stdout: stdout.map(str::to_string),
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

    /// Reports one E0100 on line 2 with the given fix edits.
    struct FixBackend(Vec<((u32, u32), (u32, u32))>);

    impl Backend for FixBackend {
        fn parse(&self, path: &str, source: &str) -> Option<Vec<Diagnostic>> {
            self.check(path, source)
        }

        fn check(&self, path: &str, _source: &str) -> Option<Vec<Diagnostic>> {
            let mut d = diagnostic(path, ("E0100", 2, 1));
            d.fixes.push(quince_diagnostics::Fix {
                description: "rename".into(),
                applicability: quince_diagnostics::Applicability::Safe,
                edits: self
                    .0
                    .iter()
                    .map(|&(start, end)| quince_diagnostics::Edit {
                        file: None,
                        start: Position(start.0, start.1),
                        end: Position(end.0, end.1),
                        text: "y".into(),
                    })
                    .collect(),
            });
            Some(vec![d])
        }

        fn run(&self, _path: &str, _source: &str) -> Option<Result<RunOutput, Vec<Diagnostic>>> {
            None
        }
    }

    #[test]
    fn reported_fixes_are_checked() {
        let source = "// rule: bind.let-var
let x = 1 //~ E0100
";
        let good = FixBackend(vec![((2, 5), (2, 6))]);
        assert!(run(source, None, &good).passed());
        let overlapping = FixBackend(vec![((2, 1), (2, 6)), ((2, 3), (2, 9))]);
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
    fn reads_the_spec_version() {
        assert_eq!(spec_version("# Spec\n\nNov 10 · Draft v0.5\n"), Some("0.5"));
        assert_eq!(spec_version("# Spec\n"), None);
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
            .map(|f| (f.name.as_str(), f.path.as_str(), f.stdout.as_deref()))
            .collect();
        assert_eq!(
            found,
            [
                (
                    "bindings/shadow_same_scope",
                    "selftest/bindings/shadow_same_scope.qnc",
                    None
                ),
                ("hello", "selftest/hello.qnc", Some("hi\n")),
            ]
        );
    }

    #[test]
    fn orphan_stdout_is_an_error() {
        let errors = discover_in(&selftest("orphan"), "selftest").unwrap_err();
        assert_eq!(
            errors,
            ["selftest/renamed.stdout has no matching .qnc fixture; rename or remove it"]
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

    #[test]
    fn loads_the_real_spec() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let rules = load_rules(&root).unwrap();
        assert_eq!(
            rules.ids.get("grammar.fn_decl").map(String::as_str),
            Some(GRAMMAR_SECTION)
        );
        assert_eq!(
            rules.ids.get("bind.shadow.same-scope").map(String::as_str),
            Some("2. Lexical structure and syntax")
        );
        assert_eq!(
            rules.sections.last().map(String::as_str),
            Some(GRAMMAR_SECTION)
        );
        assert_eq!(rules.spec_version, "0.5");
    }
}
