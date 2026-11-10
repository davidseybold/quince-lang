//! `cargo xtask diagnostics`: every registered code has explain text and a conformance test,
//! and every explain file belongs to a registered code.

use quince_conformance::FIXTURE_DIR;
use quince_diagnostics::REGISTRY;
use std::collections::BTreeSet;
use std::path::Path;

pub const EXPLAIN_DIR: &str = "compiler/diagnostics/explain";

pub fn check(root: &Path) -> Result<(), String> {
    let explain_files = explain_codes(&root.join(EXPLAIN_DIR))?;
    let expected_codes = fixture_codes(root)?;
    let problems = problems(&explain_files, &expected_codes);
    for problem in &problems {
        eprintln!("{problem}");
    }
    if problems.is_empty() {
        eprintln!(
            "diagnostics: {} registered code(s), each with explain text and a test",
            REGISTRY.len()
        );
        Ok(())
    } else {
        Err(format!("{} diagnostics problem(s)", problems.len()))
    }
}

/// Codes registered, explained and tested, compared. Retired codes need no test, since
/// nothing emits them.
fn problems(explain_files: &BTreeSet<String>, expected_codes: &BTreeSet<String>) -> Vec<String> {
    let rows = REGISTRY.iter().map(|info| Row {
        code: info.code,
        explain: info.explain,
        retired: info.retired,
    });
    problems_for(rows, explain_files, expected_codes)
}

struct Row {
    code: &'static str,
    explain: &'static str,
    retired: bool,
}

fn problems_for(
    rows: impl Iterator<Item = Row>,
    explain_files: &BTreeSet<String>,
    expected_codes: &BTreeSet<String>,
) -> Vec<String> {
    let mut problems = Vec::new();
    let mut registered = BTreeSet::new();
    for row in rows {
        registered.insert(row.code.to_string());
        if !explain_files.contains(row.code) || row.explain.trim().is_empty() {
            problems.push(format!(
                "{}: no explain text; write {EXPLAIN_DIR}/{}.md",
                row.code, row.code
            ));
        }
        if !row.retired && !expected_codes.contains(row.code) {
            problems.push(format!(
                "{}: no conformance fixture expects it; add one under {FIXTURE_DIR} with `//~ {}`",
                row.code, row.code
            ));
        }
    }
    for code in explain_files.difference(&registered) {
        problems.push(format!(
            "{EXPLAIN_DIR}/{code}.md: no registry row in compiler/diagnostics/src/registry.rs"
        ));
    }
    problems
}

/// File stems of `<dir>/*.md`, except the README.
fn explain_codes(dir: &Path) -> Result<BTreeSet<String>, String> {
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("can't read {}: {e}", dir.display()))?;
    let mut codes = BTreeSet::new();
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().is_some_and(|ext| ext == "md")
            && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
            && stem != "README"
        {
            codes.insert(stem.to_string());
        }
    }
    Ok(codes)
}

/// Every code named by a `//~` expectation in a fixture, read with the runner's own parser.
fn fixture_codes(root: &Path) -> Result<BTreeSet<String>, String> {
    let mut codes = BTreeSet::new();
    let files = quince_conformance::discover(root).map_err(|errors| errors.join("\n"))?;
    for file in files {
        let (expectations, _) = quince_conformance::fixture::expectations(&file.source);
        codes.extend(expectations.into_iter().map(|e| e.code.to_string()));
    }
    Ok(codes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    fn row(code: &'static str, explain: &'static str, retired: bool) -> Row {
        Row {
            code,
            explain,
            retired,
        }
    }

    #[test]
    fn complete_code_passes() {
        let rows = [row("E0412", "text", false)].into_iter();
        assert_eq!(
            problems_for(rows, &set(&["E0412"]), &set(&["E0412"])),
            Vec::<String>::new()
        );
    }

    #[test]
    fn code_without_a_test() {
        let rows = [row("E0412", "text", false)].into_iter();
        let found = problems_for(rows, &set(&["E0412"]), &set(&[]));
        assert_eq!(found.len(), 1);
        assert!(found[0].starts_with("E0412: no conformance fixture expects it"));
    }

    #[test]
    fn retired_code_needs_no_test() {
        let rows = [row("E0007", "text", true)].into_iter();
        assert!(problems_for(rows, &set(&["E0007"]), &set(&[])).is_empty());
    }

    #[test]
    fn code_without_explain_text() {
        let rows = [row("E0412", "  \n", false)].into_iter();
        let found = problems_for(rows, &set(&["E0412"]), &set(&["E0412"]));
        assert_eq!(
            found,
            ["E0412: no explain text; write compiler/diagnostics/explain/E0412.md"]
        );
    }

    #[test]
    fn orphan_explain_file() {
        let found = problems_for([].into_iter(), &set(&["E0999"]), &set(&[]));
        assert_eq!(
            found,
            [
                "compiler/diagnostics/explain/E0999.md: no registry row in compiler/diagnostics/src/registry.rs"
            ]
        );
    }
}
