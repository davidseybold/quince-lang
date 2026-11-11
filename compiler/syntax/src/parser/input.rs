//! The parser's input: the significant tokens, each with the break before it.

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
}

pub(super) struct Input {
    pub tokens: Vec<Sig>,
}

impl Input {
    pub fn new(lexed: &Lexed) -> Input {
        use SyntaxKind::*;
        let mut tokens: Vec<Sig> = Vec::new();
        let mut brk = Break::None;
        for (index, (kind, range)) in lexed.ranges().enumerate() {
            match kind {
                Newline if brk == Break::None => brk = Break::Line,
                Semicolon => brk = Break::Hard,
                DocComment => {}
                _ if kind.is_trivia() => {}
                _ => {
                    // Nothing comes before the first token, so no break does.
                    if tokens.is_empty() {
                        brk = Break::None;
                    }
                    tokens.push(Sig {
                        kind,
                        index,
                        range,
                        brk,
                    });
                    brk = Break::None;
                }
            }
        }
        Input { tokens }
    }
}
