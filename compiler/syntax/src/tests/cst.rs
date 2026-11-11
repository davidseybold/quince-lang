//! The tree builder on hand-built events: trivia placement, joins and the lossless invariant.
//! Also the tree dumps that the parser's tests compare.

use crate::cst::{Event, QuinceLanguage, build};
use crate::{Lexed, SyntaxError, SyntaxKind, SyntaxNode, lex_str};
use SyntaxKind::*;
use rowan::{Language, NodeOrToken, WalkEvent};

/// The tree, one line per node or token, indented two spaces per level: the kind, and for a
/// token its text.
pub(crate) fn dump_tree(node: &SyntaxNode) -> String {
    dump(node, true)
}

/// [`dump_tree`] without trivia tokens.
pub(crate) fn dump_tree_without_trivia(node: &SyntaxNode) -> String {
    dump(node, false)
}

fn dump(node: &SyntaxNode, trivia: bool) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for event in node.preorder_with_tokens() {
        match event {
            WalkEvent::Enter(NodeOrToken::Node(node)) => {
                out += &format!("{:indent$}{:?}\n", "", node.kind(), indent = depth * 2);
                depth += 1;
            }
            WalkEvent::Leave(NodeOrToken::Node(_)) => depth -= 1,
            WalkEvent::Enter(NodeOrToken::Token(token)) => {
                if trivia || !token.kind().is_trivia() {
                    let (kind, text) = (token.kind(), token.text());
                    out += &format!("{:indent$}{kind:?} {text:?}\n", "", indent = depth * 2);
                }
            }
            WalkEvent::Leave(NodeOrToken::Token(_)) => {}
        }
    }
    out
}

/// The tree's text is `text`, and its tokens are the lexer's, in order, apart from the joins.
/// No token is empty, and no node but the root.
pub(crate) fn check_lossless(text: &str, lexed: &Lexed, root: &SyntaxNode) {
    assert_eq!(root.text().to_string(), text);
    let mut lexer_tokens = lexed.ranges();
    for element in root.descendants_with_tokens() {
        let token = match element {
            NodeOrToken::Node(node) => {
                let is_root = node.parent().is_none();
                assert!(
                    is_root || !node.text_range().is_empty(),
                    "an empty {node:?}"
                );
                continue;
            }
            NodeOrToken::Token(token) => token,
        };
        let (kind, range) = lexer_tokens.next().expect("the tree has extra tokens");
        if SyntaxKind::JOINED.iter().any(|&(_, k)| k == token.kind()) {
            let (_, second) = lexer_tokens.next().expect("a join of the last token");
            assert_eq!(kind, Gt, "a join that doesn't start with `>`");
            assert_eq!(token.text(), &text[range.start..second.end]);
        } else {
            assert_eq!((token.kind(), token.text()), (kind, &text[range]));
        }
    }
    assert_eq!(lexer_tokens.next(), None, "the tree lost a token");
}

/// Builds the tree of `text` from `events`, with every token significant but trivia and doc
/// comments, and checks it is lossless.
fn tree(text: &str, events: Vec<Event>) -> SyntaxNode {
    let lexed = lex_str(text);
    let significant = (0..lexed.tokens.len())
        .filter(|&i| !lexed.tokens[i].kind.is_trivia() && lexed.tokens[i].kind != DocComment)
        .collect::<Vec<_>>();
    tree_with(text, &lexed, &significant, events)
}

fn tree_with(text: &str, lexed: &Lexed, significant: &[usize], events: Vec<Event>) -> SyntaxNode {
    let (green, _) = build(text, &lexed.tokens, significant, events);
    let root = SyntaxNode::new_root(green);
    check_lossless(text, lexed, &root);
    root
}

/// `Start(kind)`, for short.
fn s(kind: SyntaxKind) -> Event {
    Event::Start(kind)
}

const T: Event = Event::Token;
const F: Event = Event::Finish;

/// The events for `fn f() {}`, ending with the `FnDecl`'s finish.
fn empty_fn() -> Vec<Event> {
    vec![s(FnDecl), T, s(Name), T, F, T, T, s(Block), T, T, F, F]
}

/// The events for a file of `fn f() {}` declarations, one per element of `fns`.
fn file_of(fns: usize) -> Vec<Event> {
    let mut events = vec![s(File)];
    for _ in 0..fns {
        events.extend(empty_fn());
    }
    events.push(F);
    events
}

