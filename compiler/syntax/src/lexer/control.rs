//! Control and bidirectional-format characters in literals and comments (E0015).

use super::Lexer;
use crate::{SyntaxError, codes};
use quince_diagnostics::Applicability;
use std::ops::Range;

/// The kind of literal or comment a control character is in, which decides how its E0015
/// reads and whether it has a fix.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ControlIn {
    /// A `"` or `"""` string, a test's name included. The fix writes `\u{...}` escapes.
    String,
    /// A broken interpolation's braces in a string, where an escape would still be an error,
    /// so there is no fix.
    Interpolation,
    /// The fix writes `\u{...}` escapes.
    Char,
    /// The fix writes `\x` escapes. Only ASCII control characters get here: a bidirectional
    /// character in bytes is non-ASCII, an E0011.
    Bytes,
    Raw,
    /// A tagged literal, or one with an unknown tag, which is lexed as tagged.
    Tagged,
    Comment,
}

/// True for a character `r[lex.source.control]` keeps out of literals and comments: a control
/// character other than a tab or a line ending, or a bidirectional-format character. A lone
/// CR is an E0013 instead (`r[lex.source.line-end]`).
pub(super) fn is_control(c: char) -> bool {
    matches!(
        c,
        '\0'..='\u{8}' | '\u{B}'..='\u{C}' | '\u{E}'..='\u{1F}' | '\u{7F}'
    ) || is_bidi(c)
}

fn is_bidi(c: char) -> bool {
    matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}

/// `c` as `U+XXXX`, so that a message never holds the character itself.
pub(super) fn code_point(c: char) -> String {
    format!("U+{:04X}", u32::from(c))
}

impl Lexer<'_> {
    /// One E0015 for each run of control or bidirectional-format characters in `range`, which
    /// is in a literal or comment of the kind `within`. An escape its fix writes can't join an
    /// invalid escape before it: those that can end right before such a character have a
    /// fixed length or a closing `}`, and an unclosed `\u{` takes the character itself.
    pub(super) fn control_characters(&mut self, range: Range<usize>, within: ControlIn) {
        let text = self.text;
        let mut at = range.start;
        while let Some(found) = text[at..range.end].find(is_control) {
            let start = at + found;
            let len =
                (text[start..range.end].find(|c| !is_control(c))).unwrap_or(range.end - start);
            let run = start..start + len;
            self.errors
                .push(control_error(&text[run.clone()], run, within));
            at = start + len;
        }
    }
}

/// The E0015 for the characters `run`, at `range`, in a literal or comment of the kind
/// `within`.
fn control_error(run: &str, range: Range<usize>, within: ControlIn) -> SyntaxError {
    let first = run.chars().next().expect("a run isn't empty");
    let shown = match run.chars().count() {
        1 => code_point(first),
        2 => format!("{} and the character after it", code_point(first)),
        n => format!(
            "{} and the {} characters after it",
            code_point(first),
            n - 1
        ),
    };
    let unicode = format!("\\u{{{:X}}}", u32::from(first));
    let what = "a control or bidirectional-format character";
    let message = match within {
        ControlIn::String => format!(
            "{shown} can't appear in a string: a string writes {what} as an escape, as in \
             `{unicode}`"
        ),
        ControlIn::Interpolation => {
            format!("{shown} can't appear in an interpolation, which holds only a path")
        }
        ControlIn::Char => format!(
            "{shown} can't appear in a char literal: a char literal writes {what} as an escape, \
             as in `'{unicode}'`"
        ),
        ControlIn::Bytes => format!(
            "{shown} can't appear in a bytes literal: a bytes literal writes a control \
             character as a `\\x` escape, as in `\\x{:02X}`",
            u32::from(first)
        ),
        ControlIn::Raw => format!(
            "{shown} can't appear in a raw string, which can't hold {what}: a `\"` or \
             `\"\"\"` string writes one as an escape, as in `{unicode}`"
        ),
        ControlIn::Tagged => {
            format!("{shown} can't appear in a tagged literal, which can't hold {what}")
        }
        ControlIn::Comment => format!("{shown} can't appear in a comment, which can't hold {what}"),
    };
    let error = SyntaxError::new(range.clone(), codes::CONTROL_CHARACTER, message);
    let escapes: String = match within {
        ControlIn::String | ControlIn::Char => (run.chars())
            .map(|c| format!("\\u{{{:X}}}", u32::from(c)))
            .collect(),
        ControlIn::Bytes => run.bytes().map(|b| format!("\\x{b:02X}")).collect(),
        _ => return error,
    };
    error.with_fix(
        "write it as an escape",
        Applicability::Safe,
        vec![(range, escapes)],
    )
}
