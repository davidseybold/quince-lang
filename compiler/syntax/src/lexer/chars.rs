//! Char literals: their one token, invalid escapes (E0006) and an invalid char (E0009).

use super::Lexer;
use super::control::{ControlIn, is_control};
use super::string::{EscapeIn, escape_error, opens_unicode};
use crate::literal::{char_body_len, char_escape};
use crate::{LexError, SyntaxKind, codes};
use quince_diagnostics::Applicability;
use std::ops::Range;

impl Lexer<'_> {
    /// A char literal, at its `'`: one `Char` token, up to and including its closing `'`, or up
    /// to its line ending when it has none. It holds one Unicode scalar value, written as itself
    /// or as one escape (`r[lex.lit.char]`, `r[lex.lit.char.escapes]`), and anything else is an
    /// E0009.
    pub(super) fn char_literal(&mut self) {
        let text = self.text;
        let open = self.pos;
        let first_error = self.errors.len();
        let body_start = open + 1;
        let line_end = open + self.to_line_end();
        let body_len = char_body_len(&text[body_start..line_end]);
        let body = &text[body_start..body_start + body_len];
        let closed = body_start + body_len < line_end;
        let mut chars = 0;
        let mut valid = true;
        let mut open_unicode_end = None;
        let mut i = 0;
        while let Some(c) = body[i..].chars().next() {
            if is_control(c) {
                let run = (body[i..].find(|c| !is_control(c))).unwrap_or(body.len() - i);
                let range = body_start + i..body_start + i + run;
                self.control_characters(range, ControlIn::Char);
                valid = false;
                chars += body[i..i + run].chars().count();
                i += run;
                continue;
            }
            if c != '\\' {
                i += c.len_utf8();
            } else {
                let escape = char_escape(&body[i..]);
                if let Err(problem) = escape.value {
                    valid = false;
                    let escape_text = &body[i..i + escape.len];
                    let mut error =
                        escape_error(escape_text, body_start + i, problem, EscapeIn::Char);
                    if open_unicode_end == Some(i) {
                        error.fixes.clear();
                    }
                    if opens_unicode(escape_text) {
                        open_unicode_end = Some(i + escape.len);
                    }
                    self.errors.push(error);
                }
                i += escape.len;
            }
            chars += 1;
        }
        let end = body_start + body_len + usize::from(closed);
        if !closed {
            self.unclosed_literal_end = Some(end);
            // The CR is what is wrong, and E0013 reports it alone.
            let lone_cr = text[end..].starts_with('\r') && !text[end..].starts_with("\r\n");
            if !lone_cr {
                let error = LexError::new(
                    open..open + 1,
                    codes::INVALID_CHAR,
                    "this char literal has no closing `'` on its line: a char literal is one \
                     Unicode scalar value between single quotes, as in `'a'`"
                        .to_string(),
                );
                self.errors.insert(first_error, error);
            }
        } else if chars == 0 {
            let error = LexError::new(
                open..end,
                codes::INVALID_CHAR,
                "`''` is empty: a char literal holds exactly one Unicode scalar value".to_string(),
            );
            self.errors.insert(first_error, error);
        } else if chars > 1 {
            let error = too_many_error(text, open..end, body, valid);
            self.errors.insert(first_error, error);
        }
        self.bump(SyntaxKind::Char, end - open);
    }
}

/// The E0009 for the char literal at `range` of `text`, whose `body` holds more than one
/// character. Its fix makes it a string, unless the body has an invalid escape or a control
/// character, which `valid` is false for, since the fix for those errors comes first, or the
/// `"` would join what comes before it: a name character, which would make it a literal's
/// prefix, or a `"`.
fn too_many_error(text: &str, range: Range<usize>, body: &str, valid: bool) -> LexError {
    let error = LexError::new(
        range.clone(),
        codes::INVALID_CHAR,
        "a char literal holds exactly one Unicode scalar value: text goes in a `\"` string"
            .to_string(),
    );
    let before = text[..range.start].chars().next_back();
    let joins = before.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '"');
    if !valid || joins {
        return error;
    }
    error.with_fix(
        "make it a string",
        Applicability::NeedsReview,
        vec![(range, format!("\"{}\"", as_string_body(body)))],
    )
}

/// A valid char body rewritten as a `"` string's text with the same value: `\'` becomes `'`,
/// and `"` and `{` are escaped, so that the string neither ends early nor interpolates.
fn as_string_body(body: &str) -> String {
    let mut out = String::with_capacity(body.len() + 2);
    let mut i = 0;
    while let Some(c) = body[i..].chars().next() {
        match c {
            '\\' => {
                let len = char_escape(&body[i..]).len;
                match &body[i..i + len] {
                    "\\'" => out.push('\''),
                    escape => out.push_str(escape),
                }
                i += len;
                continue;
            }
            '"' => out.push_str("\\\""),
            '{' => out.push_str("\\{"),
            c => out.push(c),
        }
        i += c.len_utf8();
    }
    out
}
