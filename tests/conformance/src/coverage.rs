//! The coverage report: which rule IDs have at least one passing fixture.

use crate::{FixtureResult, RuleSet};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt::Write as _;

#[derive(Debug, Serialize)]
pub struct Coverage {
    pub total: usize,
    pub covered: usize,
    /// In the order sections first appear in the spec, then grammar productions.
    pub sections: Vec<SectionCoverage>,
}

#[derive(Debug, Serialize)]
pub struct SectionCoverage {
    pub section: String,
    pub total: usize,
    pub covered: usize,
    pub rules: Vec<RuleCoverage>,
}

#[derive(Debug, Serialize)]
pub struct RuleCoverage {
    pub id: String,
    /// The passing fixtures that cite this rule. Empty means uncovered.
    pub covered_by: Vec<String>,
}

/// Builds the report. `section_order` lists sections in spec order; a failing fixture covers
/// nothing.
pub fn build(rules: &RuleSet, section_order: &[String], results: &[FixtureResult]) -> Coverage {
    let mut covered_by: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for result in results.iter().filter(|r| r.passed()) {
        for id in &result.rules {
            covered_by.entry(id).or_default().push(result.name.clone());
        }
    }
    let mut sections: Vec<SectionCoverage> = section_order
        .iter()
        .map(|section| SectionCoverage {
            section: section.clone(),
            total: 0,
            covered: 0,
            rules: Vec::new(),
        })
        .collect();
    for (id, section) in rules {
        let index = match sections.iter().position(|s| &s.section == section) {
            Some(i) => i,
            None => {
                sections.push(SectionCoverage {
                    section: section.clone(),
                    total: 0,
                    covered: 0,
                    rules: Vec::new(),
                });
                sections.len() - 1
            }
        };
        let fixtures = covered_by.get(id.as_str()).cloned().unwrap_or_default();
        let entry = &mut sections[index];
        entry.total += 1;
        if !fixtures.is_empty() {
            entry.covered += 1;
        }
        entry.rules.push(RuleCoverage {
            id: id.clone(),
            covered_by: fixtures,
        });
    }
    sections.retain(|s| s.total > 0);
    Coverage {
        total: sections.iter().map(|s| s.total).sum(),
        covered: sections.iter().map(|s| s.covered).sum(),
        sections,
    }
}

/// One line per section, then the total.
pub fn summary(coverage: &Coverage) -> String {
    let width = coverage
        .sections
        .iter()
        .map(|s| s.section.len())
        .max()
        .unwrap_or(0);
    let mut out = String::from("spec coverage (rules with a passing fixture)\n");
    for s in &coverage.sections {
        let _ = writeln!(
            out,
            "  {:<width$}  {:>4} / {:<4}",
            s.section, s.covered, s.total
        );
    }
    let _ = writeln!(
        out,
        "  {:<width$}  {:>4} / {:<4}",
        "total", coverage.covered, coverage.total
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> RuleSet {
        [
            ("bind.let-var", "2. Syntax"),
            ("bind.var-param", "2. Syntax"),
            ("ty.int", "3. Types"),
            ("grammar.expr", "Grammar productions"),
        ]
        .into_iter()
        .map(|(id, s)| (id.to_string(), s.to_string()))
        .collect()
    }

    fn order() -> Vec<String> {
        vec!["2. Syntax".into(), "3. Types".into()]
    }

    fn result(name: &str, rules: &[&str], passed: bool) -> FixtureResult {
        FixtureResult {
            name: name.into(),
            rules: rules.iter().map(|s| s.to_string()).collect(),
            failures: if passed {
                vec![]
            } else {
                vec!["failed".into()]
            },
        }
    }

    #[test]
    fn empty_suite_lists_every_rule_uncovered() {
        let c = build(&rules(), &order(), &[]);
        assert_eq!((c.total, c.covered), (4, 0));
        let names: Vec<_> = c.sections.iter().map(|s| s.section.as_str()).collect();
        assert_eq!(names, ["2. Syntax", "3. Types", "Grammar productions"]);
        assert!(
            c.sections
                .iter()
                .flat_map(|s| &s.rules)
                .all(|r| r.covered_by.is_empty())
        );
    }

    #[test]
    fn passing_fixture_covers_and_failing_does_not() {
        let c = build(
            &rules(),
            &order(),
            &[
                result("ok", &["bind.let-var"], true),
                result("bad", &["ty.int"], false),
            ],
        );
        assert_eq!(c.covered, 1);
        assert_eq!(c.sections[0].rules[0].covered_by, ["ok"]);
        assert!(c.sections[1].rules[0].covered_by.is_empty());
    }

    #[test]
    fn summary_has_a_line_per_section() {
        let text = summary(&build(&rules(), &order(), &[]));
        assert!(text.contains("2. Syntax"), "{text}");
        assert!(text.contains("total"), "{text}");
        assert_eq!(text.lines().count(), 5, "{text}");
    }
}