#[test]
fn every_kind_maps_to_rowan_and_back() {
    for (i, &kind) in SyntaxKind::ALL.iter().enumerate() {
        assert_eq!(kind as usize, i);
        let raw = QuinceLanguage::kind_to_raw(kind);
        assert_eq!(QuinceLanguage::kind_from_raw(raw), kind);
    }
    let lexer_texts: Vec<&str> = SyntaxKind::PUNCTUATION.iter().map(|(t, _)| *t).collect();
    for &(text, kind) in SyntaxKind::JOINED {
        assert!(!lexer_texts.contains(&text), "the lexer produces {text}");
        assert!(text.starts_with('>') && !kind.is_trivia(), "{kind:?}");
    }
}

#[test]
fn an_empty_file_is_an_empty_root() {
    assert_eq!(dump_tree(&tree("", vec![s(File), F])), "File\n");
}

#[test]
fn the_root_holds_trivia_before_and_after_everything() {
    let root = tree("  a // c\n", vec![s(File), s(Name), T, F, F]);
    assert_eq!(
        dump_tree(&root),
        r#"File
  Whitespace "  "
  Name
    Ident "a"
  Whitespace " "
  Comment "// c"
  Newline "\n"
"#
    );
}

/// Trivia before a node goes outside it, and so does trivia after it: a comment at the end
/// of a line belongs to the node that holds the line.
#[test]
fn trivia_goes_outside_nodes() {
    let events = vec![
        s(File),
        s(FnDecl),
        T,
        s(Name),
        T,
        F,
        T,
        T,
        s(Block),
        T,
        s(Name),
        T,
        F,
        T,
        F,
        F,
        F,
    ];
    let root = tree("fn f() {\n  a // c\n}", events);
    assert_eq!(
        dump_tree(&root),
        r#"File
  FnDecl
    FnKw "fn"
    Whitespace " "
    Name
      Ident "f"
    LParen "("
    RParen ")"
    Whitespace " "
    Block
      LBrace "{"
      Newline "\n"
      Whitespace "  "
      Name
        Ident "a"
      Whitespace " "
      Comment "// c"
      Newline "\n"
      RBrace "}"
"#
    );
    assert_eq!(
        dump_tree_without_trivia(&root),
        r#"File
  FnDecl
    FnKw "fn"
    Name
      Ident "f"
    LParen "("
    RParen ")"
    Block
      LBrace "{"
      Name
        Ident "a"
      RBrace "}"
"#
    );
}

/// Comment lines directly above a declaration go inside it, with their line endings. A blank
/// line ends them, and the comment before it stays outside.
#[test]
fn comments_above_a_declaration_go_inside_it() {
    let root = tree("// header\n\n// one\n// two\nfn f() {}\n", file_of(1));
    assert_eq!(
        dump_tree(&root),
        r#"File
  Comment "// header"
  Newline "\n"
  Newline "\n"
  FnDecl
    Comment "// one"
    Newline "\n"
    Comment "// two"
    Newline "\n"
    FnKw "fn"
    Whitespace " "
    Name
      Ident "f"
    LParen "("
    RParen ")"
    Whitespace " "
    Block
      LBrace "{"
      RBrace "}"
  Newline "\n"
"#
    );
}

/// A comment at the end of a line belongs to that line, not to the declaration below it, and
/// a line with a `;` on it isn't a comment line.
#[test]
fn a_comment_after_code_stays_outside() {
    let text = "fn f() {} // a\nfn f() {}\n/* b */;\nfn f() {}";
    let root = tree(text, file_of(3));
    let trivia_between = dump_tree(&root)
        .lines()
        .filter(|line| line.starts_with("  ") && !line.starts_with("   "))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        trivia_between,
        r#"  FnDecl
  Whitespace " "
  Comment "// a"
  Newline "\n"
  FnDecl
  Newline "\n"
  BlockComment "/* b */"
  Semicolon ";"
  Newline "\n"
  FnDecl"#
    );
}

