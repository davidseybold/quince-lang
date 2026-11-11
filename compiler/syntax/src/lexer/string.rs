//! One-line strings: their tokens, invalid escapes (E0006), interpolation (E0007) and a
//! missing closing quote (E0004), which raw, bytes and tagged literals share.

use super::Lexer;
use crate::literal::{BadEscape, escape};
use crate::{LexError, SyntaxKind, codes};
use quince_diagnostics::Applicability;
use std::ops::Range;

impl Lexer<'_> {
    /// A string, at its `"`. A test's name, the string after `test` and any spaces, doesn't
    /// interpolate (`r[test.name]`), but takes a string's escapes (`r[test.name-escapes]`).
    pub(super) fn string(&mut self) {
        let test_name = (self.tokens.iter().rev())
            .find(|t| t.kind != SyntaxKind::Whitespace)
            .is_some_and(|t| t.kind == SyntaxKind::TestKw);
        if self.rest().starts_with("\"\"\"") {
            return self.multiline_string(0, false, !test_name);
        }
        let open = self.pos;
        let first_error = self.errors.len();
        let end = self.pos + self.to_line_end();
        self.bump(SyntaxKind::StringStart, 1);
        loop {
            let dangling = self.string_text(end, !test_name, true);
            match self.rest().chars().next() {
                Some('"') => return self.bump(SyntaxKind::StringEnd, 1),
                Some('{') if self.pos < end => self.interpolation(end, true),
                _ => {
                    let message = "this string has no closing `\"` on its line: a `\"` string \
                                   stays on one line, and text over several lines goes between \
                                   `\"\"\"` lines";
                    return self.unclosed_string(open..open + 1, first_error, dangling, message);
                }
            }
        }
    }

    /// The text up to `end`, which is a line ending or the end of input, or up to a `{` when
    /// the string `interpolates`, or a `"` when a `quote_ends` it, if there is any, as one
    /// `StringText`, with an E0006 for each invalid escape. True when it ends with a `\` that
    /// escapes nothing.
    pub(super) fn string_text(&mut self, end: usize, interpolates: bool, quote_ends: bool) -> bool {
        let start = self.pos;
        let rest = &self.text[start..end];
        let mut len = 0;
        let mut dangling = false;
        let mut open_unicode_end = None;
        let ends: &[char] = match (interpolates, quote_ends) {
            (true, true) => &['\\', '{', '"'],
            (true, false) => &['\\', '{'],
            (false, true) => &['\\', '"'],
            (false, false) => &['\\'],
        };
        loop {
            match rest[len..].find(ends) {
                Some(at) if rest[len + at..].starts_with('\\') => {
                    len += at;
                    let escape = escape(&rest[len..]);
                    if let Err(problem) = escape.value {
                        dangling = problem == BadEscape::Dangling;
                        let text = &rest[len..len + escape.len];
                        let after = &rest[len + escape.len..];
                        let mut error =
                            escape_error(text, after, start + len, problem, interpolates, false);
                        if open_unicode_end == Some(len) {
                            error.fixes.clear();
                        }
                        if opens_unicode(text) {
                            open_unicode_end = Some(len + escape.len);
                        }
                        self.errors.push(error);
                    }
                    len += escape.len;
                }
                Some(at) => break len += at,
                None => break len = rest.len(),
            }
        }
        if len > 0 {
            self.bump(SyntaxKind::StringText, len);
        }
        dangling
    }

    /// An interpolation, at its `{`: `InterpStart`, the path's tokens and `InterpEnd`. Anything
    /// else in the braces is one `Error` token and one E0007, up to and including the next `}`,
    /// or up to the next `"` when a `quote_ends` the string, or up to `line_end`, and the
    /// string goes on after it.
    pub(super) fn interpolation(&mut self, line_end: usize, quote_ends: bool) {
        let start = self.pos;
        let rest = &self.text[start + 1..line_end];
        let stops: &[char] = if quote_ends { &['}', '"'] } else { &['}'] };
        let end = rest.find(stops).unwrap_or(rest.len());
        let body = &rest[..end];
        let closed = rest[end..].starts_with('}');
        if !(closed && is_path(body)) {
            self.errors.push(interpolation_error(start, body, closed));
            return self.bump(SyntaxKind::Error, 1 + end + usize::from(closed));
        }
        self.bump(SyntaxKind::InterpStart, 1);
        for (i, step) in body.split('.').enumerate() {
            if i > 0 {
                self.bump(SyntaxKind::Dot, 1);
            }
            if step.starts_with(|c: char| c.is_ascii_digit()) {
                // After the `.`, so a tuple position: `{id.0}`.
                self.number();
            } else if step == "self" {
                self.bump(SyntaxKind::SelfKw, step.len());
            } else {
                self.bump(SyntaxKind::Ident, step.len());
            }
        }
        debug_assert_eq!(self.pos, start + 1 + end, "the path's tokens cover it");
        self.bump(SyntaxKind::InterpEnd, 1);
    }

    /// A one-line literal whose `StringStart` is `open` has reached a line ending, or the end
    /// of input, with no closing `"`: an E0004 with `message`, unless a lone CR ended it. It
    /// goes before the literal's other errors, from `first_error` on, which are later in the
    /// text. When it is `dangling`, its text ends with a `\` that would escape an inserted `"`.
    pub(super) fn unclosed_string(
        &mut self,
        open: Range<usize>,
        first_error: usize,
        dangling: bool,
        message: &str,
    ) {
        let end = self.pos;
        self.unclosed_literal_end = Some(end);
        // The CR is what is wrong, and E0013 reports it alone.
        if self.rest().starts_with('\r') && !self.rest().starts_with("\r\n") {
            return;
        }
        let error = LexError::new(open, codes::UNCLOSED_STRING, message.to_string());
        // After a `\`, an inserted `"` would be escaped, and the string still unclosed.
        let error = if dangling {
            error
        } else {
            error.with_fix(
                "insert `\"` at the end of the line",
                Applicability::NeedsReview,
                vec![(end..end, "\"".to_string())],
            )
        };
        self.errors.insert(first_error, error);
    }
}

