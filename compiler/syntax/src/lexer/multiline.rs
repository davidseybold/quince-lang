//! Multi-line strings, `"""` and `r"""`: their lines, the indentation each line drops, a
//! missing closing line (E0005) and layout errors (E0012).

use super::Lexer;
use super::control::ControlIn;
use crate::literal::escape;
use crate::{LexError, SyntaxKind, codes};
use quince_diagnostics::Applicability;
use std::ops::Range;

/// The quotes that open and close a multi-line string.
const QUOTES: &str = "\"\"\"";

impl Lexer<'_> {
    /// A multi-line string, at its `"""`, or at its prefix of `prefix_len` bytes, as the `r`
    /// of `r"""` when it is `raw`. Any other prefix is an error the caller reports, and its
    /// string is lexed as raw. A `"""` string `interpolates` unless it is a test's name.
    pub(super) fn multiline_string(&mut self, prefix_len: usize, raw: bool, interpolates: bool) {
        let open = self.pos;
        let first_error = self.errors.len();
        let open_len = prefix_len + QUOTES.len();
        self.bump(SyntaxKind::StringStart, open_len);
        if self.opening_line(raw) {
            return;
        }
        // The lines after the opening one, up to the first `"""` that an escape doesn't take
        // (`r[lex.lit.multiline-quotes]`): after a line's indentation, which makes it the
        // closing line, or after text, which is an error that still ends the string.
        let mut content = Vec::new();
        let mut closing = None;
        for line in lines_after(self.text, self.pos) {
            let line_text = &self.text[line.clone()];
            let indent_len = indent_len(line_text);
            if line_text[indent_len..].starts_with(QUOTES) {
                closing = Some(line.start..line.start + indent_len);
                break;
            }
            if let Some(at) = find_quotes(line_text, raw) {
                content.push(line.clone());
                return self.closed_after_text(&content, line.start + at, raw, interpolates);
            }
            content.push(line);
        }
        let Some(indent) = closing else {
            return self.unclosed_multiline(open..open + open_len, first_error, &content);
        };
        let indent_text = &self.text[indent.clone()];
        let leads: Vec<&str> = (content.iter())
            .map(|line| &self.text[line.clone()])
            .filter(|line| !is_blank(line))
            .map(|line| &line[..indent_len(line)])
            .collect();
        // When no line has the closing line's indentation, the closing line is the slip.
        let closing_is_wrong =
            !leads.is_empty() && leads.iter().all(|l| !l.starts_with(indent_text));
        self.content_lines(&content, indent_text, !closing_is_wrong, raw, interpolates);
        self.string_newline();
        if closing_is_wrong {
            self.errors
                .push(closing_error(indent.clone(), indent_text, &leads));
        }
        if !indent.is_empty() {
            self.bump(SyntaxKind::StringIndent, indent.len());
        }
        self.bump(SyntaxKind::StringEnd, QUOTES.len());
    }

    /// The content `lines`, each after its line ending, dropping the closing line's `indent`.
    /// A line that doesn't start with it is an E0012 when `report_indent` says so.
    fn content_lines(
        &mut self,
        lines: &[Range<usize>],
        indent: &str,
        report_indent: bool,
        raw: bool,
        interpolates: bool,
    ) {
        for line in lines {
            self.string_newline();
            let text = &self.text[line.clone()];
            if is_blank(text) {
                // A line that holds only whitespace becomes empty (`r[lex.lit.multiline-indent]`).
                if !text.is_empty() {
                    self.bump(SyntaxKind::StringIndent, text.len());
                }
                continue;
            }
            let kept = if text.starts_with(indent) {
                indent.len()
            } else {
                if report_indent {
                    self.errors
                        .push(indent_error(self.text, line.clone(), indent));
                }
                common_prefix(text, indent)
            };
            if kept > 0 {
                self.bump(SyntaxKind::StringIndent, kept);
            }
            self.line_text(line.end, raw, interpolates);
        }
    }

    /// A string whose last line of `lines` holds `"""` after text, at `at`. That is an E0012
    /// (`r[lex.lit.multiline-quotes]`), and the string ends there, so the rest of the file is
    /// still checked. The fix moves the quotes to a line of their own, with the indentation the
    /// lines share, which is what the lines drop here too.
    fn closed_after_text(
        &mut self,
        lines: &[Range<usize>],
        at: usize,
        raw: bool,
        interpolates: bool,
    ) {
        let text = self.text;
        let mut lines = lines.to_vec();
        lines.last_mut().expect("the line with the quotes").end = at;
        let shared = (lines.iter())
            .map(|line| &text[line.clone()])
            .filter(|line| !is_blank(line))
            .map(|line| &line[..indent_len(line)])
            .reduce(|a, b| &a[..common_prefix(a, b)])
            .expect("the line with the quotes isn't blank");
        let newline = if text[..lines[0].start].ends_with("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        self.content_lines(&lines, shared, true, raw, interpolates);
        let indented = match shared {
            "" => String::new(),
            _ => format!(", indented with {}", describe(shared)),
        };
        let error = LexError::new(
            at..at + QUOTES.len(),
            codes::MULTILINE_LAYOUT,
            format!(
                "a multi-line string's text can't hold `\"\"\"`, and its closing `\"\"\"` goes on \
                 a line of its own: to end the string here, move it to the next line{indented}"
            ),
        )
        .with_fix(
            "move the closing `\"\"\"` to a line of its own",
            Applicability::NeedsReview,
            vec![(at..at, format!("{newline}{shared}"))],
        );
        self.errors.push(error);
        self.bump(SyntaxKind::StringEnd, QUOTES.len());
    }

    /// The rest of the opening line, which may hold only spaces and tabs
    /// (`r[lex.lit.multiline-open]`): they are a `StringIndent`, since they aren't part of the
    /// value. Text there is one E0012 and one `StringText`, with no escapes checked, and a
    /// `"""` after it closes the string, so that `"""text"""` can't swallow the file. True
    /// when it does.
    fn opening_line(&mut self, raw: bool) -> bool {
        let line = &self.rest()[..self.to_line_end()];
        let spaces = indent_len(line);
        if spaces > 0 {
            self.bump(SyntaxKind::StringIndent, spaces);
        }
        let tail = &line[spaces..];
        if tail.is_empty() {
            return false;
        }
        let start = self.pos;
        let starts_late =
            "a multi-line string's text starts on the line after its opening `\"\"\"`";
        let Some(close) = find_quotes(tail, raw) else {
            let len = tail.trim_end_matches([' ', '\t']).len();
            self.errors.push(LexError::new(
                start..start + len,
                codes::MULTILINE_LAYOUT,
                format!("{starts_late}: move this text to the next line"),
            ));
            self.bump(SyntaxKind::StringText, tail.len());
            return false;
        };
        let content = &tail[..close];
        // The text without its trailing spaces, or the closing quotes when there is none.
        let range = match content.trim_end_matches([' ', '\t']).len() {
            0 => start + close..start + close + QUOTES.len(),
            len => start..start + len,
        };
        self.errors.push(LexError::new(
            range,
            codes::MULTILINE_LAYOUT,
            format!("{starts_late}, and it closes on a line of its own"),
        ));
        if !content.is_empty() {
            self.bump(SyntaxKind::StringText, content.len());
        }
        self.bump(SyntaxKind::StringEnd, QUOTES.len());
        true
    }

    /// A multi-line string with no closing line, which `open` starts, runs to the end of the
    /// file. Its `content` lines are each one `StringText`, with no escapes or interpolation
    /// checked, since they are more likely code than text. The E0005 goes before the string's
    /// other errors, from `first_error` on, which are later in the text.
    fn unclosed_multiline(
        &mut self,
        open: Range<usize>,
        first_error: usize,
        content: &[Range<usize>],
    ) {
        for line in content {
            self.string_newline();
            if !line.is_empty() {
                self.bump(SyntaxKind::StringText, line.len());
            }
        }
        let error = LexError::new(
            open,
            codes::UNCLOSED_MULTILINE,
            "this multi-line string has no closing `\"\"\"` line, so it runs to the end of the \
             file: end it with a line that starts with `\"\"\"` after its indentation"
                .to_string(),
        );
        self.errors.insert(first_error, error);
    }

    /// The line ending at the current position, as a `StringNewline`. A lone CR is still an
    /// E0013, and its fix to a line feed keeps the value, in which every line break is one.
    fn string_newline(&mut self) {
        let at = self.pos;
        if self.rest().starts_with("\r\n") {
            return self.bump(SyntaxKind::StringNewline, 2);
        }
        if self.rest().starts_with('\r') {
            self.lone_carriage_return(at);
        }
        self.bump(SyntaxKind::StringNewline, 1);
    }

    /// A content line's text after its indentation, up to `end`. In a raw string it is one
    /// `StringText`. Otherwise it takes a `"` string's escapes, and its interpolations when it
    /// `interpolates`, but a `"` in it is text.
    fn line_text(&mut self, end: usize, raw: bool, interpolates: bool) {
        if raw {
            if self.pos < end {
                self.control_characters(self.pos..end, ControlIn::Raw);
                self.bump(SyntaxKind::StringText, end - self.pos);
            }
            return;
        }
        while self.pos < end {
            self.string_text(end, interpolates, false);
            if self.pos < end {
                self.interpolation(end, false);
            }
        }
    }
}

