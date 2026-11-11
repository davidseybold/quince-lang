//! The lexer: source text to a lossless token stream (`README.md` has the contract).

use crate::{Decoded, LexError, SyntaxKind, codes};
use quince_diagnostics::{Applicability, Diagnostic, LineIndex};
use std::ops::Range;

mod number;
mod string;

/// A token is its kind and its length in bytes. It starts where the previous token ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    pub kind: SyntaxKind,
    pub len: u32,
}

/// The tokens of a text, whose texts concatenate to it, and every problem found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lexed {
    pub tokens: Vec<Token>,
    /// Decoding errors, then lexing errors, each in order of position.
    pub errors: Vec<LexError>,
}

impl Lexed {
    /// Each token with its byte range.
    pub fn ranges(&self) -> impl Iterator<Item = (SyntaxKind, Range<usize>)> + '_ {
        self.tokens.iter().scan(0, |start, token| {
            let range = *start..*start + token.len as usize;
            *start = range.end;
            Some((token.kind, range))
        })
    }

    /// Every error as a diagnostic in `file`. `text` is the text that was lexed.
    pub fn diagnostics(&self, text: &str, file: &str) -> Vec<Diagnostic> {
        let index = LineIndex::new(text);
        self.errors
            .iter()
            .map(|error| error.to_diagnostic(file, &index))
            .collect()
    }
}

/// Lexes decoded source. A U+FFFD that decoding put in place of invalid bytes becomes an
/// `Error` token with no diagnostic of its own, since decoding reported it.
pub fn lex(decoded: &Decoded) -> Lexed {
    let mut lexed = Lexer::new(&decoded.text, &decoded.replaced).run();
    lexed.errors.splice(0..0, decoded.errors.iter().cloned());
    lexed
}

/// Lexes text that never came from bytes, so nothing in it was replaced. The text is at most
/// 4 GiB, as [`crate::decode_source`] ensures for files, so that every offset fits in a `u32`.
pub fn lex_str(text: &str) -> Lexed {
    Lexer::new(text, &[]).run()
}

struct Lexer<'a> {
    text: &'a str,
    pos: usize,
    /// Ranges decoding replaced that the lexer hasn't reached yet.
    replaced: &'a [Range<u32>],
    /// Where a one-line literal with no closing quote stopped: a line ending.
    unclosed_literal_end: Option<usize>,
    tokens: Vec<Token>,
    errors: Vec<LexError>,
}

