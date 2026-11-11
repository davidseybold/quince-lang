//! Multi-line strings: their tokens and values, E0005 and E0012.

use super::errors::errors;
use super::strings::{message, tokens};
use crate::literal::string_value;
use crate::{SyntaxKind, lex_str};
use quince_diagnostics::Applicability;

/// The value of `text`, which is one multi-line string with no interpolation, by the rule in
/// the README: the tokens between the first `StringNewline` and the last, with each
/// `StringNewline` between them a line feed, and each `StringIndent` nothing.
fn value(text: &str) -> String {
    let lexed = lex_str(text);
    assert!(lexed.errors.is_empty(), "{text:?}: {:?}", lexed.errors);
    let raw = text.starts_with('r');
    let tokens: Vec<_> = lexed.ranges().collect();
    let newlines: Vec<usize> = (0..tokens.len())
        .filter(|&i| tokens[i].0 == SyntaxKind::StringNewline)
        .collect();
    let (first, last) = (newlines[0], *newlines.last().unwrap());
    (tokens[first + 1..last.max(first + 1)].iter())
        .map(|(kind, range)| match kind {
            SyntaxKind::StringText if raw => text[range.clone()].to_string(),
            SyntaxKind::StringText => string_value(&text[range.clone()]),
            SyntaxKind::StringNewline => "\n".to_string(),
            SyntaxKind::StringIndent => String::new(),
            kind => panic!("{text:?}: {kind:?} in a string with no interpolation"),
        })
        .collect()
}

/// Each token's meaning, for checking that a fix keeps it: decoded text, and a line feed for
/// every line ending, which a multi-line string's value has whatever the file uses.
fn meanings(text: &str) -> Vec<String> {
    let lexed = lex_str(text);
    let mut raw = false;
    (lexed.ranges())
        .map(|(kind, range)| {
            let token = &text[range];
            match kind {
                SyntaxKind::StringStart => {
                    raw = token.starts_with('r');
                    token.to_string()
                }
                SyntaxKind::StringText if !raw => string_value(token),
                SyntaxKind::Newline | SyntaxKind::StringNewline => "\n".to_string(),
                _ => token.to_string(),
            }
        })
        .collect()
}

