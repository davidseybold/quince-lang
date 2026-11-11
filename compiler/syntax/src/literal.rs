//! Literal values, computed from token text on demand rather than stored in tokens.

/// The value of an `Int` token's text, or `None` when it doesn't fit in a `u128`.
/// Underscores don't count. For a malformed literal, which the lexer has already reported,
/// it reads the base's digits up to the first character that isn't one, so `None` still means
/// only that the value is too big. That reading can differ from the lexer's: a tuple position
/// has no base prefix, so `t.0x1` is `0` with a suffix, but `int_value("0x1")` is 1.
pub fn int_value(text: &str) -> Option<u128> {
    let (radix, digits) = match text.get(..2) {
        Some("0x" | "0X") => (16, &text[2..]),
        Some("0b" | "0B") => (2, &text[2..]),
        Some("0o" | "0O") => (8, &text[2..]),
        _ => (10, text),
    };
    let mut value: u128 = 0;
    for c in digits.chars().filter(|&c| c != '_') {
        let Some(digit) = c.to_digit(radix) else {
            break;
        };
        value = value
            .checked_mul(u128::from(radix))?
            .checked_add(u128::from(digit))?;
    }
    Some(value)
}

/// The value of a `Float` token's text, rounded to the nearest `f64`, and infinite when it is
/// too big. Underscores don't count. For a malformed literal, which the lexer has already
/// reported, a suffix is ignored, and so is an exponent with no digits.
///
/// Panics if `text` doesn't start with a digit, as every `Float` token does.
pub fn float_value(text: &str) -> f64 {
    let mut chars = text.chars().filter(|&c| c != '_').peekable();
    let mut number = String::new();
    while let Some(c) = chars.next_if(|c| c.is_ascii_digit() || *c == '.') {
        number.push(c);
    }
    if chars.next_if(|c| matches!(c, 'e' | 'E')).is_some() {
        let sign = chars.next_if(|c| matches!(c, '+' | '-'));
        let digits: String = chars.take_while(char::is_ascii_digit).collect();
        if !digits.is_empty() {
            number.push('e');
            number.extend(sign);
            number.push_str(&digits);
        }
    }
    number.parse().expect("a Float token starts with a digit")
}

/// The value of one `StringText` token's text in a `"` string, with its escapes decoded.
/// Interpolation splits a string's text, and no escape spans two `StringText` tokens, so each
/// is decoded on its own. For an invalid escape, which the lexer has already reported, `\'`
/// and `\}` decode to the character, as their fixes make them, and any other is kept as
/// written.
pub fn string_value(text: &str) -> String {
    let mut value = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find('\\') {
        value.push_str(&rest[..i]);
        let escape = escape(&rest[i..]);
        match escape.value {
            Ok(c) | Err(BadEscape::Needless(c)) => value.push(c),
            Err(_) => value.push_str(&rest[i..i + escape.len]),
        }
        rest = &rest[i + escape.len..];
    }
    value.push_str(rest);
    value
}

/// An escape: its length in bytes, and the character it stands for or what is wrong with it.
pub(crate) struct Escape {
    pub len: usize,
    pub value: Result<char, BadEscape>,
}

/// What is wrong with an escape in a `"` string (`r[lex.lit.string.escapes]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BadEscape {
    /// A `\` at a line end or the end of the text.
    Dangling,
    /// `\'` or `\}`, whose character needs no escape.
    Needless(char),
    /// `\x`, with the value of the two hex digits after it, if there are two.
    Hex(Option<u8>),
    /// `\u` with no braces, with what `\u` and four hex digits would mean in other languages.
    UnicodeNoBraces(Option<char>),
    UnicodeUnclosed,
    UnicodeEmpty,
    UnicodeNotHex,
    /// More than six digits, with the scalar value they name, if they name one.
    UnicodeTooLong(Option<char>),
    UnicodeSurrogate,
    UnicodeTooBig,
    /// Any other character after the `\`.
    Unknown(char),
}

/// The escape at the start of `text`, which starts with `\`. It never takes a line ending, a
/// `"` that isn't escaped, or the `\` of the next escape.
pub(crate) fn escape(text: &str) -> Escape {
    let ok = |len, c| Escape { len, value: Ok(c) };
    let bad = |len, problem| Escape {
        len,
        value: Err(problem),
    };
    let Some(c) = text[1..].chars().next() else {
        return bad(1, BadEscape::Dangling);
    };
    match c {
        'n' => ok(2, '\n'),
        't' => ok(2, '\t'),
        'r' => ok(2, '\r'),
        '0' => ok(2, '\0'),
        '\\' | '"' | '{' => ok(2, c),
        '\n' | '\r' => bad(1, BadEscape::Dangling),
        '\'' | '}' => bad(2, BadEscape::Needless(c)),
        'x' => match hex_run(&text[2..], 2) {
            2 => bad(4, BadEscape::Hex(u8::from_str_radix(&text[2..4], 16).ok())),
            _ => bad(2, BadEscape::Hex(None)),
        },
        'u' => unicode_escape(text),
        c => bad(1 + c.len_utf8(), BadEscape::Unknown(c)),
    }
}

