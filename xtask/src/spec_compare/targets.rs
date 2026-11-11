//! The final-target check (docs/design/spec-restructure.md, decision 7): every link that a
//! row of `refs.tsv` wrote leads where the row's final target says. The move layers retarget
//! links by script, following headings, so a link whose words name one chapter could follow an
//! old heading into another. `spec-links` can't see that, because the link still resolves.
//!
//! A link is checked once the unit holding it sits in a chapter file, or in `std/API.md`. A
//! link that still leads into the legacy file, or sits in it, is interim and is skipped, so once
//! the legacy file is gone every link is checked. A row whose unit can't be found, or no longer
//! holds the row's phrase, fails. A unit that sat in a converted table is looked for in the item
//! `tables.tsv` rebuilt it into.

use super::segment::{Kind, Unit};
use super::tag_format::Rebuilt;
use super::{Outcome, Role, STD_API, Source, Tables, UnitRef, link_targets_removed};
use quince_spec::links;
use std::collections::{HashMap, HashSet};

/// Checks the links of each `refs.tsv` row against its final target. `before` is the
/// baseline's units with the tables applied, link targets removed and converted tables rebuilt
/// as `rebuilt`; `after` is the current files, with their links.
pub fn check(
    before: &[Unit],
    after: &[Source],
    tables: &Tables,
    std_text: &str,
    rebuilt: &[Rebuilt],
    outcome: &mut Outcome,
) {
    // The first rule's ID of each converted row that has one, by baseline line: the row's
    // untagged cells are in that rule's item.
    let merged: HashMap<usize, &str> = rebuilt
        .iter()
        .filter_map(|item| item.id.as_deref().map(|id| (item.line, id)))
        .rev()
        .collect();
    let legacy: HashSet<&str> = after
        .iter()
        .filter(|s| s.role == Role::Legacy)
        .map(|s| s.path.as_str())
        .collect();
    let std_lines: Vec<&str> = std_text.lines().collect();
    // The phrase matches already claimed in each unit, so two rows with the same phrase in one
    // unit check different links.
    let mut claimed: HashMap<(&str, usize), Vec<usize>> = HashMap::new();
    for (rewrite, in_std) in tables
        .rewrites
        .iter()
        .map(|r| (r, false))
        .chain(tables.std_rewrites.iter().map(|r| (r, true)))
    {
        let pattern = Pattern::new(&rewrite.new);
        if pattern.holes() == 0 {
            // A reference inside a code block, or a self-reference, becomes plain text.
            continue;
        }
        let finals: Vec<&str> = rewrite.target.split_whitespace().collect();
        if finals.len() != pattern.holes() {
            outcome.problems.push(format!(
                "{}: the new phrase has {} link(s) but {} final target(s)",
                rewrite.at,
                pattern.holes(),
                finals.len()
            ));
            continue;
        }
        let places: Vec<(&str, usize, &str)> = if in_std {
            let UnitRef::Line(line) = rewrite.unit else {
                continue;
            };
            std_lines
                .get(line.wrapping_sub(1))
                .map(|text| (STD_API, line, *text))
                .into_iter()
                .collect()
        } else {
            match places(before, after, &rewrite.unit, &merged) {
                Some(places) => places,
                // A reference inside a code block became plain text.
                None => continue,
            }
        };
        if places.is_empty() {
            outcome.problems.push(format!(
                "{}: no chapter unit holds this row's unit, so the final target of its link \
                 can't be checked",
                rewrite.at
            ));
        }
        for (path, line, text) in places {
            if legacy.contains(path) {
                continue;
            }
            let taken = claimed.entry((path, line)).or_default();
            let Some((at, dests)) = pattern.find(text, taken) else {
                // In std/API.md, undoing the row already reported it.
                if in_std {
                    continue;
                }
                outcome.problems.push(format!(
                    "{}: {path}:{line} doesn't hold the row's new phrase `{}` with a link in \
                     each hole, so the final target of its link can't be checked",
                    rewrite.at, rewrite.new
                ));
                continue;
            };
            taken.push(at);
            for (dest, wanted) in dests.iter().zip(&finals) {
                let fragment = dest.split_once('#').map(|(_, f)| f);
                let Some(file) = links::resolve(path, dest.split('#').next().unwrap_or_default())
                else {
                    continue; // spec-links reports a link that climbs above the repo root.
                };
                if legacy.contains(file.as_str()) {
                    continue;
                }
                let reached = match fragment {
                    Some(fragment) => format!("{file}#{fragment}"),
                    None => file.clone(),
                };
                let ok = match wanted.split_once('#') {
                    Some(_) => reached == *wanted,
                    None => file == *wanted,
                };
                outcome.final_targets += 1;
                if !ok {
                    outcome.problems.push(format!(
                        "{}: the link `{dest}` at {path}:{line} leads to {reached}, but its final \
                         target is {wanted}",
                        rewrite.at
                    ));
                }
            }
        }
    }
}

