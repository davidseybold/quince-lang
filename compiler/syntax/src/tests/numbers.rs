//! Numbers: where they end, their kinds, and E0008.

use super::errors::errors;
use crate::literal::{float_value, int_value};
use crate::{SyntaxKind, lex_str};
use quince_diagnostics::Applicability;

/// The tokens other than whitespace, as `Kind:text`, separated by spaces.
fn tokens(text: &str) -> String {
    let lexed = lex_str(text);
    let shown: Vec<String> = lexed
        .ranges()
        .filter(|(kind, _)| *kind != SyntaxKind::Whitespace)
        .map(|(kind, range)| format!("{kind:?}:{}", &text[range]))
        .collect();
    shown.join(" ")
}

/// Every text of up to four characters from those that matter to numbers keeps the
/// invariants, and every number token has a value.
#[test]
fn every_short_text() {
    let alphabet: Vec<char> = "019xXbBoeE._+-u ".chars().collect();
    let mut texts = vec![String::new()];
    for _ in 0..4 {
        let longer: Vec<String> = texts
            .iter()
            .flat_map(|t| alphabet.iter().map(move |c| format!("{t}{c}")))
            .collect();
        for text in &longer {
            errors(text);
            for (kind, range) in lex_str(text).ranges() {
                match kind {
                    SyntaxKind::Int => assert!(int_value(&text[range]).is_some(), "{text}"),
                    SyntaxKind::Float => assert!(!float_value(&text[range]).is_nan(), "{text}"),
                    _ => {}
                }
            }
        }
        texts = longer;
    }
}

#[test]
fn well_formed_numbers() {
    assert_eq!(
        tokens("42 1_000_000 0xFF 0xab_CD 0b1010 0o17 0"),
        "Int:42 Int:1_000_000 Int:0xFF Int:0xab_CD Int:0b1010 Int:0o17 Int:0"
    );
    assert_eq!(
        tokens("3.14 1e-9 2.5e10 1e+9 0.5 1_000.000_5e1_0 0e3"),
        "Float:3.14 Float:1e-9 Float:2.5e10 Float:1e+9 Float:0.5 Float:1_000.000_5e1_0 Float:0e3"
    );
    for text in ["42", "0xFF", "0b1", "0o7", "3.14", "1e-9", "1_0.0_1e+1_0"] {
        assert_eq!(errors(text), Vec::<String>::new(), "{text}");
    }
}

#[test]
fn a_point_is_part_of_a_float_only_before_a_digit() {
    assert_eq!(tokens("1..10"), "Int:1 DotDot:.. Int:10");
    assert_eq!(tokens("1..=9"), "Int:1 DotDotEq:..= Int:9");
    assert_eq!(
        tokens("3.14159.to_fixed(2)"),
        "Float:3.14159 Dot:. Ident:to_fixed LParen:( Int:2 RParen:)"
    );
    assert_eq!(
        tokens("1.max(2)"),
        "Int:1 Dot:. Ident:max LParen:( Int:2 RParen:)"
    );
    assert_eq!(tokens("1. .5"), "Int:1 Dot:. Dot:. Int:5");
    assert_eq!(tokens("0x1.5"), "Int:0x1 Dot:. Int:5");
}

