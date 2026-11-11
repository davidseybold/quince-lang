//! One-line strings: their tokens, the fallback for forms not lexed yet, and E0004, E0006 and
//! E0007.

use super::errors::errors;
use crate::literal::string_value;
use crate::{SyntaxKind, lex_str};
use quince_diagnostics::Applicability;

/// The tokens, as `Kind:text`, separated by spaces.
fn tokens(text: &str) -> String {
    let lexed = lex_str(text);
    let shown: Vec<String> = (lexed.ranges())
        .map(|(kind, range)| format!("{kind:?}:{}", &text[range]))
        .collect();
    shown.join(" ")
}

fn message(text: &str) -> String {
    let lexed = lex_str(text);
    assert_eq!(lexed.errors.len(), 1, "{text}: {:?}", lexed.errors);
    lexed.errors[0].message.clone()
}

/// Every text of up to four characters from those that matter to strings keeps the
/// invariants, never puts two errors on one span, and decodes. A safe E0006 or E0007 fix
/// removes its error and adds none.
#[test]
fn every_short_text() {
    let alphabet: Vec<char> = "\"{}\\ux.0a '\n\r".chars().collect();
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
            spans.sort_by_key(|r| (r.start, r.end));
            spans.dedup();
            assert_eq!(
                spans.len(),
                lexed.errors.len(),
                "{text:?}: {:?}",
                lexed.errors
            );
            for (kind, range) in lexed.ranges() {
                if kind == SyntaxKind::StringText {
                    string_value(&text[range]);
                }
            }
            for error in &lexed.errors {
                let string_code = ["E0006", "E0007"].contains(&error.code);
                let Some(fix) = error.fixes.first().filter(|_| string_code) else {
                    continue;
                };
                assert_eq!(fix.applicability, Applicability::Safe, "{text:?}");
                let mut fixed = text.clone();
                for (range, with) in fix.edits.iter().rev() {
                    fixed.replace_range(range.start as usize..range.end as usize, with);
                }
                assert_eq!(
                    lex_str(&fixed).errors.len(),
                    lexed.errors.len() - 1,
                    "{text:?} fixed to {fixed:?}"
                );
            }
        }
        texts = longer;
    }
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
    assert_eq!(errors(r#""\x41\x4""#), ["E0006 1..5", "E0006 5..7"]);
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
    assert_eq!(errors(r#""\u0041 \u""#), ["E0006 1..7", "E0006 8..10"]);
    assert_eq!(
        lex_str(r#""\'""#).errors[0].fixes[0].applicability,
        Applicability::Safe
    );
}

/// Each message says what to write instead, where the lexer can tell.
#[test]
fn escape_messages() {
    assert!(message(r#""\q""#).starts_with("`\\q` isn't an escape: a string's escapes are"));
    assert!(message(r#""\'""#).starts_with("`'` needs no escape in a string"));
    assert!(message(r#""\}""#).contains("a lone `}` is text"));
    assert!(message(r#""\x41""#).ends_with("in a string, write `\\u{41}`"));
    assert!(message(r#""\xE9""#).contains("write the character itself"));
    assert!(message(r#""\u0041""#).ends_with("write `\\u{41}`"));
    assert!(message(r#""\u{0000041}""#).ends_with("write `\\u{41}`"));
    assert!(message(r#""\u{D800}""#).starts_with("`\\u{D800}` is a surrogate"));
    assert!(message(r#""\u{110000}""#).contains("the largest Unicode scalar value"));
    assert!(message(r#""\u{12""#).starts_with("this `\\u{` escape needs a closing `}`"));
    assert!(message(r#""\a""#).ends_with("write the character as `\\u{7}`"));
    assert!(message(r#""\e""#).ends_with("write the character as `\\u{1B}`"));
    assert!(message(r#""\(x)""#).contains("Quince interpolates with `{name}`"));
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
    assert_eq!(
        message(r#""{ a }""#),
        "an interpolation has no spaces: write `{a}`"
    );
}

#[test]
fn interpolation_messages() {
    assert_eq!(
        message(r#""{}""#),
        "`{}` needs a name inside it, as in `{name}`; a literal brace is written `\\{`"
    );
    assert_eq!(
        message(r#""{a + b}""#),
        "only a path can go in braces, such as `{user.name}`: to show `a + b`, bind it first, \
         as in `let value = a + b`, then write `{value}`"
    );
    assert_eq!(
        message(r#""{Self.MAX}""#),
        "a path in braces can't start with a type: bind it first, as in \
         `let value = Self.MAX`, then write `{value}`"
    );
    assert!(message(r#""{total:.2}""#).contains("`let value = total.to_fixed(2)`"));
    assert!(message(r#""{x:?}""#).contains("such as `to_fixed` or `pad_start`"));
    assert!(message(r#""{0}""#).contains("no positional placeholders"));
    assert!(message(r#""{a.}""#).starts_with("`{a.}` isn't a path"));
    assert_eq!(
        message(r#""{name""#),
        "this `{` is never closed: write `{name}`, or `\\{` for a literal brace"
    );
    assert!(message(r#""{""#).starts_with("`{` opens an interpolation, which needs a name"));
    assert!(message(r#""{a + b""#).starts_with("this `{` is never closed, and only a path"));
}

/// Raw, bytes, tagged and multi-line strings keep the fallback until their own layer, so `"`
/// rules don't report false errors in them.
#[test]
fn forms_not_lexed_yet_are_not_checked() {
    for text in [
        "r\"C:\\path\"",
        "re\"a{2}\"",
        "b\"\\x00\"",
        "s = \"\"\"\n  {a + b}\n  \"\"\"\n",
        "r\"\"\"\n  \"\"\"",
        "\"\"\"\"",
    ] {
        assert_eq!(errors(text), Vec::<String>::new(), "{text:?}");
    }
    assert_eq!(
        tokens("\"\"\"x"),
        r#"StringStart:" StringEnd:" StringStart:" StringText:x"#
    );
    // A keyword is never a prefix, so its string is checked.
    assert_eq!(errors("test\"\\q\""), ["E0006 5..7"]);
}

/// E0001's hint for a Rust raw string still reads the tokens around the `#`.
#[test]
fn raw_string_hint_around_checked_strings() {
    let lexed = lex_str("r#\"{a b}\"#");
    let codes: Vec<_> = lexed.errors.iter().map(|e| e.code).collect();
    assert_eq!(codes, ["E0001", "E0007", "E0001"]);
    assert!(lexed.errors[0].message.contains("`r\"\"\"`"));
    assert!(lexed.errors[2].message.contains("`r\"\"\"`"));
}
