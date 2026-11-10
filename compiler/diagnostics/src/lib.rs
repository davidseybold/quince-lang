//! Error-code registry, JSON diagnostic format, fixes and `explain` text for Quince.
//!
//! The JSON shape is spec section 8. What the spec leaves open (positions, edits, code format)
//! is fixed in `openspec/specs/diagnostics/spec.md`.

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
/// the line. A line ends at LF, CR LF or a lone CR, as in LSP, and a leading UTF-8 byte order
/// mark isn't counted.
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
        let offset = source.floor_char_boundary(byte_offset.min(source.len()));
        let before = &source[..offset];
        let bytes = source.as_bytes();
        let (mut line, mut line_start) = (1, 0);
        for (i, &b) in before.as_bytes().iter().enumerate() {
            let ends_line = b == b'\n' || (b == b'\r' && bytes.get(i + 1) != Some(&b'\n'));
            if ends_line {
                line += 1;
                line_start = i + 1;
            }
        }
        let mut text = &before[line_start..];
        if line == 1 {
            text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
        }
        // Between the CR and LF of a CRLF, the CR isn't a column.
        if source[offset..].starts_with('\n') {
            text = text.strip_suffix('\r').unwrap_or(text);
        }
        Position(to_u32(line), to_u32(text.chars().count() + 1))
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

impl Fix {
    /// Checks the rules for a fix's edits: each edit's `start` is not after its `end`, edits to
    /// the same file never overlap, and no two insertions share a position. `span_file` is the
    /// file of an edit that names none. Positions all refer to the text before any edit, so a
    /// tool can apply the edits as if all at once.
    pub fn check(&self, span_file: &str) -> Result<(), String> {
        let mut edits: Vec<(&str, &Edit)> = self
            .edits
            .iter()
            .map(|edit| (edit.file.as_deref().unwrap_or(span_file), edit))
            .collect();
        for (file, edit) in &edits {
            if edit.start > edit.end {
                return Err(format!(
                    "edit in {file} starts at {:?}, after its end {:?}",
                    edit.start, edit.end
                ));
            }
        }
        edits.sort_by_key(|(file, edit)| (*file, edit.start, edit.end));
        for pair in edits.windows(2) {
            let ((file, a), (next_file, b)) = (pair[0], pair[1]);
            if file != next_file {
                continue;
            }
            // An insertion touching another edit could land on either side of its text.
            let insertion_touches = a.end == b.start && (a.start == a.end || b.start == b.end);
            if a.end > b.start || insertion_touches {
                return Err(format!(
                    "edits in {file} at {:?}-{:?} and {:?}-{:?} overlap; every edit in a fix \
                     refers to the original text, so edits must not overlap",
                    a.start, a.end, b.start, b.end
                ));
            }
        }
        Ok(())
    }
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
/// empty `text` deletes. Without `file`, the edit applies to the diagnostic's span file; an
/// edit at `[1, 1]` to a file that doesn't exist creates it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Edit {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
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

    fn span() -> Span {
        Span {
            file: "src/a.qnc".into(),
            start: Position(1, 1),
            end: Position(1, 2),
        }
    }

    fn edit(start: (u32, u32), end: (u32, u32), file: Option<&str>) -> Edit {
        Edit {
            file: file.map(str::to_string),
            start: Position(start.0, start.1),
            end: Position(end.0, end.1),
            text: String::new(),
        }
    }

    fn fix(edits: Vec<Edit>) -> Fix {
        Fix {
            description: "d".into(),
            applicability: Applicability::Safe,
            edits,
        }
    }

    #[test]
    fn promoted_warning_keeps_its_code() {
        let d = Diagnostic::new(Code::parse("W0003").unwrap(), Severity::Error, "x", span());
        let value = serde_json::to_value(&d).unwrap();
        assert_eq!(value["code"], "W0003");
        assert_eq!(value["severity"], "error");
        assert_eq!(value["fixes"], serde_json::json!([]));
    }

    #[test]
    fn deletion_and_file_edits_round_trip() {
        let deletion = r#"{"start":[9,14],"end":[9,20],"text":""}"#;
        let parsed: Edit = serde_json::from_str(deletion).unwrap();
        assert_eq!(parsed, edit((9, 14), (9, 20), None));
        assert_eq!(serde_json::to_string(&parsed).unwrap(), deletion);
        let create = r#"{"file":"cmd/app/main.qnc","start":[1,1],"end":[1,1],"text":""}"#;
        let parsed: Edit = serde_json::from_str(create).unwrap();
        assert_eq!(parsed.file.as_deref(), Some("cmd/app/main.qnc"));
        assert_eq!(serde_json::to_string(&parsed).unwrap(), create);
    }

    #[test]
    fn fix_check() {
        let ok = fix(vec![
            edit((3, 1), (3, 5), None),
            edit((3, 5), (3, 9), None),
            edit((3, 1), (3, 9), Some("src/b.qnc")),
            edit((1, 1), (1, 1), Some("cmd/app/main.qnc")),
        ]);
        assert_eq!(ok.check("src/a.qnc"), Ok(()));
        let overlap = fix(vec![
            edit((3, 5), (3, 12), None),
            edit((3, 1), (3, 9), None),
        ]);
        let err = overlap.check("src/a.qnc").unwrap_err();
        assert!(err.contains("overlap"), "{err}");
        let same_file = fix(vec![
            edit((3, 1), (3, 9), Some("src/a.qnc")),
            edit((3, 5), (3, 12), None),
        ]);
        assert!(same_file.check("src/a.qnc").is_err());
        let two_inserts = fix(vec![edit((2, 1), (2, 1), None), edit((2, 1), (2, 1), None)]);
        assert!(two_inserts.check("src/a.qnc").is_err());
        for at in [(3, 1), (3, 5)] {
            let edge = fix(vec![edit((3, 1), (3, 5), None), edit(at, at, None)]);
            assert!(edge.check("src/a.qnc").is_err(), "insertion at {at:?}");
        }
        let apart = fix(vec![edit((3, 1), (3, 5), None), edit((3, 6), (3, 6), None)]);
        assert_eq!(apart.check("src/a.qnc"), Ok(()));
        let backwards = fix(vec![edit((2, 5), (2, 1), None)]);
        assert!(
            backwards
                .check("src/a.qnc")
                .unwrap_err()
                .contains("after its end")
        );
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

    #[test]
    fn positions_over_line_endings_and_bom() {
        let crlf = "a\r\nbc";
        assert_eq!(Position::at(crlf, 1), Position(1, 2));
        assert_eq!(Position::at(crlf, 2), Position(1, 2));
        assert_eq!(Position::at(crlf, 3), Position(2, 1));
        assert_eq!(Position::at(crlf, 5), Position(2, 3));
        let lone_cr = "a\rbc";
        assert_eq!(Position::at(lone_cr, 1), Position(1, 2));
        assert_eq!(Position::at(lone_cr, 2), Position(2, 1));
        assert_eq!(Position::at(lone_cr, 4), Position(2, 3));
        assert_eq!(Position::at("a\tb", 2), Position(1, 3));
        let bom = "\u{FEFF}ab\ncd";
        assert_eq!(Position::at(bom, 3), Position(1, 1));
        assert_eq!(Position::at(bom, 4), Position(1, 2));
        assert_eq!(Position::at("a\n", 2), Position(2, 1));
    }
}
