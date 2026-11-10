//! Error-code registry, JSON diagnostic format, fixes and `explain` text for Quince.
//!
//! The JSON shape is spec section 8. What the spec leaves open (positions, edits, code format)
//! is fixed in `openspec/changes/m0-repo-harness/specs/diagnostics/spec.md`.

mod registry;
pub mod schema;

pub use registry::{CodeInfo, REGISTRY, lookup};

use serde::{Deserialize, Serialize};

/// One diagnostic, serialized exactly as `quince ... --format json` prints it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Diagnostic {
    pub code: Code,
    pub severity: Severity,
    pub message: String,
    pub span: Span,
    pub fixes: Vec<Fix>,
    /// Always `quince explain <code>`.
    pub docs: String,
}

impl Diagnostic {
    /// A diagnostic with no fixes yet. `docs` is filled in from the code.
    pub fn new(code: Code, severity: Severity, message: impl Into<String>, span: Span) -> Self {
        let docs = format!("quince explain {}", code.as_str());
        Diagnostic {
            code,
            severity,
            message: message.into(),
            span,
            fixes: Vec::new(),
            docs,
        }
    }
}

/// An error code (`E` and four digits) or warning code (`W` and four digits).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Code(String);

impl Code {
    pub fn parse(text: &str) -> Result<Code, String> {
        let bytes = text.as_bytes();
        let valid = bytes.len() == 5
            && matches!(bytes[0], b'E' | b'W')
            && bytes[1..].iter().all(u8::is_ascii_digit);
        if valid {
            Ok(Code(text.to_string()))
        } else {
            Err(format!(
                "`{text}` is not a diagnostic code: expected `E` or `W` followed by four digits"
            ))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The severity a code's letter implies: `E` is an error, `W` a warning.
    pub fn default_severity(&self) -> Severity {
        if self.0.starts_with('E') {
            Severity::Error
        } else {
            Severity::Warning
        }
    }
}

impl TryFrom<String> for Code {
    type Error = String;
    fn try_from(text: String) -> Result<Code, String> {
        Code::parse(&text)
    }
}

impl From<Code> for String {
    fn from(code: Code) -> String {
        code.0
    }
}

impl std::fmt::Display for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// `--deny warnings` reports a warning with severity `error`, and its code is unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

/// A source range. `file` is relative to the project root with `/` separators; `end` is
/// exclusive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Span {
    pub file: String,
    pub start: Position,
    pub end: Position,
}

/// `[line, column]`, both 1-based. The column counts Unicode scalar values from the start of
/// the line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Position(pub u32, pub u32);

impl Position {
    pub fn line(self) -> u32 {
        self.0
    }

    pub fn column(self) -> u32 {
        self.1
    }

    /// The position of a byte offset in `source`. An offset past the end gives the position
    /// just after the last character.
    pub fn at(source: &str, byte_offset: usize) -> Position {
        let before = &source[..source.floor_char_boundary(byte_offset.min(source.len()))];
        let line_start = before.rfind('\n').map_or(0, |i| i + 1);
        let line = before.matches('\n').count() + 1;
        let column = before[line_start..].chars().count() + 1;
        Position(to_u32(line), to_u32(column))
    }
}

fn to_u32(n: usize) -> u32 {
    u32::try_from(n).expect("source positions fit in u32")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fix {
    pub description: String,
    pub applicability: Applicability,
    pub edits: Vec<Edit>,
}

/// `safe` fixes are applied automatically; `placeholder` fixes insert a `todo`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Applicability {
    Safe,
    NeedsReview,
    Placeholder,
}

/// Replaces the text from `start` up to `end` with `text`. `start == end` inserts, and an
/// empty `text` deletes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Edit {
    pub start: Position,
    pub end: Position,
    pub text: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The example diagnostic from spec section 8, read from the spec itself so the two can't
    /// drift.
    pub(crate) fn spec_example() -> String {
        let spec = include_str!("../../../spec/quince-spec.md");
        let section = &spec[spec
            .find("### Structured diagnostics")
            .expect("spec has a Structured diagnostics section")..];
        let open = section.find("```\n").expect("code block") + 4;
        let close = open + section[open..].find("```").expect("closed code block");
        section[open..close].to_string()
    }

    #[test]
    fn spec_example_round_trips() {
        let text = spec_example();
        let diagnostic: Diagnostic = serde_json::from_str(&text).unwrap();
        assert_eq!(diagnostic.code.as_str(), "E0412");
        assert_eq!(diagnostic.span.start, Position(42, 3));
        assert_eq!(
            diagnostic.fixes[0].applicability,
            Applicability::NeedsReview
        );
        let edit = &diagnostic.fixes[0].edits[0];
        assert_eq!((edit.start, edit.end), (Position(46, 5), Position(46, 5)));
        let original: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(serde_json::to_value(&diagnostic).unwrap(), original);
    }

    #[test]
    fn new_fills_docs() {
        let span = Span {
            file: "src/a.qnc".into(),
            start: Position(1, 1),
            end: Position(1, 2),
        };
        let d = Diagnostic::new(Code::parse("W0003").unwrap(), Severity::Warning, "x", span);
        assert_eq!(d.docs, "quince explain W0003");
        assert_eq!(d.code.default_severity(), Severity::Warning);
    }

    #[test]
    fn code_format() {
        for ok in ["E0412", "W0003", "E9999"] {
            assert!(Code::parse(ok).is_ok(), "{ok}");
        }
        for bad in ["E412", "e0412", "X0001", "E04120", "W00a1", ""] {
            assert!(Code::parse(bad).is_err(), "{bad}");
        }
        assert!(serde_json::from_str::<Code>("\"E12\"").is_err());
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let text = spec_example().replacen("\"docs\"", "\"notes\": [], \"docs\"", 1);
        assert!(serde_json::from_str::<Diagnostic>(&text).is_err());
    }

    #[test]
    fn positions_count_scalar_values() {
        let source = "let a = 1\nlet café = 2\n";
        let start = source.find("café").unwrap();
        let end = start + "café".len();
        assert_eq!(Position::at(source, start), Position(2, 5));
        assert_eq!(Position::at(source, end), Position(2, 9));
        assert_eq!(Position::at("café", 0), Position(1, 1));
        assert_eq!(Position::at("café", "café".len()), Position(1, 5));
        assert_eq!(Position::at("ab", 99), Position(1, 3));
    }
}