/// Every multi-line string, raw or not or a test's name, whose text between the quotes is up
/// to five characters that matter to it, keeps the invariants and never puts two errors on
/// one span. `StringIndent` is only spaces and tabs. A safe fix removes its error and adds
/// none, and an E0006 or E0013 fix keeps every token's meaning. The only other fix here is
/// E0004's, which needs review, on a `"` after the string.
#[test]
fn every_short_multiline_string() {
    let alphabet: Vec<char> = " \t\na\"\\{}\r".chars().collect();
    let mut middles = vec![String::new()];
    let mut all = vec![String::new()];
    for _ in 0..5 {
        middles = (middles.iter())
            .flat_map(|m| alphabet.iter().map(move |c| format!("{m}{c}")))
            .collect();
        all.extend(middles.iter().cloned());
    }
    for prefix in ["", "r", "test "] {
        for middle in &all {
            let text = format!("{prefix}\"\"\"{middle}\"\"\"");
            errors(&text);
            let lexed = lex_str(&text);
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
                if kind == SyntaxKind::StringIndent {
                    assert!(
                        text[range].bytes().all(|b| b == b' ' || b == b'\t'),
                        "{text:?}"
                    );
                }
            }
            for error in &lexed.errors {
                let Some(fix) = error.fixes.first() else {
                    continue;
                };
                if fix.applicability != Applicability::Safe {
                    assert_eq!(error.code, "E0004", "{text:?}");
                    continue;
                }
                let mut fixed = text.clone();
                for (range, with) in fix.edits.iter().rev() {
                    fixed.replace_range(range.start as usize..range.end as usize, with);
                }
                assert_eq!(
                    lex_str(&fixed).errors.len(),
                    lexed.errors.len() - 1,
                    "{text:?} fixed to {fixed:?}"
                );
                if ["E0006", "E0013"].contains(&error.code) {
                    assert_eq!(
                        meanings(&fixed),
                        meanings(&text),
                        "{text:?} fixed to {fixed:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn multiline_shapes() {
    assert_eq!(
        tokens("\"\"\"\n  a {name}\n\n   \n  \"\"\")"),
        "StringStart:\"\"\" StringNewline:\n StringIndent:   StringText:a  InterpStart:{ \
         Ident:name InterpEnd:} StringNewline:\n StringNewline:\n StringIndent:    \
         StringNewline:\n StringIndent:   StringEnd:\"\"\" RParen:)"
    );
    // Spaces after the opening quotes aren't part of the value either.
    assert_eq!(
        tokens("r\"\"\" \t\n{a}\n\"\"\""),
        "StringStart:r\"\"\" StringIndent: \t StringNewline:\n StringText:{a} StringNewline:\n \
         StringEnd:\"\"\""
    );
    // A quote inside a line is text, and so is a `"""` that doesn't start one.
    assert_eq!(
        tokens("\"\"\"\n a\"\"\"\n \\\"\"\"\n \"\"\"\"x\""),
        "StringStart:\"\"\" StringNewline:\n StringIndent:  StringText:a\"\"\" StringNewline:\n \
         StringIndent:  StringText:\\\"\"\" StringNewline:\n StringIndent:  StringEnd:\"\"\" \
         StringStart:\" StringText:x StringEnd:\""
    );
    assert_eq!(errors("\"\"\"\n  \"\"\""), Vec::<String>::new());
    assert_eq!(errors("\"\"\"\n\"\"\""), Vec::<String>::new());
}

/// `r[lex.lit.multiline-value]` and `r[lex.lit.multiline-indent]`, through the README's rule
/// for reading the value from the tokens.
#[test]
fn multiline_values() {
    assert_eq!(value("\"\"\"\n  a\n  b\n  \"\"\""), "a\nb");
    assert_eq!(value("\"\"\"\r\n  a\r\n  b\r\n  \"\"\""), "a\nb");
    assert_eq!(value("\"\"\"\n    a\n  \n\n    b\n    \"\"\""), "a\n\n\nb");
    assert_eq!(value("\"\"\"\n\t    a\n\t  \"\"\""), "  a");
    assert_eq!(value("\"\"\"  \n  a  \n  \"\"\""), "a  ");
    assert_eq!(value("\"\"\"\n\"\"\""), "");
    assert_eq!(value("\"\"\"\n\n\"\"\""), "");
    assert_eq!(value("\"\"\"\n\n\n\"\"\""), "\n");
    assert_eq!(value("\"\"\"\n\\t\\\"\"\" }\n\"\"\""), "\t\"\"\" }");
    assert_eq!(
        value("r\"\"\"\n  C:\\p {x} \"q\" \\q\n  \"\"\""),
        "C:\\p {x} \"q\" \\q"
    );
}

#[test]
fn escapes_and_interpolation_in_multiline_strings() {
    assert_eq!(
        errors("\"\"\"\n  \\q {a b} \\}\n  \"\"\""),
        [
            "E0006 6..8",
            "E0007 9..14",
            r#"E0006 15..17 => "\"\"\"\n  \\q {a b} }\n  \"\"\"""#
        ]
    );
    // A `"` doesn't end a broken interpolation, since it doesn't end the string.
    assert_eq!(
        tokens("\"\"\"\n{a\"b\n\"\"\""),
        "StringStart:\"\"\" StringNewline:\n Error:{a\"b StringNewline:\n StringEnd:\"\"\""
    );
    assert_eq!(errors("\"\"\"\na\\\n\"\"\""), ["E0006 5..6"]);
    // A test's name doesn't interpolate, in this form too.
    assert_eq!(
        errors("test \"\"\"\n  {a b}\n  \"\"\" {"),
        Vec::<String>::new()
    );
    // A lone CR is a line ending, and its fix to a line feed keeps the value.
    assert_eq!(
        errors("\"\"\"\ra\r\"\"\""),
        [
            r#"E0013 3..4 => "\"\"\"\na\r\"\"\"""#,
            r#"E0013 5..6 => "\"\"\"\ra\n\"\"\"""#
        ]
    );
    assert_eq!(
        tokens("\"\"\"\ra\r\"\"\""),
        "StringStart:\"\"\" StringNewline:\r StringText:a StringNewline:\r StringEnd:\"\"\""
    );
}

#[test]
fn text_on_the_opening_line() {
    assert_eq!(errors("\"\"\"ab  \n\"\"\""), ["E0012 3..5"]);
    assert_eq!(
        tokens("\"\"\" ab \\q\n\"\"\""),
        "StringStart:\"\"\" StringIndent:  StringText:ab \\q StringNewline:\n StringEnd:\"\"\""
    );
    // A `"""` on the opening line closes the string there.
    assert_eq!(
        tokens("x(\"\"\"ab\"\"\", 1)"),
        "Ident:x LParen:( StringStart:\"\"\" StringText:ab StringEnd:\"\"\" Comma:, \
         Whitespace:  Int:1 RParen:)"
    );
    assert_eq!(errors("\"\"\"ab\"\"\""), ["E0012 3..5"]);
    assert_eq!(errors("\"\"\"ab \t\"\"\""), ["E0012 3..5"]);
    assert_eq!(errors("\"\"\"\"\"\""), ["E0012 3..6"]);
    assert_eq!(errors("\"\"\"  \"\"\""), ["E0012 5..8"]);
    assert!(message("\"\"\"ab\n\"\"\"").ends_with("move this text to the next line"));
    assert!(message("\"\"\"ab\"\"\"").ends_with("write a `\"` string"));
    assert!(message("\"\"\"a\"b\"\"\"").ends_with("written `\\\"`"));
    assert!(message("r\"\"\"ab\"\"\"").ends_with("write an `r\"` string"));
    assert!(message("r\"\"\"a\"b\"\"\"").contains("can't hold a quote"));
    assert!(message("\"\"\"\"\"\"").ends_with("an empty string is written `\"\"`"));
}

#[test]
fn indentation() {
    // Indented less, with no indentation, and with a tab where the closing line has spaces.
    assert_eq!(
        errors("\"\"\"\n    a\n  b\nc\n\tc\n    \"\"\""),
        ["E0012 10..12", "E0012 14..16", "E0012 16..17"]
    );
    assert!(message("\"\"\"\n    a\n  b\n    \"\"\"").starts_with("this line is indented less"));
    assert!(message("\"\"\"\n    a\n  b\n    \"\"\"").contains(", 4 spaces, so indent it"));
    assert!(message("\"\"\"\n  a\nb\n  \"\"\"").starts_with("this line isn't indented"));
    assert!(message("\"\"\"\n\t\ta\n  b\n\t\t\"\"\"").starts_with(
        "this line's indentation, 2 spaces, doesn't match the closing line's, 2 tabs:"
    ));
    // The line ending in the span is CR LF when the file uses it.
    assert_eq!(errors("\"\"\"\r\n  a\r\nb\r\n  \"\"\""), ["E0012 10..13"]);
    // When no line has the closing line's indentation, it is reported once, there.
    assert_eq!(errors("\"\"\"\n  a\n  b\n\n    \"\"\""), ["E0012 13..17"]);
    assert!(
        message("\"\"\"\n  a\n  b\n    \"\"\"")
            .ends_with("indent the closing `\"\"\"` with 2 spaces, as the lines are")
    );
    assert!(
        message("\"\"\"\na\n  \"\"\"")
            .ends_with("put the closing `\"\"\"` at the start of its line")
    );
    assert!(message("\"\"\"\n\ta\n  \"\"\"").starts_with(
        "no line of this string starts with the closing line's indentation, 2 spaces:"
    ));
    assert!(message("\"\"\"\n\ta\n\t  \"\"\"").ends_with("with a tab, as the lines are"));
    assert!(message("\"\"\"\n  a\n\t  b\n\t  \"\"\"").contains(
        "this line's indentation, 2 spaces, doesn't match the closing line's, a tab then 2 spaces"
    ));
}

#[test]
fn unclosed_multiline_strings() {
    // The rest of the file isn't checked as string text.
    assert_eq!(errors("x = \"\"\"\n  {a b} \\q\n}\n"), ["E0005 4..7"]);
    assert_eq!(
        tokens("x = r\"\"\"\n a\n"),
        "Ident:x Whitespace:  Eq:= Whitespace:  StringStart:r\"\"\" StringNewline:\n \
         StringText: a StringNewline:\n"
    );
    assert_eq!(errors("\"\"\"a\nb"), ["E0005 0..3", "E0012 3..4"]);
    assert_eq!(errors("\"\"\""), ["E0005 0..3"]);
    assert!(!message("\"\"\"\n a").contains("after other text"));
    assert!(message("\"\"\"\n a\"\"\"").ends_with("is text, not the end of the string"));
    // A lone CR in it is still reported.
    assert_eq!(
        errors("\"\"\"\na\rb"),
        ["E0005 0..3", r#"E0013 5..6 => "\"\"\"\na\nb""#]
    );
}
