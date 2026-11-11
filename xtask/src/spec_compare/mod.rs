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
/// Old rule IDs and what became of them, relative to the repo root.
pub const ID_MAP: &str = "docs/design/spec-restructure/id-map.tsv";
/// Baseline text that left the spec, relative to the repo root.
pub const REMOVED: &str = "docs/design/spec-restructure/removed.tsv";
/// Where the spec's library material went, relative to the repo root.
pub const STD_API: &str = "std/API.md";

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

/// What the restructure's tables say about the baseline.
#[derive(Debug, Default)]
pub struct Tables {
    /// Old ID to new ID, from `id-map.tsv`.
    pub renamed: HashMap<String, String>,
    /// Old IDs whose rule moved to `std/API.md` without its tag, from `id-map.tsv`.
    pub removed_ids: HashSet<String>,
    /// Baseline line ranges whose units moved to `std/API.md`, from `removed.tsv`.
    pub to_std: Vec<(usize, usize)>,
    /// Text removed outright from the baseline unit that starts on a line, from `removed.tsv`.
    pub removed_text: Vec<(usize, String)>,
}

impl Tables {
    /// Parses `id-map.tsv` (old ID, new ID or `removed`, then `mechanical` or `hand` for a
    /// rename) and `removed.tsv` (`to-std` and a baseline line range, or `removed`, a baseline
    /// line and the text removed from the unit that starts there). Columns are tab-separated;
    /// blank lines and `#` comments are ignored.
    pub fn parse(id_map: &str, removed: &str) -> (Tables, Vec<String>) {
        let mut tables = Tables::default();
        let mut errors = Vec::new();
        for (line, columns) in rows(id_map) {
            let mut error = |message: String| errors.push(format!("{ID_MAP}:{line}: {message}"));
            match columns.as_slice() {
                [old, ..] if !quince_spec::rules::is_valid_id(old) => {
                    error(format!("`{old}` is not a valid rule ID"))
                }
                [old, ..]
                    if tables.removed_ids.contains(*old) || tables.renamed.contains_key(*old) =>
                {
                    error(format!("`{old}` is listed twice"))
                }
                [old, "removed"] | [old, "removed", ""] => {
                    tables.removed_ids.insert(old.to_string());
                }
                [old, new, "mechanical" | "hand"] if quince_spec::rules::is_valid_id(new) => {
                    tables.renamed.insert(old.to_string(), new.to_string());
                }
                _ => error(
                    "a row is `old<TAB>removed`, or `old<TAB>new<TAB>mechanical` or `hand`"
                        .to_string(),
                ),
            }
        }
        for (line, columns) in rows(removed) {
            let mut error = |message: String| errors.push(format!("{REMOVED}:{line}: {message}"));
            match columns.as_slice() {
                ["to-std", lines, ..] => match line_range(lines) {
                    Some(range) => tables.to_std.push(range),
                    None => error(format!(
                        "`{lines}` is not a line or a range such as `12-40`"
                    )),
                },
                ["removed", at, text] if !text.is_empty() => match at.parse() {
                    Ok(at) => tables.removed_text.push((at, text.to_string())),
                    Err(_) => error(format!("`{at}` is not a line number")),
                },
                _ => error(
                    "a row is `to-std<TAB>lines<TAB>note`, or `removed<TAB>line<TAB>text`"
                        .to_string(),
                ),
            }
        }
        (tables, errors)
    }

    fn in_to_std(&self, line: usize) -> bool {
        self.to_std
            .iter()
            .any(|&(from, to)| (from..=to).contains(&line))
    }
}

/// The non-blank, non-comment lines of a table, with their 1-based numbers, split on tabs.
fn rows(text: &str) -> impl Iterator<Item = (usize, Vec<&str>)> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty() && !line.starts_with('#'))
        .map(|(index, line)| (index + 1, line.split('\t').collect()))
}

fn line_range(text: &str) -> Option<(usize, usize)> {
    let (from, to) = text.split_once('-').unwrap_or((text, text));
    let (from, to) = (from.parse().ok()?, to.parse().ok()?);
    (from <= to).then_some((from, to))
}

