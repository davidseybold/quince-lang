//! Error-code registry, JSON diagnostic format, fixes and `explain` text for Quince.
//!
//! The JSON shape is spec section 8. What the spec leaves open (positions, edits, code format)
//! is fixed in this crate's `README.md`.

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
    /// just after the last character. For more than one offset, build a [`LineIndex`] once.
    pub fn at(source: &str, byte_offset: usize) -> Position {
        LineIndex::new(source).position(byte_offset)
    }
}

/// Where each line of a source text starts, found once, to convert between byte offsets and
/// positions. Lines and columns count as [`Position`] says.
#[derive(Debug, Clone)]
pub struct LineIndex<'a> {
    source: &'a str,
    /// The byte offset each line starts at. Line 1 starts after a leading byte order mark.
    line_starts: Vec<usize>,
}

impl<'a> LineIndex<'a> {
    pub fn new(source: &'a str) -> Self {
        let bom_len = if source.starts_with('\u{FEFF}') {
            '\u{FEFF}'.len_utf8()
        } else {
            0
        };
        let mut line_starts = vec![bom_len];
        let bytes = source.as_bytes();
        for (i, &b) in bytes.iter().enumerate() {
            if b == b'\n' || (b == b'\r' && bytes.get(i + 1) != Some(&b'\n')) {
                line_starts.push(i + 1);
            }
        }
        LineIndex {
            source,
            line_starts,
        }
    }

    /// The number of lines. Text that ends with a line break has an empty last line after it,
    /// where the end of input is.
    pub fn line_count(&self) -> u32 {
        to_u32(self.line_starts.len())
    }

