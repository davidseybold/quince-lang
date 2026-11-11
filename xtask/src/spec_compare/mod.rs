//! `cargo xtask spec-compare`: proves that the spec restructure changes no meaning
//! (docs/design/spec-restructure.md, decision 8). It splits the pinned baseline spec and the
//! current spec into units and checks that they match: every tagged rule keeps its text, and
//! the untagged units are the same multiset. This check and the baseline are deleted when the
//! restructure is done.

pub mod segment;
mod targets;

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
/// Positional references rewritten as links, relative to the repo root.
pub const REFS: &str = "docs/design/spec-restructure/refs.tsv";
/// Tagged sentences whose wording changed, relative to the repo root.
pub const REWORDINGS: &str = "docs/design/spec-restructure/rewordings.tsv";
/// What became of the baseline's headings, relative to the repo root.
pub const HEADINGS: &str = "docs/design/spec-restructure/headings.tsv";

/// The Introduction's link to the note that some examples are signature listings: the one
/// paragraph the restructure adds to a chapter (decision 6), as a file and its text with link
/// targets removed.
const INTRO_LINK: (&str, &str) = (
    "spec/reference/introduction.md",
    "Some examples are signature listings rather than compilable code: [Prelude and special \
     interfaces]() says which.",
);

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
    /// Rewrites of baseline text, from `refs.tsv` and then `rewordings.tsv`, in table order.
    pub rewrites: Vec<Rewrite>,
    /// Rewrites made in `std/API.md`, from the `std` rows of `refs.tsv`. Each unit is a line
    /// of `std/API.md`.
    pub std_rewrites: Vec<Rewrite>,
    /// What became of the baseline's headings, and the headings the restructure adds, from
    /// `headings.tsv`.
    pub headings: Vec<HeadingRow>,
}

/// A row of `headings.tsv`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadingRow {
    /// The table row, as `path:line`.
    pub at: String,
    pub change: HeadingChange,
}

/// What a row of `headings.tsv` says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeadingChange {
    /// The restructure adds this heading line to this file.
    New { path: String, heading: String },
    /// The baseline heading `old` on `line` became this heading line in this file. A heading
    /// whose section split has a row for each part.
    Moved {
        line: usize,
        old: String,
        path: String,
        heading: String,
    },
    /// The baseline heading `old` on `line` is gone, and its section's text sits under other
    /// headings.
    Dropped { line: usize, old: String },
}

/// One rewrite of a baseline unit's text: its first `old` becomes `new`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rewrite {
    /// The table row, as `path:line`.
    pub at: String,
    pub unit: UnitRef,
    pub old: String,
    pub new: String,
    /// Where each link in `new` must lead once the restructure is done, as paths from the repo
    /// root, space-separated; empty for a rewording.
    pub target: String,
}

/// The baseline unit a rewrite applies to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnitRef {
    /// The tagged rule with this ID, as the spec tags it now.
    Rule(String),
    /// The untagged unit that starts on this baseline line.
    Line(usize),
}

impl UnitRef {
    /// A rule ID, or `L` and a line number.
    fn parse(text: &str) -> Option<UnitRef> {
        match text.strip_prefix('L').map(str::parse) {
            Some(Ok(line)) => Some(UnitRef::Line(line)),
            _ if quince_spec::rules::is_valid_id(text) => Some(UnitRef::Rule(text.to_string())),
            _ => None,
        }
    }

    fn matches(&self, unit: &Unit) -> bool {
        match self {
            UnitRef::Rule(id) => unit.id.as_deref() == Some(id),
            UnitRef::Line(line) => unit.id.is_none() && unit.line == *line,
        }
    }
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