impl<'a> Lexer<'a> {
    fn new(text: &'a str, replaced: &'a [Range<u32>]) -> Self {
        Lexer {
            text,
            pos: 0,
            replaced,
            unclosed_literal_end: None,
            tokens: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn run(mut self) -> Lexed {
        if self.text.starts_with('\u{FEFF}') {
            self.bump(SyntaxKind::Bom, '\u{FEFF}'.len_utf8());
        }
        while self.pos < self.text.len() {
            self.token();
        }
        Lexed {
            tokens: self.tokens,
            errors: self.errors,
        }
    }

    fn rest(&self) -> &'a str {
        &self.text[self.pos..]
    }

    /// Adds a token of `len` bytes at the current position.
    fn bump(&mut self, kind: SyntaxKind, len: usize) {
        debug_assert!(len > 0, "{kind:?} is empty");
        self.tokens.push(Token {
            kind,
            len: crate::offset(len),
        });
        self.pos += len;
    }

    /// The length of the longest prefix of the rest whose characters all match `f`.
    fn run_of(&self, f: impl Fn(char) -> bool) -> usize {
        self.rest().find(|c| !f(c)).unwrap_or(self.rest().len())
    }

    /// The length of the rest of the line, up to its line ending or the end of input.
    fn to_line_end(&self) -> usize {
        self.rest().find(['\n', '\r']).unwrap_or(self.rest().len())
    }

    fn token(&mut self) {
        let start = self.pos;
        // Replacements inside a comment or literal are part of it.
        let passed = self
            .replaced
            .partition_point(|r| (r.start as usize) < start);
        self.replaced = &self.replaced[passed..];
        if self
            .replaced
            .first()
            .is_some_and(|r| r.start as usize == start)
        {
            return self.bump(SyntaxKind::Error, char::REPLACEMENT_CHARACTER.len_utf8());
        }
        let rest = self.rest();
        let c = rest.chars().next().expect("not at the end");
        match c {
            ' ' | '\t' => self.bump(
                SyntaxKind::Whitespace,
                self.run_of(|c| c == ' ' || c == '\t'),
            ),
            '\n' => self.bump(SyntaxKind::Newline, 1),
            '\r' if rest.starts_with("\r\n") => self.bump(SyntaxKind::Newline, 2),
            '\r' => {
                self.lone_carriage_return(start);
                self.bump(SyntaxKind::Newline, 1);
            }
            '/' if rest.starts_with("//") => {
                let doc = rest.starts_with("///") && !rest.starts_with("////");
                let kind = if doc {
                    SyntaxKind::DocComment
                } else {
                    SyntaxKind::Comment
                };
                self.bump(kind, self.to_line_end());
            }
            '/' if rest.starts_with("/*") => self.block_comment(),
            'a'..='z' | 'A'..='Z' | '_' => {
                let len = self.run_of(|c| c.is_ascii_alphanumeric() || c == '_');
                let kind = match &rest[..len] {
                    "_" => SyntaxKind::Underscore,
                    word => SyntaxKind::keyword(word).unwrap_or(SyntaxKind::Ident),
                };
                self.bump(kind, len);
            }
            '0'..='9' => self.number(),
            '"' => self.string(),
            '\'' => self.char_literal(),
            ';' => self.semicolon(),
            _ => match punctuation(rest) {
                Some((kind, len)) => self.bump(kind, len),
                None => self.unknown_characters(),
            },
        }
    }

    /// Until chars have their own rules: everything from a `'` to the next `'` on the line.
    fn char_literal(&mut self) {
        let rest = &self.rest()[1..];
        let len = match rest.find(['\'', '\n', '\r']) {
            Some(i) if rest[i..].starts_with('\'') => i + 2,
            found => {
                let len = found.unwrap_or(rest.len()) + 1;
                self.unclosed_literal_end = Some(self.pos + len);
                len
            }
        };
        self.bump(SyntaxKind::Char, len);
    }

    fn lone_carriage_return(&mut self, at: usize) {
        let error = LexError::new(
            at..at + 1,
            codes::LONE_CARRIAGE_RETURN,
            "a carriage return must be followed by a line feed: a line ends at LF or CR LF"
                .to_string(),
        );
        // A CR that ends an unclosed literal would leave it unclosed if it became a line feed.
        let error = if self.unclosed_literal_end == Some(at) {
            error
        } else {
            error.with_fix(
                "replace it with a line feed",
                Applicability::Safe,
                vec![(at..at + 1, "\n".to_string())],
            )
        };
        self.errors.push(error);
    }

    fn block_comment(&mut self) {
        let start = self.pos;
        let close = self.rest()[2..].find("*/");
        let end = close.map_or(self.text.len(), |close| start + 2 + close + 2);
        let error = match close {
            Some(_) => {
                let error = LexError::new(
                    start..end,
                    codes::BLOCK_COMMENT,
                    "Quince has no block comments; use `//` line comments".to_string(),
                );
                match self.as_line_comments(start, end) {
                    Some(lines) => error.with_fix(
                        "rewrite it as `//` comments",
                        Applicability::Safe,
                        vec![(start..end, lines)],
                    ),
                    None => error,
                }
            }
            None => LexError::new(
                start..start + 2,
                codes::BLOCK_COMMENT,
                "Quince has no block comments, and this one is never closed, so it runs to the \
                 end of the file; use `//` line comments"
                    .to_string(),
            ),
        };
        let comment = &self.text[start..end];
        self.errors.push(error);
        // A lone CR inside the comment is still a line ending, and still an error.
        for (i, _) in comment.match_indices('\r') {
            if !comment[i..].starts_with("\r\n") {
                self.lone_carriage_return(start + i);
            }
        }
        self.bump(SyntaxKind::BlockComment, end - start);
    }

    /// The `//` lines that replace a block comment from `start` to `end`, when it covers
    /// whole lines: only spaces and tabs before it and after it on their lines.
    fn as_line_comments(&self, start: usize, end: usize) -> Option<String> {
        let blank = |s: &str| s.chars().all(|c| c == ' ' || c == '\t');
        let line_start = self.text[..start].rfind(['\n', '\r']).map_or(0, |i| i + 1);
        let indent = self.text[line_start..start].trim_start_matches('\u{FEFF}');
        let after = &self.text[end..];
        let body = &self.text[start + 2..end - 2];
        // `lines()` doesn't split at a lone CR, which ends a line, so a `//` line would stop at
        // it and turn the text after it into code. E0013 reports the CR, and its fix comes first.
        let lone_cr = body.replace("\r\n", "\n").contains('\r');
        if lone_cr
            || !blank(indent)
            || !blank(&after[..after.find(['\n', '\r']).unwrap_or(after.len())])
        {
            return None;
        }
        // Each line, without a leading `* ` as in `/** ... */` comments.
        let lines: Vec<&str> = (body.lines())
            .map(|line| match line.trim() {
                "*" => "",
                line => line.strip_prefix("* ").unwrap_or(line),
            })
            .collect();
        let first = lines.iter().position(|line| !line.is_empty());
        let last = lines.iter().rposition(|line| !line.is_empty());
        let lines = first.zip(last).map_or(&[""][..], |(f, l)| &lines[f..=l]);
        let comments: Vec<String> = (lines.iter())
            .map(|line| format!("// {line}").trim_end().to_string())
            .collect();
        let newline = if body.contains("\r\n") { "\r\n" } else { "\n" };
        Some(comments.join(&format!("{newline}{indent}")))
    }

    fn semicolon(&mut self) {
        let start = self.pos;
        let after = self.rest()[1..].trim_start_matches([' ', '\t']);
        let at_line_end =
            after.is_empty() || after.starts_with(['\n', '\r']) || after.starts_with("//");
        // Deleting a `;` between `/` and `/` would make a doc comment.
        let joins_slashes = self.text[..start].ends_with('/') && self.rest()[1..].starts_with('/');
        let delete = vec![(start..start + 1, String::new())];
        let error = if at_line_end {
            let error = LexError::new(
                start..start + 1,
                codes::SEMICOLON,
                "a statement ends at the line break, so it needs no `;`".to_string(),
            );
            if joins_slashes {
                error
            } else if next_line_starts_with_dot(after) {
                // `a;` then `.b()` is two statements, and `a` then `.b()` is one.
                error.with_fix(
                    "delete the `;`, which joins the next line, starting with `.`, to this statement",
                    Applicability::NeedsReview,
                    delete,
                )
            } else {
                error.with_fix("delete the `;`", Applicability::Safe, delete)
            }
        } else {
            LexError::new(
                start..start + 1,
                codes::SEMICOLON,
                "statements end at line breaks, not at `;`: put the next statement on a line of its own".to_string(),
            )
        };
        self.errors.push(error);
        self.bump(SyntaxKind::Semicolon, 1);
    }

    /// A run of characters that start no token, as one `Error` token and one E0001.
    fn unknown_characters(&mut self) {
        let start = self.pos;
        let mut len = 0;
        for c in self.rest().chars() {
            let at = start + len;
            let replaced = self
                .replaced
                .first()
                .is_some_and(|r| r.start as usize == at);
            if replaced || (len > 0 && starts_token(c, &self.text[at..])) {
                break;
            }
            len += c.len_utf8();
        }
        let run = &self.text[start..start + len];
        // The `#`s of a Rust raw string, `r#"..."#`: before its opening quote, or after its end.
        let previous = self.tokens.last().map(|t| t.kind);
        let opens = previous == Some(SyntaxKind::Ident)
            && self.text[..start].ends_with('r')
            && self.tokens.last().is_some_and(|t| t.len == 1)
            && self.text[start + len..].starts_with('"');
        // A `#` glued to a closing quote, as in `"x"# note`, gets the hint too. A comment
        // written from habit has a space before its `#`, so this is nearly always a raw string.
        let raw_hashes = run.starts_with('#') && (opens || previous == Some(SyntaxKind::StringEnd));
        let mut error = LexError::new(
            start..start + len,
            codes::UNKNOWN_CHARACTER,
            unknown_message(run, raw_hashes),
        );
        if run.chars().all(is_other_space) {
            let spaces = " ".repeat(run.chars().count());
            error = error.with_fix(
                "replace it with spaces",
                Applicability::Safe,
                vec![(start..start + len, spaces)],
            );
        }
        self.errors.push(error);
        self.bump(SyntaxKind::Error, len);
    }
}

/// The longest punctuation token at the start of `text`.
fn punctuation(text: &str) -> Option<(SyntaxKind, usize)> {
    SyntaxKind::PUNCTUATION
        .iter()
        .filter(|(punct, _)| text.starts_with(punct))
        .max_by_key(|(punct, _)| punct.len())
        .map(|&(punct, kind)| (kind, punct.len()))
}

/// True when the first token after `text`'s line, skipping whitespace and comments, block
/// comments included, is a `.`, which continues a statement (`r[lex.continue.more]`).
fn next_line_starts_with_dot(mut text: &str) -> bool {
    loop {
        text = text.trim_start_matches([' ', '\t', '\n', '\r']);
        if text.starts_with("//") {
            text = &text[text.find(['\n', '\r']).unwrap_or(text.len())..];
        } else if let Some(comment) = text.strip_prefix("/*") {
            // An unclosed block comment runs to the end of the file.
            let Some(end) = comment.find("*/") else {
                return false;
            };
            text = &comment[end + 2..];
        } else {
            return text.starts_with('.') && !text.starts_with("..");
        }
    }
}

/// True when `c`, at the start of `text`, starts a token other than an `Error`.
fn starts_token(c: char, text: &str) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(c, '_' | ' ' | '\t' | '\n' | '\r' | '"' | '\'' | ';')
        || punctuation(text).is_some()
}

