//! `cargo xtask spec-compare`: proves that the spec restructure changes no meaning
//! (docs/design/spec-restructure.md, decision 8). It splits the pinned baseline spec and the
//! current spec into units and checks that they match: every tagged rule keeps its text, and
//! the untagged units are the same multiset. This check and the baseline are deleted when the
//! restructure is done.

pub mod segment;

use crate::spec_rules::{load, read};
use quince_spec::manifest::LEGACY_PREFIX;
use quince_spec::{README_PATH, grammar};
use segment::{Kind, Unit, segment};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// The spec as it was before the restructure (81aa5f0), relative to the repo root.
pub const BASELINE_SPEC: &str = "docs/design/spec-restructure/baseline/quince-spec.md";

/// The headings in the baseline whose code block holds the grammar.
const BASELINE_GRAMMAR_HEADINGS: [&str; 2] = [
    "## Appendix: Expression grammar",
    "### Declarations, types and patterns",
];

/// The units of one file on the "after" side.
#[derive(Debug)]
pub struct Source {
    /// Relative to the repo root.
    pub path: String,
    pub units: Vec<Unit>,
    pub role: Role,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// A chapter file, which must hold every surviving unit and nothing new.
    Chapter,
    /// The chapter file being split.
    Legacy,
    /// A file that may take units from the baseline, and whose other text isn't compared,
    /// such as `spec/README.md`, which took the version header.
    Landing,
}

/// The result of a comparison.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    pub problems: Vec<String>,
    /// Baseline units found again.
    pub matched: usize,
    /// Matched units that now sit outside the legacy file.
    pub moved: usize,
    /// Units allowed without a baseline match: chapter titles and repeated table headers.
    pub added: usize,
}

pub fn check(root: &Path) -> Result<(), String> {
    let baseline = read(root, BASELINE_SPEC)?;
    let spec = load(root)?;
    let mut sources: Vec<Source> = spec
        .files
        .iter()
        .map(|file| Source {
            path: file.path.clone(),
            units: segment(&file.text),
            role: if file.prefix == LEGACY_PREFIX {
                Role::Legacy
            } else {
                Role::Chapter
            },
        })
        .collect();
    sources.push(Source {
        path: README_PATH.to_string(),
        units: segment(&spec.readme),
        role: Role::Landing,
    });
    let before = segment(&baseline);
    let mut outcome = compare(&before, &sources);

    let tags_before = before.iter().filter(|u| u.id.is_some()).count();
    let tags_after = sources
        .iter()
        .filter(|s| s.role != Role::Landing)
        .flat_map(|s| &s.units)
        .filter(|u| u.id.is_some())
        .count();
    if tags_before != tags_after {
        outcome.problems.push(format!(
            "the baseline has {tags_before} rule tags and the spec has {tags_after}"
        ));
    }
    let productions_before = baseline_productions(&baseline)?;
    let productions_after = grammar::extract(&spec.files)
        .map(|extracted| grammar::productions(&extracted.text).len())
        .map_err(|e| e.to_string())?;
    if productions_before != productions_after {
        outcome.problems.push(format!(
            "the baseline has {productions_before} grammar productions and the spec has {productions_after}"
        ));
    }

    for problem in &outcome.problems {
        eprintln!("{problem}");
    }
    eprintln!(
        "spec-compare against {BASELINE_SPEC}: rule tags {tags_before} before, {tags_after} after; \
         productions {productions_before} before, {productions_after} after; units {} matched, \
         {} moved, {} added",
        outcome.matched, outcome.moved, outcome.added
    );
    if outcome.problems.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{} difference(s) from the baseline spec",
            outcome.problems.len()
        ))
    }
}

