//! Error recovery: the slips of the parser design doc's decision 9 and their diagnostics, one
//! slip to one error, and the errors the parser leaves to the lexer.

use super::cst::dump_tree_without_trivia;
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

/// A member start closes the lists it can't be in, with an E0017 at each opener.
#[test]
fn a_statement_start_closes_a_call() {
    assert_eq!(
        errors("fn f() {\n  g(a, b\n  let y = 2\n}\n"),
        ["E0017 12..13"]
    );
    assert_eq!(
        messages("fn f() {\n  g(a, b\n  let y = 2\n}\n"),
        ["this `(` has no matching `)`"]
    );
}

/// An item on the next line that isn't a member start is an E0016, whose fix inserts the
/// missing `,`. The skip then closes the `(` silently at the block's `}`.
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

/// After an error, a construct closed while the parser skips gets no E0017: once the first
/// slip is fixed, the missing closers show.
#[test]
fn closers_after_an_error_are_silent() {
    assert_eq!(errors("fn f() {\n  g(a b\nfn h() {}\n"), ["E0016 15..16"]);
}

#[test]
fn a_declaration_closes_a_block() {
    assert_eq!(errors("fn f() {\n  g()\nfn h() {}\n"), ["E0017 7..8"]);
    // So does `pub` and a declaration.
    let text = "fn f() {\n  g(\npub fn h() {}\n";
    assert_eq!(errors(text), ["E0017 7..8", "E0017 12..13"]);
    assert_eq!(decls(text), [SyntaxKind::FnDecl, SyntaxKind::FnDecl]);
    // In an `impl`, a method closes the block of the one before it, but not the body.
    assert_eq!(
        errors("impl A {\n  fn f() {\n    g()\n  fn h() {}\n}\n"),
        ["E0017 18..19"]
    );
}

/// An `impl` header that goes wrong syncs at the `{` on its line, so its methods stay in its
/// body, and `pub` on a line of its own there ends at the break.
#[test]
fn a_broken_impl_header_keeps_its_body() {
    for text in ["impl  {\n  fn f() {}\n}\n", "impl A A {\n  fn f() {}\n}\n"] {
        assert_eq!(codes(text), ["E0016"], "{text:?}");
        assert_eq!(decls(text), [SyntaxKind::ImplDecl], "{text:?}");
    }
    assert_eq!(
        messages("impl A A {\n  fn f() {}\n}\n"),
        ["expected `{`, found the name `A`"]
    );
    let text = "impl A {\n  pub\n  fn f() {}\n}\n";
    assert_eq!(messages(text), ["expected `fn`, found a line break"]);
    assert_eq!(decls(text), [SyntaxKind::ImplDecl]);
}

/// A member start ends the constructs it closes even after a token that continues the line,
/// so the next declaration is parsed, and its own slips reported.
#[test]
fn a_declaration_after_a_token_that_continues_the_line() {
    let text = "fn f() {\nfn g() {\n  x y\n}\n";
    assert_eq!(errors(text), ["E0017 7..8", "E0016 22..23"]);
    assert_eq!(decls(text), [SyntaxKind::FnDecl, SyntaxKind::FnDecl]);
    let text = "fn f() {\n  g(\nfn h() {\n  x y\n}\n";
    assert_eq!(errors(text), ["E0017 7..8", "E0017 12..13", "E0016 27..28"]);
    assert_eq!(decls(text), [SyntaxKind::FnDecl, SyntaxKind::FnDecl]);
    let text = "fn f() {\n  g(a,\nfn h() {\n  x y\n}\n";
    assert_eq!(errors(text), ["E0017 7..8", "E0017 12..13", "E0016 29..30"]);
    assert_eq!(decls(text), [SyntaxKind::FnDecl, SyntaxKind::FnDecl]);
    let text = "fn f(a: Int,\nfn g() {}\n";
    assert_eq!(errors(text), ["E0017 4..5"]);
    assert_eq!(decls(text), [SyntaxKind::FnDecl, SyntaxKind::FnDecl]);
}