    /// Adds the rows of `refs.tsv`, `rewordings.tsv` and `headings.tsv`, and returns an error
    /// for each bad row. Columns are tab-separated; blank lines and `#` comments are ignored.
    /// - `refs.tsv`: where the reference is (`spec`, or `std` for `std/API.md`), its unit (a
    ///   rule ID, or `L` and the line the unit starts on: in the baseline for `spec`, in
    ///   `std/API.md` for `std`), the old phrase, the new phrase, and the final target of its
    ///   link as a path from the repo root. `spec` rows rewrite the baseline. `std` rows are
    ///   undone in `std/API.md` before checking that the units that moved there still hold
    ///   their text; its other text isn't compared.
    /// - `rewordings.tsv`: a rule ID, the old text and the new text.
    /// - `headings.tsv`: `new`, a file and the heading line the restructure adds there;
    ///   `moved`, a baseline line (`L120`), the heading on it, a file and the heading line it
    ///   became there; or `dropped`, a baseline line and the heading on it.
    pub fn parse_rewrites(&mut self, refs: &str, rewordings: &str, headings: &str) -> Vec<String> {
        let mut errors = Vec::new();
        let filled = |columns: &[&str]| columns.iter().all(|c| !c.is_empty());
        for (line, columns) in rows(refs) {
            let at = format!("{REFS}:{line}");
            match columns.as_slice() {
                [origin @ ("spec" | "std"), unit, old, new, target] if filled(&columns) => {
                    let rewrite = |unit| Rewrite {
                        at: at.clone(),
                        unit,
                        old: old.to_string(),
                        new: new.to_string(),
                        target: target.to_string(),
                    };
                    match (*origin, UnitRef::parse(unit)) {
                        ("spec", Some(unit)) => self.rewrites.push(rewrite(unit)),
                        ("std", Some(unit @ UnitRef::Line(_))) => {
                            self.std_rewrites.push(rewrite(unit))
                        }
                        ("std", _) => errors.push(format!(
                            "{at}: a `std` row names a line of {STD_API}, such as `L120`"
                        )),
                        _ => errors.push(format!(
                            "{at}: `{unit}` is not a rule ID or a line such as `L120`"
                        )),
                    }
                }
                _ => errors.push(format!(
                    "{at}: a row is `spec` or `std`, the unit, the old phrase, the new phrase and \
                     the final target"
                )),
            }
        }
        for (line, columns) in rows(rewordings) {
            let at = format!("{REWORDINGS}:{line}");
            match columns.as_slice() {
                [id, old, new] if filled(&columns) && quince_spec::rules::is_valid_id(id) => {
                    self.rewrites.push(Rewrite {
                        at,
                        unit: UnitRef::Rule(id.to_string()),
                        old: old.to_string(),
                        new: new.to_string(),
                        target: String::new(),
                    })
                }
                _ => errors.push(format!(
                    "{at}: a row is a rule ID, the old text and the new text"
                )),
            }
        }
        let is_heading = |text: &str| text.starts_with('#');
        let baseline_line = |unit: &str| match UnitRef::parse(unit) {
            Some(UnitRef::Line(line)) => Some(line),
            _ => None,
        };
        for (line, columns) in rows(headings) {
            let at = format!("{HEADINGS}:{line}");
            let change = match columns.as_slice() {
                ["new", path, heading] if !path.is_empty() && is_heading(heading) => {
                    Some(HeadingChange::New {
                        path: path.to_string(),
                        heading: heading.to_string(),
                    })
                }
                ["moved", unit, old, path, heading]
                    if is_heading(old) && !path.is_empty() && is_heading(heading) =>
                {
                    baseline_line(unit).map(|line| HeadingChange::Moved {
                        line,
                        old: old.to_string(),
                        path: path.to_string(),
                        heading: heading.to_string(),
                    })
                }
                ["dropped", unit, old] if is_heading(old) => {
                    baseline_line(unit).map(|line| HeadingChange::Dropped {
                        line,
                        old: old.to_string(),
                    })
                }
                _ => None,
            };
            match change {
                Some(change) => self.headings.push(HeadingRow { at, change }),
                None => errors.push(format!(
                    "{at}: a row is `new`, a file and the heading line it adds; `moved`, a \
                     baseline line such as `L120`, its heading, a file and the heading line there; \
                     or `dropped`, a baseline line and its heading"
                )),
            }
        }
        errors
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
    /// Units allowed without a baseline match: chapter titles, repeated table headers and
    /// the headings `headings.tsv` lists as new.
    pub added: usize,
    /// Rewrites from `refs.tsv` and `rewordings.tsv` applied to the baseline.
    pub rewritten: usize,
    /// Links checked against their final target in `refs.tsv`.
    pub final_targets: usize,
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
    let (mut tables, mut table_errors) = Tables::parse(&optional(ID_MAP)?, &optional(REMOVED)?);
    table_errors.extend(tables.parse_rewrites(
        &optional(REFS)?,
        &optional(REWORDINGS)?,
        &optional(HEADINGS)?,
    ));
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
         units {} matched, {} moved, {} to std/API.md, {} added; {} rewrites; {} links at their \
         final target",
        tables.removed_ids.len(),
        outcome.matched,
        outcome.moved,
        outcome.to_std,
        outcome.added,
        outcome.rewritten,
        outcome.final_targets
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
/// the units that moved to `std/API.md` (checking that `std_text` holds each one), maps
/// renamed IDs to their new names, and applies the rewrites.
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

    let std_before = std_before_references(std_text, tables, problems);
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
        if !std_before.contains(&link_targets_removed(&unit.text)) {
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
    for rewrite in &tables.rewrites {
        match kept.iter_mut().find(|unit| rewrite.unit.matches(unit)) {
            Some(unit) if unit.text.contains(&rewrite.old) => {
                unit.text = unit.text.replacen(&rewrite.old, &rewrite.new, 1);
                outcome.rewritten += 1;
            }
            Some(unit) => problems.push(format!(
                "{}: the baseline unit on line {} doesn't hold `{}`",
                rewrite.at,
                unit.line,
                snippet(&rewrite.old)
            )),
            None => problems.push(format!(
                "{}: no baseline unit is {}",
                rewrite.at,
                match &rewrite.unit {
                    UnitRef::Rule(id) => format!("tagged `{id}`"),
                    UnitRef::Line(line) => format!("untagged on line {line}"),
                }
            )),
        }
    }
    kept
}

/// `std_text` as it was before its positional references were rewritten: each `std` row of
/// `refs.tsv` undone on its line, with link targets removed from the whole text so a link that
/// was retargeted later still matches.
fn std_before_references(std_text: &str, tables: &Tables, problems: &mut Vec<String>) -> String {
    let mut lines: Vec<String> = std_text.lines().map(link_targets_removed).collect();
    let mut cursor: HashMap<usize, usize> = HashMap::new();
    for rewrite in &tables.std_rewrites {
        let UnitRef::Line(line) = rewrite.unit else {
            continue;
        };
        let new = link_targets_removed(&rewrite.new);
        let from = cursor.get(&line).copied().unwrap_or(0);
        let found = lines
            .get(line.wrapping_sub(1))
            .and_then(|text| text[from..].find(&new).map(|at| from + at));
        match found {
            Some(at) => {
                lines[line - 1].replace_range(at..at + new.len(), &rewrite.old);
                cursor.insert(line, at + rewrite.old.len());
            }
            None => problems.push(format!(
                "{}: line {line} of {STD_API} doesn't hold `{}`",
                rewrite.at,
                snippet(&new)
            )),
        }
    }
    lines.join("\n")
}

/// `unit` with its link targets removed (see [`link_targets_removed`]). Code is left alone.
fn without_link_targets(mut unit: Unit) -> Unit {
    if unit.kind != Kind::Code {
        unit.text = link_targets_removed(&unit.text);
    }
    unit
}

/// `text` with every Markdown link target emptied, so `[text](target)` becomes `[text]()`.
/// Targets change as text moves, and `spec-links` checks them.
fn link_targets_removed(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("](") {
        out.push_str(&rest[..at + 2]);
        let target = &rest[at + 2..];
        match target.find(')') {
            Some(end) => rest = &target[end..],
            None => {
                rest = target;
                break;
            }
        }
    }
    out.push_str(rest);
    out
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
    let before: Vec<Unit> = normalize(before, tables, std_text, &mut outcome)
        .into_iter()
        .map(without_link_targets)
        .collect();
    let before = before.as_slice();
    targets::check(before, after, tables, std_text, &mut outcome);
    let after: Vec<Source> = after
        .iter()
        .map(|source| Source {
            path: source.path.clone(),
            units: source
                .units
                .iter()
                .cloned()
                .map(without_link_targets)
                .collect(),
            role: source.role,
        })
        .collect();
    let after = after.as_slice();
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

    // Headings are checked on their own, against `headings.tsv`.
    let mut pool: HashMap<(Kind, &str), Vec<usize>> = HashMap::new();
    for unit in before
        .iter()
        .rev()
        .filter(|u| u.id.is_none() && u.kind != Kind::Heading)
    {
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
        for unit in &source.units {
            if unit.id.is_some() || unit.kind == Kind::Heading {
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
                    if unit.kind == Kind::Paragraph
                        && (source.path.as_str(), unit.text.as_str()) == INTRO_LINK =>
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
    compare_headings(before, after, tables, &mut outcome);
    outcome
}

/// A heading the "after" side must have, and why.
enum Expected<'a> {
    /// The baseline heading on this line, which has no row in `headings.tsv`, so it is still
    /// in the legacy file.
    Kept(usize),
    /// A `moved` or `new` row of `headings.tsv`.
    Row { at: &'a str, new: bool },
}

/// Checks the headings of the chapter files and the legacy file:
/// - a baseline heading without a row in `headings.tsv` is still in the legacy file, so once
///   the legacy file is gone every baseline heading needs a row;
/// - each `moved` and `new` row's heading is in its file, and each `moved` and `dropped` row
///   names a baseline heading;
/// - every other heading is a chapter's `# Title` line.
fn compare_headings(before: &[Unit], after: &[Source], tables: &Tables, outcome: &mut Outcome) {
    let baseline: HashMap<usize, &str> = before
        .iter()
        .filter(|u| u.kind == Kind::Heading)
        .map(|u| (u.line, u.text.as_str()))
        .collect();
    let legacy = after
        .iter()
        .find(|s| s.role == Role::Legacy)
        .map(|s| s.path.as_str());
    let mut expected: Vec<(&str, &str, Expected)> = Vec::new();
    let mut targets: HashSet<(&str, &str)> = HashSet::new();
    let mut with_rows: HashMap<usize, (&str, bool)> = HashMap::new();
    for row in &tables.headings {
        let (line, old) = match &row.change {
            HeadingChange::New { path, heading } => {
                expected.push((
                    path,
                    heading,
                    Expected::Row {
                        at: &row.at,
                        new: true,
                    },
                ));
                continue;
            }
            HeadingChange::Moved {
                line,
                old,
                path,
                heading,
            } => {
                if targets.insert((path, heading)) {
                    expected.push((
                        path,
                        heading,
                        Expected::Row {
                            at: &row.at,
                            new: false,
                        },
                    ));
                }
                (*line, old)
            }
            HeadingChange::Dropped { line, old } => (*line, old),
        };
        match baseline.get(&line) {
            Some(text) if text == old => {}
            Some(text) => outcome.problems.push(format!(
                "{}: the baseline heading on line {line} is `{text}`, not `{old}`",
                row.at
            )),
            None => outcome.problems.push(format!(
                "{}: no baseline heading that stays in the spec is on line {line}",
                row.at
            )),
        }
        let dropped = matches!(row.change, HeadingChange::Dropped { .. });
        if let Some((_, other_dropped)) = with_rows.insert(line, (&row.at, dropped))
            && (dropped || other_dropped)
        {
            outcome.problems.push(format!(
                "{}: the baseline heading on line {line} has another row; a dropped heading has \
                 only one",
                row.at
            ));
        }
    }
    let mut kept: Vec<(&str, &str, Expected)> = before
        .iter()
        .filter(|u| u.kind == Kind::Heading && !with_rows.contains_key(&u.line))
        .map(|u| {
            (
                legacy.unwrap_or_default(),
                u.text.as_str(),
                Expected::Kept(u.line),
            )
        })
        .collect();
    kept.append(&mut expected);

    // Each heading on the "after" side, and whether something accounts for it.
    let mut found: Vec<(&Source, &Unit, bool)> = after
        .iter()
        .filter(|s| s.role != Role::Landing)
        .flat_map(|s| {
            s.units
                .iter()
                .filter(|u| u.kind == Kind::Heading)
                .map(move |u| (s, u, false))
        })
        .collect();
    for (path, heading, why) in kept {
        let hit = found
            .iter_mut()
            .find(|(s, u, taken)| !*taken && s.path == path && u.text == heading);
        match (hit, why) {
            (Some(hit), why) => {
                hit.2 = true;
                match why {
                    Expected::Kept(_) => outcome.matched += 1,
                    Expected::Row { new: true, .. } => outcome.added += 1,
                    Expected::Row { new: false, .. } => {
                        outcome.matched += 1;
                        outcome.moved += 1;
                    }
                }
            }
            (None, Expected::Kept(line)) => outcome.problems.push(match legacy {
                Some(legacy) => format!(
                    "{BASELINE_SPEC}:{line}: heading `{heading}` is missing from {legacy}; a \
                     heading that moved or went needs a row in {HEADINGS}"
                ),
                None => {
                    format!("{BASELINE_SPEC}:{line}: heading `{heading}` has no row in {HEADINGS}")
                }
            }),
            (None, Expected::Row { at, .. }) => outcome.problems.push(format!(
                "{at}: `{heading}` is listed in {path}, but {path} has no such heading"
            )),
        }
    }
    for (source, unit, taken) in found {
        if taken {
            continue;
        }
        let is_title = source.role == Role::Chapter
            && unit.text.starts_with("# ")
            && source
                .units
                .first()
                .is_some_and(|first| std::ptr::eq(first, unit));
        if is_title {
            outcome.added += 1;
        } else {
            outcome.problems.push(format!(
                "{}:{}: this heading isn't in the baseline or {HEADINGS}: {}",
                source.path, unit.line, unit.text
            ));
        }
    }
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

    fn rewrite_tables(refs: &str, rewordings: &str, headings: &str) -> Tables {
        let mut tables = Tables::default();
        let errors = tables.parse_rewrites(refs, rewordings, headings);
        assert_eq!(errors, Vec::<String>::new());
        tables
    }

    #[test]
    fn rewrite_rows() {
        let mut tables = Tables::default();
        let errors = tables.parse_rewrites(
            "# where\tunit\told\tnew\ttarget\nspec\tval.a\tx\t[x](a.md)\tspec/a.md\nstd\tL3\ty\t[y](b.md)\tspec/b.md\n\
             spec\tL9\tz\tZ\t\ndoc\tval.a\tx\ty\tz\nspec\tBad\tx\ty\tz\n",
            "val.b\told\tnew\nval.b\told\n",
            "new\tspec/quince-spec.md\t### Kinds\nnew\tspec/quince-spec.md\tKinds\n\
             moved\tL5\t## 1. Values\tspec/a.md\t# Values\ndropped\tL34\t### Old\n\
             moved\t5\t## 1. Values\tspec/a.md\t# Values\nmoved\tL5\t## 1. Values\tspec/a.md\n\
             dropped\tL34\tOld\n",
        );
        assert_eq!(
            tables.rewrites,
            [
                Rewrite {
                    at: "docs/design/spec-restructure/refs.tsv:2".into(),
                    unit: UnitRef::Rule("val.a".into()),
                    old: "x".into(),
                    new: "[x](a.md)".into(),
                    target: "spec/a.md".into(),
                },
                Rewrite {
                    at: "docs/design/spec-restructure/rewordings.tsv:1".into(),
                    unit: UnitRef::Rule("val.b".into()),
                    old: "old".into(),
                    new: "new".into(),
                    target: String::new(),
                },
            ]
        );
        assert_eq!(
            tables.headings,
            [
                HeadingRow {
                    at: "docs/design/spec-restructure/headings.tsv:1".into(),
                    change: HeadingChange::New {
                        path: "spec/quince-spec.md".into(),
                        heading: "### Kinds".into(),
                    },
                },
                HeadingRow {
                    at: "docs/design/spec-restructure/headings.tsv:3".into(),
                    change: HeadingChange::Moved {
                        line: 5,
                        old: "## 1. Values".into(),
                        path: "spec/a.md".into(),
                        heading: "# Values".into(),
                    },
                },
                HeadingRow {
                    at: "docs/design/spec-restructure/headings.tsv:4".into(),
                    change: HeadingChange::Dropped {
                        line: 34,
                        old: "### Old".into(),
                    },
                },
            ]
        );
        let lines: Vec<_> = errors
            .iter()
            .map(|e| e.split(": ").next().unwrap_or_default())
            .collect();
        assert_eq!(
            lines,
            [
                "docs/design/spec-restructure/refs.tsv:4",
                "docs/design/spec-restructure/refs.tsv:5",
                "docs/design/spec-restructure/refs.tsv:6",
                "docs/design/spec-restructure/rewordings.tsv:2",
                "docs/design/spec-restructure/headings.tsv:2",
                "docs/design/spec-restructure/headings.tsv:5",
                "docs/design/spec-restructure/headings.tsv:6",
                "docs/design/spec-restructure/headings.tsv:7",
            ],
            "{errors:?}"
        );
    }

    #[test]
    fn rewrites_and_new_headings_pass() {
        let tables = rewrite_tables(
            "spec\tval.a\timmutable\t[immutable](#values)\tspec/reference/values.md\n\
             spec\tL15\ttoo\t[too](#kinds)\tspec/reference/values.md#kinds\n",
            "val.b\tCopies are cheap.\tCopies cost little.\n",
            "new\tspec/quince-spec.md\t### Kinds\n",
        );
        let spec = BASELINE
            .replace("are immutable.", "are [immutable](reference/values.md).")
            .replace("Copies are cheap.", "Copies cost little.")
            .replace("* Maps are too.", "* Maps are [too](other.md#x).")
            .replace("| Kind | Rule |", "### Kinds\n\n| Kind | Rule |");
        let outcome = compare(&segment(BASELINE), &[legacy(&spec)], &tables, "");
        assert_eq!(outcome.problems, Vec::<String>::new());
        assert_eq!((outcome.rewritten, outcome.added), (3, 1));

        let outcome = compare(&segment(BASELINE), &[legacy(&spec)], &Tables::default(), "");
        let problems = outcome.problems.join("\n");
        assert!(
            problems.contains("the text of rule `val.a` changed"),
            "{problems}"
        );
        assert!(
            problems.contains(
                "spec/quince-spec.md:9: this heading isn't in the baseline or docs/design/spec-restructure/headings.tsv: ### Kinds"
            ),
            "{problems}"
        );
    }

    #[test]
    fn stale_rewrites_and_headings_fail() {
        let tables = rewrite_tables(
            "spec\tval.a\tmutable!\tx\tspec/a.md\nspec\tval.zzz\tx\ty\tspec/a.md\nspec\tL16\tx\ty\tspec/a.md\n",
            "",
            "new\tspec/quince-spec.md\t### Gone\n",
        );
        let outcome = compare(&segment(BASELINE), &[legacy(BASELINE)], &tables, "");
        assert_eq!(
            outcome.problems,
            [
                "docs/design/spec-restructure/refs.tsv:1: the baseline unit on line 7 doesn't hold `mutable!`",
                "docs/design/spec-restructure/refs.tsv:2: no baseline unit is tagged `val.zzz`",
                "docs/design/spec-restructure/refs.tsv:3: no baseline unit is untagged on line 16",
                "docs/design/spec-restructure/headings.tsv:1: `### Gone` is listed in spec/quince-spec.md, but spec/quince-spec.md has no such heading",
            ]
        );
    }

    #[test]
    fn only_the_introduction_may_add_its_link() {
        let link = "Some examples are signature listings rather than compilable code: \
                    [Prelude and special interfaces](prelude.md) says which.";
        let intro = format!("# Introduction\n\n{link}\n");
        let outcome = compare_with(&[
            legacy(BASELINE),
            source("spec/reference/introduction.md", Role::Chapter, &intro),
        ]);
        assert_eq!(outcome.problems, Vec::<String>::new());
        assert_eq!(outcome.added, 2);

        let elsewhere = format!("# Values\n\n{link}\n");
        let outcome = compare_with(&[
            legacy(BASELINE),
            source("spec/reference/values.md", Role::Chapter, &elsewhere),
        ]);
        assert_eq!(
            outcome.problems,
            [
                "spec/reference/values.md:3: this paragraph isn't in the baseline: Some examples are signature listings rather than compilable code: [Prelude and special interfaces]()…"
            ]
        );
    }

    #[test]
    fn moved_split_and_dropped_headings_pass() {
        // The Values section becomes a chapter whose title and `## Kinds` heading both come
        // from `## 1. Values`, and the second grammar heading goes.
        let values_at = BASELINE.find("## 1. Values").unwrap();
        let grammar_at = BASELINE.find("## Appendix").unwrap();
        let chapter = BASELINE[values_at..grammar_at]
            .replace("## 1. Values", "# Values")
            .replace("| Kind | Rule |", "## Kinds\n\n| Kind | Rule |");
        let rest = format!(
            "{}{}",
            &BASELINE[..values_at],
            BASELINE[grammar_at..].replace("### Declarations, types and patterns\n\n", "")
        );
        let tables = rewrite_tables(
            "",
            "",
            "moved\tL5\t## 1. Values\tspec/reference/values.md\t# Values\n\
             moved\tL5\t## 1. Values\tspec/reference/values.md\t## Kinds\n\
             dropped\tL34\t### Declarations, types and patterns\n",
        );
        let outcome = compare(
            &segment(BASELINE),
            &[
                legacy(&rest),
                source("spec/reference/values.md", Role::Chapter, &chapter),
            ],
            &tables,
            "",
        );
        assert_eq!(outcome.problems, Vec::<String>::new());
        // The chapter title is a moved heading, not an added one.
        assert_eq!(outcome.added, 0);

        // Without the rows, both chapter headings are new and the baseline ones are missing.
        let outcome = compare(
            &segment(BASELINE),
            &[
                legacy(&rest),
                source("spec/reference/values.md", Role::Chapter, &chapter),
            ],
            &Tables::default(),
            "",
        );
        assert_eq!(
            outcome.problems,
            [
                "docs/design/spec-restructure/baseline/quince-spec.md:5: heading `## 1. Values` is missing from spec/quince-spec.md; a heading that moved or went needs a row in docs/design/spec-restructure/headings.tsv",
                "docs/design/spec-restructure/baseline/quince-spec.md:34: heading `### Declarations, types and patterns` is missing from spec/quince-spec.md; a heading that moved or went needs a row in docs/design/spec-restructure/headings.tsv",
                "spec/reference/values.md:5: this heading isn't in the baseline or docs/design/spec-restructure/headings.tsv: ## Kinds",
            ]
        );
    }

    #[test]
    fn stale_or_conflicting_heading_rows_fail() {
        let tables = rewrite_tables(
            "",
            "",
            "moved\tL6\t## 1. Values\tspec/reference/values.md\t# Values\n\
             dropped\tL5\t## Values\n\
             moved\tL5\t## 1. Values\tspec/quince-spec.md\t## 1. Values\n",
        );
        let outcome = compare(&segment(BASELINE), &[legacy(BASELINE)], &tables, "");
        assert_eq!(
            outcome.problems,
            [
                "docs/design/spec-restructure/headings.tsv:1: no baseline heading that stays in the spec is on line 6",
                "docs/design/spec-restructure/headings.tsv:2: the baseline heading on line 5 is `## 1. Values`, not `## Values`",
                "docs/design/spec-restructure/headings.tsv:3: the baseline heading on line 5 has another row; a dropped heading has only one",
                "docs/design/spec-restructure/headings.tsv:1: `# Values` is listed in spec/reference/values.md, but spec/reference/values.md has no such heading",
            ]
        );
    }

    #[test]
    fn without_the_legacy_file_every_heading_needs_a_row() {
        let tables = rewrite_tables(
            "",
            "",
            "moved\tL5\t## 1. Values\tspec/reference/values.md\t# Values\n",
        );
        let chapter = "# Values\n\n## Kinds\n\n# Late title\n";
        let outcome = compare(
            &segment(BASELINE),
            &[source("spec/reference/values.md", Role::Chapter, chapter)],
            &tables,
            "",
        );
        let headings: Vec<&String> = outcome
            .problems
            .iter()
            .filter(|p| p.contains("heading"))
            .collect();
        assert_eq!(
            headings,
            [
                "docs/design/spec-restructure/baseline/quince-spec.md:1: heading `# Spec` has no row in docs/design/spec-restructure/headings.tsv",
                "docs/design/spec-restructure/baseline/quince-spec.md:27: heading `## Appendix: Expression grammar` has no row in docs/design/spec-restructure/headings.tsv",
                "docs/design/spec-restructure/baseline/quince-spec.md:34: heading `### Declarations, types and patterns` has no row in docs/design/spec-restructure/headings.tsv",
                "spec/reference/values.md:3: this heading isn't in the baseline or docs/design/spec-restructure/headings.tsv: ## Kinds",
                "spec/reference/values.md:5: this heading isn't in the baseline or docs/design/spec-restructure/headings.tsv: # Late title",
            ]
        );
    }

    #[test]
    fn std_rows_are_undone_before_checking_moved_text() {
        let (spec, tables_moved, std) = library_move();
        // Retargeted since: the link keeps its final file and gains a fragment.
        let rewritten = std.replace("Maps are too.", "Maps are [too](../spec/a.md#y).");
        let mut tables_moved = tables_moved;
        let errors =
            tables_moved.parse_rewrites("std\tL4\ttoo\t[too](../spec/a.md)\tspec/a.md\n", "", "");
        assert_eq!(errors, Vec::<String>::new());
        let outcome = compare(
            &segment(BASELINE),
            &[legacy(&spec)],
            &tables_moved,
            &rewritten,
        );
        assert_eq!(outcome.problems, Vec::<String>::new());

        let errors = tables_moved.parse_rewrites(
            "std\tL3\tLists\t[Lists](a.md)\tspec/a.md\nstd\tval.a\tx\ty\tz\n",
            "",
            "",
        );
        assert_eq!(
            errors,
            [
                "docs/design/spec-restructure/refs.tsv:2: a `std` row names a line of std/API.md, such as `L120`"
            ]
        );
        let outcome = compare(
            &segment(BASELINE),
            &[legacy(&spec)],
            &tables_moved,
            &rewritten,
        );
        assert_eq!(
            outcome.problems,
            [
                "docs/design/spec-restructure/refs.tsv:1: line 3 of std/API.md doesn't hold `[Lists]()`"
            ]
        );
    }

    #[test]
    fn links_lead_to_their_final_targets() {
        // The Values section becomes a chapter, and two of its references are links.
        let values_at = BASELINE.find("## 1. Values").unwrap();
        let grammar_at = BASELINE.find("## Appendix").unwrap();
        let chapter = |immutable: &str, too: &str| {
            BASELINE[values_at..grammar_at]
                .replace("## 1. Values", "# Values")
                .replace("are immutable.", &format!("are [immutable]({immutable})."))
                .replace("are too.", &format!("are [too]({too})."))
        };
        let rest = format!("{}{}", &BASELINE[..values_at], &BASELINE[grammar_at..]);
        let refs = "spec\tval.a\timmutable\t[immutable](#values)\tspec/reference/values.md\n\
                    spec\tL15\ttoo\t[too](#kinds)\tspec/reference/types.md#kinds\n\
                    spec\tL7\tIntro.\tIntro.\t-\n";
        let tables = rewrite_tables(
            refs,
            "",
            "moved\tL5\t## 1. Values\tspec/reference/values.md\t# Values\n",
        );
        let run = |immutable: &str, too: &str| {
            let chapter = chapter(immutable, too);
            let after = [
                legacy(&rest),
                source("spec/reference/values.md", Role::Chapter, &chapter),
            ];
            let outcome = compare(&segment(BASELINE), &after, &tables, "");
            (outcome.problems, outcome.final_targets)
        };
        // A final target without a fragment names the file, so a link may name any of its
        // headings; one with a fragment names the heading too.
        assert_eq!(run("#values", "types.md#kinds"), (vec![], 2));
        assert_eq!(run("values.md#intro", "types.md#kinds"), (vec![], 2));
        assert_eq!(
            run("types.md", "types.md#other"),
            (
                vec![
                    "docs/design/spec-restructure/refs.tsv:1: the link `types.md` at spec/reference/values.md:3 leads to spec/reference/types.md, but its final target is spec/reference/values.md".to_string(),
                    "docs/design/spec-restructure/refs.tsv:2: the link `types.md#other` at spec/reference/values.md:11 leads to spec/reference/types.md#other, but its final target is spec/reference/types.md#kinds".to_string(),
                ],
                2
            )
        );
        // A link into the legacy file is interim, and so is any link in it.
        assert_eq!(run("#values", "../quince-spec.md#kinds"), (vec![], 1));
        let in_legacy = BASELINE
            .replace("are immutable.", "are [immutable](reference/x.md).")
            .replace("are too.", "are [too](reference/x.md).");
        let unmoved = rewrite_tables(refs, "", "");
        let outcome = compare(&segment(BASELINE), &[legacy(&in_legacy)], &unmoved, "");
        assert_eq!((outcome.problems, outcome.final_targets), (vec![], 0));

        let miscounted = rewrite_tables(
            "spec\tval.a\timmutable\t[immutable](#values)\tspec/a.md spec/b.md\n",
            "",
            "",
        );
        let spec = BASELINE.replace("are immutable.", "are [immutable](reference/x.md).");
        let outcome = compare(&segment(BASELINE), &[legacy(&spec)], &miscounted, "");
        assert_eq!(
            outcome.problems,
            [
                "docs/design/spec-restructure/refs.tsv:1: the new phrase has 1 link(s) but 2 final target(s)"
            ]
        );
    }

    #[test]
    fn link_targets_are_ignored() {
        let unit = |kind, text: &str| Unit {
            kind,
            id: None,
            text: text.into(),
            line: 1,
        };
        assert_eq!(
            without_link_targets(unit(Kind::Item, "[a](b.md) and [c](d.md#e), [f] (g) [h](i")).text,
            "[a]() and [c](), [f] (g) [h](i"
        );
        assert_eq!(
            without_link_targets(unit(Kind::Code, "xs[0](y)")).text,
            "xs[0](y)"
        );
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