/// The lines after the line ending at `at`, each up to its own line ending or the end of the
/// text. A lone CR ends a line, as it does everywhere.
fn lines_after(text: &str, mut at: usize) -> impl Iterator<Item = Range<usize>> + '_ {
    std::iter::from_fn(move || {
        let rest = &text[at..];
        let ending = match rest.as_bytes() {
            [b'\r', b'\n', ..] => 2,
            [b'\n' | b'\r', ..] => 1,
            _ => return None,
        };
        let start = at + ending;
        at = start
            + text[start..]
                .find(['\n', '\r'])
                .unwrap_or(text.len() - start);
        Some(start..at)
    })
}

/// Where the first `"""` in `line` starts. In a string that isn't `raw`, quotes that an
/// escape takes, as in `\"""`, don't count.
fn find_quotes(line: &str, raw: bool) -> Option<usize> {
    if raw {
        return line.find(QUOTES);
    }
    let mut at = 0;
    loop {
        let rest = &line[at..];
        match rest.chars().next() {
            None => return None,
            Some('\\') => at += escape(rest).len,
            Some(_) if rest.starts_with(QUOTES) => return Some(at),
            Some(c) => at += c.len_utf8(),
        }
    }
}

/// The length of the spaces and tabs that start `line`.
fn indent_len(line: &str) -> usize {
    line.len() - line.trim_start_matches([' ', '\t']).len()
}