/// `let`, `var` and `while` start a statement of a block, so each closes the lists inside it.
#[test]
fn a_statement_closes_a_list() {
    assert_eq!(errors("fn f() {\n  g(\n  let x = 1\n}\n"), ["E0017 12..13"]);
    assert_eq!(errors("fn f() {\n  g(\n  var x = 1\n}\n"), ["E0017 12..13"]);
    assert_eq!(
        errors("fn f() {\n  g(\n  while x {}\n}\n"),
        ["E0017 12..13"]
    );
}

/// A closing bracket closes every list inside the bracket it matches.
#[test]
fn a_closing_bracket_further_out_closes_a_list() {
    assert_eq!(errors("fn f() {\n  g(a\n}\n"), ["E0017 12..13"]);
    assert_eq!(
        errors("fn f() {\n  g(h(a\n}\n"),
        ["E0017 12..13", "E0017 14..15"]
    );
    assert_eq!(
        errors("fn f() {\n  g([a\n}\n"),
        ["E0017 12..13", "E0017 13..14"]
    );
}

/// A closing bracket that matches nothing open closes the innermost list in its place, with
/// one E0016. In a line context it is skipped, as one with nothing open is.
#[test]
fn a_closing_bracket_that_matches_nothing() {
    assert_eq!(errors("fn f() {\n  g(a, b]\n}\n"), ["E0016 17..18"]);
    assert_eq!(
        messages("fn f() {\n  g(a, b]\n}\n"),
        ["expected `,` or `)`, found `]`"]
    );
    assert_eq!(errors("fn f() {\n  g)\n}\n"), ["E0016 12..13"]);
    assert_eq!(errors("}\nfn f() {}"), ["E0016 0..1"]);
    assert_eq!(messages("}\n"), ["expected a declaration, found `}`"]);
    assert_eq!(
        messages("fn f() {}\n}\n"),
        ["expected a declaration, found `}`"]
    );
    assert_eq!(
        messages("fn f() {\n  g(]\n}\n"),
        ["expected an expression or `)`, found `]`"]
    );
    // Closed in its place, the list doesn't swallow the next line.
    assert_eq!(codes("fn f() {\n  g(a]\n  h(\n}\n"), ["E0016", "E0017"]);
}

/// A skip stops at a declaration, even inside a bracket it skipped, so the declaration is
/// parsed.
#[test]
fn a_skip_stops_at_a_declaration() {
    assert_eq!(
        errors("fn match() {\n  let a = 1\n\nfn g() {\n  h(\n}\n"),
        ["E0016 3..8", "E0017 38..39"]
    );
}

/// The end of the file closes every open bracket, with an E0017 at each.
#[test]
fn the_end_of_the_file_closes_everything() {
    assert_eq!(errors("fn f() {\n  g(1"), ["E0017 7..8", "E0017 12..13"]);
    assert_eq!(
        messages("const A: Int ="),
        ["expected an expression, found the end of the file"]
    );
    assert_eq!(errors("const A: Int ="), ["E0016 14..14"]);
    assert_eq!(
        messages("fn f() ->"),
        ["expected a type, found the end of the file"]
    );
    assert_eq!(errors("fn f() ->"), ["E0016 9..9"]);
    // What the constructs around an unclosed bracket lack is part of that slip.
    assert_eq!(errors("fn f(a:"), ["E0017 4..5"]);
    assert_eq!(errors("fn f(a: Int"), ["E0017 4..5"]);
}

/// An unclosed one-line literal ends its line's lists with no error, and nothing else on the
/// line is reported. A block's `{` stays open.
#[test]
fn an_unclosed_literal_ends_its_line() {
    assert_eq!(codes("fn f() {\n  log(\"done)\n}\n"), ["E0004"]);
    assert_eq!(
        codes("fn f() {\n  let a = [g(\"hello)\n  let b = 'c\n  h(x y\n}\n"),
        ["E0004", "E0009", "E0016"]
    );
    assert_eq!(
        codes("fn f() {\n  let a = g(h(\"hello)\n  let b = 'c\n  h(x y\n}\n"),
        ["E0004", "E0009", "E0016"]
    );
    assert_eq!(codes("fn f() { log(\"done)\n}\n"), ["E0004"]);
    assert_eq!(codes("fn f() {\n  g('c\n}\n"), ["E0009"]);
    // Not even an error before the literal.
    assert_eq!(codes("fn f() {\n  let = \"abc\n}\n"), ["E0004"]);
    // A bracket skipped after the error at `b`, itself not reported, closes at the line's end
    // too, so the next line is parsed, and its own slip reported.
    assert_eq!(
        codes("fn f() {\n  g(a b (\"x\n  h(\n}\n"),
        ["E0004", "E0017"]
    );
}

