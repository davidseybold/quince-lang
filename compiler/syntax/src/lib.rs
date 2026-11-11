//! Lexer, parser, lossless CST and formatter for Quince source.
//!
//! The token contract the parser builds on is in this crate's `README.md`.

mod kind;
mod lexer;

pub use kind::SyntaxKind;
pub use lexer::{Lexed, Token, lex_str};

#[cfg(test)]
mod tests;