fn is_blank(line: &str) -> bool {
    indent_len(line) == line.len()
}

fn common_prefix(a: &str, b: &str) -> usize {
    a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count()
}

/// Indentation in words, such as "4 spaces" or "a tab then 2 spaces".
fn describe(indent: &str) -> String {
    let mut runs: Vec<(u8, usize)> = Vec::new();
    for b in indent.bytes() {
        match runs.last_mut() {
            Some((last, count)) if *last == b => *count += 1,
            _ => runs.push((b, 1)),
        }
    }
    let words: Vec<String> = (runs.iter())
        .map(|&(b, count)| {
            let name = if b == b'\t' { "tab" } else { "space" };
            match count {
                1 => format!("a {name}"),
                n => format!("{n} {name}s"),
            }
        })
        .collect();
    words.join(" then ")
}

/// The E0012 for a content `line` that doesn't start with the closing line's `indent`. It
/// spans the line's indentation, or, when it has none, the line and its line ending, so that
/// it never shares a span with an error inside the line.
fn indent_error(text: &str, line: Range<usize>, indent: &str) -> LexError {
    let lead = &text[line.start..line.start + indent_len(&text[line.clone()])];
    let range = if lead.is_empty() {
        let ending = if text[line.end..].starts_with("\r\n") {
            2
        } else {
            1
        };
        line.start..line.end + ending
    } else {
        line.start..line.start + lead.len()
    };
    let expected = describe(indent);
    let message = if !indent.starts_with(lead) {
        format!(
            "this line's indentation, {}, doesn't match the closing line's, {expected}: each line \
             of a multi-line string starts with exactly the closing line's spaces and tabs",
            describe(lead)
        )
    } else if lead.is_empty() {
        format!(
            "this line isn't indented, but each line of a multi-line string starts with the \
             closing line's indentation, {expected}: indent it at least that far, or move the \
             closing `\"\"\"` left"
        )
    } else {
        format!(
            "this line is indented less than the closing `\"\"\"`: each line of a multi-line \
             string starts with the closing line's indentation, {expected}, so indent it at \
             least that far, or move the closing `\"\"\"` left"
        )
    };
    LexError::new(range, codes::MULTILINE_LAYOUT, message)
}

/// The E0012 at the closing line's `indent`, at `range`, when none of the lines, whose
/// indentations are `leads`, starts with it.
fn closing_error(range: Range<usize>, indent: &str, leads: &[&str]) -> LexError {
    let message = if leads.iter().all(|lead| indent.starts_with(lead)) {
        // Each lead is a prefix of the others, so the shortest is what they share.
        let shared = leads.iter().min_by_key(|lead| lead.len()).expect("a line");
        let fix = match shared.len() {
            0 => "put the closing `\"\"\"` at the start of its line".to_string(),
            _ => format!(
                "indent the closing `\"\"\"` with {}, as the lines are",
                describe(shared)
            ),
        };
        format!(
            "the closing `\"\"\"` is indented more than every line of the string, but each line \
             must start with the closing line's indentation: {fix}"
        )
    } else {
        format!(
            "no line of this string starts with the closing line's indentation, {}: indent the \
             lines and the closing `\"\"\"` with the same spaces and tabs",
            describe(indent)
        )
    };
    LexError::new(range, codes::MULTILINE_LAYOUT, message)
}
