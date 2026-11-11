//! The lexer: source text to a lossless token stream (`README.md` has the contract).

use crate::SyntaxKind;
use std::ops::Range;

/// A token is its kind and its length in bytes. It starts where the previous token ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    pub kind: SyntaxKind,
    pub len: u32,
}

/// The tokens of a text, whose texts concatenate to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lexed {
    pub tokens: Vec<Token>,
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
}

/// Lexes source text, which is at most 4 GiB so that every offset fits in a `u32`.
pub fn lex_str(text: &str) -> Lexed {
    Lexer::new(text).run()
}

struct Lexer<'a> {
    text: &'a str,
    pos: usize,
    tokens: Vec<Token>,
}

impl<'a> Lexer<'a> {
    fn new(text: &'a str) -> Self {
        Lexer {
            text,
            pos: 0,
            tokens: Vec::new(),
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
        }
    }

    fn rest(&self) -> &'a str {
        &self.text[self.pos..]
    }

    /// Adds a token of `len` bytes at the current position.
    fn bump(&mut self, kind: SyntaxKind, len: usize) {
        debug_assert!(len > 0, "{kind:?} is empty");
        let len = u32::try_from(len).expect("a token fits in u32");
        self.tokens.push(Token { kind, len });
        self.pos += len as usize;
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
        let rest = self.rest();
        let c = rest.chars().next().expect("not at the end");
        match c {
            ' ' | '\t' => self.bump(
                SyntaxKind::Whitespace,
                self.run_of(|c| c == ' ' || c == '\t'),
            ),
            '\n' => self.bump(SyntaxKind::Newline, 1),
            '\r' if rest.starts_with("\r\n") => self.bump(SyntaxKind::Newline, 2),
            // A lone CR ends a line too, as it does for positions.
            '\r' => self.bump(SyntaxKind::Newline, 1),
            '/' if rest.starts_with("//") => {
                let doc = rest.starts_with("///") && !rest.starts_with("////");
                let kind = if doc {
                    SyntaxKind::DocComment
                } else {
                    SyntaxKind::Comment
                };
                self.bump(kind, self.to_line_end());
            }
            '/' if rest.starts_with("/*") => {
                let len = rest[2..].find("*/").map_or(rest.len(), |close| close + 4);
                self.bump(SyntaxKind::BlockComment, len);
            }
            'a'..='z' | 'A'..='Z' | '_' => {
                let len = self.run_of(|c| c.is_ascii_alphanumeric() || c == '_');
                let kind = match &rest[..len] {
                    "_" => SyntaxKind::Underscore,
                    word => SyntaxKind::keyword(word).unwrap_or(SyntaxKind::Ident),
                };
                self.bump(kind, len);
            }
            // Until numbers have their own rules: a digit and the letters, digits and
            // underscores after it.
            '0'..='9' => self.bump(
                SyntaxKind::Int,
                self.run_of(|c| c.is_ascii_alphanumeric() || c == '_'),
            ),
            '"' => self.string(),
            '\'' => self.char_literal(),
            ';' => self.bump(SyntaxKind::Semicolon, 1),
            _ => match punctuation(rest) {
                Some((kind, len)) => self.bump(kind, len),
                None => self.unknown_characters(),
            },
        }
    }

    /// Until strings have their own rules: everything from a `"` to the next `"` on the line.
    fn string(&mut self) {
        self.bump(SyntaxKind::StringStart, 1);
        let len = self
            .rest()
            .find(['"', '\n', '\r'])
            .unwrap_or(self.rest().len());
        if len > 0 {
            self.bump(SyntaxKind::StringText, len);
        }
        if self.rest().starts_with('"') {
            self.bump(SyntaxKind::StringEnd, 1);
        }
    }

    /// Until chars have their own rules: everything from a `'` to the next `'` on the line.
    fn char_literal(&mut self) {
        let rest = &self.rest()[1..];
        let len = match rest.find(['\'', '\n', '\r']) {
            Some(i) if rest[i..].starts_with('\'') => i + 2,
            found => found.unwrap_or(rest.len()) + 1,
        };
        self.bump(SyntaxKind::Char, len);
    }

    /// A run of characters that start no token, as one `Error` token.
    fn unknown_characters(&mut self) {
        let mut len = 0;
        for c in self.rest().chars() {
            if len > 0 && starts_token(c, &self.rest()[len..]) {
                break;
            }
            len += c.len_utf8();
        }
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

/// True when `c`, at the start of `text`, starts a token other than an `Error`.
fn starts_token(c: char, text: &str) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(c, '_' | ' ' | '\t' | '\n' | '\r' | '"' | '\'' | ';')
        || punctuation(text).is_some()
}
