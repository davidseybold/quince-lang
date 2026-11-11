//! Literals with a prefix: raw strings, bytes and tagged literals, an unknown tag (E0010) and
//! a non-ASCII character in bytes (E0011).

use super::Lexer;
use super::string::escape_error;
use crate::literal::{BadEscape, TAGS, byte_escape};
use crate::{LexError, SyntaxKind, codes};
use quince_diagnostics::Applicability;
use std::ops::Range;

/// What a prefix makes of the `"` after it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Form {
    Raw,
    Bytes,
    Tagged,
    /// An unknown tag, lexed as a tagged literal.
    Unknown,
}

impl Lexer<'_> {
    /// A literal at its prefix, an identifier of `prefix_len` bytes that a `"` follows. Only
    /// `r"""` is multi-line (`r[lex.lit.multiline-forms]`): any other prefix before `"""` is an
    /// E0010, and the literal is lexed as an `r"""` string, so its closing line doesn't open
    /// a new string.
    pub(super) fn prefixed_string(&mut self, prefix_len: usize) {
        let open = self.pos;
        let prefix = &self.text[open..open + prefix_len];
        let quote = open + prefix_len;
        // A closing quote right before the prefix, as in `"say "hi" now"`.
        let glued = (self.tokens.last()).filter(|t| t.kind == SyntaxKind::StringEnd);
        let glued_quotes = glued.map(|t| t.len as usize);
        if self.text[quote..].starts_with("\"\"\"") {
            if prefix != "r" {
                self.errors
                    .push(unknown_tag_error(open..quote, prefix, glued_quotes, true));
            }
            return self.multiline_string(prefix_len, true, false);
        }
        let form = match prefix {
            "r" => Form::Raw,
            "b" => Form::Bytes,
            _ if TAGS.contains(&prefix) => Form::Tagged,
            _ => Form::Unknown,
        };
        let first_error = self.errors.len();
        let end = self.pos + self.to_line_end();
        self.bump(SyntaxKind::StringStart, prefix_len + 1);
        let dangling = match form {
            Form::Bytes => self.bytes_text(end),
            _ => {
                // A raw body: up to the next `"` on the line (`r[lex.lit.tagged.raw]`).
                let len = (self.text[self.pos..end].find('"')).unwrap_or(end - self.pos);
                if len > 0 {
                    self.bump(SyntaxKind::StringText, len);
                }
                false
            }
        };
        if self.rest().starts_with('"') {
            self.bump(SyntaxKind::StringEnd, 1);
        } else if form == Form::Unknown {
            // E0010 is the one error for the slip: lexing goes on as for a closed literal.
            self.unclosed_literal_end = Some(self.pos);
        } else {
            let message = match form {
                Form::Raw => {
                    "this raw string has no closing `\"` on its line: a raw string stays on one \
                     line and can't hold a `\"`, so text with quotes, or over several lines, \
                     goes between `r\"\"\"` lines"
                }
                Form::Bytes => {
                    "this bytes literal has no closing `\"` on its line: a bytes literal stays \
                     on one line, and writes a line break as `\\n` and a quote as `\\\"`"
                }
                _ => {
                    "this tagged literal has no closing `\"` on its line: a tagged literal stays \
                     on one line and can't hold a `\"`"
                }
            };
            self.unclosed_string(open..quote + 1, first_error, dangling, message);
        }
        if form == Form::Unknown {
            let error = unknown_tag_error(open..quote, prefix, glued_quotes, false);
            self.errors.insert(first_error, error);
        }
    }

    /// A bytes literal's text up to `end`, which is a line ending or the end of input, or up to
    /// a `"`, as one `StringText`, with an E0006 for each invalid escape and an E0011 for each
    /// run of non-ASCII characters. True when it ends with a `\` that escapes nothing.
    fn bytes_text(&mut self, end: usize) -> bool {
        let start = self.pos;
        let rest = &self.text[start..end];
        let mut len = 0;
        let mut dangling = false;
        while let Some(at) = rest[len..].find(|c: char| matches!(c, '\\' | '"') || !c.is_ascii()) {
            len += at;
            if rest[len..].starts_with('"') {
                break;
            }
            if rest[len..].starts_with('\\') {
                // A `\` before a non-ASCII character is one invalid escape, and no E0011.
                let escape = byte_escape(&rest[len..]);
                if let Err(problem) = escape.value {
                    dangling = problem == BadEscape::Dangling;
                    let text = &rest[len..len + escape.len];
                    let after = &rest[len + escape.len..];
                    let error = escape_error(text, after, start + len, problem, false, true);
                    self.errors.push(error);
                }
                len += escape.len;
            } else {
                let run = (rest[len..].find(|c: char| c.is_ascii())).unwrap_or(rest.len() - len);
                self.non_ascii(start + len..start + len + run);
                len += run;
            }
        }
        if !rest[len..].starts_with('"') {
            len = rest.len();
        }
        if len > 0 {
            self.bump(SyntaxKind::StringText, len);
        }
        dangling
    }

    /// The E0011s for a run of non-ASCII characters in bytes: one for each part of it between
    /// replacements that decoding made, which E0014 has already reported.
    fn non_ascii(&mut self, run: Range<usize>) {
        let mut part_start = run.start;
        for (i, c) in self.text[run.clone()].char_indices() {
            let at = run.start + i;
            let replaced = (self.replaced)
                .binary_search_by_key(&at, |r| r.start as usize)
                .is_ok();
            if replaced {
                self.non_ascii_part(part_start..at);
                part_start = at + c.len_utf8();
            }
        }
        self.non_ascii_part(part_start..run.end);
    }

    fn non_ascii_part(&mut self, part: Range<usize>) {
        if part.is_empty() {
            return;
        }
        let text = &self.text[part.clone()];
        let escapes: String = text.bytes().map(|b| format!("\\x{b:02X}")).collect();
        let (what, its) = match text.chars().count() {
            1 => ("this character", "its"),
            _ => ("these characters", "their"),
        };
        // The escapes are quoted only when they are short enough to read.
        let shown = match text.len() {
            ..=8 => format!(", `{escapes}`"),
            _ => String::new(),
        };
        let error = LexError::new(
            part.clone(),
            codes::NON_ASCII_BYTES,
            format!(
                "a bytes literal holds only ASCII characters: write {what} as the `\\x` escapes \
                 of {its} UTF-8 bytes{shown}"
            ),
        )
        .with_fix(
            "replace it with the `\\x` escapes of its UTF-8 bytes",
            Applicability::Safe,
            vec![(part, escapes)],
        );
        self.errors.push(error);
    }
}