/// In a body, the indentation before the first comment stays outside, and the indentation
/// of the declaration's own line goes inside with the comments.
#[test]
fn comments_above_a_member_keep_the_indentation_outside() {
    let mut events = vec![s(File), s(ImplDecl), T, s(Name), T, F, T];
    events.extend(empty_fn());
    events.extend([T, F, F]);
    let root = tree("impl T {\n    // m\n    fn f() {}\n}", events);
    let dump = dump_tree(&root);
    let impl_body = dump.lines().take(12).collect::<Vec<_>>().join("\n");
    assert_eq!(
        impl_body,
        r#"File
  ImplDecl
    ImplKw "impl"
    Whitespace " "
    Name
      Ident "T"
    Whitespace " "
    LBrace "{"
    Newline "\n"
    Whitespace "    "
    FnDecl
      Comment "// m""#
    );
    assert!(dump.contains(
        "      Comment \"// m\"\n      Newline \"\\n\"\n      Whitespace \"    \"\n      FnKw"
    ));
}

/// Block comments, `Error` tokens and doc comments the parser left as trivia are placed like
/// comments. The byte order mark belongs to the file.
#[test]
fn other_trivia_attaches_like_comments() {
    let root = tree("\u{FEFF}/* a */\n$\n/// b\nfn f() {}", file_of(1));
    let dump = dump_tree(&root);
    assert!(
        dump.starts_with(
            r#"File
  Bom "\u{feff}"
  FnDecl
    BlockComment "/* a */"
    Newline "\n"
    Error "$"
    Newline "\n"
    DocComment "/// b"
    Newline "\n"
    FnKw "fn"
"#
        ),
        "{dump}"
    );
}

/// Comments attach only to the kinds that take them. Before any other node they stay
/// outside.
#[test]
fn comments_stay_outside_other_nodes() {
    let events = vec![s(File), s(Block), T, s(Name), T, F, T, F, F];
    let root = tree("{\n  // c\n  a\n}", events);
    assert!(
        dump_tree(&root).contains(
            "    Comment \"// c\"\n    Newline \"\\n\"\n    Whitespace \"  \"\n    Name\n"
        ),
        "{}",
        dump_tree(&root)
    );
}

/// A doc comment the parser takes is a real token, first in its declaration.
#[test]
fn a_doc_comment_the_parser_takes_is_a_token() {
    let text = "/// d\nfn f() {}";
    let lexed = lex_str(text);
    let significant: Vec<usize> = (0..lexed.tokens.len())
        .filter(|&i| !lexed.tokens[i].kind.is_trivia())
        .collect();
    let mut events = vec![s(File), s(FnDecl), T];
    events.extend(empty_fn().into_iter().skip(1));
    events.push(F);
    let root = tree_with(text, &lexed, &significant, events);
    assert!(
        dump_tree_without_trivia(&root)
            .starts_with("File\n  FnDecl\n    DocComment \"/// d\"\n    FnKw \"fn\"\n"),
        "{}",
        dump_tree(&root)
    );
}

/// `>>` and `>=` are one token each, holding both lexer tokens' text.
#[test]
fn joins_make_one_token_of_two() {
    let events = vec![
        s(File),
        T,
        Event::Join(GtGt),
        T,
        Event::Join(GtEq),
        T,
        s(Skipped),
        T,
        F,
        F,
    ];
    let root = tree("a >> b >= c >", events);
    assert_eq!(
        dump_tree_without_trivia(&root),
        r#"File
  Ident "a"
  GtGt ">>"
  Ident "b"
  GtEq ">="
  Ident "c"
  Skipped
    Gt ">"
"#
    );
}

#[test]
fn errors_come_out_in_the_order_reported() {
    let lexed = lex_str("a b");
    let error = |at: usize, message: &str| SyntaxError {
        range: at as u32..at as u32 + 1,
        code: "E0001",
        message: message.to_string(),
        fixes: Vec::new(),
    };
    let events = vec![
        s(File),
        Event::Error(error(2, "second token")),
        T,
        Event::Error(error(0, "first token")),
        T,
        F,
    ];
    let (_, errors) = build("a b", &lexed.tokens, &[0, 2], events);
    let messages: Vec<_> = errors.iter().map(|e| e.message.as_str()).collect();
    assert_eq!(messages, ["second token", "first token"]);
}

#[test]
#[should_panic(expected = "only the root may be empty")]
fn an_empty_node_is_a_parser_bug() {
    tree("a", vec![s(File), s(Name), F, T, F]);
}

#[test]
#[should_panic(expected = "the parser left a significant token")]
fn a_token_left_over_is_a_parser_bug() {
    tree("a b", vec![s(File), T, F]);
}

#[test]
#[should_panic(expected = "a join of tokens that aren't adjacent")]
fn a_join_across_trivia_is_a_parser_bug() {
    tree("a > > b", vec![s(File), T, Event::Join(GtGt), T, F]);
}
