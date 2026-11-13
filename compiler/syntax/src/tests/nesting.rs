//! The nesting limit (decision 10): every construct that nests, nested past the limit, is one
//! E0022 and a tree no deeper than the limit allows, on a thread with the stack of a Rust test
//! thread, in a debug build. Each runs on a thread of its own with that stack, so a construct
//! the limit misses overflows it and fails the test.

use super::errors::describe;
use super::parser::checked;
use crate::parser::{NESTING_LIMIT, speculated_tokens};
use crate::{SyntaxNode, codes};
use rowan::WalkEvent;

/// The stack of a Rust test thread, which the limit is measured against (README).
const TEST_STACK_BYTES: usize = 2 << 20;

/// How far past [`NESTING_LIMIT`] the tree may go: the nodes a construct opens before its next
/// recursive entry checks the depth, such as a match arm around its body (README).
const NESTING_SLACK_LEVELS: usize = 4;

/// Far deeper than the limit, so a construct that the limit misses overflows the stack.
const DEEP_LEVELS: usize = 10_000;

/// Gives the text of a file that nests a construct this many levels deep.
type Input = fn(usize) -> String;

/// Each construct that nests, by name, and its input.
const CONSTRUCTS: &[(&str, Input)] = &[
    ("parentheses", |n| body(&wrap(n, "(", "x", ")"))),
    ("blocks", |n| body(&wrap(n, "if a { ", "g()", " }"))),
    ("while", |n| body(&wrap(n, "while a { ", "g()", " }"))),
    ("prefix", |n| body(&format!("{}x", "!".repeat(n)))),
    ("else if", |n| {
        body(&format!("if a {{}}{} else {{}}", " else if a {}".repeat(n)))
    }),
    ("lists", |n| body(&wrap(n, "[", "x", "]"))),
    ("tuples", |n| body(&wrap(n, "(x, ", "x", ")"))),
    ("calls", |n| body(&wrap(n, "g(", "x", ")"))),
    ("named arguments", |n| body(&wrap(n, "g(a: ", "x", ")"))),
    ("indexes", |n| body(&wrap(n, "x[", "0", "]"))),
    ("maps", |n| body(&wrap(n, "{a: ", "x", "}"))),
    ("struct updates", |n| body(&wrap(n, "P(..", "x", ", a: 1)"))),
    ("closures", |n| body(&wrap(n, "fn() { ", "x", " }"))),
    ("returns", |n| body(&wrap(n, "(return ", "x", ")"))),
    ("match", |n| body(&wrap(n, "match x { _ => ", "x", ", }"))),
    ("select", |n| {
        body(&wrap(n, "select { a = b => ", "x", ", }"))
    }),
    ("concurrent", |n| {
        body(&wrap(n, "concurrent { spawn ", "x", " }"))
    }),
    ("with", |n| body(&wrap(n, "with Db = d { ", "x", " }"))),
    ("sums", |n| body(&format!("x{}", " + x".repeat(n)))),
    ("fields", |n| body(&format!("x{}", ".a".repeat(n)))),
    ("method calls", |n| body(&format!("x{}", ".a()".repeat(n)))),
    ("index chains", |n| body(&format!("x{}", "[0]".repeat(n)))),
    ("tries", |n| body(&format!("x{}", "?".repeat(n)))),
    ("patterns", |n| {
        body(&format!("let {} = y", wrap(n, "(a, ", "a", ")")))
    }),
    ("types", |n| {
        format!("fn f(x: {}) {{}}\n", wrap(n, "List<", "Int", ">"))
    }),
    ("function types", |n| {
        format!("fn f(x: {}) {{}}\n", wrap(n, "fn(", "Int", ")"))
    }),
    ("tuple types", |n| {
        format!("fn f(x: {}) {{}}\n", wrap(n, "(Int, ", "Int", ")"))
    }),
    ("type arguments", |n| {
        body(&format!("g<{}>(x)", wrap(n, "List<", "Int", ">")))
    }),
];

