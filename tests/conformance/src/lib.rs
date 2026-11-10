//! Runs the `.qnc` conformance fixtures in `tests/conformance/testdata/` and reports spec
//! coverage by rule ID.
//!
//! The fixture format is in `openspec/changes/m0-repo-harness/specs/conformance-tests/spec.md`.
//! `cargo xtask conformance` drives this crate. The compiler plugs in through [`Backend`].

pub mod coverage;
pub mod fixture;

use fixture::{Fixture, Mode};
use quince_diagnostics::{Diagnostic, Severity};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const FIXTURE_DIR: &str = "tests/conformance/testdata";
pub const SPEC_PATH: &str = "spec/quince-spec.md";
pub const RETIRED_PATH: &str = "spec/retired-rules.txt";
pub const GRAMMAR_PATH: &str = "spec/grammar.ebnf";

/// The section that grammar-production rule IDs are reported under.
pub const GRAMMAR_SECTION: &str = "Grammar productions";

/// What a compiler must provide to run fixtures. A method returns `None` when that mode isn't
/// implemented yet, and the fixture then fails rather than being skipped.
pub trait Backend {
    /// Every diagnostic for one file. `path` is the fixture's path relative to the repo root.
    fn check(&self, path: &str, source: &str) -> Option<Vec<Diagnostic>>;
    /// Builds and runs one file as a program.
    fn run(&self, path: &str, source: &str) -> Option<RunOutput>;
}

pub struct RunOutput {
    pub exit: i32,
    pub stdout: String,
}

/// The backend until M1 wires in the parser: it can't do anything.
pub struct NoBackend;

impl Backend for NoBackend {
    fn check(&self, _path: &str, _source: &str) -> Option<Vec<Diagnostic>> {
        None
    }

    fn run(&self, _path: &str, _source: &str) -> Option<RunOutput> {
        None
    }
}

/// Every rule ID, with the spec section it belongs to.
pub type RuleSet = BTreeMap<String, String>;

/// Loads every rule ID (the spec's tags plus `grammar.<production>` for each production), and
/// the section names in spec order.
pub fn load_rules(root: &Path) -> Result<(RuleSet, Vec<String>), Vec<String>> {
    let read = |path: &str| {
        std::fs::read_to_string(root.join(path))
            .map_err(|e| vec![format!("can't read {path}: {e}")])
    };
    let located = |path: &'static str| {
        move |errors: Vec<quince_spec::SpecError>| {
            errors
                .into_iter()
                .map(|e| format!("{path}:{}: {}", e.line, e.message))
                .collect::<Vec<_>>()
        }
    };
    let retired =
        quince_spec::rules::parse_retired(&read(RETIRED_PATH)?).map_err(located(RETIRED_PATH))?;
    let rules =
        quince_spec::rules::check(&read(SPEC_PATH)?, &retired).map_err(located(SPEC_PATH))?;
    let mut sections: Vec<String> = Vec::new();
    for rule in &rules {
        if !sections.contains(&rule.section) {
            sections.push(rule.section.clone());
        }
    }
    sections.push(GRAMMAR_SECTION.to_string());
    let mut set: RuleSet = rules.into_iter().map(|r| (r.id, r.section)).collect();
    let grammar_file = read(GRAMMAR_PATH)?;
    let (_, grammar) = quince_spec::grammar::split_header(&grammar_file);
    for production in quince_spec::grammar::productions(grammar) {
        set.insert(
            format!("grammar.{}", production.name),
            GRAMMAR_SECTION.to_string(),
        );
    }
    Ok((set, sections))
}

/// One fixture file found on disk, before parsing.
pub struct FixtureFile {
    pub name: String,
    /// Relative to the repo root.
    pub path: String,
    pub source: String,
    pub stdout: Option<String>,
}

/// Every `.qnc` file under `testdata/`, in sorted path order. A missing folder means none.
pub fn discover(root: &Path) -> Result<Vec<FixtureFile>, String> {
    let dir = root.join(FIXTURE_DIR);
    let mut paths = Vec::new();
    if dir.exists() {
        collect_qnc(&dir, &mut paths)?;
    }
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let relative = path
                .strip_prefix(&dir)
                .expect("found below the fixture folder")
                .with_extension("");
            let name = relative
                .components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            let source = std::fs::read_to_string(&path)
                .map_err(|e| format!("can't read {}: {e}", path.display()))?;
            let stdout = std::fs::read_to_string(path.with_extension("stdout")).ok();
            Ok(FixtureFile {
                path: format!("{FIXTURE_DIR}/{name}.qnc"),
                name,
                source,
                stdout,
            })
        })
        .collect()
}