/// Matches the baseline's units against the current files'.
pub fn compare(before: &[Unit], after: &[Source]) -> Outcome {
    let mut outcome = Outcome::default();
    let problems = &mut outcome.problems;

    let before_tagged: HashMap<&str, &Unit> = before
        .iter()
        .filter_map(|u| u.id.as_deref().map(|id| (id, u)))
        .collect();
    let mut seen: HashMap<&str, (&str, usize)> = HashMap::new();
    for source in after {
        for unit in &source.units {
            let Some(id) = unit.id.as_deref() else {
                continue;
            };
            let at = format!("{}:{}", source.path, unit.line);
            if source.role == Role::Landing {
                problems.push(format!(
                    "{at}: rule `{id}` is tagged in {}, which isn't a chapter; tags belong in chapter files",
                    source.path
                ));
                continue;
            }
            if let Some((path, line)) = seen.insert(id, (&source.path, unit.line)) {
                problems.push(format!(
                    "{at}: rule `{id}` appears twice; it is also at {path}:{line}"
                ));
                continue;
            }
            let Some(old) = before_tagged.get(id) else {
                problems.push(format!(
                    "{at}: rule `{id}` isn't in the baseline; the restructure adds no rules"
                ));
                continue;
            };
            if old.kind != unit.kind {
                problems.push(format!(
                    "{at}: rule `{id}` was in a {} (baseline line {}) and is now in a {}",
                    old.kind.name(),
                    old.line,
                    unit.kind.name()
                ));
            } else if old.text != unit.text {
                problems.push(format!(
                    "{at}: the text of rule `{id}` changed (baseline line {}):\n  before: {}\n  after:  {}",
                    old.line, old.text, unit.text
                ));
            } else {
                outcome.matched += 1;
                if source.role == Role::Chapter {
                    outcome.moved += 1;
                }
            }
        }
    }
    for unit in before {
        if let Some(id) = unit.id.as_deref()
            && !seen.contains_key(id)
        {
            problems.push(format!(
                "{BASELINE_SPEC}:{}: rule `{id}` is missing from the spec",
                unit.line
            ));
        }
    }

    let mut pool: HashMap<(Kind, &str), Vec<usize>> = HashMap::new();
    for unit in before.iter().rev().filter(|u| u.id.is_none()) {
        pool.entry((unit.kind, &unit.text))
            .or_default()
            .push(unit.line);
    }
    let headers: HashSet<&str> = before
        .iter()
        .filter(|u| u.kind == Kind::TableHeader)
        .map(|u| u.text.as_str())
        .collect();
    // Chapter files first, so a landing file only takes what no chapter holds.
    let mut ordered: Vec<&Source> = after.iter().filter(|s| s.role != Role::Landing).collect();
    ordered.extend(after.iter().filter(|s| s.role == Role::Landing));
    for source in ordered {
        for (index, unit) in source.units.iter().enumerate() {
            if unit.id.is_some() {
                continue;
            }
            if let Some(lines) = pool.get_mut(&(unit.kind, unit.text.as_str()))
                && lines.pop().is_some()
            {
                outcome.matched += 1;
                if source.role != Role::Legacy {
                    outcome.moved += 1;
                }
                continue;
            }
            match source.role {
                Role::Landing => {}
                _ if unit.kind == Kind::TableHeader && headers.contains(unit.text.as_str()) => {
                    outcome.added += 1;
                }
                Role::Chapter
                    if index == 0 && unit.kind == Kind::Heading && unit.text.starts_with("# ") =>
                {
                    outcome.added += 1;
                }
                _ => problems.push(format!(
                    "{}:{}: this {} isn't in the baseline: {}",
                    source.path,
                    unit.line,
                    unit.kind.name(),
                    snippet(&unit.text)
                )),
            }
        }
    }
    let mut missing: Vec<(usize, Kind, &str)> = pool
        .into_iter()
        .flat_map(|((kind, text), lines)| lines.into_iter().map(move |line| (line, kind, text)))
        .collect();
    missing.sort();
    for (line, kind, text) in missing {
        problems.push(format!(
            "{BASELINE_SPEC}:{line}: this {} is missing from the spec: {}",
            kind.name(),
            snippet(text)
        ));
    }
    outcome
}

/// The first line of `text`, cut to 100 characters.
fn snippet(text: &str) -> String {
    let first = text.lines().next().unwrap_or_default();
    match first.char_indices().nth(100) {
        Some((cut, _)) => format!("{}…", &first[..cut]),
        None if text.contains('\n') => format!("{first} …"),
        None => first.to_string(),
    }
}

