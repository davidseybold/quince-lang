//! Tests: the lexer's token dumps and invariants here, and each area in its own module.

use crate::{Lexed, SyntaxKind, lex_str};
use std::path::Path;

mod chars;
mod closures;
mod concurrency;
mod control;
mod cst;
mod decls;
mod errors;
mod exprs;
mod members;
mod multiline;
mod mutations;
mod names;
mod numbers;
mod parser;
mod patterns;
mod prefixed;
mod recovery;
mod stmts;
mod strings;
mod types;

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

/// The keywords of `r[lex.keywords]`, copied by hand so the table can't lose, gain or rename
/// one unnoticed.
const KEYWORDS: [&str; 38] = [
    "fn",
    "let",
    "var",
    "type",
    "struct",
    "enum",
    "interface",
    "impl",
    "match",
    "is",
    "if",
    "else",
    "for",
    "in",
    "while",
    "return",
    "break",
    "continue",
    "import",
    "pub",
    "uses",
    "requires",
    "ensures",
    "test",
    "forall",
    "with",
    "concurrent",
    "spawn",
    "background",
    "foreign",
    "true",
    "false",
    "self",
    "Self",
    "const",
    "effect",
    "select",
    "as",
];

/// The tokens after which a line always continues (decision 4 of the lexer design doc), copied
/// by hand.
const CONTINUES_LINE: [&str; 29] = [
    ",", "(", "[", "{", "=", "+=", "-=", "*=", "/=", "%=", "=>", "->", "+", "-", "/", "%", "&",
    "|", "^", "<<", "&&", "||", "==", "!=", "<", "<=", "..", "..=", "is",
];

/// The keyword table is exactly [`KEYWORDS`], and every keyword and punctuation token lexes as
/// one token of its kind.
#[test]
fn keywords_and_punctuation() {
    let mut ours: Vec<&str> = SyntaxKind::KEYWORDS.iter().map(|(text, _)| *text).collect();
    let mut expected = KEYWORDS.to_vec();
    ours.sort();
    expected.sort();
    assert_eq!(ours, expected);
    for &(text, kind) in SyntaxKind::KEYWORDS.iter().chain(SyntaxKind::PUNCTUATION) {
        assert_eq!(single_token(text), kind, "{text}");
    }
}

/// A kind continues a line exactly when it is a keyword or punctuation in [`CONTINUES_LINE`].
#[test]
fn continues_line() {
    for text in CONTINUES_LINE {
        assert!(
            single_token(text).continues_line(),
            "{text} must continue a line"
        );
    }
    for &(text, kind) in SyntaxKind::PUNCTUATION.iter().chain(SyntaxKind::KEYWORDS) {
        assert_eq!(
            kind.continues_line(),
            CONTINUES_LINE.contains(&text),
            "{text}"
        );
    }
    for kind in SyntaxKind::TOKENS {
        assert!(!kind.continues_line(), "{kind:?} must not continue a line");
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