/// `\u{...}` at the start of `text` (`r[lex.lit.string.unicode-escape]`).
fn unicode_escape(text: &str) -> Escape {
    let bad = |len, problem| Escape {
        len,
        value: Err(problem),
    };
    let Some(inner) = text[2..].strip_prefix('{') else {
        // `\u0041`, as Java, JavaScript and Python write it.
        if hex_run(&text[2..], 4) == 4 {
            let value = u32::from_str_radix(&text[2..6], 16)
                .ok()
                .and_then(char::from_u32);
            return bad(6, BadEscape::UnicodeNoBraces(value));
        }
        return bad(2, BadEscape::UnicodeNoBraces(None));
    };
    let end = inner
        .find(['}', '"', '{', '\\', '\n', '\r'])
        .unwrap_or(inner.len());
    let digits = &inner[..end];
    if !inner[end..].starts_with('}') {
        return bad(3 + end, BadEscape::UnicodeUnclosed);
    }
    let len = 3 + end + 1;
    let value = || u32::from_str_radix(digits, 16).ok();
    let problem = if digits.is_empty() {
        BadEscape::UnicodeEmpty
    } else if !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        BadEscape::UnicodeNotHex
    } else if digits.len() > 6 {
        BadEscape::UnicodeTooLong(value().and_then(char::from_u32))
    } else {
        match value().expect("six hex digits fit in a u32") {
            0xD800..=0xDFFF => BadEscape::UnicodeSurrogate,
            n => match char::from_u32(n) {
                Some(c) => return Escape { len, value: Ok(c) },
                None => BadEscape::UnicodeTooBig,
            },
        }
    };
    bad(len, problem)
}

/// How many of the first `max` characters of `text` are hex digits, stopping at the first that
/// isn't.
fn hex_run(text: &str, max: usize) -> usize {
    text.bytes()
        .take(max)
        .take_while(u8::is_ascii_hexdigit)
        .count()
}

#[cfg(test)]
mod tests {
    use super::{float_value, int_value, string_value};

    #[test]
    fn int_values() {
        assert_eq!(int_value("42"), Some(42));
        assert_eq!(int_value("1_000_000"), Some(1_000_000));
        assert_eq!(int_value("0xFF"), Some(255));
        assert_eq!(int_value("0xff_ff"), Some(0xFFFF));
        assert_eq!(int_value("0b1010"), Some(10));
        assert_eq!(int_value("0o17"), Some(15));
        assert_eq!(int_value("0"), Some(0));
        assert_eq!(int_value("9223372036854775808"), Some(1 << 63));
        assert_eq!(int_value(&u128::MAX.to_string()), Some(u128::MAX));
        assert_eq!(int_value("340282366920938463463374607431768211456"), None);
        assert_eq!(int_value(&format!("0x1{}", "0".repeat(32))), None);
    }

    #[test]
    fn malformed_int_values_read_the_digits_before_a_problem() {
        assert_eq!(int_value("1u8"), Some(1));
        assert_eq!(int_value("0b102"), Some(2));
        assert_eq!(int_value("0XFF"), Some(255));
        assert_eq!(int_value("0x"), Some(0));
        assert_eq!(int_value("007"), Some(7));
        assert_eq!(int_value("1__0_"), Some(10));
    }

    #[test]
    fn float_values() {
        assert_eq!(float_value("2.75"), 2.75);
        assert_eq!(float_value("1e-9"), 1e-9);
        assert_eq!(float_value("2.5e10"), 2.5e10);
        assert_eq!(float_value("1e+9"), 1e9);
        assert_eq!(float_value("1_000.000_5"), 1000.0005);
        assert_eq!(float_value("0.1"), 0.1);
        assert_eq!(float_value("1e400"), f64::INFINITY);
        assert_eq!(float_value("1e-400"), 0.0);
    }

    #[test]
    fn malformed_float_values_ignore_what_is_wrong() {
        assert_eq!(float_value("1.5f32"), 1.5);
        assert_eq!(float_value("1E9"), 1e9);
        assert_eq!(float_value("1e"), 1.0);
        assert_eq!(float_value("2.5e_"), 2.5);
        assert_eq!(float_value("00.5"), 0.5);
    }

    #[test]
    fn string_values() {
        assert_eq!(string_value("plain text"), "plain text");
        assert_eq!(
            string_value(r#"a\tb\nc\rd\0e\\f\"g\{h"#),
            "a\tb\nc\rd\0e\\f\"g{h"
        );
        assert_eq!(
            string_value(r"\u{41}\u{e9}\u{1F600}\u{10FFFF}"),
            "A\u{e9}\u{1F600}\u{10FFFF}"
        );
        assert_eq!(string_value("caf\u{e9} }"), "caf\u{e9} }");
        assert_eq!(string_value(""), "");
    }

    #[test]
    fn invalid_escapes_decode_as_their_fixes_make_them_or_stay_as_written() {
        assert_eq!(string_value(r"it\'s \}"), "it's }");
        assert_eq!(string_value(r"C:\path"), r"C:\path");
        assert_eq!(
            string_value(r"\x41 \u{D800} \u{} \u41 \u{12"),
            r"\x41 \u{D800} \u{} \u41 \u{12"
        );
        assert_eq!(string_value("a\\"), "a\\");
        assert_eq!(string_value("\\\u{e9}"), "\\\u{e9}");
    }
}
