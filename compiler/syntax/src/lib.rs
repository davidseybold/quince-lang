//! Lexer, parser, lossless CST and formatter for Quince source.
//!
//! The token contract and the error contract the parser builds on are in this crate's
//! `README.md`.

mod cst;
mod kind;
mod lexer;
pub mod literal;
mod source;

pub use cst::{QuinceLanguage, SyntaxElement, SyntaxNode, SyntaxToken};
pub use kind::SyntaxKind;
pub use lexer::{Lexed, Token, lex, lex_str};
pub use source::{Decoded, decode_source};

use quince_diagnostics::{Applicability, Code, Diagnostic, Edit, Fix, LineIndex, Span};
use std::ops::Range;

/// The codes this crate reports. Each has a row in `quince_diagnostics`'s registry.
pub mod codes {
    pub const UNKNOWN_CHARACTER: &str = "E0001";
    pub const SEMICOLON: &str = "E0002";
    pub const BLOCK_COMMENT: &str = "E0003";
    pub const UNCLOSED_STRING: &str = "E0004";
    pub const UNCLOSED_MULTILINE: &str = "E0005";
    pub const INVALID_ESCAPE: &str = "E0006";
    pub const INVALID_INTERPOLATION: &str = "E0007";
    pub const MALFORMED_NUMBER: &str = "E0008";
    pub const INVALID_CHAR: &str = "E0009";
    pub const UNKNOWN_TAG: &str = "E0010";
    pub const NON_ASCII_BYTES: &str = "E0011";
    pub const MULTILINE_LAYOUT: &str = "E0012";
    pub const LONE_CARRIAGE_RETURN: &str = "E0013";
    pub const INVALID_UTF8: &str = "E0014";
    pub const CONTROL_CHARACTER: &str = "E0015";
    /// The codes the lexer and `decode_source` report. The lexer's mutation test requires
    /// every one of them from its mutants.
    pub const LEXER: &[&str] = &[
        UNKNOWN_CHARACTER,
        SEMICOLON,
        BLOCK_COMMENT,
        UNCLOSED_STRING,
        UNCLOSED_MULTILINE,
        INVALID_ESCAPE,
        INVALID_INTERPOLATION,
        MALFORMED_NUMBER,
        INVALID_CHAR,
        UNKNOWN_TAG,
        NON_ASCII_BYTES,
        MULTILINE_LAYOUT,
        LONE_CARRIAGE_RETURN,
        INVALID_UTF8,
        CONTROL_CHARACTER,
    ];
    /// The codes the parser reports.
    pub const PARSER: &[&str] = &[];

    /// Every code this crate reports.
    pub fn all() -> impl Iterator<Item = &'static str> {
        LEXER.iter().chain(PARSER).copied()
    }
}

/// A problem found while decoding, lexing or parsing, in byte offsets of the decoded text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxError {
    pub range: Range<u32>,
    /// One of [`codes`].
    pub code: &'static str,
    pub message: String,
    pub fixes: Vec<SyntaxFix>,
}

/// A fix whose edits replace byte ranges of the decoded text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxFix {
    pub description: String,
    pub applicability: Applicability,
    pub edits: Vec<(Range<u32>, String)>,
}

impl SyntaxError {
    fn new(range: Range<usize>, code: &'static str, message: String) -> SyntaxError {
        SyntaxError {
            range: offset(range.start)..offset(range.end),
            code,
            message,
            fixes: Vec::new(),
        }
    }

    fn with_fix(
        mut self,
        description: &str,
        applicability: Applicability,
        edits: Vec<(Range<usize>, String)>,
    ) -> SyntaxError {
        let edits = edits
            .into_iter()
            .map(|(range, text)| (offset(range.start)..offset(range.end), text))
            .collect();
        self.fixes.push(SyntaxFix {
            description: description.to_string(),
            applicability,
            edits,
        });
        self
    }

    /// This error as a diagnostic in `file`, with positions from `index`, the index of the
    /// decoded text.
    pub fn to_diagnostic(&self, file: &str, index: &LineIndex) -> Diagnostic {
        let code = Code::parse(self.code).expect("syntax codes are well formed");
        let span = |range: &Range<u32>| Span {
            file: file.to_string(),
            start: index.position(range.start as usize),
            end: index.position(range.end as usize),
        };
        let mut diagnostic = Diagnostic::new(
            code.clone(),
            code.default_severity(),
            self.message.clone(),
            span(&self.range),
        );
        diagnostic.fixes = self
            .fixes
            .iter()
            .map(|fix| Fix {
                description: fix.description.clone(),
                applicability: fix.applicability,
                edits: fix
                    .edits
                    .iter()
                    .map(|(range, text)| {
                        let span = span(range);
                        Edit {
                            file: None,
                            start: span.start,
                            end: span.end,
                            text: text.clone(),
                        }
                    })
                    .collect(),
            })
            .collect();
        diagnostic
    }
}

/// Source text is at most 4 GiB ([`decode_source`]), so every offset fits.
fn offset(n: usize) -> u32 {
    u32::try_from(n).expect("source offsets fit in u32")
}

#[cfg(test)]
mod tests;
