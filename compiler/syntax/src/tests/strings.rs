//! One-line strings: their tokens, and E0004, E0006 and E0007. Multi-line strings are in
//! `multiline.rs`, and raw, bytes and tagged literals in `prefixed.rs`.

use super::errors::errors;
use crate::literal::{bytes_value, char_body_value, char_value, string_value};
use crate::{SyntaxKind, lex_str};
use quince_diagnostics::Applicability;

/// The tokens, as `Kind:text`, separated by spaces.
pub(super) fn tokens(text: &str) -> String {
    let lexed = lex_str(text);
    let shown: Vec<String> = (lexed.ranges())
        .map(|(kind, range)| format!("{kind:?}:{}", &text[range]))
        .collect();
    shown.join(" ")
}

pub(super) fn message(text: &str) -> String {
    let lexed = lex_str(text);
    assert_eq!(lexed.errors.len(), 1, "{text}: {:?}", lexed.errors);
    lexed.errors[0].message.clone()
}

/// Every text of up to four characters from those that matter to literals keeps the
/// invariants, reports its errors in order, never puts two errors on one span, decodes, and
/// has no message that holds a control or bidirectional-format character. A safe E0006, E0007,
/// E0011 or E0015 fix, or E0009's fix, removes its error, leaves the others as they were,
/// moved by its edits, and keeps the bytes of every bytes literal. E0009's fix makes a string
/// whose value is the char's body, and E0015's keeps the value of every string and char.
#[test]
fn every_short_text() {
    let alphabet: Vec<char> = "\"{}\\ux.0arb# '\u{e9}\u{7}\u{202E}\n\r".chars().collect();
    let mut texts = vec![String::new()];
    for _ in 0..4 {
        let longer: Vec<String> = texts
            .iter()
            .flat_map(|t| alphabet.iter().map(move |c| format!("{t}{c}")))
            .collect();
        for text in &longer {
            errors(text);
            let lexed = lex_str(text);
            let mut spans: Vec<_> = lexed.errors.iter().map(|e| e.range.clone()).collect();
            let starts: Vec<_> = spans.iter().map(|r| r.start).collect();
            assert!(starts.is_sorted(), "{text:?}: {:?}", lexed.errors);
            spans.sort_by_key(|r| (r.start, r.end));
            spans.dedup();
            assert_eq!(
                spans.len(),
                lexed.errors.len(),
                "{text:?}: {:?}",
                lexed.errors
            );
            for (kind, range) in lexed.ranges() {
                match kind {
                    SyntaxKind::StringText => drop(string_value(&text[range])),
                    SyntaxKind::Char => drop(char_value(&text[range])),
                    _ => {}
                }
            }
            for error in &lexed.errors {
                assert!(
                    !error
                        .message
                        .contains(|c: char| c.is_control() || is_bidi(c)),
                    "{text:?}: {error:?}"
                );
            }
            for (i, error) in lexed.errors.iter().enumerate() {
                let char_fix = error.code == "E0009";
                let string_code = ["E0006", "E0007", "E0011", "E0015"].contains(&error.code);
                let Some(fix) = error.fixes.first().filter(|_| string_code || char_fix) else {
                    continue;
                };
                if !char_fix {
                    assert_eq!(fix.applicability, Applicability::Safe, "{text:?}");
                }
                let mut fixed = text.clone();
                for (range, with) in fix.edits.iter().rev() {
                    fixed.replace_range(range.start as usize..range.end as usize, with);
                }
                // Where an offset after the edits was before them.
                let moved = |at: u32| {
                    let shift: i64 = (fix.edits.iter())
                        .filter(|(range, _)| range.end <= at)
                        .map(|(range, with)| with.len() as i64 - i64::from(range.end - range.start))
                        .sum();
                    (i64::from(at) + shift) as u32
                };
                let others: Vec<_> = (lexed.errors.iter().enumerate())
                    .filter(|&(j, _)| j != i)
                    .map(|(_, e)| (e.code, moved(e.range.start)..moved(e.range.end)))
                    .collect();
                let after: Vec<_> = (lex_str(&fixed).errors.iter())
                    .map(|e| (e.code, e.range.clone()))
                    .collect();
                assert_eq!(after, others, "{text:?} fixed to {fixed:?}");
                assert_eq!(
                    all_bytes(&fixed),
                    all_bytes(text),
                    "{text:?} fixed to {fixed:?}"
                );
                if error.code == "E0015" {
                    assert_eq!(
                        all_values(&fixed),
                        all_values(text),
                        "{text:?} fixed to {fixed:?}"
                    );
                }
                if char_fix {
                    let (start, end) = (error.range.start as usize, error.range.end as usize);
                    let body = char_body_value(&text[start + 1..end - 1]);
                    assert_eq!(
                        string_at(&fixed, moved(error.range.start) as usize),
                        Some(body),
                        "{text:?} fixed to {fixed:?}"
                    );
                }
            }
        }
        texts = longer;
    }
}

