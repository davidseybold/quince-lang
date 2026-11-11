//! Every diagnostic code the toolchain can emit.
//!
//! To add a code: write `compiler/diagnostics/explain/<CODE>.md`, add a row below that loads it
//! with `include_str!`, and add a conformance fixture that expects the code with `//~ <CODE>`.
//! `cargo xtask diagnostics` checks all three. A code is never reused: to retire one, set
//! `retired: true` and keep its row and explain text.

use crate::{Code, Severity};

pub struct CodeInfo {
    pub code: &'static str,
    pub severity: Severity,
    /// A few words, such as "non-exhaustive match".
    pub title: &'static str,
    /// The full text `quince explain` prints, with examples.
    pub explain: &'static str,
    pub retired: bool,
}

/// `E0412` is reserved for a non-exhaustive match, so the spec's example diagnostic stays true.
pub const REGISTRY: &[CodeInfo] = &[
    error(
        "E0001",
        "character that starts no token",
        include_str!("../explain/E0001.md"),
    ),
    error("E0002", "semicolon", include_str!("../explain/E0002.md")),
    error(
        "E0003",
        "block comment",
        include_str!("../explain/E0003.md"),
    ),
    error(
        "E0004",
        "unclosed string",
        include_str!("../explain/E0004.md"),
    ),
    error(
        "E0005",
        "unclosed multi-line string",
        include_str!("../explain/E0005.md"),
    ),
    error(
        "E0006",
        "invalid escape",
        include_str!("../explain/E0006.md"),
    ),
    error(
        "E0007",
        "interpolation that isn't a path",
        include_str!("../explain/E0007.md"),
    ),
    error(
        "E0008",
        "malformed number",
        include_str!("../explain/E0008.md"),
    ),
    error(
        "E0009",
        "invalid char literal",
        include_str!("../explain/E0009.md"),
    ),
    error(
        "E0010",
        "unknown literal tag",
        include_str!("../explain/E0010.md"),
    ),
    error(
        "E0011",
        "non-ASCII character in a bytes literal",
        include_str!("../explain/E0011.md"),
    ),
    error(
        "E0012",
        "multi-line string layout",
        include_str!("../explain/E0012.md"),
    ),
    error(
        "E0013",
        "carriage return without a line feed",
        include_str!("../explain/E0013.md"),
    ),
    error(
        "E0014",
        "source file isn't UTF-8",
        include_str!("../explain/E0014.md"),
    ),
    error(
        "E0015",
        "control or bidirectional-format character in a literal or comment",
        include_str!("../explain/E0015.md"),
    ),
    error(
        "E0016",
        "a token that can't go here",
        include_str!("../explain/E0016.md"),
    ),
    error(
        "E0017",
        "an opening bracket with no closing one",
        include_str!("../explain/E0017.md"),
    ),
];

const fn error(code: &'static str, title: &'static str, explain: &'static str) -> CodeInfo {
    CodeInfo {
        code,
        severity: Severity::Error,
        title,
        explain,
        retired: false,
    }
}

pub fn lookup(code: &Code) -> Option<&'static CodeInfo> {
    REGISTRY.iter().find(|info| info.code == code.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn registry_rows_are_well_formed() {
        let mut seen = HashSet::new();
        for info in REGISTRY {
            let code = Code::parse(info.code).unwrap();
            assert!(seen.insert(info.code), "{} is registered twice", info.code);
            assert_eq!(
                code.default_severity(),
                info.severity,
                "{}: the letter must match the severity",
                info.code
            );
            assert!(!info.title.trim().is_empty(), "{} has no title", info.code);
            assert!(
                !info.explain.trim().is_empty(),
                "{} has no explain text",
                info.code
            );
        }
    }
}
