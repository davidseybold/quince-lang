//! Runs the `.qnc` conformance fixtures in `tests/conformance/testdata/` and reports spec
//! coverage by rule ID.
//!
//! The fixture format and the coverage report are in `tests/conformance/testdata/README.md`.
//! `cargo xtask conformance` drives this crate. The compiler plugs in through [`Backend`].

pub mod coverage;
pub mod fixture;

use fixture::{Fixture, Mode};
use quince_diagnostics::{Diagnostic, LineIndex, Severity};
use quince_spec::{GRAMMAR_PATH, MOVED_PATH, README_PATH, RETIRED_PATH, Spec};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

pub const FIXTURE_DIR: &str = "tests/conformance/testdata";

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

/// The backend until M1 wires in the parser: it can't do anything.
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
        let source = match std::fs::read(path) {
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
                Some(diagnostics) => {
                    failures.extend(match_diagnostics(fixture, path, &diagnostics));
                }
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

    /// Reports E0001 at every `$`, at the position `LineIndex` gives, as a lexer would.
    struct DollarBackend;

    impl Backend for DollarBackend {
        fn parse(&self, path: &str, source: &[u8]) -> Option<Vec<Diagnostic>> {
            let text = fixture::decode(source);
            let index = LineIndex::new(&text);
            let dollar = |(offset, _)| {
                let span = Span {
                    file: path.to_string(),
                    start: index.position(offset),
                    end: index.position(offset + 1),
                };
                let code = Code::parse("E0001").unwrap();
                Diagnostic::new(code, Severity::Error, "a `$`", span)
            };
            Some(text.match_indices('$').map(dollar).collect())
        }

        fn check(&self, path: &str, source: &[u8]) -> Option<Vec<Diagnostic>> {
            self.parse(path, source)
        }

        fn run(&self, _path: &str, _source: &[u8]) -> Option<Result<RunOutput, Vec<Diagnostic>>> {
            None
        }
    }

    /// The `lines/` fixtures hold a byte order mark, each line ending, no final line break and
    /// invalid UTF-8. The runner's line numbers must agree with the positions a backend reports.
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
            let r = run_file(file, &rules(), &DollarBackend);
            assert!(r.passed(), "{}: {:?}", file.name, r.failures);
        }
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
            rules
                .ids
                .get("stmt.bind.shadow.same-scope")
                .map(String::as_str),
            Some("Statements")
        );
        assert_eq!(
            rules.sections.last().map(String::as_str),
            Some(GRAMMAR_SECTION)
        );
        assert_eq!(rules.spec_version, "0.6");
    }
}
