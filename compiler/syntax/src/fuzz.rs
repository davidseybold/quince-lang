//! The parser's invariants on any input (decision 11), checked by panicking. The fuzz target
//! in `compiler/syntax/fuzz/` calls [`check_parse`] on each input it makes, and a unit test
//! calls it on every conformance fixture, so CI keeps it compiling and passing. This module is
//! compiled only for tests and under the `fuzz` feature, so a check that panics never reaches
//! a release build.

use crate::parser::NESTING_LIMIT;
use crate::{Lexed, SyntaxError, SyntaxKind, SyntaxNode, decode_source, lex, parse};
use rowan::{NodeOrToken, WalkEvent};

/// How far past [`NESTING_LIMIT`] a tree may go: the nodes a construct opens before its next
/// recursive entry checks the depth, such as a match arm around its body (README).
pub const NESTING_SLACK_LEVELS: usize = 4;

/// Decodes `bytes`, parses them, and panics unless every invariant holds:
/// - the parser neither panics nor loops, which debug builds turn into a panic;
/// - the tree's text is the decoded text, and its tokens are the lexer's, in order;
/// - every error's range and every fix's edit lies inside the text, on character boundaries,
///   and a fix's edits neither overlap nor insert twice at one place;
/// - the tree is no deeper than the nesting limit allows.
///
/// Bytes past what a source file may hold are an I/O error, not an input, so they pass.
pub fn check_parse(bytes: &[u8]) {
    let Ok(decoded) = decode_source(bytes) else {
        return;
    };
    let lexed = lex(&decoded);
    let parse = parse(&decoded);
    let root = parse.syntax();
    check_lossless(&decoded.text, &lexed, &root);
    check_error_ranges(&decoded.text, parse.errors());
    for diagnostic in parse.diagnostics(&decoded.text, "fuzz.qnc") {
        for fix in &diagnostic.fixes {
            if let Err(problem) = fix.check("fuzz.qnc") {
                panic!("{problem}: {diagnostic:?}");
            }
        }
    }
    let depth = tree_depth(&root);
    assert!(
        depth <= NESTING_LIMIT as usize + NESTING_SLACK_LEVELS,
        "a tree {depth} levels deep"
    );
}

/// The tree's text is `text`, and its tokens are the lexer's, in order, apart from the joins,
/// each of which spells its kind with two lexer tokens. No token is empty, and no node but
/// the root.
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
        match SyntaxKind::JOINED.iter().find(|&&(_, k)| k == token.kind()) {
            Some(&(joined_text, _)) => {
                let (_, second) = lexer_tokens.next().expect("a join of the last token");
                assert_eq!(token.text(), joined_text);
                assert_eq!(&text[range.start..second.end], joined_text);
            }
            None => assert_eq!((token.kind(), token.text()), (kind, &text[range])),
        }
    }
    assert_eq!(lexer_tokens.next(), None, "the tree lost a token");
}

/// Every error range lies inside the text on character boundaries, in source order, and so
/// does every edit of its fixes. Only an error at the end of the text may be empty.
pub(crate) fn check_error_ranges(text: &str, errors: &[SyntaxError]) {
    let inside = |start: usize, end: usize| {
        start <= end
            && end <= text.len()
            && text.is_char_boundary(start)
            && text.is_char_boundary(end)
    };
    for e in errors {
        let (start, end) = (e.range.start as usize, e.range.end as usize);
        assert!(
            inside(start, end) && (start < end || start == text.len()),
            "{text:?}: {e:?}"
        );
        for edit in e.fixes.iter().flat_map(|fix| &fix.edits) {
            let (range, _) = edit;
            assert!(
                inside(range.start as usize, range.end as usize),
                "{text:?}: {e:?}"
            );
        }
    }
    assert!(
        errors.is_sorted_by_key(|e| e.range.start),
        "{text:?}: errors out of order: {errors:?}"
    );
}

/// The levels of the tree, the root's included.
pub(crate) fn tree_depth(root: &SyntaxNode) -> usize {
    let (mut depth, mut deepest) = (0usize, 0usize);
    for event in root.preorder() {
        match event {
            WalkEvent::Enter(_) => {
                depth += 1;
                deepest = deepest.max(depth);
            }
            WalkEvent::Leave(_) => depth -= 1,
        }
    }
    deepest
}