/// `open` `levels` times, then `inner`, then `close` as many times.
fn wrap(levels: usize, open: &str, inner: &str, close: &str) -> String {
    format!("{}{inner}{}", open.repeat(levels), close.repeat(levels))
}

fn body(line: &str) -> String {
    format!("fn f() {{\n  {line}\n}}\n")
}

/// The levels of the tree, the file's included.
fn tree_depth(root: &SyntaxNode) -> usize {
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

/// Runs `check` on a thread with [`TEST_STACK_BYTES`] of stack.
fn on_test_stack<T: Send + 'static>(check: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(TEST_STACK_BYTES)
        .spawn(check)
        .expect("a thread")
        .join()
        .expect("no panic")
}

/// The codes of `text`'s errors, and the depth of its tree, parsed and checked on a test
/// stack. Dropping the tree recurses too, so it is dropped there.
fn parse_on_test_stack(text: String) -> (Vec<&'static str>, usize) {
    on_test_stack(move || {
        let parse = checked(&text);
        let codes = parse.errors().iter().map(|e| e.code).collect();
        (codes, tree_depth(&parse.syntax()))
    })
}

/// Below the limit, nothing is an error. Each construct opens a node or more per level, so 60
/// levels stay under it.
#[test]
fn nesting_under_the_limit_is_no_error() {
    for &(construct, input) in CONSTRUCTS {
        let (codes, _) = parse_on_test_stack(input(60));
        assert_eq!(codes, Vec::<&str>::new(), "{construct}");
    }
}

/// Past the limit, each construct is one E0022, and its tree stays within the limit.
#[test]
fn nesting_past_the_limit_is_one_error() {
    for &(construct, input) in CONSTRUCTS {
        let (codes, depth) = parse_on_test_stack(input(DEEP_LEVELS));
        // Type arguments in an expression are parsed ahead, where the limit is a slip, so the
        // `<` is less-than and the comparisons chain (decision 6).
        let expected: &[&str] = if construct == "type arguments" {
            &[codes::CHAINED_OPERATOR, codes::TOO_DEEP]
        } else {
            &[codes::TOO_DEEP]
        };
        assert_eq!(codes, expected, "{construct}");
        let deepest = NESTING_LIMIT as usize + NESTING_SLACK_LEVELS;
        assert!(depth <= deepest, "{construct}: {depth} levels");
    }
}