    /// The text of a line without its line break, or `None` when there is no such line. Line 1
    /// leaves out a leading byte order mark, as its columns do.
    pub fn line(&self, line: u32) -> Option<&'a str> {
        let index = usize::try_from(line).ok()?.checked_sub(1)?;
        let start = *self.line_starts.get(index)?;
        let end = self
            .line_starts
            .get(index + 1)
            .copied()
            .unwrap_or(self.source.len());
        // Only a line break can end a line before the last, and the last line has none.
        let text = &self.source[start..end];
        let text = text.strip_suffix('\n').unwrap_or(text);
        Some(text.strip_suffix('\r').unwrap_or(text))
    }

    /// Every line with its number, in order.
    pub fn lines(&self) -> impl Iterator<Item = (u32, &'a str)> + '_ {
        (1..=self.line_count()).map(|n| (n, self.line(n).expect("n is a line number")))
    }

    /// The position of a byte offset. An offset past the end gives the position just after the
    /// last character, and an offset inside a character gives that character's position.
    pub fn position(&self, byte_offset: usize) -> Position {
        let offset = self
            .source
            .floor_char_boundary(byte_offset.min(self.source.len()));
        let index = self
            .line_starts
            .partition_point(|&start| start <= offset)
            .saturating_sub(1);
        // An offset inside the byte order mark is line 1's first position.
        let start = self.line_starts[index].min(offset);
        let mut text = &self.source[start..offset];
        // Between the CR and LF of a CR LF, the CR isn't a column.
        if self.source[offset..].starts_with('\n') {
            text = text.strip_suffix('\r').unwrap_or(text);
        }
        Position(to_u32(index + 1), to_u32(text.chars().count() + 1))
    }

    /// The byte offset of a position, or `None` when the text has no such position. A line's
    /// columns run from 1 to one past its last character, which is the offset of its line break
    /// (the CR of a CR LF).
    pub fn offset(&self, position: Position) -> Option<usize> {
        let text = self.line(position.line())?;
        let column = usize::try_from(position.column()).ok()?.checked_sub(1)?;
        let within = text
            .char_indices()
            .map(|(i, _)| i)
            .chain([text.len()])
            .nth(column)?;
        let line_start = self.line_starts[usize::try_from(position.line()).ok()? - 1];
        Some(line_start + within)
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

    /// Checks that every edit's positions exist in the text it edits: the rules that need the
    /// source. `line_index_for` gives the index of an edited file that exists, and `None` for one
    /// that doesn't, which an edit creates by inserting at `[1, 1]`. `span_file` is the file of an
    /// edit that names none.
    pub fn check_positions<'i>(
        &self,
        span_file: &str,
        line_index_for: impl Fn(&str) -> Option<&'i LineIndex<'i>>,
    ) -> Result<(), String> {
        for edit in &self.edits {
            let file = edit.file.as_deref().unwrap_or(span_file);
            let Some(index) = line_index_for(file) else {
                let first = Position(1, 1);
                if (edit.start, edit.end) != (first, first) {
                    return Err(format!(
                        "edit in {file} is at {:?}-{:?}, but {file} doesn't exist; an edit that \
                         creates a file inserts at {first:?}",
                        edit.start, edit.end
                    ));
                }
                continue;
            };
            for (verb, position) in [("starts", edit.start), ("ends", edit.end)] {
                if index.offset(position).is_some() {
                    continue;
                }
                let line = position.line();
                let why = match index.line(line) {
                    Some(text) => {
                        format!("line {line} has columns 1 to {}", text.chars().count() + 1)
                    }
                    None => format!("the file has lines 1 to {}", index.line_count()),
                };
                return Err(format!(
                    "edit in {file} {verb} at {position:?}, which isn't in the file: {why}"
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

    /// A diagnostic with a fix, as a JSON fixture.
    pub(crate) fn example_text() -> String {
        include_str!("../testdata/example.json").to_string()
    }

    #[test]
    fn example_round_trips() {
        let text = example_text();
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
        let text = example_text().replacen("\"docs\"", "\"notes\": [], \"docs\"", 1);
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

    #[test]
    fn line_index_splits_lines_as_positions_do() {
        let lines = |source| {
            let index = LineIndex::new(source);
            let lines: Vec<_> = index.lines().map(|(_, text)| text).collect();
            assert_eq!(lines.len(), index.line_count() as usize);
            lines
        };
        assert_eq!(lines("a\nb\r\nc\rd"), ["a", "b", "c", "d"]);
        assert_eq!(lines("a\n"), ["a", ""]);
        assert_eq!(lines("a\r\r\n"), ["a", "", ""]);
        assert_eq!(lines(""), [""]);
        assert_eq!(lines("\u{FEFF}// a\n"), ["// a", ""]);
        assert_eq!(LineIndex::new("a").line(0), None);
        assert_eq!(LineIndex::new("a").line(2), None);
    }

    /// Every character boundary maps to a position and back. The two exceptions are offsets
    /// that share a position with an earlier one: the LF of a CR LF, and the start of a byte
    /// order mark.
    #[test]
    fn offsets_and_positions_round_trip() {
        for source in [
            "let café = 1\n",
            "a\r\nbc\rd\n\ne",
            "\u{FEFF}x\r\n",
            "\u{FEFF}",
            "",
            "\t\u{1F600}\r",
        ] {
            let index = LineIndex::new(source);
            for offset in (0..=source.len()).filter(|&o| source.is_char_boundary(o)) {
                let position = index.position(offset);
                assert_eq!(
                    position,
                    Position::at(source, offset),
                    "{source:?} at {offset}"
                );
                let expected =
                    if source[..offset].ends_with('\r') && source[offset..].starts_with('\n') {
                        offset - 1
                    } else if offset == 0 && source.starts_with('\u{FEFF}') {
                        3
                    } else {
                        offset
                    };
                assert_eq!(
                    index.offset(position),
                    Some(expected),
                    "{source:?} at {offset}"
                );
            }
        }
    }

    #[test]
    fn positions_that_dont_exist() {
        let index = LineIndex::new("ab\r\ncafé\n");
        assert_eq!(index.offset(Position(1, 3)), Some(2));
        assert_eq!(index.offset(Position(2, 5)), Some(9));
        assert_eq!(index.offset(Position(3, 1)), Some(10));
        for missing in [(1, 4), (2, 6), (3, 2), (4, 1), (0, 1), (1, 0)] {
            assert_eq!(
                index.offset(Position(missing.0, missing.1)),
                None,
                "{missing:?}"
            );
        }
    }

    #[test]
    fn fix_positions_must_exist() {
        let source = LineIndex::new("let x = 1\r\nlet y = 2");
        let line_index_for = |file: &str| (file == "src/a.qnc").then_some(&source);
        let check = |edits| fix(edits).check_positions("src/a.qnc", line_index_for);
        assert_eq!(check(vec![edit((1, 5), (1, 10), None)]), Ok(()));
        assert_eq!(check(vec![edit((2, 10), (2, 10), None)]), Ok(()));
        let err = check(vec![edit((1, 5), (1, 11), None)]).unwrap_err();
        assert_eq!(
            err,
            "edit in src/a.qnc ends at Position(1, 11), which isn't in the file: line 1 has \
             columns 1 to 10"
        );
        let err = check(vec![edit((3, 1), (3, 1), None)]).unwrap_err();
        assert!(err.ends_with("the file has lines 1 to 2"), "{err}");
        assert_eq!(
            check(vec![edit((1, 1), (1, 1), Some("src/new.qnc"))]),
            Ok(())
        );
        let err = check(vec![edit((2, 1), (2, 1), Some("src/new.qnc"))]).unwrap_err();
        assert!(err.contains("src/new.qnc doesn't exist"), "{err}");
    }
}
