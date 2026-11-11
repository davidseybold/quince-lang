//! Char literals: their token, and E0006 and E0009.

use super::errors::errors;
use super::strings::{message, tokens};
use crate::literal::char_value;

#[test]
fn a_char_is_one_token() {
    assert_eq!(
        tokens(r#"'a' '\'' '"' '{' '\u{1F600}'"#),
        r#"Char:'a' Whitespace:  Char:'\'' Whitespace:  Char:'"' Whitespace:  Char:'{' Whitespace:  Char:'\u{1F600}'"#
    );
    // An unclosed char ends before its line ending.
    assert_eq!(tokens("'ab\nc"), "Char:'ab Newline:\n Ident:c");
    // An escaped `'` doesn't close it.
    assert_eq!(tokens(r"'\' x"), r"Char:'\' x");
}

#[test]
fn valid_chars() {
    for text in [
        "'a'",
        "'\u{e9}'",
        "'\u{1F600}'",
        r#"'"'"#,
        "'{'",
        "'}'",
        r"'\n'",
        r"'\t'",
        r"'\r'",
        r"'\0'",
        r"'\\'",
        r#"'\"'"#,
        r"'\{'",
        r"'\''",
        r"'\u{41}'",
    ] {
        assert_eq!(errors(text), Vec::<String>::new(), "{text}");
    }
}

#[test]
fn invalid_chars() {
    assert_eq!(errors("''"), ["E0009 0..2"]);
    assert_eq!(errors("'ab'"), [r#"E0009 0..4 => "\"ab\"""#]);
    // A letter and a combining accent are two scalar values.
    assert_eq!(errors("'e\u{301}'"), [r#"E0009 0..5 => "\"e\u{301}\"""#]);
    // Unclosed, it is the one error, at the opening quote, with no fix.
    assert_eq!(errors("'ab\nc"), ["E0009 0..1"]);
    assert_eq!(errors(r"'\'"), ["E0009 0..1"]);
    // A lone CR ends it, and only E0013 is reported, with no fix.
    assert_eq!(errors("'a\rb"), ["E0013 2..3"]);
    assert_eq!(errors("'a\r\nb"), ["E0009 0..1"]);
}

/// The fix for more than one character makes a string with the same value.
#[test]
fn too_many_characters_become_a_string() {
    assert_eq!(
        errors(r#"'say "hi" {name}'"#),
        [r#"E0009 0..17 => "\"say \\\"hi\\\" \\{name}\"""#]
    );
    assert_eq!(errors(r"'it\'s'"), [r#"E0009 0..7 => "\"it's\"""#]);
    assert_eq!(errors(r"'\n\t'"), [r#"E0009 0..6 => "\"\\n\\t\"""#]);
    // No fix when the body has an invalid escape, so each error is reported once.
    assert_eq!(errors(r"'a\q'"), ["E0009 0..5", "E0006 2..4"]);
    // No fix when the `"` would join what comes before it: `x"ab"` has a prefix, and
    // `"""ab"` opens a multi-line string.
    assert_eq!(errors("x'ab'"), ["E0009 1..5"]);
    assert_eq!(errors("_'ab'"), ["E0009 1..5"]);
    assert_eq!(errors("1'ab'"), ["E0009 1..5"]);
    assert_eq!(errors(r#"""'ab'"#), ["E0009 2..6"]);
    assert_eq!(errors(r#"x = 'ab'"#), [r#"E0009 4..8 => "x = \"ab\"""#]);
}

#[test]
fn escapes_in_chars() {
    assert_eq!(errors(r"'\}'"), [r#"E0006 1..3 => "'}'""#]);
    assert_eq!(errors(r"'\q'"), ["E0006 1..3"]);
    assert_eq!(errors(r"'\x'"), ["E0006 1..3"]);
    assert_eq!(errors(r"'\u{D800}'"), ["E0006 1..9"]);
    // A `\u{` escape's digits stop at the closing `'`.
    assert_eq!(errors(r"'\u{41' + x}"), ["E0006 1..6"]);
    // Right after a `\u` with no closing `}`, deleting the backslash of `\}` would join them.
    assert_eq!(
        errors(r"'\u\}'"),
        ["E0009 0..6", "E0006 1..3", "E0006 3..5"]
    );
    assert_eq!(
        message(r"'\q'"),
        "`\\q` isn't an escape: a char literal's escapes are `\\n`, `\\t`, `\\r`, `\\0`, `\\\\`, \
         `\\'`, `\\\"`, `\\{` and `\\u{...}`, so a backslash is written `\\\\`"
    );
    assert_eq!(message(r"'\}'"), "`}` needs no escape in a char literal");
}

#[test]
fn char_values() {
    assert_eq!(char_value("'a'"), Some('a'));
    assert_eq!(char_value("'\u{1F600}'"), Some('\u{1F600}'));
    assert_eq!(char_value(r"'\n'"), Some('\n'));
    assert_eq!(char_value(r"'\''"), Some('\''));
    assert_eq!(char_value(r#"'\"'"#), Some('"'));
    assert_eq!(char_value(r"'\{'"), Some('{'));
    assert_eq!(char_value(r"'\u{1F600}'"), Some('\u{1F600}'));
    // Invalid, as the lexer reported: `\}` is `}`, as its fix makes it.
    assert_eq!(char_value(r"'\}'"), Some('}'));
    assert_eq!(char_value("''"), None);
    assert_eq!(char_value("'ab'"), None);
    assert_eq!(char_value(r"'\q'"), None);
    // Unclosed, the token runs to its line ending.
    assert_eq!(char_value("'a"), Some('a'));
    assert_eq!(char_value(r"'\'"), Some('\''));
    assert_eq!(char_value("'"), None);
}
