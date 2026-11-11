//! Lexer errors and their fixes, and decoding.

use super::check_invariants;
use crate::{Lexed, SyntaxError, SyntaxKind, decode_source, lex, lex_str};
use quince_diagnostics::{Applicability, Code, Position, lookup};

/// Each error as `code start..end`, then its fixed text when it has one fix.
pub(super) fn errors(text: &str) -> Vec<String> {
    let lexed = lex_str(text);
    check_error_ranges(text, &lexed);
    lexed.errors.iter().map(|e| describe(text, e)).collect()
}

/// The token invariants, and every error range lies inside the text on character boundaries.
pub(super) fn check_error_ranges(text: &str, lexed: &Lexed) {
    check_invariants(text, lexed);
    for e in &lexed.errors {
        let (start, end) = (e.range.start as usize, e.range.end as usize);
        assert!(start < end && end <= text.len(), "{text:?}: {e:?}");
        assert!(
            text.is_char_boundary(start) && text.is_char_boundary(end),
            "{text:?}: {e:?}"
        );
    }
}

pub(super) fn describe(text: &str, e: &SyntaxError) -> String {
    let mut out = format!("{} {:?}", e.code, e.range);
    if let [fix] = e.fixes.as_slice() {
        let mut fixed = text.to_string();
        for (range, with) in fix.edits.iter().rev() {
            fixed.replace_range(range.start as usize..range.end as usize, with);
        }
        out += &format!(" => {fixed:?}");
    }
    out
}

