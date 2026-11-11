//! Lexer tests: token dumps and the invariants.

use crate::{Lexed, SyntaxKind, lex_str};
use std::path::Path;

mod chars;
mod control;
mod errors;
mod multiline;
mod mutations;
mod numbers;
mod prefixed;
mod strings;

/// `Kind "text"`, one token per line, after checking the invariants.
fn dump(text: &str) -> String {
    let lexed = lex_str(text);
    check_invariants(text, &lexed);
    lexed
        .ranges()
        .map(|(kind, range)| format!("{kind:?} {:?}\n", &text[range]))
        .collect()
}

/// No token is empty, and the tokens cover the text exactly.
fn check_invariants(text: &str, lexed: &Lexed) {
    assert!(
        lexed.tokens.iter().all(|t| t.len > 0),
        "{text:?}: an empty token"
    );
    let joined: String = lexed.ranges().map(|(_, range)| &text[range]).collect();
    assert_eq!(joined, text);
}

#[test]
fn a_small_function() {
    assert_eq!(
        dump("fn add(a: Int) -> Int {\n\ta + 1 // sum\n}"),
        r#"FnKw "fn"
Whitespace " "
Ident "add"
LParen "("
Ident "a"
Colon ":"
Whitespace " "
Ident "Int"
RParen ")"
Whitespace " "
Arrow "->"
Whitespace " "
Ident "Int"
Whitespace " "
LBrace "{"
Newline "\n"
Whitespace "\t"
Ident "a"
Whitespace " "
Plus "+"
Whitespace " "
Int "1"
Whitespace " "
Comment "// sum"
Newline "\n"
RBrace "}"
"#
    );
}

#[test]
fn greater_than_is_always_alone() {
    let kinds = |text| {
        lex_str(text)
            .tokens
            .iter()
            .map(|t| t.kind)
            .collect::<Vec<_>>()
    };
    use SyntaxKind::*;
    assert_eq!(kinds(">>="), [Gt, Gt, Eq]);
    assert_eq!(
        kinds("List<List<Int>>"),
        [Ident, Lt, Ident, Lt, Ident, Gt, Gt]
    );
    assert_eq!(kinds("a..=b..c"), [Ident, DotDotEq, Ident, DotDot, Ident]);
    assert_eq!(
        kinds("<<= => == != -> -="),
        [
            LtLt, Eq, Whitespace, FatArrow, Whitespace, EqEq, Whitespace, BangEq, Whitespace,
            Arrow, Whitespace, MinusEq
        ]
    );
}

#[test]
fn identifiers_keywords_and_underscore() {
    assert_eq!(
        dump("_ _x Self self TaskScope main x2"),
        "Underscore \"_\"\nWhitespace \" \"\nIdent \"_x\"\nWhitespace \" \"\nSelfTypeKw \"Self\"\n\
         Whitespace \" \"\nSelfKw \"self\"\nWhitespace \" \"\nIdent \"TaskScope\"\nWhitespace \" \"\n\
         Ident \"main\"\nWhitespace \" \"\nIdent \"x2\"\n"
    );
}

#[test]
fn comments() {
    assert_eq!(
        dump("// a\n/// b\n//// c\r\n///"),
        "Comment \"// a\"\nNewline \"\\n\"\nDocComment \"/// b\"\nNewline \"\\n\"\n\
         Comment \"//// c\"\nNewline \"\\r\\n\"\nDocComment \"///\"\n"
    );
}

#[test]
fn line_endings_and_byte_order_marks() {
    assert_eq!(
        dump("\u{FEFF}a\r\nb\rc\n\u{FEFF}"),
        "Bom \"\\u{feff}\"\nIdent \"a\"\nNewline \"\\r\\n\"\nIdent \"b\"\nNewline \"\\r\"\n\
         Ident \"c\"\nNewline \"\\n\"\nError \"\\u{feff}\"\n"
    );
}

/// The tokens that later carry an error: a block comment, a `;` and a run of characters that
/// start no token.
#[test]
fn tokens_for_errors() {
    assert_eq!(
        dump("a/* b\n*/;$\u{E9}`(/*"),
        "Ident \"a\"\nBlockComment \"/* b\\n*/\"\nSemicolon \";\"\nError \"$\u{e9}`\"\nLParen \"(\"\n\
         BlockComment \"/*\"\n"
    );
    assert_eq!(dump("/**/"), "BlockComment \"/**/\"\n");
}

/// Every keyword and punctuation token in the lexer's tables lexes as one token of its kind.
#[test]
fn every_keyword_and_punctuation_is_one_token() {
    for &(text, kind) in SyntaxKind::KEYWORDS.iter().chain(SyntaxKind::PUNCTUATION) {
        assert_eq!(single_token(text), kind, "{text}");
    }
}

fn repo_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn single_token(text: &str) -> SyntaxKind {
    let lexed = lex_str(text);
    assert!(lexed.errors.is_empty(), "{text}: {:?}", lexed.errors);
    assert_eq!(lexed.tokens.len(), 1, "{text} lexes as {:?}", lexed.tokens);
    lexed.tokens[0].kind
}