/// The value of the `"` string that starts at `at` in `text`, its `StringText` decoded, or
/// `None` when no such string starts there, or it isn't closed or interpolates.
fn string_at(text: &str, at: usize) -> Option<String> {
    let mut value = None;
    for (kind, range) in lex_str(text).ranges() {
        match (kind, &mut value) {
            (SyntaxKind::StringStart, None) if range.start == at => {
                if &text[range] != "\"" {
                    return None;
                }
                value = Some(String::new());
            }
            (SyntaxKind::StringText, Some(v)) => v.push_str(&string_value(&text[range])),
            (SyntaxKind::StringEnd, Some(_)) => return value,
            (_, Some(_)) => return None,
            _ => {}
        }
    }
    None
}

fn is_bidi(c: char) -> bool {
    ('\u{202A}'..='\u{202E}').contains(&c) || ('\u{2066}'..='\u{2069}').contains(&c)
}

/// Each `"` or `"""` string in `text`, as its tokens with each `StringText` decoded, and each
/// char's value.
fn all_values(text: &str) -> Vec<String> {
    let mut values: Vec<String> = Vec::new();
    let mut in_string = false;
    for (kind, range) in lex_str(text).ranges() {
        let token = &text[range];
        match kind {
            SyntaxKind::StringStart => {
                in_string = token.starts_with('"');
                if in_string {
                    values.push(String::new());
                }
            }
            SyntaxKind::Char => values.push(format!("{:?}", char_value(token))),
            SyntaxKind::StringEnd | SyntaxKind::Newline => in_string = false,
            _ if in_string => {
                let value = values.last_mut().expect("after its start");
                match kind {
                    SyntaxKind::StringText => value.push_str(&string_value(token)),
                    _ => value.push_str(token),
                }
            }
            _ => {}
        }
    }
    values
}

/// The values of the bytes literals in `text`, each its `StringText` decoded.
fn all_bytes(text: &str) -> Vec<Vec<u8>> {
    let mut values = Vec::new();
    let mut in_bytes = false;
    for (kind, range) in lex_str(text).ranges() {
        match kind {
            SyntaxKind::StringStart => {
                in_bytes = text[range].starts_with('b');
                if in_bytes {
                    values.push(Vec::new());
                }
            }
            SyntaxKind::StringText if in_bytes => {
                let value = bytes_value(&text[range]);
                values.last_mut().expect("after its start").extend(value);
            }
            SyntaxKind::StringText => {}
            _ => in_bytes = false,
        }
    }
    values
}