#[test]
fn block_comments() {
    assert_eq!(errors("  /* a */\n"), [r#"E0003 2..9 => "  // a\n""#]);
    assert_eq!(
        errors("  /*\n   * one\n   *\n   * two\n   */\nx"),
        [r#"E0003 2..33 => "  // one\n  //\n  // two\nx""#]
    );
    assert_eq!(errors("/**/"), [r#"E0003 0..4 => "//""#]);
    assert_eq!(errors("x /* a */"), ["E0003 2..9"]);
    assert_eq!(errors("/* a */ x"), ["E0003 0..7"]);
    let unclosed = lex_str("x /* a\ny");
    assert_eq!(unclosed.errors[0].range, 2..4);
    assert!(unclosed.errors[0].message.contains("never closed"));
    assert_eq!(
        unclosed.tokens.last().unwrap().kind,
        SyntaxKind::BlockComment
    );
    assert_eq!(
        errors("/* a\rb */ x"),
        ["E0003 0..9", r#"E0013 4..5 => "/* a\nb */ x""#]
    );
    // A `//` line would end at the lone CR and turn `b` into code, so only E0013 has a fix.
    assert_eq!(
        errors("/* a\rb */\nx"),
        ["E0003 0..9", r#"E0013 4..5 => "/* a\nb */\nx""#]
    );
    assert_eq!(errors("/* a\r\nb */"), [r#"E0003 0..10 => "// a\r\n// b""#]);
}

#[test]
fn lone_carriage_returns_and_late_byte_order_marks() {
    assert_eq!(errors("a\r\nb\rc"), [r#"E0013 4..5 => "a\r\nb\nc""#]);
    // A line feed after the first CR would make CR LF, one line ending in place of two.
    assert_eq!(
        errors("a\r\rb"),
        [r#"E0013 1..2 => "a\n\rb""#, "E0013 2..3"]
    );
    assert_eq!(errors("\u{FEFF}a\u{FEFF}"), ["E0001 4..7"]);
    // A CR that ends an unclosed literal gets no fix.
    assert_eq!(errors("\"a\rb"), ["E0013 2..3"]);
    assert_eq!(errors("'a\rb"), ["E0013 2..3"]);
}

#[test]
fn semicolons() {
    assert_eq!(
        errors("a; // x\nb ;"),
        [
            r#"E0002 1..2 => "a // x\nb ;""#,
            r#"E0002 10..11 => "a; // x\nb ""#
        ]
    );
    assert_eq!(errors("a; b"), ["E0002 1..2"]);
    // Deleting it would make `///`, a doc comment. It still ends the line.
    assert_eq!(errors("x /;// c"), ["E0002 3..4"]);
    assert!(
        lex_str("x /;// c").errors[0]
            .message
            .contains("ends at the line break")
    );
}

/// `a;` then `.b()` is two statements, and deleting the `;` makes one, so that fix needs review.
#[test]
fn semicolon_before_a_line_that_starts_with_a_dot() {
    let applicability = |text| lex_str(text).errors[0].fixes[0].applicability;
    assert_eq!(errors("a;\n.b()"), [r#"E0002 1..2 => "a\n.b()""#]);
    assert_eq!(applicability("a;\n.b()"), Applicability::NeedsReview);
    assert_eq!(
        applicability("a; // x\n\n  // y\r\n\t.b()"),
        Applicability::NeedsReview
    );
    assert_eq!(
        applicability("a;\n/* c\n */ .b()"),
        Applicability::NeedsReview
    );
    assert_eq!(applicability("a;\n/* c"), Applicability::Safe);
    assert_eq!(applicability("a;\nb.c()"), Applicability::Safe);
    assert_eq!(applicability("a;\n..b"), Applicability::Safe);
    assert_eq!(applicability("a;"), Applicability::Safe);
}

#[test]
fn unknown_characters() {
    assert_eq!(errors("a $$ b"), ["E0001 2..4"]);
    assert_eq!(errors("caf\u{E9}"), ["E0001 3..5"]);
    assert_eq!(errors("\u{1F600}\u{1F600}\u{1F600}"), ["E0001 0..12"]);
    assert_eq!(
        errors("a\u{A0}\u{A0}b\u{C}c"),
        [
            r#"E0001 1..5 => "a  b\u{c}c""#,
            r#"E0001 6..7 => "a\u{a0}\u{a0}b c""#
        ]
    );
    let message = |text| lex_str(text).errors[0].message.clone();
    assert_eq!(
        message("# x"),
        "`#` can't appear here: it doesn't start any Quince token"
    );
    assert!(message("`x`").starts_with("a backtick can't appear here"));
    assert!(message("$$").starts_with("`$` and the character after it can't"));
    assert!(message("$$$").starts_with("`$` and the 2 characters after it"));
    // Non-ASCII is fine in comments and literals.
    assert_eq!(
        errors("// caf\u{E9}\n\"caf\u{E9}\" '\u{E9}'"),
        Vec::<String>::new()
    );
}

#[test]
fn replaced_bytes_are_errors_without_a_diagnostic() {
    // The first replacement is in a comment, so it is part of the comment token.
    let decoded = decode_source(b"//\xFF\na \xFF$ \xFF\n\xEF\xBF\xBD").unwrap();
    let lexed = lex(&decoded);
    check_error_ranges(&decoded.text, &lexed);
    let kinds: Vec<_> = lexed.tokens.iter().map(|t| t.kind).collect();
    use SyntaxKind::*;
    assert_eq!(
        kinds,
        [
            Comment, Newline, Ident, Whitespace, Error, Error, Whitespace, Error, Newline, Error
        ]
    );
    let errors: Vec<_> = lexed
        .errors
        .iter()
        .map(|e| describe(&decoded.text, e))
        .collect();
    // One E0014 for each of lines 1 and 2; the `$` and the real U+FFFD on line 3 are E0001s.
    assert_eq!(
        errors,
        ["E0014 2..5", "E0014 8..11", "E0001 11..12", "E0001 17..20"]
    );
}

/// A lossy text gets no fixes, even far from its replacements, and even one, as E0009's does,
/// that would rewrite a replacement.
#[test]
fn lossy_text_has_no_fixes() {
    assert_eq!(
        errors("a;\n'xb'"),
        [
            r#"E0002 1..2 => "a\n'xb'""#,
            r#"E0009 3..7 => "a;\n\"xb\"""#
        ]
    );
    let decoded = decode_source(b"a;\n'\xFFb'").unwrap();
    let lexed = lex(&decoded);
    let errors: Vec<_> = (lexed.errors.iter())
        .map(|e| describe(&decoded.text, e))
        .collect();
    assert_eq!(errors, ["E0014 4..7", "E0002 1..2", "E0009 3..9"]);
}

#[test]
fn diagnostics_have_positions_and_fixes() {
    let text = "let a = 1;\r\nb\u{A0}";
    let [semicolon, space] = &lex_str(text).diagnostics(text, "src/a.qnc")[..] else {
        panic!("expected two diagnostics");
    };
    let edit = &semicolon.fixes[0].edits[0];
    assert_eq!((edit.start, edit.end), (Position(1, 10), Position(1, 11)));
    assert_eq!(
        (space.code.as_str(), space.span.start),
        ("E0001", Position(2, 2))
    );
}

#[test]
fn every_code_is_registered() {
    for code in crate::codes::all() {
        let code = Code::parse(code).unwrap();
        assert!(lookup(&code).is_some(), "{code} isn't registered");
    }
}