/// Right after a `.`, a number is a tuple position: digits only, so `pair.0.1` is two steps.
#[test]
fn tuple_positions() {
    assert_eq!(tokens("pair.0.1"), "Ident:pair Dot:. Int:0 Dot:. Int:1");
    assert_eq!(tokens("t.1e5"), "Ident:t Dot:. Int:1e5");
    assert_eq!(tokens("1.0.1"), "Float:1.0 Dot:. Int:1");
    // Trivia between the `.` and the digits ends the rule.
    assert_eq!(tokens("x. 0.1"), "Ident:x Dot:. Float:0.1");
    assert_eq!(errors("t.0x1"), [r#"E0008 3..5 => "t.0""#]);
    assert_eq!(errors("t.01"), ["E0008 2..4"]);
}

#[test]
fn malformed_numbers_keep_their_kind() {
    assert_eq!(
        tokens("1u8 10px 1.5f32 1e 1E9 0XFF 0b102 007 1_ 1em"),
        "Int:1u8 Int:10px Float:1.5f32 Float:1e Float:1E9 Int:0XFF Int:0b102 Int:007 Int:1_ \
         Int:1em"
    );
    assert_eq!(tokens("1e+x"), "Float:1e Plus:+ Ident:x");
    assert_eq!(tokens("1e-"), "Float:1e Minus:-");
}

#[test]
fn suffixes() {
    // Removing the suffix would glue the number to the `.5` after it, making a float.
    assert_eq!(errors("1x.5"), ["E0008 1..2"]);
    assert_eq!(errors("1x.y"), [r#"E0008 1..2 => "1.y""#]);
    assert_eq!(errors("1u8"), [r#"E0008 1..3 => "1""#]);
    assert_eq!(errors("1.5f32"), [r#"E0008 3..6 => "1.5""#]);
    assert_eq!(errors("0xFFu8"), [r#"E0008 4..6 => "0xFF""#]);
    assert_eq!(errors("1em"), [r#"E0008 1..3 => "1""#]);
    let error = &lex_str("1.5f32").errors[0];
    assert_eq!(error.fixes[0].applicability, Applicability::NeedsReview);
    assert!(error.message.starts_with("`f32` can't follow a number"));
}

/// A Rust-style suffix gets an example with the Quince type it names, and the number itself.
#[test]
fn suffix_messages_name_the_type() {
    let example = |text| {
        let errors = lex_str(text).errors;
        let suffix = errors
            .iter()
            .find(|e| e.message.contains(", as in "))
            .unwrap();
        let message = &suffix.message;
        message[message.find(", as in ").unwrap() + 8..].to_string()
    };
    assert_eq!(example("200u8"), "`let x: UInt8 = 200`");
    assert_eq!(example("1_000i32"), "`let x: Int32 = 1_000`");
    assert_eq!(example("7i64"), "`let x: Int = 7`");
    assert_eq!(example("0xFFu16"), "`let x: UInt16 = 0xFF`");
    assert_eq!(example("1.5f32"), "`let x: Float32 = 1.5`");
    assert_eq!(example("2.5f64"), "`let x: Float = 2.5`");
    assert_eq!(example("1f32"), "`let x: Float32 = 1.0`");
    // A number with other problems, or a float type on a non-decimal integer, gets a plain one.
    assert_eq!(example("0b1f32"), "`let x: Float32 = 1.5`");
    assert_eq!(example("007f32"), "`let x: Float32 = 1.5`");
    assert_eq!(example("1_f32"), "`let x: Float32 = 1.5`");
    assert_eq!(example("007u8"), "`let x: UInt8 = 1`");
    assert_eq!(example("0b1u8"), "`let x: UInt8 = 0b1`");
    // No Quince type for these, or the wrong kind of number for it.
    assert_eq!(example("10px"), "`let x: UInt8 = 1`");
    assert_eq!(example("1usize"), "`let x: UInt8 = 1`");
    assert_eq!(example("1.5u8"), "`let x: Float32 = 1.5`");
    assert_eq!(example("1.5em"), "`let x: Float32 = 1.5`");
}

#[test]
fn prefixes_and_digits() {
    assert_eq!(errors("0XFF"), [r#"E0008 1..2 => "0xFF""#]);
    assert_eq!(errors("0B1"), [r#"E0008 1..2 => "0b1""#]);
    assert_eq!(errors("0O7"), [r#"E0008 1..2 => "0o7""#]);
    assert_eq!(errors("0x"), ["E0008 0..2"]);
    // An uppercase prefix with no digits is one error, whose message gives both.
    assert_eq!(errors("0X"), ["E0008 0..2"]);
    assert_eq!(errors("0B_"), ["E0008 0..3"]);
    assert_eq!(errors("0b_"), ["E0008 0..3"]);
    assert_eq!(errors("0b102"), ["E0008 4..5"]);
    assert_eq!(errors("0o78"), ["E0008 3..4"]);
    let message = |text| lex_str(text).errors[0].message.clone();
    assert_eq!(message("0x"), "`0x` needs at least one hex digit after it");
    assert_eq!(
        message("0b_"),
        "`0b` needs at least one binary digit after it"
    );
    assert_eq!(
        message("0X"),
        "`0x` needs at least one hex digit after it, and a base prefix is written lowercase"
    );
    assert!(message("0b102").starts_with("`2` isn't a binary digit"));
    assert!(message("0o78").starts_with("`8` isn't an octal digit"));
    assert_eq!(
        lex_str("0XFF").errors[0].fixes[0].applicability,
        Applicability::Safe
    );
}

#[test]
fn underscores() {
    assert_eq!(errors("1_"), ["E0008 1..2"]);
    assert_eq!(errors("1__0"), ["E0008 1..3"]);
    assert_eq!(errors("0x_FF"), ["E0008 2..3"]);
    assert_eq!(errors("1_.5"), ["E0008 1..2"]);
    assert_eq!(errors("1e_5"), ["E0008 2..3"]);
    assert_eq!(errors("2.5_"), ["E0008 3..4"]);
    // One problem per group: the integer part, the fraction and the exponent.
    assert_eq!(errors("1__2__3"), ["E0008 1..3"]);
    assert_eq!(
        errors("1_.5_e5_"),
        ["E0008 1..2", "E0008 4..5", "E0008 7..8"]
    );
    assert_eq!(errors("1_u8"), ["E0008 1..2", r#"E0008 2..4 => "1_""#]);
}

#[test]
fn leading_zeros_and_exponents() {
    assert_eq!(errors("007"), ["E0008 0..3"]);
    assert_eq!(errors("00.5"), ["E0008 0..2"]);
    assert_eq!(errors("0_1"), ["E0008 0..3"]);
    assert_eq!(errors("0 0.5 0e3 0_"), ["E0008 11..12"]);
    assert!(lex_str("007").errors[0].message.contains("`0o17`"));
    assert_eq!(errors("1E9"), [r#"E0008 1..2 => "1e9""#]);
    assert_eq!(errors("1.5E-3"), [r#"E0008 3..4 => "1.5e-3""#]);
    assert_eq!(errors("1e"), ["E0008 1..2"]);
    assert_eq!(errors("1e+x"), ["E0008 1..2"]);
    // An uppercase `E` with no digits is one error, whose message gives both.
    assert_eq!(errors("1E"), ["E0008 1..2"]);
    assert_eq!(errors("1E+x"), ["E0008 1..2"]);
    assert_eq!(errors("1E_"), ["E0008 1..3"]);
    assert_eq!(
        lex_str("1e").errors[0].message,
        "an exponent needs digits after its `e`"
    );
    assert_eq!(
        lex_str("1E").errors[0].message,
        "an exponent is a lowercase `e` followed by digits"
    );
    assert_eq!(errors("1e5x"), [r#"E0008 3..4 => "1e5""#]);
}