/// The result of a comparison.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    pub problems: Vec<String>,
    /// Baseline units found again.
    pub matched: usize,
    /// Matched units that now sit outside the legacy file.
    pub moved: usize,
    /// Baseline units that left the spec for `std/API.md`.
    pub to_std: usize,
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
    let optional = |path: &str| -> Result<String, String> {
        match std::fs::read_to_string(root.join(path)) {
            Ok(text) => Ok(text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
            Err(e) => Err(format!("can't read {path}: {e}")),
        }
    };
    let (tables, table_errors) = Tables::parse(&optional(ID_MAP)?, &optional(REMOVED)?);
    let std_text = optional(STD_API)?;
    let before = segment(&baseline);
    let mut outcome = compare(&before, &sources, &tables, &std_text);
    outcome.problems.splice(0..0, table_errors);

    let tags_before = before.iter().filter(|u| u.id.is_some()).count();
    let tags_expected = tags_before - tables.removed_ids.len();
    let tags_after = sources
        .iter()
        .filter(|s| s.role != Role::Landing)
        .flat_map(|s| &s.units)
        .filter(|u| u.id.is_some())
        .count();
    if tags_expected != tags_after {
        outcome.problems.push(format!(
            "the baseline has {tags_before} rule tags and {ID_MAP} removes {}, so the spec should have \
             {tags_expected}, but it has {tags_after}",
            tables.removed_ids.len()
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
        "spec-compare against {BASELINE_SPEC}: rule tags {tags_before} before, {} removed, \
         {tags_after} after; productions {productions_before} before, {productions_after} after; \
         units {} matched, {} moved, {} to std/API.md, {} added",
        tables.removed_ids.len(),
        outcome.matched,
        outcome.moved,
        outcome.to_std,
        outcome.added
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

/// Applies the tables to the baseline's units: removes the text `removed.tsv` lists, takes out
/// the units that moved to `std/API.md` (checking that `std_text` holds each one), and maps
/// renamed IDs to their new names.
fn normalize(before: &[Unit], tables: &Tables, std_text: &str, outcome: &mut Outcome) -> Vec<Unit> {
    let problems = &mut outcome.problems;
    let mut units = before.to_vec();
    for (line, text) in &tables.removed_text {
        match units
            .iter_mut()
            .find(|u| u.line == *line && u.text.contains(text.as_str()))
        {
            Some(unit) => unit.text = remove_text(&unit.text, text),
            None => problems.push(format!(
                "{REMOVED}: no baseline unit on line {line} holds the text `{}`",
                snippet(text)
            )),
        }
    }
    let baseline_ids: HashSet<&str> = before.iter().filter_map(|u| u.id.as_deref()).collect();
    let mut listed: Vec<&String> = tables
        .removed_ids
        .iter()
        .chain(tables.renamed.keys())
        .filter(|id| !baseline_ids.contains(id.as_str()))
        .collect();
    listed.sort();
    for id in listed {
        problems.push(format!("{ID_MAP}: `{id}` is not a rule ID in the baseline"));
    }
    for unit in segment(std_text) {
        if let Some(id) = &unit.id {
            problems.push(format!(
                "{STD_API}:{}: rule tag `{id}`; text that leaves the spec loses its tag",
                unit.line
            ));
        }
    }

    let mut kept = Vec::new();
    for unit in units {
        let to_std = match &unit.id {
            Some(id) if tables.removed_ids.contains(id) => true,
            Some(id) if tables.in_to_std(unit.line) => {
                problems.push(format!(
                    "{BASELINE_SPEC}:{}: rule `{id}` is in a to-std range of {REMOVED}, but {ID_MAP} doesn't remove it",
                    unit.line
                ));
                false
            }
            None => tables.in_to_std(unit.line),
            Some(_) => false,
        };
        if !to_std {
            if !unit.text.is_empty() {
                kept.push(unit);
            }
            continue;
        }
        // A heading of a section that left whole goes with it, in whatever form std/API.md
        // gives it.
        if unit.kind == Kind::Heading {
            continue;
        }
        outcome.to_std += 1;
        if !std_text.contains(&unit.text) {
            let what = match &unit.id {
                Some(id) => format!("rule `{id}`"),
                None => format!("this {}", unit.kind.name()),
            };
            problems.push(format!(
                "{BASELINE_SPEC}:{}: {what} left the spec for {STD_API}, but {STD_API} doesn't hold its text: {}",
                unit.line,
                snippet(&unit.text)
            ));
        }
    }
    for unit in &mut kept {
        if let Some(new) = unit.id.as_ref().and_then(|id| tables.renamed.get(id)) {
            unit.id = Some(new.clone());
        }
    }
    kept
}

/// `unit` without `text`, keeping one space where two would meet.
fn remove_text(unit: &str, text: &str) -> String {
    let at = unit.find(text).expect("checked by the caller");
    let (head, tail) = (&unit[..at], &unit[at + text.len()..]);
    let tail = if head.ends_with(' ') {
        tail.strip_prefix(' ').unwrap_or(tail)
    } else {
        tail
    };
    format!("{head}{tail}").trim().to_string()
}

/// Matches the baseline's units against the current files'.
pub fn compare(before: &[Unit], after: &[Source], tables: &Tables, std_text: &str) -> Outcome {
    let mut outcome = Outcome::default();
    let before = normalize(before, tables, std_text, &mut outcome);
    let before = before.as_slice();
    let renamed_from: HashMap<&str, &str> = tables
        .renamed
        .iter()
        .map(|(old, new)| (old.as_str(), new.as_str()))
        .collect();
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
                let why = if tables.removed_ids.contains(id) {
                    format!("{ID_MAP} removes it, so its text belongs in {STD_API} without the tag")
                } else if let Some(new) = renamed_from.get(id) {
                    format!("{ID_MAP} renames it to `{new}`")
                } else {
                    "it isn't in the baseline, and the restructure adds no rules".to_string()
                };
                problems.push(format!("{at}: rule `{id}` is tagged, but {why}"));
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
        compare(&segment(BASELINE), after, &Tables::default(), "")
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
        assert!(
            outcome.problems[0].contains("rule `val.new` is tagged, but it isn't in the baseline")
        );
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

    fn tables(id_map: &str, removed: &str) -> Tables {
        let (tables, errors) = Tables::parse(id_map, removed);
        assert_eq!(errors, Vec::<String>::new());
        tables
    }

    #[test]
    fn table_rows() {
        let (tables, errors) = Tables::parse(
            "# old\tnew\thow\nval.a\tremoved\nval.b\tvalue.b\tmechanical\nval.c\tremoved\t\nBad\tremoved\nval.a\tremoved\nval.d\tvalue.d\nval.e\tvalue.e\tsomehow\n",
            "to-std\t15-16\tlists\nto-std\t22\tone line\nremoved\t7\tIntro.\nto-std\t9-3\tbackwards\nremoved\tx\ttext\nremoved\t7\n",
        );
        assert_eq!(tables.removed_ids.len(), 2);
        assert_eq!(
            tables.renamed.get("val.b").map(String::as_str),
            Some("value.b")
        );
        assert_eq!(tables.to_std, [(15, 16), (22, 22)]);
        assert_eq!(tables.removed_text, [(7, "Intro.".to_string())]);
        let lines: Vec<_> = errors
            .iter()
            .map(|e| e.split(": ").next().unwrap_or_default())
            .collect();
        assert_eq!(
            lines,
            [
                "docs/design/spec-restructure/id-map.tsv:5",
                "docs/design/spec-restructure/id-map.tsv:6",
                "docs/design/spec-restructure/id-map.tsv:7",
                "docs/design/spec-restructure/id-map.tsv:8",
                "docs/design/spec-restructure/removed.tsv:4",
                "docs/design/spec-restructure/removed.tsv:5",
                "docs/design/spec-restructure/removed.tsv:6",
            ],
            "{errors:?}"
        );
    }

    /// The baseline with the list and the first code declaration moved to std/API.md, the
    /// `val.list` rule removed, and the intro sentence dropped.
    fn library_move() -> (String, Tables, String) {
        let spec = BASELINE
            .replace("Intro. ", "")
            .replace("- r[val.list] Lists are values.\n* Maps are too.\n", "")
            .replace("fn f() {\n  1\n}\n\n", "");
        let tables = tables(
            "val.list\tremoved\n",
            "to-std\t14-15\tthe list\nto-std\t18-20\tfn f\nremoved\t7\tIntro.\n",
        );
        let std = "# API\n\n- Lists are values.\n- Maps are too.\n\n```\nfn f() {\n  1\n}\n```\n"
            .to_string();
        (spec, tables, std)
    }

    #[test]
    fn library_moves_pass() {
        let (spec, tables, std) = library_move();
        let outcome = compare(&segment(BASELINE), &[legacy(&spec)], &tables, &std);
        assert_eq!(outcome.problems, Vec::<String>::new());
        assert_eq!(outcome.to_std, 3);
    }

    #[test]
    fn library_text_must_land_in_std() {
        let (spec, tables, std) = library_move();
        let lost = std
            .replace("- Maps are too.\n", "")
            .replace("  1\n", "  2\n");
        let outcome = compare(&segment(BASELINE), &[legacy(&spec)], &tables, &lost);
        assert_eq!(outcome.problems.len(), 2, "{:?}", outcome.problems);
        assert!(outcome.problems[0].contains(":15: this list item left the spec for std/API.md, but std/API.md doesn't hold its text: Maps are too."));
        assert!(outcome.problems[1].contains(":18: this code left the spec"));

        let tagged = std.replace("- Lists", "- r[val.list] Lists");
        let outcome = compare(&segment(BASELINE), &[legacy(&spec)], &tables, &tagged);
        assert_eq!(
            outcome.problems,
            ["std/API.md:3: rule tag `val.list`; text that leaves the spec loses its tag"]
        );
    }

    #[test]
    fn removed_rules_lose_their_tags_and_ranges_need_them_removed() {
        let (_, tables, std) = library_move();
        let still_tagged = BASELINE.replace("Intro. ", "");
        let outcome = compare(&segment(BASELINE), &[legacy(&still_tagged)], &tables, &std);
        assert!(
            outcome.problems.iter().any(|p| p.contains(
                "rule `val.list` is tagged, but docs/design/spec-restructure/id-map.tsv removes it"
            )),
            "{:?}",
            outcome.problems
        );

        let unremoved = Tables {
            to_std: vec![(14, 14)],
            ..Tables::default()
        };
        let outcome = compare(&segment(BASELINE), &[legacy(BASELINE)], &unremoved, "");
        assert!(
            outcome.problems[0].contains(":14: rule `val.list` is in a to-std range"),
            "{:?}",
            outcome.problems
        );
    }

    #[test]
    fn stale_table_rows_fail() {
        let stale = tables("val.gone\tremoved\n", "removed\t7\tNot there.\n");
        let outcome = compare(&segment(BASELINE), &[legacy(BASELINE)], &stale, "");
        assert_eq!(
            outcome.problems,
            [
                "docs/design/spec-restructure/removed.tsv: no baseline unit on line 7 holds the text `Not there.`",
                "docs/design/spec-restructure/id-map.tsv: `val.gone` is not a rule ID in the baseline",
            ]
        );
    }

    #[test]
    fn renamed_ids_map_to_their_new_names() {
        let renamed = tables("val.a\tvalue.a\tmechanical\n", "");
        let spec = BASELINE.replace("r[val.a]", "r[value.a]");
        let outcome = compare(&segment(BASELINE), &[legacy(&spec)], &renamed, "");
        assert_eq!(outcome.problems, Vec::<String>::new());
        let outcome = compare(&segment(BASELINE), &[legacy(BASELINE)], &renamed, "");
        assert!(outcome.problems[0].contains("rule `val.a` is tagged, but docs/design/spec-restructure/id-map.tsv renames it to `value.a`"), "{:?}", outcome.problems);
    }

    #[test]
    fn removing_text_keeps_one_space() {
        assert_eq!(remove_text("A. B. C.", "B."), "A. C.");
        assert_eq!(remove_text("A. B.", "B."), "A.");
        assert_eq!(remove_text("B. C.", "B."), "C.");
    }

    #[test]
    fn counts_baseline_productions() {
        assert_eq!(baseline_productions(BASELINE), Ok(3));
        assert!(baseline_productions("# Spec\n").is_err());
    }
}
