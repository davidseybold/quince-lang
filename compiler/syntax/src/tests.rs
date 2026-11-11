//! Lexer tests: token dumps, the invariants, and sync with the spec.

use crate::{Lexed, SyntaxKind, lex_str};
use std::path::Path;

mod errors;

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

/// Until their own layers, literals lex by a fallback rule (`docs/design/m1-lexer.md`).
#[test]
fn literal_fallbacks() {
    assert_eq!(
        dump("\"a b\" \"\"\"x"),
        "StringStart \"\\\"\"\nStringText \"a b\"\nStringEnd \"\\\"\"\nWhitespace \" \"\n\
         StringStart \"\\\"\"\nStringEnd \"\\\"\"\nStringStart \"\\\"\"\nStringText \"x\"\n"
    );
    assert_eq!(
        dump("re\"[a]\""),
        "Ident \"re\"\nStringStart \"\\\"\"\nStringText \"[a]\"\nStringEnd \"\\\"\"\n"
    );
    assert_eq!(
        dump("'a' 'b\n"),
        "Char \"'a'\"\nWhitespace \" \"\nChar \"'b\"\nNewline \"\\n\"\n"
    );
    assert_eq!(
        dump("1_000 0xFF 2u8"),
        "Int \"1_000\"\nWhitespace \" \"\nInt \"0xFF\"\nWhitespace \" \"\nInt \"2u8\"\n"
    );
}

#[test]
fn continues_line_is_the_grammars_binary_operators_and_more() {
    let grammar = std::fs::read_to_string(repo_root().join(quince_spec::GRAMMAR_PATH)).unwrap();
    let (_, grammar) = quince_spec::grammar::split_header(&grammar);
    let binary = [
        "logic", "compare", "cmp_op", "bits", "arith", "term", "range",
    ];
    let mut expected: Vec<String> = productions_terminals(grammar, &binary);
    expected.extend(productions_terminals(grammar, &["assign_op"]));
    expected.extend(["=>", "->", ",", "(", "[", "{"].map(String::from));
    // `>` and `*` depend on context, and `>=` and `>>` aren't tokens.
    expected.retain(|t| ![">", "*", ">=", ">>"].contains(&t.as_str()));
    for terminal in &expected {
        let kind = single_token(terminal);
        assert!(kind.continues_line(), "{terminal} must continue a line");
    }
    for &(text, kind) in SyntaxKind::PUNCTUATION.iter().chain(SyntaxKind::KEYWORDS) {
        assert_eq!(
            kind.continues_line(),
            expected.iter().any(|t| t == text),
            "{text}"
        );
    }
}

#[test]
fn every_grammar_terminal_is_one_token() {
    let grammar = std::fs::read_to_string(repo_root().join(quince_spec::GRAMMAR_PATH)).unwrap();
    let (_, grammar) = quince_spec::grammar::split_header(&grammar);
    let terminals = productions_terminals(grammar, &[]);
    assert!(terminals.len() > 70, "{terminals:?}");
    for terminal in &terminals {
        match terminal.as_str() {
            ">=" | ">>" => {
                let kinds: Vec<_> = lex_str(terminal).tokens.iter().map(|t| t.kind).collect();
                assert_eq!(kinds[0], SyntaxKind::Gt, "{terminal}");
                assert_eq!(kinds.len(), 2, "{terminal}");
            }
            "TaskScope" => assert_eq!(single_token(terminal), SyntaxKind::Ident),
            _ => {
                let kind = single_token(terminal);
                assert_ne!(kind, SyntaxKind::Ident, "{terminal}");
            }
        }
    }
}

#[test]
fn keywords_match_the_spec() {
    let spec = quince_spec::Spec::load(&repo_root()).unwrap();
    let chapter = spec.file("reference/lexical.md").unwrap();
    let line = chapter
        .text
        .lines()
        .skip_while(|l| *l != "r[lex.keywords]")
        .nth(1)
        .unwrap();
    let mut spec_keywords: Vec<&str> = line.split('`').skip(1).step_by(2).collect();
    let mut ours: Vec<&str> = SyntaxKind::KEYWORDS.iter().map(|(text, _)| *text).collect();
    spec_keywords.sort();
    ours.sort();
    assert_eq!(ours.len(), 38);
    assert_eq!(ours, spec_keywords);
    for (text, kind) in SyntaxKind::KEYWORDS {
        assert_eq!(single_token(text), *kind);
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

/// The quoted terminals of the named productions, or of every production when `names` is
/// empty. A production starts at a line `name =`, and indented lines continue it.
fn productions_terminals(grammar: &str, names: &[&str]) -> Vec<String> {
    let mut found = Vec::new();
    let mut in_named = false;
    for line in grammar.lines() {
        if !line.starts_with(char::is_whitespace) && !line.starts_with("//") {
            let name = line.split('=').next().unwrap_or("").trim();
            in_named = names.is_empty() || names.contains(&name);
        }
        if !in_named {
            continue;
        }
        let rhs = if line.starts_with(char::is_whitespace) {
            line
        } else {
            line.split_once('=').map_or("", |(_, r)| r)
        };
        let mut rest = rhs;
        while let Some(i) = rest.find(['"', '/']) {
            if rest[i..].starts_with("//") {
                break;
            }
            if rest[i..].starts_with('/') {
                rest = &rest[i + 1..];
                continue;
            }
            let after = &rest[i + 1..];
            let end = after.find('"').expect("a closed string");
            if !found.iter().any(|t| t == &after[..end]) {
                found.push(after[..end].to_string());
            }
            rest = &after[end + 1..];
        }
    }
    found
}