/// The number of productions in the baseline's two grammar code blocks.
fn baseline_productions(baseline: &str) -> Result<usize, String> {
    let lines: Vec<&str> = baseline.lines().collect();
    let mut count = 0;
    for heading in BASELINE_GRAMMAR_HEADINGS {
        let start = lines
            .iter()
            .position(|line| line.trim_end() == heading)
            .ok_or_else(|| format!("{BASELINE_SPEC}: heading `{heading}` not found"))?;
        let fence = |i: &usize| lines[*i].trim_start().starts_with("```");
        let open = (start + 1..lines.len())
            .find(fence)
            .ok_or_else(|| format!("{BASELINE_SPEC}: no code block after `{heading}`"))?;
        let close = (open + 1..lines.len()).find(fence).unwrap_or(lines.len());
        count += grammar::productions(&lines[open + 1..close].join("\n")).len();
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASELINE: &str = "\
# Spec

Nov 10 · Draft v0.5

## 1. Values

Intro. r[val.a] Values are immutable. r[val.b] Copies are cheap.

| Kind | Rule |
| --- | --- |
| Int | r[val.int] Ints are 64-bit. |
| Bool | r[val.bool] Bools are one byte. |

- r[val.list] Lists are values.
* Maps are too.

```
fn f() {
  1
}

fn g() {
  2
}
```

## Appendix: Expression grammar

```
expr = term
term = name
```

### Declarations, types and patterns

```
file = { expr }
```
";

    fn source(path: &str, role: Role, text: &str) -> Source {
        Source {
            path: path.into(),
            units: segment(text),
            role,
        }
    }

    fn compare_with(after: &[Source]) -> Outcome {
        compare(&segment(BASELINE), after)
    }

    fn legacy(text: &str) -> Source {
        source("spec/quince-spec.md", Role::Legacy, text)
    }

    #[test]
    fn unchanged_spec_passes() {
        let outcome = compare_with(&[legacy(BASELINE)]);
        assert_eq!(outcome.problems, Vec::<String>::new());
        assert_eq!(outcome.moved, 0);
        assert_eq!(outcome.added, 0);
    }

    #[test]
    fn moves_and_splits_pass() {
        // The version header moves to the README; the table, the code block and the list
        // split across a new chapter; the grammar block gains its info string.
        let rest = BASELINE
            .replace("Nov 10 · Draft v0.5\n", "")
            .replace("| Bool | r[val.bool] Bools are one byte. |\n", "")
            .replace("fn g() {\n  2\n}\n", "")
            .replace("* Maps are too.\n", "")
            .replace("```\nexpr", "```ebnf\nexpr");
        let chapter = "# Values\n\n| Kind | Rule |\n| --- | --- |\n| Bool | r[val.bool] Bools are one byte. |\n\n- Maps are too.\n\n```\nfn g() {\n  2\n}\n```\n";
        let outcome = compare_with(&[
            legacy(&rest),
            source("spec/reference/values.md", Role::Chapter, chapter),
            source(
                "spec/README.md",
                Role::Landing,
                "# Spec\n\nNov 10 · Draft v0.5\n\nNew index text.\n",
            ),
        ]);
        assert_eq!(outcome.problems, Vec::<String>::new());
        // The rule, the rest of its table row, the list item, the code unit and the version
        // header moved.
        assert_eq!(outcome.moved, 5);
        // The chapter title and the repeated table header.
        assert_eq!(outcome.added, 2);
    }

    #[test]
    fn lost_sentence_fails() {
        let outcome = compare_with(&[legacy(&BASELINE.replace("Intro. ", ""))]);
        assert_eq!(
            outcome.problems,
            [
                "docs/design/spec-restructure/baseline/quince-spec.md:7: this paragraph is missing from the spec: Intro."
            ]
        );
    }

    #[test]
    fn edited_rule_fails() {
        let outcome = compare_with(&[legacy(&BASELINE.replace("are cheap", "are free"))]);
        assert_eq!(
            outcome.problems,
            [
                "spec/quince-spec.md:7: the text of rule `val.b` changed (baseline line 7):\n  before: Copies are cheap.\n  after:  Copies are free."
            ]
        );
    }

    #[test]
    fn edited_code_fails() {
        let outcome = compare_with(&[legacy(&BASELINE.replace("  2\n", "  3\n"))]);
        assert_eq!(outcome.problems.len(), 2, "{:?}", outcome.problems);
        assert!(outcome.problems[0].contains("this code isn't in the baseline: fn g() { …"));
        assert!(outcome.problems[1].contains(":22: this code is missing from the spec: fn g() {"));
    }

    #[test]
    fn duplicated_text_fails() {
        let doubled = format!("{BASELINE}\n* Maps are too.\n");
        let outcome = compare_with(&[legacy(&doubled)]);
        assert_eq!(outcome.problems.len(), 1, "{:?}", outcome.problems);
        assert!(
            outcome.problems[0].contains("this list item isn't in the baseline: Maps are too.")
        );
        let rule = format!("{BASELINE}\nr[val.a] Values are immutable.\n");
        let outcome = compare_with(&[legacy(&rule)]);
        assert!(
            outcome.problems[0]
                .contains("rule `val.a` appears twice; it is also at spec/quince-spec.md:7")
        );
    }

    #[test]
    fn swapped_rule_texts_fail() {
        let swapped = BASELINE.replace(
            "r[val.a] Values are immutable. r[val.b] Copies are cheap.",
            "r[val.a] Copies are cheap. r[val.b] Values are immutable.",
        );
        let outcome = compare_with(&[legacy(&swapped)]);
        assert_eq!(outcome.problems.len(), 2, "{:?}", outcome.problems);
        assert!(outcome.problems.iter().all(|p| p.contains("changed")));
    }

    #[test]
    fn dropped_or_new_tags_fail() {
        let untagged = BASELINE.replace("r[val.list] Lists", "Lists");
        let outcome = compare_with(&[legacy(&untagged)]);
        assert_eq!(outcome.problems.len(), 2, "{:?}", outcome.problems);
        assert!(outcome.problems[0].contains("rule `val.list` is missing from the spec"));
        assert!(
            outcome.problems[1].contains("this list item isn't in the baseline: Lists are values.")
        );

        let new_rule = format!("{BASELINE}\nr[val.new] A new rule.\n");
        let outcome = compare_with(&[legacy(&new_rule)]);
        assert!(outcome.problems[0].contains("rule `val.new` isn't in the baseline"));
    }

    #[test]
    fn rule_changing_block_kind_fails() {
        let moved = BASELINE.replace(
            "- r[val.list] Lists are values.\n",
            "\nr[val.list] Lists are values.\n",
        );
        let outcome = compare_with(&[legacy(&moved)]);
        assert_eq!(
            outcome.problems,
            [
                "spec/quince-spec.md:15: rule `val.list` was in a list item (baseline line 14) and is now in a paragraph"
            ]
        );
    }

    #[test]
    fn landing_files_take_only_baseline_text_and_no_tags() {
        let readme = source(
            "spec/README.md",
            Role::Landing,
            "Nov 10 · Draft v0.5\n\nr[val.a] Values are immutable.\n",
        );
        let outcome = compare_with(&[legacy(BASELINE), readme]);
        assert_eq!(
            outcome.problems,
            [
                "spec/README.md:3: rule `val.a` is tagged in spec/README.md, which isn't a chapter; tags belong in chapter files"
            ]
        );
    }

    #[test]
    fn new_headings_and_table_headers_fail() {
        let changed = BASELINE
            .replace("## 1. Values", "## Values")
            .replace("| Kind | Rule |", "| Kind | Rules |");
        let outcome = compare_with(&[legacy(&changed)]);
        assert_eq!(outcome.problems.len(), 4, "{:?}", outcome.problems);
    }

    #[test]
    fn counts_baseline_productions() {
        assert_eq!(baseline_productions(BASELINE), Ok(3));
        assert!(baseline_productions("# Spec\n").is_err());
    }
}
