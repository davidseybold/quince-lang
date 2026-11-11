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

#[cfg(test)]
mod tests {
    use super::{float_value, int_value};

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
}
