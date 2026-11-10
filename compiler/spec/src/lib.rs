//! Reads the Quince language spec: rule IDs and the grammar appendices.
//!
//! The formats are defined in `openspec/changes/m0-repo-harness/specs/spec-rule-ids/spec.md`.

pub mod rules;

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