/// True when `body` is a path (`r[lex.lit.string.interp-root]`): a name, `self` or a constant,
/// then any number of `.` and a field name or tuple position. A type can't start one, but only
/// `Self` is known to be a type here, so the resolver reports a type name such as `Config`.
fn is_path(body: &str) -> bool {
    let mut steps = body.split('.');
    let root = steps.next().expect("split yields at least one piece");
    let tuple_position =
        |s: &str| s.starts_with(|c: char| c.is_ascii_digit()) && s.bytes().all(is_word);
    (is_name(root) || root == "self") && steps.all(|s| is_name(s) || tuple_position(s))
}

/// An identifier that isn't a keyword or `_`.
fn is_name(word: &str) -> bool {
    word.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
        && word.bytes().all(is_word)
        && word != "_"
        && SyntaxKind::keyword(word).is_none()
}

fn is_word(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// The E0007 for the braces at `start` around `body`, and their `}` when `closed`.
fn interpolation_error(start: usize, body: &str, closed: bool) -> LexError {
    let range = start..start + 1 + body.len() + usize::from(closed);
    let error =
        |message: String| LexError::new(range.clone(), codes::INVALID_INTERPOLATION, message);
    let brace = "a literal brace is written `\\{`";
    let trimmed = body.trim_matches([' ', '\t']);
    if !closed {
        return error(if trimmed.is_empty() {
            format!(
                "`{{` opens an interpolation, which needs a name and a closing `}}`, as in \
                 `{{name}}`; {brace}"
            )
        } else if is_path(trimmed) {
            format!(
                "this `{{` is never closed: write `{{{trimmed}}}`, or `\\{{` for a literal brace"
            )
        } else {
            format!(
                "this `{{` is never closed, and only a path can go in braces, such as \
                 `{{user.name}}`; {brace}"
            )
        });
    }
    if trimmed.is_empty() {
        return error(format!(
            "`{{{body}}}` needs a name inside it, as in `{{name}}`; {brace}"
        ));
    }
    let squeezed: String = body.chars().filter(|c| !matches!(c, ' ' | '\t')).collect();
    // A malformed tuple step would trade this error for an E0008.
    let well_formed = |path: &str| {
        is_path(path)
            && path.split('.').skip(1).all(|step| {
                !step.starts_with(|c: char| c.is_ascii_digit())
                    || step == "0"
                    || (step.bytes().all(|b| b.is_ascii_digit()) && !step.starts_with('0'))
            })
    };
    if let Some(edits) = space_deletions(start + 1, body).filter(|_| well_formed(&squeezed)) {
        return error(format!(
            "an interpolation has no spaces: write `{{{squeezed}}}`"
        ))
        .with_fix("delete the spaces", Applicability::Safe, edits);
    }
    let bind =
        |value: &str| format!("bind it first, as in `let value = {value}`, then write `{{value}}`");
    if trimmed == "Self" {
        return error(
            "`Self` is a type, not a value: an interpolation holds a name, `self` or a constant, \
             as in `{self.name}`"
                .to_string(),
        );
    }
    // The body is quoted in the message only when it can't break the message's own quoting.
    let quotable = !trimmed.contains(['`', '{']);
    if trimmed.starts_with("Self.") && quotable {
        return error(format!(
            "a path in braces can't start with a type: {}",
            bind(trimmed)
        ));
    }
    let words: Vec<&str> = (trimmed.split([' ', '\t']))
        .filter(|w| !w.is_empty())
        .collect();
    if words.len() > 1 && words.iter().all(|w| is_path(w)) {
        let each: Vec<String> = words.iter().map(|w| format!("{{{w}}}")).collect();
        return error(format!(
            "an interpolation holds one path, with no spaces: write each in its own braces, as \
             in `{}`",
            each.join(" ")
        ));
    }
    if let Some((value, spec)) = trimmed.split_once(':') {
        let value = value.trim_end_matches([' ', '\t']);
        let decimals = spec
            .strip_prefix('.')
            .map(|d| d.strip_suffix('f').unwrap_or(d));
        return error(match decimals {
            Some(d) if is_path(value) && !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()) => {
                format!(
                    "Quince strings have no format specifiers: format the value first, as in \
                     `let value = {value}.to_fixed({d})`, then write `{{value}}`"
                )
            }
            _ => "Quince strings have no format specifiers: format the value with a method, \
                  such as `to_fixed` or `pad_start`, bind the result with `let`, and write \
                  its name in the braces"
                .to_string(),
        });
    }
    if trimmed.bytes().all(|b| is_word(b) || b == b'.') {
        return error(if trimmed.starts_with(|c: char| c.is_ascii_digit()) {
            "an interpolation starts with a name, not a number: Quince strings have no \
             positional placeholders, so write the value's name, as in `{name}`"
                .to_string()
        } else {
            format!(
                "`{{{body}}}` isn't a path: a path is a name, `self` or a constant, then any \
                 number of `.` and a field name or tuple position, as in `{{user.name}}` or \
                 `{{id.0}}`"
            )
        });
    }
    if !quotable {
        return error(
            "only a path can go in braces, such as `{user.name}`: bind an expression with `let` \
             first, and write its name in the braces"
                .to_string(),
        );
    }
    error(format!(
        "only a path can go in braces, such as `{{user.name}}`: to show `{trimmed}`, {}",
        bind(trimmed)
    ))
}