fn collect_qnc(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("can't read {}: {e}", dir.display()))?;
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_dir() {
            collect_qnc(&path, out)?;
        } else if path.extension().is_some_and(|ext| ext == "qnc") {
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
pub fn run_file(file: &FixtureFile, rules: &RuleSet, backend: &dyn Backend) -> FixtureResult {
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
    rules: &RuleSet,
    backend: &dyn Backend,
) -> FixtureResult {
    let mut failures: Vec<String> = fixture
        .rules
        .iter()
        .filter(|id| !rules.contains_key(*id))
        .map(|id| format!("cites unknown rule ID `{id}`"))
        .collect();
    match fixture.mode {
        Mode::Check => match backend.check(path, &fixture.source) {
            Some(diagnostics) => failures.extend(match_diagnostics(fixture, &diagnostics)),
            None => failures.push(
                "no backend: the compiler can't check fixtures yet (nothing is wired into the runner)"
                    .to_string(),
            ),
        },
        Mode::Run => match backend.run(path, &fixture.source) {
            Some(output) => {
                if output.exit != fixture.exit {
                    failures.push(format!(
                        "exited with {}, expected {}",
                        output.exit, fixture.exit
                    ));
                }
                if let Some(expected) = &fixture.stdout
                    && &output.stdout != expected
                {
                    failures.push(format!(
                        "stdout differs from {}.stdout:\n--- expected\n{expected}--- actual\n{}",
                        fixture.name, output.stdout
                    ));
                }
            }
            None => failures.push(
                "no backend: the compiler can't build and run fixtures yet (nothing is wired into the runner)"
                    .to_string(),
            ),
        },
    }
    FixtureResult {
        name: fixture.name.clone(),
        rules: fixture.rules.clone(),
        failures,
    }
}

/// Pairs each expectation with a diagnostic of the same code starting on its line. Unmatched
/// expectations and unexpected errors or warnings are failures.
fn match_diagnostics(fixture: &Fixture, diagnostics: &[Diagnostic]) -> Vec<String> {
    let mut used = vec![false; diagnostics.len()];
    let mut failures = Vec::new();
    for expectation in &fixture.expectations {
        let found = diagnostics.iter().enumerate().position(|(i, d)| {
            !used[i] && d.code == expectation.code && d.span.start.line() == expectation.line
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
            failures.push(format!(
                "unexpected {} {} on line {}: {}",
                severity_word(d.severity),
                d.code,
                d.span.start.line(),
                d.message
            ));
        }
    }
    failures
}

fn severity_word(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quince_diagnostics::{Code, Position, Span};

    /// Returns fixed diagnostics, or fixed run output.
    struct Fake {
        diagnostics: Vec<(&'static str, u32)>,
        run: Option<(i32, &'static str)>,
    }

    impl Backend for Fake {
        fn check(&self, path: &str, _source: &str) -> Option<Vec<Diagnostic>> {
            Some(
                self.diagnostics
                    .iter()
                    .map(|&(code, line)| {
                        let code = Code::parse(code).unwrap();
                        let severity = code.default_severity();
                        let span = Span {
                            file: path.to_string(),
                            start: Position(line, 1),
                            end: Position(line, 2),
                        };
                        Diagnostic::new(code, severity, "from the fake", span)
                    })
                    .collect(),
            )
        }

        fn run(&self, _path: &str, _source: &str) -> Option<RunOutput> {
            self.run.map(|(exit, stdout)| RunOutput {
                exit,
                stdout: stdout.to_string(),
            })
        }
    }

    fn rules() -> RuleSet {
        [
            ("bind.let-var", "2. Syntax"),
            ("main.signature", "5. Effects"),
        ]
        .into_iter()
        .map(|(id, section)| (id.to_string(), section.to_string()))
        .collect()
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
            diagnostics,
            run: None,
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
    fn unknown_rule_fails() {
        let r = run("// rule: bind.nonexistent\nx\n", None, &fake(vec![]));
        assert_eq!(r.failures, ["cites unknown rule ID `bind.nonexistent`"]);
    }

    #[test]
    fn no_backend_fails_loudly() {
        let r = run("// rule: bind.let-var\nx\n", None, &NoBackend);
        assert!(!r.passed());
        assert!(r.failures[0].starts_with("no backend:"), "{:?}", r.failures);
        let r = run(
            "// rule: main.signature\n// mode: run\nx\n",
            None,
            &NoBackend,
        );
        assert!(r.failures[0].starts_with("no backend:"), "{:?}", r.failures);
    }

    #[test]
    fn run_mode_compares_exit_and_stdout() {
        let source = "// rule: main.signature\n// mode: run\nfn main() {}\n";
        let ok = Fake {
            diagnostics: vec![],
            run: Some((0, "hi\n")),
        };
        assert!(run(source, Some("hi\n"), &ok).passed());
        let wrong = Fake {
            diagnostics: vec![],
            run: Some((1, "bye\n")),
        };
        assert_eq!(run(source, Some("hi\n"), &wrong).failures.len(), 2);
    }

    #[test]
    fn malformed_fixture_fails() {
        let r = run("fn main() {}\n", None, &NoBackend);
        assert!(r.failures[0].contains("must cite at least one rule"));
    }
}
