//! Raw strings, bytes and tagged literals, and E0010 and E0011.

use super::errors::errors;
use super::strings::{message, tokens};
use crate::literal::bytes_value;
use crate::{decode_source, lex, lex_str};

#[test]
fn prefixed_shapes() {
    assert_eq!(
        tokens(r#"r"C:\p" b"\x00" re"a{2}" foo"x""#),
        r#"StringStart:r" StringText:C:\p StringEnd:" Whitespace:  StringStart:b" StringText:\x00 StringEnd:" Whitespace:  StringStart:re" StringText:a{2} StringEnd:" Whitespace:  StringStart:foo" StringText:x StringEnd:""#
    );
    assert_eq!(tokens(r#"r"""#), r#"StringStart:r" StringEnd:""#);
    // A keyword is never a prefix, and `_` isn't an identifier.
    assert_eq!(
        tokens(r#"test"a" _"b""#),
        r#"TestKw:test StringStart:" StringText:a StringEnd:" Whitespace:  Underscore:_ StringStart:" StringText:b StringEnd:""#
    );
    assert_eq!(errors("test\"\\q\""), ["E0006 5..7"]);
}

#[test]
fn raw_strings_and_tags_have_raw_bodies() {
    for text in [
        r#"r"C:\path\{x}\q""#,
        r#"r"C:\dir\""#,
        r#"re"\d+{ 2 }""#,
        r#"url"{}""#,
    ] {
        assert_eq!(errors(text), Vec::<String>::new(), "{text:?}");
    }
    assert_eq!(errors("r\"ab\nc"), [r#"E0004 0..2 => "r\"ab\"\nc""#]);
    // In a raw body a `\` escapes nothing, so the fix is offered after one.
    assert_eq!(errors("r\"ab\\"), [r#"E0004 0..2 => "r\"ab\\\"""#]);
    assert_eq!(errors("dur\"5s"), [r#"E0004 0..4 => "dur\"5s\"""#]);
    assert!(message("r\"ab").contains("`r\"\"\"` lines"));
    assert!(message("ts\"ab").starts_with("this tagged literal has no closing"));
    // A lone CR ends it, and only E0013 is reported.
    assert_eq!(errors("r\"ab\rc"), ["E0013 4..5"]);
}

#[test]
fn bytes_escapes() {
    assert_eq!(
        errors(r#"b"\x00\xFF\xab\n\r\t\0\\\"{a b}""#),
        Vec::<String>::new()
    );
    assert_eq!(
        errors(r#"b"\x4 \xG0 \q \u{41} \'\{\}""#),
        [
            "E0006 2..4",
            "E0006 6..8",
            "E0006 11..13",
            "E0006 14..20",
            r#"E0006 21..23 => "b\"\\x4 \\xG0 \\q \\u{41} '\\{\\}\"""#,
            r#"E0006 23..25 => "b\"\\x4 \\xG0 \\q \\u{41} \\'{\\}\"""#,
            r#"E0006 25..27 => "b\"\\x4 \\xG0 \\q \\u{41} \\'\\{}\"""#,
        ]
    );
    assert_eq!(
        message(r#"b"\x4""#),
        "a `\\x` escape takes exactly two hex digits, as in `\\x0A`"
    );
    assert_eq!(
        message(r#"b"\u{41}""#),
        "a bytes literal takes `\\x` escapes, not `\\u{...}`: write the byte as `\\x41`"
    );
    assert_eq!(
        message(r#"b"\u{e9}""#),
        "a bytes literal takes `\\x` escapes, not `\\u{...}`: write its UTF-8 bytes as `\\xC3\\xA9`"
    );
    assert!(message(r#"b"\u0041""#).starts_with("a bytes literal takes `\\x` escapes, not `\\u`"));
    assert_eq!(
        message(r#"b"\a""#),
        "`\\a` isn't a Quince escape: write the byte as `\\x07`"
    );
    assert!(message(r#"b"\q""#).contains("a bytes literal's escapes are `\\x`"));
    assert_eq!(
        message(r#"b"\{""#),
        "`{` needs no escape in a bytes literal"
    );
    // A `"` after the `\` would be escaped, so E0004 has no fix.
    assert_eq!(errors("b\"ab\\\n"), ["E0004 0..2", "E0006 4..5"]);
}

#[test]
fn non_ascii_bytes() {
    assert_eq!(
        errors("b\"caf\u{e9} \u{1F600}\u{e9}\""),
        [
            r#"E0011 5..7 => "b\"caf\\xC3\\xA9 😀é\"""#,
            r#"E0011 8..14 => "b\"café \\xF0\\x9F\\x98\\x80\\xC3\\xA9\"""#,
        ]
    );
    assert_eq!(
        message("b\"\u{e9}\""),
        "a bytes literal holds only ASCII characters: write this character as the `\\x` escapes \
         of its UTF-8 bytes, `\\xC3\\xA9`"
    );
    assert!(message("b\"\u{1F600}\u{1F600}\u{1F600}\"").ends_with("of their UTF-8 bytes"));
    // After a `\` it is an invalid escape, and no E0011 shares its span.
    assert_eq!(errors("b\"\\\u{e9}\""), ["E0006 2..5"]);
    // A replacement that decoding made is E0014's alone.
    let decoded = decode_source(b"b\"\xFF\xC3\xA9\"").unwrap();
    let lexed = lex(&decoded);
    let codes: Vec<_> = lexed
        .errors
        .iter()
        .map(|e| (e.code, e.range.clone()))
        .collect();
    assert_eq!(codes, [("E0014", 2..5), ("E0011", 5..7)]);
}

#[test]
fn non_ascii_fixes_keep_the_bytes() {
    for body in ["caf\u{e9}", "\u{1F600}x", "\u{e9}0", "\\\\\u{e9}"] {
        let text = format!("b\"{body}\"");
        let lexed = lex_str(&text);
        let fix = &lexed.errors[0].fixes[0];
        let mut fixed = text.clone();
        for (range, with) in fix.edits.iter().rev() {
            fixed.replace_range(range.start as usize..range.end as usize, with);
        }
        assert_eq!(errors(&fixed), Vec::<String>::new(), "{fixed:?}");
        let inner = |t: &str| bytes_value(&t[2..t.len() - 1]);
        assert_eq!(inner(&fixed), inner(&text), "{fixed:?}");
    }
}

#[test]
fn unknown_tags() {
    assert_eq!(errors(r#"foo"x\q{}""#), ["E0010 0..3"]);
    // An unknown tag is the one error, even when its literal is unclosed.
    assert_eq!(errors("foo\"x\ny"), ["E0010 0..3"]);
    assert_eq!(
        message(r#"f"hi {name}""#),
        "Quince has no `f\"` strings: every `\"` string interpolates already, as in \
         `\"total: {total}\"`, so delete the `f`"
    );
    assert_eq!(
        message(r#"R"x""#),
        "literal prefixes are lowercase: write `r\"`"
    );
    assert_eq!(
        message(r#"URL"x""#),
        "literal prefixes are lowercase: write `url\"`"
    );
    assert!(message(r#"br"x""#).starts_with("Quince has no raw bytes literals"));
    assert_eq!(
        message(r#"u8"x""#),
        "`u8\"` isn't a literal: the tags are `url`, `re`, `dur` and `ts`, and `r\"` and `b\"` \
         start raw strings and bytes literals"
    );
    // An unescaped quote in a string.
    assert_eq!(errors(r#""say "hi" now""#), ["E0010 6..8"]);
    assert_eq!(
        message(r#""say "hi" now""#),
        "`hi\"` reads as a literal tag, since it is glued to the closing quote before it: if \
         that quote is part of the text, write `\\\"`; the tags are `url`, `re`, `dur` and `ts`"
    );
}

#[test]
fn prefixed_multiline_forms_dont_exist() {
    let text = "b\"\"\"\n  GET \\q\n  \"\"\"\nx";
    assert_eq!(errors(text), ["E0010 0..1"]);
    assert_eq!(
        tokens(text),
        "StringStart:b\"\"\" StringNewline:\n StringIndent:   StringText:GET \\q StringNewline:\n \
         StringIndent:   StringEnd:\"\"\" Newline:\n Ident:x"
    );
    assert!(message("b\"\"\"\n\"\"\"").starts_with("bytes literals have no multi-line form"));
    assert_eq!(
        message("re\"\"\"\n\"\"\""),
        "tagged literals have no multi-line form: write `re\"...\"` on one line"
    );
    assert!(message("f\"\"\"\n\"\"\"").starts_with("Quince has no `f\"` strings"));
    // `r"""` is the one prefixed multi-line form.
    assert_eq!(errors("r\"\"\"\n  \\q\n  \"\"\""), Vec::<String>::new());
    // With no closing line, E0005 spans the opening quotes, and E0010 only the prefix.
    assert_eq!(errors("b\"\"\"\nx"), ["E0010 0..1", "E0005 0..4"]);
}

/// Quotes after text in a multi-line string end it, and the word glued to them reads as a tag
/// whose string runs to the original closing line.
#[test]
fn quotes_after_text_then_a_word() {
    let text = "\"\"\"\n  say \"\"\"hi\"\"\"\n  \"\"\"";
    let lexed = lex_str(text);
    let codes: Vec<_> = lexed.errors.iter().map(|e| e.code).collect();
    assert_eq!(codes, ["E0012", "E0010"]);
    assert!(lexed.errors[1].message.contains("write `\\\"\"\"`"));
}