/// The edits that delete the spaces and tabs in `body`, which starts at `at`, when each run of
/// them touches a brace or a `.`, so that deleting it can't join two words.
fn space_deletions(at: usize, body: &str) -> Option<Vec<(Range<usize>, String)>> {
    let mut edits = Vec::new();
    let mut i = 0;
    while let Some(found) = body[i..].find([' ', '\t']) {
        let run_start = i + found;
        let run_end = run_start
            + body[run_start..]
                .bytes()
                .take_while(|b| matches!(b, b' ' | b'\t'))
                .count();
        let before = body[..run_start].chars().next_back();
        let after = body[run_end..].chars().next();
        if !matches!(before, None | Some('.')) && !matches!(after, None | Some('.')) {
            return None;
        }
        edits.push((at + run_start..at + run_end, String::new()));
        i = run_end;
    }
    (!edits.is_empty()).then_some(edits)
}

/// True when the invalid escape `text` is a `\u` with no closing `}`. Deleting the `\` of a
/// needless escape right after it would join the two into one escape, so that one gets no fix.
pub(super) fn opens_unicode(text: &str) -> bool {
    text.starts_with("\\u") && !text.ends_with('}')
}

/// The E0006 for the escape `text`, at `at`, which the text `after` follows, in a string
/// that `interpolates` or in a bytes literal when `bytes` is true.
pub(super) fn escape_error(
    text: &str,
    after: &str,
    at: usize,
    problem: BadEscape,
    interpolates: bool,
    bytes: bool,
) -> LexError {
    let error =
        |message: String| LexError::new(at..at + text.len(), codes::INVALID_ESCAPE, message);
    let example = "as in `\\u{1F600}`";
    let literal = if bytes { "a bytes literal" } else { "a string" };
    let message = match problem {
        BadEscape::Dangling => {
            "a `\\` at the end of a line escapes nothing: a backslash is written `\\\\`".to_string()
        }
        BadEscape::Needless(c) => {
            let why = match c {
                '}' if interpolates => ": only `{` opens an interpolation, so a lone `}` is text",
                _ => "",
            };
            return error(format!("`{c}` needs no escape in {literal}{why}")).with_fix(
                "delete the backslash",
                Applicability::Safe,
                vec![(at..at + 1, String::new())],
            );
        }
        BadEscape::Hex(_) if bytes => {
            "a `\\x` escape takes exactly two hex digits, as in `\\x0A`".to_string()
        }
        BadEscape::NotInBytes(Some(c)) => {
            let mut utf8 = [0; 4];
            let escapes: String = (c.encode_utf8(&mut utf8).bytes())
                .map(|b| format!("\\x{b:02X}"))
                .collect();
            let what = if c.is_ascii() {
                "the byte"
            } else {
                "its UTF-8 bytes"
            };
            format!(
                "a bytes literal takes `\\x` escapes, not `\\u{{...}}`: write {what} as `{escapes}`"
            )
        }
        BadEscape::NotInBytes(None) => "a bytes literal takes `\\x` escapes, not `\\u`: write \
                                        each byte as `\\x` and two hex digits, as in `\\xFF`"
            .to_string(),
        BadEscape::Hex(Some(b)) if b.is_ascii() => format!(
            "`\\x` escapes appear only in bytes literals: in a string, write `\\u{{{b:X}}}`"
        ),
        BadEscape::Hex(_) => format!(
            "`\\x` escapes appear only in bytes literals: in a string, write the character \
             itself or its `\\u{{...}}` escape, {example}"
        ),
        BadEscape::UnicodeNoBraces(Some(c)) => format!(
            "a `\\u` escape takes its hex digits in braces: write `\\u{{{:X}}}`",
            u32::from(c)
        ),
        BadEscape::UnicodeNoBraces(None) => {
            format!("a `\\u` escape takes one to six hex digits in braces, {example}")
        }
        BadEscape::UnicodeUnclosed => {
            format!("this `\\u{{` escape needs a closing `}}`, {example}")
        }
        BadEscape::UnicodeEscapedBrace(Some(c)) => format!(
            "the `}}` that closes a `\\u{{` escape takes no backslash: write `\\u{{{:X}}}`",
            u32::from(c)
        ),
        BadEscape::UnicodeEscapedBrace(None) => format!(
            "the `}}` that closes a `\\u{{` escape takes no backslash, and the escape holds one \
             to six hex digits naming a Unicode scalar value, {example}"
        ),
        BadEscape::UnicodeEmpty => format!("`\\u{{}}` needs one to six hex digits, {example}"),
        BadEscape::UnicodeNotHex => format!(
            "`{text}` holds a character that isn't a hex digit: a `\\u` escape takes one to six \
             hex digits, {example}"
        ),
        BadEscape::UnicodeTooLong(Some(c)) => format!(
            "a `\\u` escape takes at most six hex digits: write `\\u{{{:X}}}`",
            u32::from(c)
        ),
        BadEscape::UnicodeTooLong(None) => "a `\\u` escape takes at most six hex digits, and \
                                            the largest Unicode scalar value is `\\u{10FFFF}`"
            .to_string(),
        BadEscape::UnicodeSurrogate => format!(
            "`{text}` is a surrogate, which isn't a Unicode scalar value: write the character \
             itself, or the `\\u` escape of its scalar value, {example}"
        ),
        BadEscape::UnicodeTooBig => {
            format!("`{text}` is above `\\u{{10FFFF}}`, the largest Unicode scalar value")
        }
        BadEscape::Unknown(c) => match c {
            // C's control-character escapes.
            'a' | 'b' | 'f' | 'v' | 'e' => {
                let code = match c {
                    'a' => 0x7,
                    'b' => 0x8,
                    'f' => 0xC,
                    'v' => 0xB,
                    _ => 0x1B,
                };
                if bytes {
                    format!("`{text}` isn't a Quince escape: write the byte as `\\x{code:02X}`")
                } else {
                    format!(
                        "`{text}` isn't a Quince escape: write the character as `\\u{{{code:X}}}`"
                    )
                }
            }
            // Swift's `\(name)`, where the string interpolates.
            '(' if interpolates => {
                let path = after
                    .find(')')
                    .map(|end| &after[..end])
                    .filter(|p| is_path(p));
                match path {
                    Some(path) => format!(
                        "`\\(` isn't an escape: Quince interpolates with braces, so write \
                         `{{{path}}}`"
                    ),
                    None => "`\\(` isn't an escape: Quince interpolates with `{name}`".to_string(),
                }
            }
            _ if bytes => format!(
                "`{text}` isn't an escape: a bytes literal's escapes are `\\x` with two hex \
                 digits, `\\n`, `\\t`, `\\r`, `\\0`, `\\\\` and `\\\"`, so a backslash is written \
                 `\\\\`"
            ),
            _ => format!(
                "`{text}` isn't an escape: a string's escapes are `\\n`, `\\t`, `\\r`, `\\0`, \
                 `\\\\`, `\\\"`, `\\{{` and `\\u{{...}}`, so a backslash is written `\\\\`"
            ),
        },
    };
    error(message)
}
