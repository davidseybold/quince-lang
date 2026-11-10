//! Reads the Quince language spec: rule IDs and the grammar appendices.
//!
//! The formats are defined in `openspec/changes/m0-repo-harness/specs/spec-rule-ids/spec.md`.

pub mod grammar;
pub mod rules;

/// The language spec, relative to the repo root.
pub const SPEC_PATH: &str = "spec/quince-spec.md";
/// Rule IDs that must never be used again, relative to the repo root.
pub const RETIRED_PATH: &str = "spec/retired-rules.txt";
/// The allowed first segments of rule IDs, relative to the repo root.
pub const TOPICS_PATH: &str = "spec/rule-topics.txt";
/// The grammar extracted from the spec's appendices, relative to the repo root.
pub const GRAMMAR_PATH: &str = "spec/grammar.ebnf";

/// A problem found in a spec file, with its 1-based line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecError {
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for SpecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}
