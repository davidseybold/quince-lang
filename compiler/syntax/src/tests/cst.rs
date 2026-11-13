//! The tree builder on hand-built events: trivia placement, joins and the lossless invariant.
//! Also the tree dumps that the parser's tests compare.

use crate::cst::{Event, QuinceLanguage, build};
use crate::fuzz::check_lossless;
use crate::{Lexed, SyntaxError, SyntaxKind, SyntaxNode, lex_str};
use SyntaxKind::*;
use rowan::{Language, NodeOrToken, WalkEvent};

/// The tree, one line per node or token, indented two spaces per level: the kind, and for a
/// token its text.
pub(crate) fn dump_tree(node: &SyntaxNode) -> String {
    dump(node, true)
}

/// [`dump_tree`] without the tokens whose kind is trivia. A doc comment the parser left as
/// trivia still shows, since only the tree's position says it is one.
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

/// The events for a file of `decls` declarations, each the single token `fn`. The builder
/// doesn't check grammar, so a one-token declaration keeps the trivia tests' dumps short.
fn file_of(decls: usize) -> Vec<Event> {
    let mut events = vec![s(File)];
    for _ in 0..decls {
        events.extend([s(FnDecl), T, F]);
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
    let block = [s(Block), T, s(Name), T, F, T, F];
    let mut events = vec![s(File), s(FnDecl), T, s(Name), T, F, T, T];
    events.extend(block);
    events.extend([F, F]);
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

/// Trivia between two nodes goes in the innermost node that holds both, not in the node of
/// the token after it.
#[test]
fn trivia_between_nodes_goes_in_the_node_around_both() {
    let root = tree("fn\n\nfn", file_of(2));
    assert_eq!(
        dump_tree(&root),
        r#"File
  FnDecl
    FnKw "fn"
  Newline "\n"
  Newline "\n"
  FnDecl
    FnKw "fn"
"#
    );
}

/// Comment lines directly above a declaration go inside it, with their line endings. A blank
/// line ends them, and the comment before it stays outside.
#[test]
fn comments_above_a_declaration_go_inside_it() {
    let root = tree("// header\n\n// one\n// two\nfn\n", file_of(1));
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
  Newline "\n"
"#
    );
}

/// A comment on the file's first line starts a line, as one after a line ending does.
#[test]
fn a_comment_on_the_first_line_goes_inside() {
    let root = tree("// a\nfn", file_of(1));
    assert_eq!(
        dump_tree(&root),
        r#"File
  FnDecl
    Comment "// a"
    Newline "\n"
    FnKw "fn"
"#
    );
}

/// Every comment on a comment line goes inside, the first one included.
#[test]
fn a_line_of_several_comments_goes_inside_whole() {
    let root = tree("/* a */ // b\nfn", file_of(1));
    assert_eq!(
        dump_tree(&root),
        r#"File
  FnDecl
    BlockComment "/* a */"
    Whitespace " "
    Comment "// b"
    Newline "\n"
    FnKw "fn"
"#
    );
}

/// A comment at the end of a line of code belongs to that line, not to the declaration below
/// it, and a line with a `;` on it isn't a comment line.
#[test]
fn a_comment_after_code_stays_outside() {
    let root = tree("fn // a\nfn\n/* b */;\nfn", file_of(3));
    assert_eq!(
        dump_tree(&root),
        r#"File
  FnDecl
    FnKw "fn"
  Whitespace " "
  Comment "// a"
  Newline "\n"
  FnDecl
    FnKw "fn"
  Newline "\n"
  BlockComment "/* b */"
  Semicolon ";"
  Newline "\n"
  FnDecl
    FnKw "fn"
"#
    );
}

/// In a body, the indentation before the first comment stays outside, and the indentation
/// of the declaration's own line goes inside with the comments.
#[test]
fn comments_above_a_member_keep_the_indentation_outside() {
    let events = vec![
        s(File),
        s(ImplDecl),
        T,
        s(Name),
        T,
        F,
        T,
        s(FnDecl),
        T,
        F,
        T,
        F,
        F,
    ];
    let root = tree("impl T {\n    // m\n    fn\n}", events);
    assert_eq!(
        dump_tree(&root),
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
      Comment "// m"
      Newline "\n"
      Whitespace "    "
      FnKw "fn"
    Newline "\n"
    RBrace "}"
"#
    );
}

/// Block comments, `Error` tokens and doc comments the parser left as trivia are placed like
/// comments. The byte order mark belongs to the file.
#[test]
fn other_trivia_attaches_like_comments() {
    let root = tree("\u{FEFF}/* a */\n$\n/// b\nfn", file_of(1));
    assert_eq!(
        dump_tree(&root),
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
    );
}

/// Comments attach only to the kinds that take them. Before any other node they stay
/// outside.
#[test]
fn comments_stay_outside_other_nodes() {
    let events = vec![s(File), s(Block), T, s(Name), T, F, T, F, F];
    let root = tree("{\n  // c\n  a\n}", events);
    assert_eq!(
        dump_tree(&root),
        r#"File
  Block
    LBrace "{"
    Newline "\n"
    Whitespace "  "
    Comment "// c"
    Newline "\n"
    Whitespace "  "
    Name
      Ident "a"
    Newline "\n"
    RBrace "}"
"#
    );
}

/// When nodes that take comments start at the same token, the outermost takes the whole run,
/// indented or not.
#[test]
fn the_outermost_node_takes_the_comments() {
    let events = || vec![s(File), s(ImplDecl), s(FnDecl), T, F, F, F];
    let root = tree("  // a\n  // b\n  fn", events());
    assert_eq!(
        dump_tree(&root),
        r#"File
  Whitespace "  "
  ImplDecl
    Comment "// a"
    Newline "\n"
    Whitespace "  "
    Comment "// b"
    Newline "\n"
    Whitespace "  "
    FnDecl
      FnKw "fn"
"#
    );
    let root = tree("// a\nfn", events());
    assert_eq!(
        dump_tree(&root),
        r#"File
  ImplDecl
    Comment "// a"
    Newline "\n"
    FnDecl
      FnKw "fn"
"#
    );
}

/// A node that holds only nodes isn't empty.
#[test]
fn a_node_of_nodes_holds_tokens() {
    let root = tree("a", vec![s(File), s(Skipped), s(Name), T, F, F, F]);
    assert_eq!(
        dump_tree(&root),
        "File\n  Skipped\n    Name\n      Ident \"a\"\n"
    );
}

/// A doc comment the parser takes is a real token, first in its declaration.
#[test]
fn a_doc_comment_the_parser_takes_is_a_token() {
    let text = "/// d\nfn";
    let lexed = lex_str(text);
    let significant: Vec<usize> = (0..lexed.tokens.len())
        .filter(|&i| !lexed.tokens[i].kind.is_trivia())
        .collect();
    let events = vec![s(File), s(FnDecl), T, T, F, F];
    let root = tree_with(text, &lexed, &significant, events);
    assert_eq!(
        dump_tree(&root),
        r#"File
  FnDecl
    DocComment "/// d"
    Newline "\n"
    FnKw "fn"
"#
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

/// The comments a declaration takes don't make it hold anything.
#[test]
#[should_panic(expected = "only the root may be empty")]
fn a_node_of_only_comments_is_a_parser_bug() {
    tree("// a\nfn", vec![s(File), s(FnDecl), F, T, F]);
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

#[test]
#[should_panic(expected = "a join whose tokens don't spell its kind")]
fn a_join_of_the_wrong_kind_is_a_parser_bug() {
    tree("a >> b", vec![s(File), T, Event::Join(GtEq), T, F]);
}
