//! Errors: E0016, "expected X, found Y", and the skip after it, so one slip is one error.

use super::errors::describe;
use super::parser::{check_error_ranges, checked};
use crate::{SyntaxKind, decode_source, parse};

/// Each error as `code start..end`, then its fixed text when it has one fix.
fn errors(text: &str) -> Vec<String> {
    let parse = checked(text);
    parse.errors().iter().map(|e| describe(text, e)).collect()
}

fn codes(text: &str) -> Vec<&'static str> {
    checked(text).errors().iter().map(|e| e.code).collect()
}

fn messages(text: &str) -> Vec<String> {
    let parse = checked(text);
    parse.errors().iter().map(|e| e.message.clone()).collect()
}

/// An item on the next line after a list item is an E0016, whose fix inserts the missing
/// `,`. The skip then closes the `(` at the block's `}`, with nothing more reported.
#[test]
fn an_item_on_the_next_line_needs_a_comma() {
    assert_eq!(
        errors("fn f() {\n  g(a, b\n  h()\n}\n"),
        [r#"E0016 20..21 => "fn f() {\n  g(a, b,\n  h()\n}\n""#]
    );
    assert_eq!(
        errors("fn f() {\n  g(\n    a\n    b\n  )\n}\n"),
        [r#"E0016 24..25 => "fn f() {\n  g(\n    a,\n    b\n  )\n}\n""#]
    );
    assert_eq!(
        messages("fn f() {\n  g(a, b\n  h()\n}\n"),
        ["expected `,` or `)`, found the name `h`"]
    );
}

/// A break that ends a construct ends it even where a token after it could go on: a
/// parameter after a `;`, and a declaration after a `pub` on its own line. At a `;`, the
/// lexer's E0002 is the only error.
#[test]
fn a_break_ends_a_parameter_or_a_pub() {
    assert_eq!(errors("fn f(;a: Int) {}"), ["E0002 5..6"]);
    assert_eq!(errors("fn f(a: Int,;b: Int) {}"), ["E0002 12..13"]);
    assert_eq!(errors("fn f(var;self) {}"), ["E0002 8..9"]);
    let text = "pub\nfn f() {}\n";
    assert_eq!(errors(text), ["E0016 3..4"]);
    assert_eq!(
        messages(text),
        ["expected a declaration, found a line break"]
    );
    assert_eq!(decls(text), [SyntaxKind::Skipped, SyntaxKind::FnDecl]);
}

/// The kinds of the file's children.
fn decls(text: &str) -> Vec<SyntaxKind> {
    checked(text)
        .syntax()
        .children()
        .map(|n| n.kind())
        .collect()
}

/// The skip after an error takes a bracket whole, and stops at a sync point: a `,` in a list,
/// a break in a line context.
#[test]
fn the_skip_takes_brackets_whole() {
    assert_eq!(
        errors("fn f() {\n  g(a b (c, d), e)\n}\n"),
        ["E0016 15..16"]
    );
    assert_eq!(
        codes("fn match() {\n  let a = 1\n}\nfn g() {}\n"),
        ["E0016"]
    );
}

/// A list or block closed by something else expects its closing bracket.
#[test]
fn a_missing_closing_bracket() {
    assert_eq!(
        messages("fn f() {\n  g(a\n}\n"),
        ["expected `)`, found `}`"]
    );
    assert_eq!(errors("fn f() {\n  g(1"), ["E0016 14..14"]);
    assert_eq!(
        messages("fn f() {\n  g(1"),
        ["expected `)`, found the end of the file"]
    );
}

/// A closing bracket that matches nothing is skipped as an E0016.
#[test]
fn a_closing_bracket_that_matches_nothing() {
    assert_eq!(errors("fn f() {\n  g)\n}\n"), ["E0016 12..13"]);
    assert_eq!(errors("}\nfn f() {}"), ["E0016 0..1"]);
    assert_eq!(
        messages("fn f() {}\n}\n"),
        ["expected a declaration, found `}`"]
    );
}

#[test]
fn the_end_of_the_file() {
    assert_eq!(
        messages("fn f() ->"),
        ["expected a type, found the end of the file"]
    );
    assert_eq!(errors("fn f() ->"), ["E0016 9..9"]);
}

/// A `;` is the lexer's error, and a hard break: it ends the statement even before a `.`.
#[test]
fn a_semicolon_ends_a_statement() {
    assert_eq!(errors("fn f() {\n  a; b\n}\n"), ["E0002 12..13"]);
    assert_eq!(codes("fn f() {\n  a;\n  .b()\n}\n"), ["E0002", "E0016"]);
    // Where the construct can't end, the E0002 is the only error, and nothing after it on the
    // construct's line is reported either.
    assert_eq!(errors("fn f() ->; Int {}"), ["E0002 9..10"]);
    assert_eq!(errors("fn f() ->\n; Int {}"), ["E0002 10..11"]);
    assert_eq!(errors("fn f() {\n  let a =; 1\n}\n"), ["E0002 18..19"]);
}

/// Two expressions on one line are an E0016 that expects a line break.
#[test]
fn two_expressions_on_a_line() {
    let text = "fn f() {\n  let s = \"press \"b\" now\"\n}\n";
    assert_eq!(errors(text), ["E0016 27..29"]);
    assert_eq!(
        messages(text),
        ["expected a line break or `}`, found a string"]
    );
}

/// A line break that ends a construct before its end is "found a line break", at the break.
#[test]
fn a_break_too_early() {
    assert_eq!(errors("fn f\nfn g() {}"), ["E0016 4..5"]);
    assert_eq!(
        messages("fn f\nfn g() {}"),
        ["expected `(`, found a line break"]
    );
    assert_eq!(
        messages("fn f() {\n  let x\n  = 1\n}"),
        ["expected `:` or `=`, found a line break"]
    );
}

/// A keyword can't be a name, and the error names it (`r[lex.keyword-not-name]`).
#[test]
fn a_keyword_where_a_name_goes() {
    assert_eq!(
        messages("fn match() {}\nfn f(while: Int) {}\nfn g() {\n  let for = 1\n  a.fn()\n}"),
        [
            "expected a function name, found the keyword `match`",
            "expected a parameter, found the keyword `while`",
            "expected a name, found the keyword `for`",
            "expected a field or method name, found the keyword `fn`",
        ]
    );
}

/// In a lossy text, no error has a fix, the parser's included.
#[test]
fn no_fixes_in_a_lossy_text() {
    let bytes = b"fn f() {\n  g(\n    a\n    b\n  )\n}\n\xff\n";
    let decoded = decode_source(bytes).unwrap();
    let parse = parse(&decoded);
    check_error_ranges(&decoded.text, parse.errors());
    let codes: Vec<_> = parse.errors().iter().map(|e| e.code).collect();
    assert_eq!(codes, ["E0016", "E0014"]);
    assert!(parse.errors().iter().all(|e| e.fixes.is_empty()));
    // The same text, not lossy, has the fix.
    let text = std::str::from_utf8(&bytes[..bytes.len() - 2]).unwrap();
    assert_eq!(checked(text).errors()[0].fixes.len(), 1);
}