/// Where the baseline unit `unit` now sits in a chapter file or the legacy file: its path,
/// line and text. `None` for a code unit, whose references became plain text.
fn places<'a>(
    before: &[Unit],
    after: &'a [Source],
    unit: &UnitRef,
    merged: &HashMap<usize, &str>,
) -> Option<Vec<(&'a str, usize, &'a str)>> {
    let files = after.iter().filter(|s| s.role != Role::Landing);
    let found = |keep: &dyn Fn(&Unit) -> bool| -> Vec<(&'a str, usize, &'a str)> {
        files
            .clone()
            .flat_map(|s| s.units.iter().map(move |u| (s.path.as_str(), u)))
            .filter(|(_, u)| keep(u))
            .map(|(path, u)| (path, u.line, u.text.as_str()))
            .collect()
    };
    let by_id = |id: &str| found(&|u| u.id.as_deref() == Some(id));
    Some(match unit {
        UnitRef::Rule(id) => by_id(id),
        UnitRef::Line(line) => match merged.get(line) {
            Some(id) => by_id(id),
            None => {
                let Some(old) = before.iter().find(|u| u.id.is_none() && u.line == *line) else {
                    return Some(Vec::new());
                };
                if old.kind == Kind::Code {
                    return None;
                }
                found(&|u| {
                    u.id.is_none()
                        && u.kind == old.kind
                        && link_targets_removed(&u.text) == old.text
                })
            }
        },
    })
}

/// A new phrase with its link destinations as holes: `[Effects](#x) and [Contracts](#y)` is the
/// text `[Effects](`, a hole, `) and [Contracts](`, a hole and `)`.
struct Pattern {
    /// The text around the holes; one more piece than there are holes.
    pieces: Vec<String>,
}

impl Pattern {
    fn new(phrase: &str) -> Pattern {
        let mut pieces = Vec::new();
        let mut rest = phrase;
        let mut piece = String::new();
        while let Some(at) = rest.find("](") {
            piece.push_str(&rest[..at + 2]);
            let target = &rest[at + 2..];
            match target.find(')') {
                Some(end) => {
                    pieces.push(std::mem::take(&mut piece));
                    rest = &target[end..];
                }
                None => {
                    rest = target;
                    break;
                }
            }
        }
        piece.push_str(rest);
        pieces.push(piece);
        Pattern { pieces }
    }

    fn holes(&self) -> usize {
        self.pieces.len() - 1
    }

    /// The first match in `text` that doesn't start at a `taken` offset: where it starts and
    /// the destination in each hole.
    fn find<'t>(&self, text: &'t str, taken: &[usize]) -> Option<(usize, Vec<&'t str>)> {
        let first = &self.pieces[0];
        text.match_indices(first.as_str())
            .map(|(at, _)| at)
            .filter(|at| !taken.contains(at))
            .find_map(|at| {
                let mut cursor = at + first.len();
                let mut dests = Vec::new();
                for piece in &self.pieces[1..] {
                    let end = cursor + text[cursor..].find(')')?;
                    dests.push(&text[cursor..end]);
                    if !text[end..].starts_with(piece.as_str()) {
                        return None;
                    }
                    cursor = end + piece.len();
                }
                Some((at, dests))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_find_their_destinations() {
        let pattern = Pattern::new("[Effects](#5-effects) and [Contracts](#requires)");
        assert_eq!(pattern.holes(), 2);
        let text = "See [Effects](reference/effects.md) and [Contracts](reference/contracts.md#x).";
        assert_eq!(
            pattern.find(text, &[]),
            Some((4, vec!["reference/effects.md", "reference/contracts.md#x"]))
        );
        assert_eq!(pattern.find(text, &[4]), None);
        assert_eq!(
            pattern.find("See [Effects](a.md) or [Contracts](b.md)", &[]),
            None
        );
        assert_eq!(Pattern::new("(Interop with C)").holes(), 0);

        let twice = Pattern::new("[Effects](#e)");
        let text = "[Effects](a.md), then [Effects](b.md)";
        assert_eq!(twice.find(text, &[0]), Some((22, vec!["b.md"])));
    }
}