#[test]
fn string_shapes() {
    assert_eq!(
        tokens(r#""a \n b""#),
        r#"StringStart:" StringText:a \n b StringEnd:""#
    );
    assert_eq!(tokens(r#""""#), r#"StringStart:" StringEnd:""#);
    assert_eq!(
        tokens(r#""hi {name}!""#),
        r#"StringStart:" StringText:hi  InterpStart:{ Ident:name InterpEnd:} StringText:! StringEnd:""#
    );
    assert_eq!(
        tokens(r#""{self.email}{id.0.1}{MAX}""#),
        "StringStart:\" InterpStart:{ SelfKw:self Dot:. Ident:email InterpEnd:} InterpStart:{ \
         Ident:id Dot:. Int:0 Dot:. Int:1 InterpEnd:} InterpStart:{ Ident:MAX InterpEnd:} \
         StringEnd:\""
    );
    // A lone `}`, an escaped `{` and a `$` are text.
    assert_eq!(
        tokens(r#""} \{x} ${a}""#),
        r#"StringStart:" StringText:} \{x} $ InterpStart:{ Ident:a InterpEnd:} StringEnd:""#
    );
    // A type name such as `Config` can start a path here: the resolver reports it.
    assert_eq!(errors(r#""{Config.MAX}""#), Vec::<String>::new());
    // A malformed tuple position is an E0008, with no E0007.
    assert_eq!(errors(r#""{t.01}""#), ["E0008 4..6"]);
}

#[test]
fn unclosed_strings() {
    assert_eq!(errors("\"ab\nc"), [r#"E0004 0..1 => "\"ab\"\nc""#]);
    assert_eq!(errors("\"ab\r\n"), [r#"E0004 0..1 => "\"ab\"\r\n""#]);
    assert_eq!(
        errors("x = \"ab {n}"),
        [r#"E0004 4..5 => "x = \"ab {n}\"""#]
    );
    assert_eq!(
        tokens("\"ab\nc"),
        r#"StringStart:" StringText:ab Newline:
 Ident:c"#
    );
    let error = &lex_str("\"ab").errors[0];
    assert_eq!(error.fixes[0].applicability, Applicability::NeedsReview);
    // A `"` after the `\` would be escaped, so E0004 has no fix, and E0006 reports the `\`.
    assert_eq!(errors("\"ab\\\n"), ["E0004 0..1", "E0006 3..4"]);
    // A lone CR ends the string, and only E0013 is reported, with no fix.
    assert_eq!(errors("\"ab\rc"), ["E0013 3..4"]);
}

#[test]
fn escapes() {
    assert_eq!(
        errors(r#""\n\t\r\0\\\"\{\u{1F600}\u{10FFFF}\u{0}""#),
        Vec::<String>::new()
    );
    assert_eq!(errors(r#""it\'s""#), [r#"E0006 3..5 => "\"it's\"""#]);
    assert_eq!(errors(r#""a\}""#), [r#"E0006 2..4 => "\"a}\"""#]);
    assert_eq!(errors(r#""\q""#), ["E0006 1..3"]);
    assert_eq!(errors("\"\\\u{E9}\""), ["E0006 1..4"]);
    assert_eq!(errors(r#""\x41\x4""#), ["E0006 1..3", "E0006 5..7"]);
    assert_eq!(
        errors(r#""\u{} \u{D800} \u{110000} \u{1234567} \u{12G} \u{12" "#),
        [
            "E0006 1..5",
            "E0006 6..14",
            "E0006 15..25",
            "E0006 26..37",
            "E0006 38..45",
            "E0006 46..51"
        ]
    );
    assert_eq!(errors(r#""\u0041 \u""#), ["E0006 1..3", "E0006 8..10"]);
    // One error with no fix: deleting the backslash would make a valid escape, changing the value.
    assert_eq!(errors(r#""\u{ab\}""#), ["E0006 1..8"]);
    // Right after a `\u` with no closing `}`, deleting a backslash would join the two escapes
    // into one, so that escape has no fix. The `\}` after the `\'` keeps its fix.
    assert_eq!(
        errors(r#""\u{AB\'\}""#),
        [
            "E0006 1..6",
            "E0006 6..8",
            r#"E0006 8..10 => "\"\\u{AB\\'}\"""#
        ]
    );
    assert_eq!(errors(r#""\u{4\'1}""#), ["E0006 1..5", "E0006 5..7"]);
    assert_eq!(errors(r#""\u\}""#), ["E0006 1..3", "E0006 3..5"]);
    assert_eq!(
        lex_str(r#""\'""#).errors[0].fixes[0].applicability,
        Applicability::Safe
    );
}

#[test]
fn escape_messages() {
    assert!(message(r#""\q""#).starts_with("`\\q` isn't an escape: a string's escapes are"));
    assert!(message(r#""\x41""#).starts_with("`\\x` isn't an escape: a string's escapes are"));
    assert!(message(r#""\'""#).starts_with("`'` needs no escape in a string"));
    assert!(message(r#""\}""#).contains("a lone `}` is text"));
    assert!(message(r#""\u0041""#).starts_with("a `\\u` escape takes one to six hex digits"));
    assert!(message(r#""\u{0000041}""#).ends_with("write `\\u{41}`"));
    assert!(message(r#""\u{D800}""#).starts_with("`\\u{D800}` is a surrogate"));
    assert!(message(r#""\u{110000}""#).contains("the largest Unicode scalar value"));
    assert!(message(r#""\u{12""#).starts_with("this `\\u{` escape needs a closing `}`"));
    assert!(message(r#""\u{ab\}""#).ends_with("takes no backslash: write `\\u{AB}`"));
    assert!(message(r#""\u{D800\}""#).contains("one to six hex digits naming"));
    assert_eq!(
        lex_str("\"a\\").errors[1].message,
        "a `\\` at the end of a line escapes nothing: a backslash is written `\\\\`"
    );
}

#[test]
fn broken_interpolations() {
    // Each is one `Error` token, and the string goes on after it.
    assert_eq!(
        tokens(r#""{a + b} x""#),
        r#"StringStart:" Error:{a + b} StringText: x StringEnd:""#
    );
    assert_eq!(tokens(r#""{""#), r#"StringStart:" Error:{ StringEnd:""#);
    assert_eq!(
        tokens("\"{a\nb"),
        "StringStart:\" Error:{a Newline:\n Ident:b"
    );
    assert_eq!(errors(r#""{}""#), ["E0007 1..3"]);
    // Escapes don't count in the braces, so the `"` of `\"` still ends the string.
    assert_eq!(
        tokens(r#""{\"}""#),
        r#"StringStart:" Error:{\ StringEnd:" RBrace:} StringStart:""#
    );
    assert_eq!(
        errors(r#""{a"#),
        [r#"E0004 0..1 => "\"{a\"""#, "E0007 1..3"]
    );
    for text in [
        r#""{0}""#,
        r#""{a.}""#,
        r#""{.a}""#,
        r#""{a..b}""#,
        r#""{_}""#,
        r#""{fn}""#,
        r#""{a.self}""#,
        r#""{Self}""#,
        r#""{a.b()}""#,
        r#""{a b}""#,
        r#""{x:.2}""#,
    ] {
        assert_eq!(
            lex_str(text)
                .errors
                .iter()
                .map(|e| e.code)
                .collect::<Vec<_>>(),
            ["E0007"],
            "{text}"
        );
    }
}

/// Spaces that touch a brace or a `.` are deleted by a safe fix. Spaces between two words
/// aren't, since deleting them would join the words into one name.
#[test]
fn spaces_in_braces() {
    assert_eq!(errors(r#""{ a }""#), [r#"E0007 1..6 => "\"{a}\"""#]);
    assert_eq!(
        errors("\"{\tuser . name}\""),
        [r#"E0007 1..15 => "\"{user.name}\"""#]
    );
    assert_eq!(errors(r#""{a b}""#), ["E0007 1..6"]);
    assert_eq!(errors(r#""{ a + b }""#), ["E0007 1..10"]);
    // Deleting the space would trade the E0007 for an E0008 on `01`.
    assert_eq!(errors(r#""{a.01 }""#), ["E0007 1..8"]);
    assert_eq!(
        message(r#""{ a }""#),
        "an interpolation has no spaces: write `{a}`"
    );
}

/// Apart from spaces, which have a fix, there is one message for a closed interpolation that
/// isn't a path, whatever is in it, and one for an unclosed one.
#[test]
fn interpolation_messages() {
    assert_eq!(
        message(r#""{}""#),
        "`{}` needs a name inside it, as in `{name}`; a literal brace is written `\\{`"
    );
    let not_a_path = "only a path can go in braces: a name, `self` or a constant, then any number \
                      of `.` and a field name or tuple position, as in `{user.name}` or `{id.0}`; \
                      a literal brace is written `\\{`";
    for text in [
        r#""{a + b}""#,
        r#""{Self}""#,
        r#""{x:.2}""#,
        r#""{0}""#,
        r#""{a b}""#,
        r#""{a.}""#,
    ] {
        assert_eq!(message(text), not_a_path, "{text}");
    }
    let unclosed = "this `{` is never closed: an interpolation is a path in braces, as in \
                    `{user.name}`; a literal brace is written `\\{`";
    for text in [r#""{name""#, r#""{""#, r#""{a + b""#] {
        assert_eq!(message(text), unclosed, "{text}");
    }
}

/// A test's name doesn't interpolate, so `{` is text in it, but its escapes are checked.
#[test]
fn test_names() {
    assert_eq!(
        tokens(r#"test  "{} { a } {name}" {"#),
        r#"TestKw:test Whitespace:   StringStart:" StringText:{} { a } {name} StringEnd:" Whitespace:  LBrace:{"#
    );
    assert_eq!(errors(r#"test "\{ \q" {"#), ["E0006 9..11"]);
    // Only spaces are skipped: after a comment or another token, a string interpolates.
    assert_eq!(errors("test // c\n\"{}\""), ["E0007 11..13"]);
    assert_eq!(errors(r#"test(x, "{}")"#), ["E0007 9..11"]);
    // Messages about interpolation don't apply in a test name.
    assert_eq!(message(r#"test "\}" {"#), "`}` needs no escape in a string");
}
