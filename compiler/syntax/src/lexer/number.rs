//! Numbers: where an `Int` or `Float` token ends, and what is wrong with it (E0008).

use super::Lexer;
use crate::{SyntaxError, SyntaxKind, codes};
use quince_diagnostics::Applicability;
use std::ops::Range;

/// One thing wrong with a number, at a byte range of its token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Problem {
    /// Letters glued to the number, such as `u8` in `1u8`.
    Suffix,
    UppercasePrefix,
    UppercaseExponent,
    /// A base prefix or an exponent with no digits after it.
    NoDigits,
    /// A digit that isn't in the base, such as `2` in `0b102`.
    BadDigit(u32),
    /// An underscore that isn't between two digits.
    Underscore,
    LeadingZero,
}

/// A number's kind and length, and its problems in order of position.
struct Scan {
    kind: SyntaxKind,
    len: usize,
    problems: Vec<(Range<usize>, Problem)>,
}

impl Lexer<'_> {
    /// A number, at a digit. Right after a `.` it is a tuple position, as in `pair.0.1`, so it
    /// is a decimal integer, with no point, exponent or base prefix of its own.
    pub(super) fn number(&mut self) {
        let start = self.pos;
        let tuple_position = self.tokens.last().map(|t| t.kind) == Some(SyntaxKind::Dot);
        let scan = scan(self.rest(), tuple_position);
        let text = &self.text[start..start + scan.len];
        let after = &self.text[start + scan.len..];
        // Removing the suffix of `1x.5` would leave the float `1.5`.
        let joins_fraction =
            after.starts_with('.') && after[1..].starts_with(|c: char| c.is_ascii_digit());
        for (range, problem) in &scan.problems {
            let mut error = problem_error(text, start, scan.kind, range.clone(), *problem);
            if *problem == Problem::Suffix && joins_fraction {
                error.fixes.clear();
            }
            self.errors.push(error);
        }
        self.bump(scan.kind, scan.len);
    }
}

fn scan(text: &str, tuple_position: bool) -> Scan {
    let b = text.as_bytes();
    let mut problems = Vec::new();
    let mut kind = SyntaxKind::Int;
    let base = match b.get(..2) {
        _ if tuple_position => 10,
        Some([b'0', b'x' | b'X']) => 16,
        Some([b'0', b'b' | b'B']) => 2,
        Some([b'0', b'o' | b'O']) => 8,
        _ => 10,
    };
    let mut end;
    if base != 10 {
        end = digits_end(b, 2, base);
        // With no digits, the missing digits are the one problem, and its message gives the
        // lowercase prefix, so two errors don't share one span.
        if b[1].is_ascii_uppercase() && has_digit(&b[2..end]) {
            problems.push((1..2, Problem::UppercasePrefix));
        }
        group(b, 0..2, 2..end, base, &mut problems);
    } else {
        end = digits_end(b, 0, 10);
        group(b, 0..0, 0..end, 10, &mut problems);
        let int_digits: Vec<u8> = text[..end].bytes().filter(|&c| c != b'_').collect();
        if int_digits.len() > 1 && int_digits[0] == b'0' {
            problems.push((0..end, Problem::LeadingZero));
        }
        if !tuple_position {
            // A point belongs to the number only with a digit after it, so `1..10` and
            // `1.max(2)` aren't floats.
            if b.get(end) == Some(&b'.') && b.get(end + 1).is_some_and(u8::is_ascii_digit) {
                kind = SyntaxKind::Float;
                let fraction = end + 1;
                end = digits_end(b, fraction, 10);
                group(b, fraction - 1..fraction, fraction..end, 10, &mut problems);
            }
            if let Some(exponent_end) = exponent(b, end) {
                kind = SyntaxKind::Float;
                let digits = digits_end(b, exponent_end, 10);
                if b[end] == b'E' && has_digit(&b[exponent_end..digits]) {
                    problems.push((end..end + 1, Problem::UppercaseExponent));
                }
                group(
                    b,
                    end..exponent_end,
                    exponent_end..digits,
                    10,
                    &mut problems,
                );
                end = digits;
            }
        }
    }
    let suffix = end + b[end..].iter().take_while(|c| is_word(**c)).count();
    if suffix > end {
        problems.push((end..suffix, Problem::Suffix));
    }
    problems.sort_by_key(|(range, _)| range.start);
    Scan {
        kind,
        len: suffix,
        problems,
    }
}

/// Where the `e`, and its sign if it has one, ends when an exponent starts at `at`: an `e` or
/// `E` with digits or `_` after it, an optional sign between, or with nothing glued after it
/// at all, as in `1e`. An `e` with letters after it, as in `1em`, starts a suffix instead.
fn exponent(b: &[u8], at: usize) -> Option<usize> {
    if !matches!(b.get(at), Some(b'e' | b'E')) {
        return None;
    }
    let digit_or_underscore = |i: usize| b.get(i).is_some_and(|c| c.is_ascii_digit() || *c == b'_');
    if matches!(b.get(at + 1), Some(b'+' | b'-')) && digit_or_underscore(at + 2) {
        Some(at + 2)
    } else if digit_or_underscore(at + 1) || !b.get(at + 1).copied().is_some_and(is_word) {
        Some(at + 1)
    } else {
        None
    }
}