/// The E0022 is at the token that goes past the limit. The rest of its line is skipped, or of
/// its brackets in a list, and the parser parses on after them, so the next error is reported.
#[test]
fn the_error_and_what_is_skipped() {
    let sum = format!(
        "fn f() {{\n  let total = x{}\n  let after = 1 1\n}}\n",
        " + x".repeat(300)
    );
    let found = on_test_stack(move || {
        let parse = checked(&sum);
        parse
            .errors()
            .iter()
            .map(|e| {
                let range = e.range.start as usize..e.range.end as usize;
                format!("{} {:?}", describe(&sum, e), &sum[range])
            })
            .collect::<Vec<_>>()
    });
    // The 252nd `+`, at 1029, would wrap a sum as deep as the limit allows.
    assert_eq!(
        found,
        [r#"E0022 1029..1030 "+""#, r#"E0016 1241..1242 "1""#]
    );

    let list = format!(
        "fn f() {{\n  g({}x{}, 1 1)\n}}\n",
        "[".repeat(300),
        "]".repeat(300)
    );
    let codes = on_test_stack(move || {
        let parse = checked(&list);
        parse.errors().iter().map(|e| e.code).collect::<Vec<_>>()
    });
    // The skip stops at the `]` of the list it began in, so the call's next argument parses.
    assert_eq!(codes, [codes::TOO_DEEP, codes::UNEXPECTED_TOKEN]);

    let message = on_test_stack(|| checked(&body(&"!".repeat(300))).errors()[0].message.clone());
    assert_eq!(
        message,
        "this nests deeper than the limit of 256 levels: move the inner part into a binding or \
         a function"
    );
}

/// Parsing ahead from a `<` nests type arguments as deep as the `<`s it meets, far deeper
/// than the tree that results, so the limit covers it, where it is a slip: `g(a < b, a < b,
/// ...)` is comparisons, with no error. Whether type arguments parse past the limit depends
/// on how deep the parser was, so none around it is recorded, and each `<` is parsed ahead
/// from again: at most a limit's levels each, so still linear. Here each `a < b, ` is 4 tokens
/// and 2 levels.
#[test]
fn parsing_ahead_is_within_the_limit() {
    let text = body(&format!("g({}a)", "a < b, ".repeat(DEEP_LEVELS)));
    let (codes, _) = parse_on_test_stack(text.clone());
    assert_eq!(codes, Vec::<&str>::new());
    let speculated = on_test_stack(move || speculated_tokens(&text));
    let per_lt = 4 * NESTING_LIMIT as usize / 2;
    assert!(speculated <= DEEP_LEVELS * per_lt, "{speculated}");
}

/// A `<` that was parsed ahead from inside a deeper speculation, which reached the limit,
/// still starts type arguments where the parser reaches it, less deep (found in review).
#[test]
fn parsing_ahead_past_the_limit_records_nothing() {
    let generic = format!("f<{}>(x)", wrap(8, "List<", "Int", ">"));
    for comparisons in [116, 120, 125, 126] {
        let line = format!("g({}{generic})", "a < b, ".repeat(comparisons));
        let (codes, _) = parse_on_test_stack(body(&line));
        assert_eq!(codes, Vec::<&str>::new(), "{comparisons}");
    }
    let list = body(&format!("[{}make<List<Int>>()]", "a < b, ".repeat(120)));
    assert_eq!(parse_on_test_stack(list).0, Vec::<&str>::new());
}

/// An E0022 whose skip starts a block's line still takes a token, so the block's loop over its
/// lines moves on, and the lines after it are part of that slip (found in review: a release
/// build looped forever).
#[test]
fn a_block_line_at_the_limit_moves_on() {
    let text = format!(
        "fn f() {{\n  ({}x\ny\n{})\n}}\n",
        "if a {\n".repeat(126),
        "}\n".repeat(126)
    );
    let (codes, _) = parse_on_test_stack(text);
    assert_eq!(codes, [codes::TOO_DEEP]);
}

/// A chain step around a node that holds an E0022, or a sibling at the limit beside one, is
/// part of that slip, not another error per level (found in review).
#[test]
fn levels_past_an_error_are_one_slip() {
    let deep = 300;
    for (shape, text) in [
        ("fields", wrap(deep, "(", "x", ").a")),
        ("sums", wrap(deep, "(", "a", ") + b")),
        ("indexes", wrap(deep, "[", "x", "][0]")),
        ("comparisons", wrap(deep, "(", "x", ") == 1")),
        ("is", wrap(deep, "(", "x", ") is A")),
        ("ranges", wrap(deep, "(", "x", ")..1")),
        ("method calls", wrap(deep, "g(", "x", ").len()")),
        ("tries", wrap(deep, "g(", "x", ")?")),
        ("grouping", wrap(deep, "(a * ", "b", " + c)")),
        ("closure calls", wrap(deep, "fn() { ", "x", " }()")),
        ("if bodies", wrap(deep, "if a { ", "x", " }.len()")),
    ] {
        let (codes, _) = parse_on_test_stack(body(&text));
        assert_eq!(codes, [codes::TOO_DEEP], "{shape}");
    }
    // Two parts that each go past the limit on their own are two places to split, so two
    // errors.
    let types = format!(
        "fn f(q: {0}) -> {0} {{}}\n",
        wrap(deep, "List<", "Int", ">")
    );
    assert_eq!(
        parse_on_test_stack(types).0,
        [codes::TOO_DEEP, codes::TOO_DEEP]
    );
}