/// Whitespace other than spaces and line endings, such as a no-break space or a form feed.
fn is_other_space(c: char) -> bool {
    c.is_whitespace() && !matches!(c, ' ' | '\t' | '\n' | '\r')
}

fn unknown_message(run: &str, raw_hashes: bool) -> String {
    let first = run.chars().next().expect("a run isn't empty");
    let shown = match first {
        '`' => "a backtick".to_string(),
        c if c.is_ascii_graphic() => format!("`{c}`"),
        c => format!("U+{:04X}", c as u32),
    };
    let more = match run.chars().count() {
        1 => String::new(),
        2 => " and the character after it".to_string(),
        n => format!(" and the {} characters after it", n - 1),
    };
    let why = match first {
        '#' if raw_hashes => {
            "Quince has no `r#\"` strings: raw text that holds quotes is written `r\"\"\"`"
        }
        '#' => "comments start with `//`",
        '`' => "strings use `\"`, and interpolate with `{name}`",
        '\u{FEFF}' => "a byte order mark is allowed only at the start of a file",
        c if is_other_space(c) => "within a line, only spaces and tabs are whitespace",
        c if !c.is_ascii() => {
            "outside literals and comments Quince source is ASCII, so names use ASCII letters, digits and `_`"
        }
        _ => "it doesn't start any Quince token",
    };
    format!("{shown}{more} can't appear here: {why}")
}