/// The end of the digits and underscores from `from`. Binary and octal take every decimal
/// digit, so that `group` can report one outside the base.
fn digits_end(b: &[u8], from: usize, base: u32) -> usize {
    let digit = |c: u8| match base {
        16 => c.is_ascii_hexdigit(),
        _ => c.is_ascii_digit(),
    };
    from + b[from..]
        .iter()
        .take_while(|c| digit(**c) || **c == b'_')
        .count()
}

/// Checks one group of digits, `digits`, which `lead` (a prefix, a point or an `e`) comes
/// before: it has a digit, each digit is in the base, and each underscore is between two
/// digits.
fn group(
    b: &[u8],
    lead: Range<usize>,
    digits: Range<usize>,
    base: u32,
    problems: &mut Vec<(Range<usize>, Problem)>,
) {
    let text = &b[digits.clone()];
    if !has_digit(text) {
        return problems.push((lead.start..digits.end, Problem::NoDigits));
    }
    if let Some(i) = text
        .iter()
        .position(|c| !(*c as char).is_digit(base) && *c != b'_')
    {
        let at = digits.start + i;
        problems.push((at..at + 1, Problem::BadDigit(base)));
    }
    let is_digit = |i: Option<usize>| i.is_some_and(|i| text.get(i).is_some_and(|c| *c != b'_'));
    let bad = (0..text.len())
        .find(|&i| text[i] == b'_' && !(is_digit(i.checked_sub(1)) && is_digit(Some(i + 1))));
    if let Some(i) = bad {
        let run = text[i..].iter().take_while(|c| **c == b'_').count();
        let at = digits.start + i;
        problems.push((at..at + run, Problem::Underscore));
    }
}

/// True when `group` holds a digit, not only underscores.
fn has_digit(group: &[u8]) -> bool {
    group.iter().any(u8::is_ascii_hexdigit)
}

/// A letter, digit or underscore: what glues to a number as a suffix.
fn is_word(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// The E0008 for `problem` at `range` of `text`, a number of `kind` that starts at `start`.
fn problem_error(
    text: &str,
    start: usize,
    kind: SyntaxKind,
    range: Range<usize>,
    problem: Problem,
) -> SyntaxError {
    let part = &text[range.clone()];
    let at = start + range.start..start + range.end;
    let error = |message: String| SyntaxError::new(at.clone(), codes::MALFORMED_NUMBER, message);
    let lowercase = |what: &str| {
        let lower = part.to_ascii_lowercase();
        error(format!("{what} is written lowercase")).with_fix(
            &format!("write `{lower}`"),
            Applicability::Safe,
            vec![(at.clone(), lower.clone())],
        )
    };
    match problem {
        Problem::Suffix => {
            let example = match kind {
                SyntaxKind::Float => "`let x: Float32 = 1.5`",
                _ => "`let x: UInt8 = 1`",
            };
            error(format!(
                "`{part}` can't follow a number: a numeric literal has no type suffix, and takes \
                 its type from context, as in {example}"
            ))
            .with_fix(
                "remove the suffix",
                Applicability::NeedsReview,
                vec![(at.clone(), String::new())],
            )
        }
        Problem::UppercasePrefix => lowercase("a base prefix (`0x`, `0b` or `0o`)"),
        Problem::UppercaseExponent => lowercase("an exponent's `e`"),
        Problem::NoDigits if part.starts_with('0') => error(format!(
            "`{}` needs at least one {} digit after it{}",
            part[..2].to_ascii_lowercase(),
            base_name(part),
            match part.as_bytes()[1].is_ascii_uppercase() {
                true => ", and a base prefix is written lowercase",
                false => "",
            }
        )),
        Problem::NoDigits if part.starts_with('E') => {
            error("an exponent is a lowercase `e` followed by digits".to_string())
        }
        Problem::NoDigits => error("an exponent needs digits after its `e`".to_string()),
        Problem::BadDigit(base) => error(format!(
            "`{part}` isn't {}",
            match base {
                2 => "a binary digit: binary uses only `0` and `1`",
                _ => "an octal digit: octal uses only `0` to `7`",
            }
        )),
        Problem::Underscore => {
            error("an underscore in a number must sit between two digits".to_string())
        }
        Problem::LeadingZero => {
            error("a number doesn't start with `0` unless its integer part is `0`".to_string())
        }
    }
}

/// `hex`, `binary` or `octal`, for a text that starts with a base prefix.
fn base_name(prefix: &str) -> &'static str {
    match prefix.as_bytes().get(1) {
        Some(b'x' | b'X') => "hex",
        Some(b'b' | b'B') => "binary",
        _ => "octal",
    }
}