/// After an item whose line an unclosed one-line literal ends, a missing `,` is part of that
/// slip, and may be inside the literal, so it isn't reported. The next item is parsed as one.
#[test]
fn a_lists_comma_after_an_unclosed_literal() {
    assert_eq!(
        codes("fn f() {\n  g(\n    \"abc\n    b,\n  )\n}\n"),
        ["E0004"]
    );
    assert_eq!(
        codes("fn f() {\n  g(\n    a,\n    \"abc\n    b,\n  )\n}\n"),
        ["E0004"]
    );
    assert_eq!(
        codes("fn f() {\n  let x = [\n    \"abc\n    b,\n  ]\n}\n"),
        ["E0004"]
    );
    assert_eq!(codes("fn f() {\n  g(\n    'a\n    b,\n  )\n}\n"), ["E0009"]);
    assert_eq!(
        codes("fn f() {\n  g(\n    x: \"abc\n    y: 1,\n  )\n}\n"),
        ["E0004"]
    );
    // An item that takes nothing at the line break is skipped, so the parser goes on: a
    // parameter stops at a break, and a field argument needs its `name:`.
    assert_eq!(
        codes("fn f(\n  a: String = \"abc\n  b: Int,\n) {}\n"),
        ["E0004"]
    );
    assert_eq!(
        codes("fn f() {\n  g(\n    ..base,\n    x: \"abc\n    y,\n  )\n}\n"),
        ["E0004"]
    );
    assert_eq!(
        codes("fn f() {\n  let Shape.Rect(\n    width: \"abc\n    height,\n  ) = s\n}\n"),
        ["E0004"]
    );
    let text = "fn f() {\n  g(\n    \"abc\n    b,\n  )\n}\n";
    let args = checked(text)
        .syntax()
        .descendants()
        .find(|node| node.kind() == SyntaxKind::ArgList)
        .expect("an argument list");
    assert_eq!(
        args.children().count(),
        2,
        "{}",
        dump_tree_without_trivia(&args)
    );
    // A closed string leaves the `,` to the list, with its fix.
    assert_eq!(
        errors("fn f() {\n  g(\n    \"abc\"\n    b,\n  )\n}\n"),
        [r#"E0016 28..29 => "fn f() {\n  g(\n    \"abc\",\n    b,\n  )\n}\n""#]
    );
}

/// An unclosed multi-line string or block comment swallows the end of the file, so the
/// parser reports nothing there.
#[test]
fn nothing_is_reported_at_a_swallowed_end() {
    assert_eq!(codes("fn f() {\n  g(\"\"\"\n  text\n"), ["E0005"]);
    assert_eq!(codes("fn f() {\n  g(\n  /* never closed\n"), ["E0003"]);
    assert_eq!(codes("fn f /* never closed"), ["E0003"]);
    // `/*/` ends with `*/`, but the two share the `*`, so it's still open.
    assert_eq!(codes("fn f() {\n  g(\n/*/"), ["E0003"]);
    // A closed one still lets the end of the file report.
    assert_eq!(
        codes("fn f() {\n  g(\n  /* closed */\n"),
        ["E0017", "E0017", "E0003"]
    );
}

/// What the parser would report right after a character that starts no token is the
/// lexer's error alone.
#[test]
fn nothing_is_reported_after_an_error_token() {
    assert_eq!(errors("fn f() {\n  let x = $\n}\n"), ["E0001 19..20"]);
    assert_eq!(codes("fn f() {\n  let x = 1 $ 2\n}\n"), ["E0001"]);
    // An `Error` token last in the file: nothing is reported at the end.
    assert_eq!(errors("fn f() ->$"), ["E0001 9..10"]);
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
    // The same holds when the end of the file comes after the `;`.
    assert_eq!(errors("fn f() ->;"), [r#"E0002 9..10 => "fn f() ->""#]);
    assert_eq!(errors("fn f() ->;\n"), [r#"E0002 9..10 => "fn f() ->\n""#]);
    assert_eq!(codes("const A: Int =; // done\n"), ["E0002"]);
    // An unclosed bracket still gets its E0017 there.
    assert_eq!(codes("fn f() {\n  let a =;"), ["E0017", "E0002"]);
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
        messages("fn f() {\n  let\n  x = 1\n}"),
        ["expected a pattern, found a line break"]
    );
}

/// The tree of `text`, whatever its errors.
fn tree(text: &str) -> String {
    dump_tree_without_trivia(&checked(text).syntax())
}

/// A line that starts with what can't start a statement but continues the line above is one
/// E0018 at the break, whose fix joins the lines, and the tree is the joined one (decision 4).
#[test]
fn a_line_break_too_early() {
    let cases = [
        (
            "fn f() {\n  let x\n  = 1\n}",
            "fn f() {\n  let x = 1\n}",
            16..17,
        ),
        ("fn f() {\n  x\n  += 1\n}", "fn f() {\n  x += 1\n}", 12..13),
        (
            "fn f() {\n  if a {\n  }\n  else {}\n}",
            "fn f() {\n  if a {\n  } else {}\n}",
            21..22,
        ),
        (
            "fn f() {\n  if a\n\n  {}\n}",
            "fn f() {\n  if a {}\n}",
            15..16,
        ),
        (
            "fn f() {\n  if a {} else\n  {}\n}",
            "fn f() {\n  if a {} else {}\n}",
            23..24,
        ),
        ("fn f() {\n  a\n    * b\n}", "fn f() {\n  a * b\n}", 12..13),
        ("fn f() {\n  g()\n    ?\n}", "fn f() {\n  g() ?\n}", 14..15),
        (
            "fn f() {\n  g(if a {}\n  else {})\n}",
            "fn f() {\n  g(if a {} else {})\n}",
            20..21,
        ),
        ("impl T\n{}", "impl T {}", 6..7),
        ("enum E\n{ A }", "enum E { A }", 6..7),
        ("test \"t\"\n{}", "test \"t\" {}", 8..9),
        ("const N: Int\n  = 1", "const N: Int = 1", 12..13),
        ("type T\n  = Int", "type T = Int", 6..7),
        (
            "fn f() {\n  let x\n  : Int = 1\n}",
            "fn f() {\n  let x : Int = 1\n}",
            16..17,
        ),
        ("const N\n  : Int = 1", "const N : Int = 1", 7..8),
        ("struct S\n{ a: Int }", "struct S { a: Int }", 8..9),
        (
            "fn f() {\n  g(\n    a\n    + b\n  )\n}",
            "fn f() {\n  g(\n    a + b\n  )\n}",
            19..20,
        ),
    ];
    for (text, joined, at) in cases {
        assert_eq!(
            errors(text),
            [format!("E0018 {at:?} => {joined:?}")],
            "{text:?}"
        );
        assert_eq!(tree(text), tree(joined), "{text:?}");
    }
    assert_eq!(
        messages("fn f() {\n  let x\n  = 1\n}"),
        ["a line break can't come before `=`, which continues the line above"]
    );
    assert_eq!(
        messages("fn f() {\n  a\n  >= b\n}"),
        ["a line break can't come before `>=`, which continues the line above"]
    );
}

/// Type arguments accepted because a line break follows their `>` would be less-than once the
/// lines were joined, so no fix joins them unless the next token may follow type arguments.
#[test]
fn no_join_that_changes_type_arguments() {
    assert_eq!(
        errors("fn f() {\n  let v = make<Int>\n    + 1\n}"),
        ["E0018 28..29"]
    );
    assert_eq!(
        errors("fn f() {\n  let v = make<Int>\n    ?\n}"),
        [r#"E0018 28..29 => "fn f() {\n  let v = make<Int> ?\n}""#]
    );
}

/// No fix joins lines with a comment between them.
#[test]
fn no_join_across_a_comment() {
    assert_eq!(
        errors("fn f() {\n  let x // the total\n  = 1\n}"),
        ["E0018 29..30"]
    );
    assert_eq!(
        errors("fn f() {\n  let x\n  // the total\n  = 1\n}"),
        ["E0018 16..17"]
    );
}

/// A line that starts with `-` or `(` is a new statement, and in a list a line break is only
/// spacing (`r[lex.continue.lists]`). After a `;` nothing joins: `.` there is an E0016 after
/// the lexer's E0002.
#[test]
fn what_doesnt_join() {
    assert_eq!(errors("fn f() {\n  a\n  -b\n  (c)\n}"), [] as [String; 0]);
    // In a list, a line break before a `,` or the closing bracket is spacing, and a line that
    // starts with `-` or `(` is a new item, which needs its `,` (`r[lex.continue.lists]`).
    assert_eq!(
        tree("fn f() {\n  g(\n    a\n    , b\n  )\n}"),
        tree("fn f() {\n  g(a, b)\n}")
    );
    assert_eq!(
        errors("fn f() {\n  g(\n    a\n    -1\n  )\n}"),
        [r#"E0016 24..25 => "fn f() {\n  g(\n    a,\n    -1\n  )\n}""#]
    );
    assert_eq!(
        errors("fn f() {\n  g(\n    a\n    (b)\n  )\n}"),
        [r#"E0016 24..25 => "fn f() {\n  g(\n    a,\n    (b)\n  )\n}""#]
    );
    assert_eq!(codes("fn f() {\n  a;\n  .b()\n}"), ["E0002", "E0016"]);
    assert_eq!(codes("fn f() {\n  a;\n  + b\n}"), ["E0002", "E0016"]);
    assert_eq!(
        codes("fn f() {\n  if a {};\n  else {}\n}"),
        ["E0002", "E0016"]
    );
    // In a list a `;` ends the item too: `+` after it is an E0016.
    assert_eq!(codes("fn f() {\n  g(a;\n    + b)\n}"), ["E0002", "E0016"]);
}

/// A keyword can't be a name, and the error names it (`r[lex.keyword-not-name]`).
#[test]
fn a_keyword_where_a_name_goes() {
    assert_eq!(
        messages("fn match() {}\nfn f(while: Int) {}\nfn g() {\n  let for = 1\n  a.fn()\n}"),
        [
            "expected a function name, found the keyword `match`",
            "expected a parameter, found the keyword `while`",
            "expected a pattern, found the keyword `for`",
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

/// A contract's expression starts on its line, so a line that starts with `{` or another
/// contract after `requires` is the expression missing, at the line break. After a slip in a
/// contract the parser skips to the next contract or the body's `{`, never to a `{` inside a
/// later contract, so the body is still the body.
#[test]
fn a_slip_in_a_contract() {
    let has_body = |text: &str| {
        checked(text)
            .syntax()
            .descendants()
            .any(|node| node.kind() == SyntaxKind::Block)
    };
    for text in [
        "fn f()\n  requires\n{\n  x\n}\n",
        "fn f()\n  requires\n  ensures r > 0\n{\n  x\n}\n",
    ] {
        assert_eq!(errors(text), ["E0016 17..18"], "{text:?}");
        assert_eq!(
            messages(text),
            ["expected an expression, found a line break"]
        );
        assert!(has_body(text), "{text:?}");
    }
    for (text, error) in [
        (
            "fn f()\n  requires x >\n  ensures r > 0\n{\n  x\n}\n",
            "E0016 24..31",
        ),
        (
            "fn withdraw(a: Account) -> Int\n  requires  > 0\n  ensures(r) match r {\n    Ok(a) => true,\n    Err(_) => true,\n  }\n{\n  pay(a)\n}\n",
            "E0016 43..44",
        ),
        (
            "fn f(x: Int) -> Int\n  requires x >\n  ensures(r) if r > 0 { true } else { false }\n{\n  x\n}\n",
            "E0016 37..44",
        ),
    ] {
        assert_eq!(errors(text), [error], "{text:?}");
        assert!(has_body(text), "{text:?}");
    }
    // Something after a whole condition, on its line, is one error, and the rest of the
    // declaration is skipped, as after a return type: the first `{` after it may not be the
    // body's.
    for (text, error) in [
        ("fn f(a: Int) requires a b {\n  a\n}\n", "E0016 24..25"),
        (
            "fn f() requires a b if c { d } else { e }\n{\n  x\n}\n",
            "E0016 18..19",
        ),
        (
            "fn f() -> Int\n  ensures() match r {\n    Ok(a) => true,\n  }\n{\n  x\n}\n",
            "E0016 26..31",
        ),
    ] {
        assert_eq!(errors(text), [error], "{text:?}");
    }
    // After a contract, only another contract or the body can come.
    assert_eq!(
        messages("fn f() requires x Int {}\n"),
        ["expected `requires`, `ensures` or `{`, found the name `Int`"]
    );
    assert_eq!(
        messages("fn f()\n  requires a\n  requires b\nx\n"),
        ["expected `requires`, `ensures` or `{`, found a line break"]
    );
    // Braces right after `requires` are an expression, a block that can't go there. The body
    // missing after it is part of that slip.
    assert_eq!(errors("fn f(n: Int) requires { n }\n"), ["E0016 22..23"]);
}

/// A `select`'s arms are a `match`'s list: an arm's `=>` closes the brackets opened in its own
/// arm, even in a `select` inside a `match` arm, so the arms after it parse. After an arm
/// whose line an unclosed string ends, the missing `,` is part of that slip.
#[test]
fn slips_in_select_arms() {
    assert_eq!(
        errors("fn f() {\n  match s { 0 => select { x = c.recv( => 1, }, 1 => 2, }\n}\n"),
        ["E0017 45..46"]
    );
    // A bracket left open in a select arm's pattern closes at the arm's `=>`, and the
    // `select`'s braces stay open.
    assert_eq!(
        errors("fn f() {\n  match s { 0 => select { (x => 1, }, 1 => 2, }\n}\n"),
        ["E0017 35..36"]
    );
    assert_eq!(
        codes("fn f() {\n  select {\n    x = \"abc\n  }\n}\n"),
        ["E0004"]
    );
}

/// The braces of `concurrent`, `select` and `with`, and a `background`'s block, go on the line
/// of what comes before them, as a body's do: on the next line they are an E0018, whose fix
/// joins the lines.
#[test]
fn braces_on_the_next_line() {
    for (text, at) in [
        (
            "fn f() {\n  concurrent\n  { spawn a() }\n}\n",
            "E0018 21..22",
        ),
        (
            "fn f() {\n  select\n  { x = c.r() => 1, }\n}\n",
            "E0018 17..18",
        ),
        ("fn f() {\n  with Db = db\n  { run() }\n}\n", "E0018 23..24"),
        (
            "fn f() {\n  background(bg: TaskScope)\n  { run() }\n}\n",
            "E0018 36..37",
        ),
    ] {
        assert_eq!(codes(text), ["E0018"], "{text:?}");
        assert!(
            errors(text)[0].starts_with(at),
            "{text:?}: {:?}",
            errors(text)
        );
    }
}

/// A `concurrent`'s spawns are a list, so a block's member start closes them, with one E0017.
/// After a slip in a `with` handler, the rest of its line is skipped up to the `{`, so the
/// block is still the block.
#[test]
fn slips_in_concurrency_forms() {
    assert_eq!(
        errors("fn f() {\n  let x = concurrent {\n    spawn a()\n  let y = 1\n}\n"),
        ["E0017 30..31"]
    );
    let text = "fn f() {\n  with = db { run() }\n}\n";
    assert_eq!(errors(text), ["E0016 16..17"]);
    let block = checked(text)
        .syntax()
        .descendants()
        .any(|node| node.kind() == SyntaxKind::Block && node.text() == "{ run() }");
    assert!(block);
}

/// `fn` and `(` is a closure, never a member start (decision 9): a closure on a line of its
/// own closes nothing, and the attribute look-past reads past one in an attribute's
/// arguments.
#[test]
fn a_closure_line_closes_nothing() {
    assert_eq!(
        errors("fn f() {\n  g(a,\n  fn(x) { x }\n}\n"),
        ["E0017 12..13"]
    );
    assert_eq!(
        errors("fn f() {\n  g(1,\n  @attr(\n    fn(x: Int) { x },\n  )\nfn h() {}\n"),
        ["E0017 7..8", "E0017 12..13"]
    );
}