/// The E0010 for the identifier `prefix`, at `range`, before a `"`, or before `"""` when it is
/// `multiline`. `glued_quotes` is the length of the closing quotes right before it, if any.
fn unknown_tag_error(
    range: Range<usize>,
    prefix: &str,
    glued_quotes: Option<usize>,
    multiline: bool,
) -> LexError {
    let tags = "the tags are `url`, `re`, `dur` and `ts`";
    let lower = prefix.to_ascii_lowercase();
    let message = if let Some(quotes) = glued_quotes {
        let (them, written) = match quotes {
            1 => ("that quote is", "`\\\"`"),
            _ => ("those quotes are", "`\\\"\"\"`"),
        };
        format!(
            "`{prefix}\"` reads as a literal tag, since it is glued to the closing quote before \
             it: if {them} part of the text, write {written}; {tags}"
        )
    } else if multiline && prefix == "b" {
        "bytes literals have no multi-line form: write a `b\"...\"` literal on one line, with \
         `\\n` for each line break"
            .to_string()
    } else if multiline && TAGS.contains(&prefix) {
        format!("tagged literals have no multi-line form: write `{prefix}\"...\"` on one line")
    } else if prefix == "f" {
        "Quince has no `f\"` strings: every `\"` string interpolates already, as in \
         `\"total: {total}\"`, so delete the `f`"
            .to_string()
    } else if lower != prefix && (TAGS.contains(&lower.as_str()) || lower == "r" || lower == "b") {
        format!("literal prefixes are lowercase: write `{lower}\"`")
    } else if prefix == "br" || prefix == "rb" {
        "Quince has no raw bytes literals: write a `b\"...\"` literal, with `\\\\` for each \
         backslash"
            .to_string()
    } else {
        format!(
            "`{prefix}\"` isn't a literal: {tags}, and `r\"` and `b\"` start raw strings and \
             bytes literals"
        )
    };
    LexError::new(range, codes::UNKNOWN_TAG, message)
}
