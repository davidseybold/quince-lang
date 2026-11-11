//! The parser's input: the significant tokens, each with the break before it, plus what error
//! recovery needs to know about the trivia between them.

use crate::literal::char_body_len;
use crate::{Lexed, SyntaxKind};
use std::ops::Range;

/// What lies between a significant token and the one before it (decision 4 of the parser
/// design doc).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Break {
    None,
    /// A `Newline`.
    Line,
    /// A `Semicolon`, with or without a `Newline`. It never continues a line.
    Hard,
}

#[derive(Debug, Clone)]
pub(super) struct Sig {
    pub kind: SyntaxKind,
    /// The token's index in the lexer's tokens.
    pub index: usize,
    pub range: Range<usize>,
    pub brk: Break,
    /// An `Error` token lies between this token and the one before it.
    pub after_error: bool,
    /// When this is the first token after a line that holds an unclosed one-line literal, where
    /// that line starts. The lists opened on that line close before this token.
    pub closes_line_from: Option<usize>,
}

pub(super) struct Input {
    pub tokens: Vec<Sig>,
    /// The lines that hold an unclosed one-line literal, from their start up to their line
    /// ending, in order. Nothing the parser would report on them is reported.
    pub unclosed_lines: Vec<Range<usize>>,
    /// An `Error` token lies after the last significant token.
    pub eof_after_error: bool,
    /// A multi-line string or a block comment runs to the end of the file unclosed, so what
    /// the parser would report there is part of that slip.
    pub eof_swallowed: bool,
}

impl Input {
    pub fn new(text: &str, lexed: &Lexed) -> Input {
        use SyntaxKind::*;
        let mut input = Input {
            tokens: Vec::new(),
            unclosed_lines: Vec::new(),
            eof_after_error: false,
            eof_swallowed: false,
        };
        let mut brk = Break::None;
        let mut after_error = false;
        let mut closes_line_from = None;
        let mut line_start = 0;
        // A one-line literal on this line has no closing quote.
        let mut unclosed = false;
        // The multi-line string the lexer is in has no closing quotes yet.
        let mut open_multiline = false;
        for (index, (kind, range)) in lexed.ranges().enumerate() {
            let token_text = &text[range.clone()];
            match kind {
                Newline => {
                    if unclosed {
                        input.unclosed_lines.push(line_start..range.start);
                        closes_line_from = Some(line_start);
                        unclosed = false;
                    }
                    line_start = range.end;
                    if brk == Break::None {
                        brk = Break::Line;
                    }
                    continue;
                }
                Semicolon => brk = Break::Hard,
                Error => after_error = true,
                DocComment => {}
                _ if kind.is_trivia() => {}
                _ => {
                    match kind {
                        StringStart if token_text.ends_with("\"\"\"") => open_multiline = true,
                        StringStart => unclosed = true,
                        StringEnd => (unclosed, open_multiline) = (false, false),
                        StringNewline => line_start = range.end,
                        Char if 1 + char_body_len(&token_text[1..]) == token_text.len() => {
                            unclosed = true;
                        }
                        _ => {}
                    }
                    // Nothing comes before the first token, so no break does.
                    if input.tokens.is_empty() {
                        brk = Break::None;
                    }
                    input.tokens.push(Sig {
                        kind,
                        index,
                        range,
                        brk,
                        after_error,
                        closes_line_from: closes_line_from.take(),
                    });
                    brk = Break::None;
                    after_error = false;
                }
            }
        }
        if unclosed {
            input.unclosed_lines.push(line_start..text.len());
        }
        input.eof_after_error = after_error;
        let unclosed_comment = lexed.tokens.last().is_some_and(|last| {
            let comment = &text[text.len() - last.len as usize..];
            last.kind == BlockComment && (comment.len() < 4 || !comment.ends_with("*/"))
        });
        input.eof_swallowed = open_multiline || unclosed_comment;
        input
    }

    /// True when an error starting at `offset` lies on a line with an unclosed one-line
    /// literal.
    pub fn on_unclosed_line(&self, offset: usize) -> bool {
        let after = self
            .unclosed_lines
            .partition_point(|line| line.end < offset);
        self.unclosed_lines
            .get(after)
            .is_some_and(|line| line.start <= offset)
    }
}
