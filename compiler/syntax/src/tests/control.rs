//! Control and bidirectional-format characters in literals and comments: E0015.

use super::errors::errors;
use super::strings::message;
use crate::lex_str;

/// The characters `r[lex.source.control]` names, other than a tab and line endings.
fn is_control(c: char) -> bool {
    (c.is_ascii_control() && !matches!(c, '\t' | '\n' | '\r'))
        || ('\u{202A}'..='\u{202E}').contains(&c)
        || ('\u{2066}'..='\u{2069}').contains(&c)
}

#[test]
fn tabs_and_escapes_are_fine() {
    for text in [
        "\"a\tb\"",
        "// a\tb",
        "/// a\tb",
        "r\"a\tb\"",
        "re\"a\tb\"",
        "b\"a\tb\"",
        "'\t'",
        "\"\"\"\n  a\tb\n  \"\"\"",
        r#""\u{7} \u{1B} \u{7F} \u{202E} \u{2066} \u{0}""#,
        r"'\u{202E}'",
        r#"b"\x07\x1B\x7F\0""#,
        "test \"a\tb\" {}",
    ] {
        assert_eq!(errors(text), Vec::<String>::new(), "{text:?}");
    }
}

/// In a string, a test's name, a char or bytes, the fix writes each character as its escape.
#[test]
fn escaped_forms_get_a_fix() {
    assert_eq!(errors("\"a\u{7}b\""), [r#"E0015 2..3 => "\"a\\u{7}b\"""#]);
    // A run is one error, and its fix writes every character.
    assert_eq!(
        errors("\"\u{7}\u{1B}\u{7F}\""),
        [r#"E0015 1..4 => "\"\\u{7}\\u{1B}\\u{7F}\"""#]
    );
    assert_eq!(
        errors("\"\u{202E}x\""),
        [r#"E0015 1..4 => "\"\\u{202E}x\"""#]
    );
    assert_eq!(
        errors("\"a\u{2066}b\u{2069}\""),
        [
            r#"E0015 2..5 => "\"a\\u{2066}b\u{2069}\"""#,
            r#"E0015 6..9 => "\"a\u{2066}b\\u{2069}\"""#
        ]
    );
    assert_eq!(
        errors("test \"a\u{0}\" {}"),
        [r#"E0015 7..8 => "test \"a\\u{0}\" {}""#]
    );
    assert_eq!(
        errors("\"\"\"\n  a\u{1F}\n  \"\"\""),
        [r#"E0015 7..8 => "\"\"\"\n  a\\u{1F}\n  \"\"\"""#]
    );
    // The closing line of a string whose quotes come after text keeps its place.
    assert_eq!(
        errors("\"\"\"\n  a\u{7}\"\"\""),
        [
            r#"E0015 7..8 => "\"\"\"\n  a\\u{7}\"\"\"""#,
            r#"E0012 8..11 => "\"\"\"\n  a\u{7}\n  \"\"\"""#
        ]
    );
    assert_eq!(errors("'\u{7}'"), [r#"E0015 1..2 => "'\\u{7}'""#]);
    assert_eq!(errors("'\u{202E}'"), [r#"E0015 1..4 => "'\\u{202E}'""#]);
    assert_eq!(errors("b\"a\u{1B}\""), [r#"E0015 3..4 => "b\"a\\x1B\"""#]);
    // A char with more than one character gets its E0009 fix only after its E0015 fix, since
    // making it a string would move the character.
    assert_eq!(
        errors("'a\u{7}'"),
        ["E0009 0..4", r#"E0015 2..3 => "'a\\u{7}'""#]
    );
    // An unclosed literal has both errors.
    assert_eq!(
        errors("\"a\u{7}"),
        [
            r#"E0004 0..1 => "\"a\u{7}\"""#,
            r#"E0015 2..3 => "\"a\\u{7}""#
        ]
    );
}

/// A bidirectional-format character isn't ASCII, so in bytes it is an E0011.
#[test]
fn bidi_in_bytes_is_non_ascii() {
    assert_eq!(
        errors("b\"\u{202E}\""),
        [r#"E0011 2..5 => "b\"\\xE2\\x80\\xAE\"""#]
    );
}

#[test]
fn unescaped_forms_get_no_fix() {
    assert_eq!(errors("r\"a\u{7}\""), ["E0015 3..4"]);
    assert_eq!(errors("r\"\"\"\n  \u{202E}\n  \"\"\""), ["E0015 7..10"]);
    assert_eq!(errors("re\"\u{7}\""), ["E0015 3..4"]);
    assert_eq!(errors("foo\"\u{7}\""), ["E0010 0..3", "E0015 4..5"]);
    assert_eq!(errors("// a\u{7}"), ["E0015 4..5"]);
    assert_eq!(errors("/// a\u{202E}"), ["E0015 5..8"]);
    // A vertical tab and a form feed are control characters too.
    assert_eq!(errors("// \u{B}\u{C}"), ["E0015 3..5"]);
    assert_eq!(
        errors("/* \u{7} */"),
        [r#"E0003 0..7 => "// \u{7}""#, "E0015 3..4"]
    );
    // Around a lone CR in a block comment, the errors stay in order.
    assert_eq!(
        errors("/* \u{7}\r\u{7} */"),
        [
            "E0003 0..9",
            "E0015 3..4",
            r#"E0013 4..5 => "/* \u{7}\n\u{7} */""#,
            "E0015 5..6"
        ]
    );
    // In an interpolation's braces, an escape would still be an E0007.
    assert_eq!(errors("\"{a\u{7}}\""), ["E0007 1..5", "E0015 3..4"]);
}

/// Outside literals and comments, these characters are E0001, and a lone CR is always E0013.
#[test]
fn other_codes_keep_their_characters() {
    assert_eq!(errors("a\u{7}b"), ["E0001 1..2"]);
    assert_eq!(errors("a\u{202E}b"), ["E0001 1..4"]);
    assert_eq!(errors("a\u{C}b"), [r#"E0001 1..2 => "a b""#]);
    assert_eq!(errors("// a\rb"), [r#"E0013 4..5 => "// a\nb""#]);
}

/// The text after an opening `"""`, and the lines of an unclosed one, aren't checked, as they
/// aren't for escapes.
#[test]
fn unchecked_text_has_no_e0015() {
    assert_eq!(errors("\"\"\"a\u{7}\n\"\"\""), ["E0012 3..5"]);
    assert_eq!(errors("\"\"\"\u{7}\"\"\""), ["E0012 3..4"]);
    assert_eq!(errors("\"\"\"\n\u{7}\n"), ["E0005 0..3"]);
}

/// A character inside an invalid escape is part of it, an E0006 alone, whose message names it by
/// its code point.
#[test]
fn an_invalid_escape_owns_its_characters() {
    assert_eq!(errors("\"\\\u{7}\""), ["E0006 1..3"]);
    assert_eq!(
        message("\"\\\u{7}\""),
        "a `\\` before U+0007 isn't an escape: a string's escapes are `\\n`, `\\t`, `\\r`, `\\0`, \
         `\\\\`, `\\\"`, `\\{` and `\\u{...}`, so a backslash is written `\\\\`"
    );
    assert!(message("b\"\\\u{202E}\"").starts_with("a `\\` before U+202E isn't an escape"));
    assert!(message("'\\\u{7}'").starts_with("a `\\` before U+0007 isn't an escape"));
    assert_eq!(errors("\"\\u{1\u{7}}\""), ["E0006 1..7"]);
    assert_eq!(
        message("\"\\u{1\u{7}}\""),
        "this `\\u{` escape holds U+0007, which isn't a hex digit: a `\\u` escape takes one to \
         six hex digits, as in `\\u{1F600}`"
    );
}

/// An escape written right after an invalid one leaves it as it was: an invalid escape that can
/// end right before such a character has a fixed length or a closing `}`. The fuzz in
/// `strings.rs` checks the positions, but a closed `\u{...}` needs more characters than it tries.
#[test]
fn a_fix_right_after_an_invalid_escape() {
    assert_eq!(
        errors("\"\\u\u{7}\""),
        ["E0006 1..3", r#"E0015 3..4 => "\"\\u\\u{7}\"""#]
    );
    assert_eq!(
        errors("\"\\q\u{7}\""),
        ["E0006 1..3", r#"E0015 3..4 => "\"\\q\\u{7}\"""#]
    );
    assert_eq!(
        errors("\"\\}\u{7}\""),
        [
            r#"E0006 1..3 => "\"}\u{7}\"""#,
            r#"E0015 3..4 => "\"\\}\\u{7}\"""#
        ]
    );
    assert_eq!(
        errors("\"\\u{D800}\u{7}\""),
        ["E0006 1..9", r#"E0015 9..10 => "\"\\u{D800}\\u{7}\"""#]
    );
    assert_eq!(
        errors("b\"\\x\u{7}\""),
        ["E0006 2..4", r#"E0015 4..5 => "b\"\\x\\x07\"""#]
    );
    assert_eq!(
        errors("'\\q\u{7}'"),
        ["E0009 0..5", "E0006 1..3", r#"E0015 3..4 => "'\\q\\u{7}'""#]
    );
    // An unclosed `\u{` takes the character, so it is part of the E0006.
    assert_eq!(errors("\"\\u{12\u{7}\""), ["E0006 1..7"]);
}

/// Each form's message names the character by its code point and says what Quince expects.
#[test]
fn messages() {
    let cases = [
        (
            "\"\u{7}\"",
            "U+0007 can't appear in a string: a string writes a control or bidirectional-format \
             character as an escape, as in `\\u{7}`",
        ),
        (
            "\"\u{202E}\u{7}\u{7}\"",
            "U+202E and the 2 characters after it can't appear in a string: a string writes a \
             control or bidirectional-format character as an escape, as in `\\u{202E}`",
        ),
        (
            "'\u{7}'",
            "U+0007 can't appear in a char literal: a char literal writes a control or \
             bidirectional-format character as an escape, as in `'\\u{7}'`",
        ),
        (
            "b\"\u{1B}\u{7}\"",
            "U+001B and the character after it can't appear in a bytes literal: a bytes literal \
             writes a control character as a `\\x` escape, as in `\\x1B`",
        ),
        (
            "r\"\u{7}\"",
            "U+0007 can't appear in a raw string, which can't hold a control or \
             bidirectional-format character: a `\"` string writes one as an escape, as in \
             `\\u{7}`",
        ),
        (
            "url\"\u{7}\"",
            "U+0007 can't appear in a tagged literal, which can't hold a control or \
             bidirectional-format character",
        ),
        (
            "// \u{7F}",
            "U+007F can't appear in a comment, which can't hold a control or \
             bidirectional-format character",
        ),
    ];
    for (text, expected) in cases {
        assert_eq!(message(text), expected, "{text:?}");
    }
}

/// No message, for any form E0015 can appear in, holds one of these characters itself.
#[test]
fn no_message_holds_the_character() {
    for c in ['\u{0}', '\u{7}', '\u{7F}', '\u{202E}', '\u{2069}'] {
        for form in [
            "\"{c}\"",
            "\"\"\"\n{c}\n\"\"\"",
            "test \"{c}\" {}",
            "r\"{c}\"",
            "r\"\"\"\n{c}\n\"\"\"",
            "b\"{c}\"",
            "re\"{c}\"",
            "foo\"{c}\"",
            "'{c}'",
            "'a{c}'",
            "// {c}",
            "/// {c}",
            "/* {c} */",
            "\"{c}",
            "\"{a{c}}\"",
            "\"\\{c}\"",
            "\"\\u{{c}}\"",
            "b\"\\{c}\"",
            "'\\{c}'",
        ] {
            let text = form.replace("{c}", &c.to_string());
            let lexed = lex_str(&text);
            assert!(!lexed.errors.is_empty(), "{text:?}");
            for error in &lexed.errors {
                assert!(!error.message.contains(is_control), "{text:?}: {error:?}");
            }
        }
    }
}
