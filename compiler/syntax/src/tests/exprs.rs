//! The parser's trees for operators, and their errors. Each expected dump is written by hand.

use super::cst::dump_tree_without_trivia;
use super::parser::{checked, stmts};
use crate::SyntaxKind;

/// The tree of `expr`, a `let`'s value, after checking that it parses with no error.
fn tree(expr: &str) -> String {
    let dump = stmts(&format!("  let x = {expr}"));
    let value = dump.find("  Eq \"=\"\n").expect("a `let`") + "  Eq \"=\"\n".len();
    dump[value..]
        .lines()
        .map(|line| format!("{}\n", &line[2..]))
        .collect()
}

/// The tree of `expr`, a `let`'s value, whatever its errors.
fn tree_with_errors(expr: &str) -> String {
    let text = format!("fn f() {{\n  let x = {expr}\n}}\n");
    let parse = checked(&text);
    let value = parse
        .syntax()
        .descendants()
        .find(|node| node.kind() == SyntaxKind::LetStmt)
        .and_then(|stmt| stmt.children().nth(1))
        .expect("a value");
    dump_tree_without_trivia(&value)
}

const PREFIX: &str = "fn f() {\n  let x = ";

/// The errors of `expr`, a `let`'s value: each as `code start..end` in `expr`, then the
/// fixed expression when it has one fix.
fn errors(expr: &str) -> Vec<String> {
    let text = format!("{PREFIX}{expr}\n}}\n");
    let parse = checked(&text);
    parse
        .errors()
        .iter()
        .map(|e| {
            let at = |offset: u32| offset as usize - PREFIX.len();
            let mut out = format!("{} {:?}", e.code, at(e.range.start)..at(e.range.end));
            if let [fix] = e.fixes.as_slice() {
                let mut fixed = text.clone();
                for (range, with) in fix.edits.iter().rev() {
                    fixed.replace_range(range.start as usize..range.end as usize, with);
                }
                let fixed = &fixed[PREFIX.len()..fixed.len() - "\n}\n".len()];
                out += &format!(" => {fixed}");
            }
            out
        })
        .collect()
}

fn messages(expr: &str) -> Vec<String> {
    let text = format!("{PREFIX}{expr}\n}}\n");
    let parse = checked(&text);
    parse.errors().iter().map(|e| e.message.clone()).collect()
}

/// `r[expr.op.levels]`: postfix, prefix, `*`, `+`, comparisons, then `&&` and `||`, tightest
/// first.
#[test]
fn the_levels() {
    assert_eq!(
        tree("a || b < c + d * -e.f"),
        r#"Binary
  NameRef
    Ident "a"
  PipePipe "||"
  Binary
    NameRef
      Ident "b"
    Lt "<"
    Binary
      NameRef
        Ident "c"
      Plus "+"
      Binary
        NameRef
          Ident "d"
        Star "*"
        Prefix
          Minus "-"
          Field
            NameRef
              Ident "e"
            Dot "."
            NameRef
              Ident "f"
"#
    );
    assert_eq!(
        tree("a % b / c"),
        r#"Binary
  Binary
    NameRef
      Ident "a"
    Percent "%"
    NameRef
      Ident "b"
  Slash "/"
  NameRef
    Ident "c"
"#
    );
}

/// Operators of one level group to the left, and prefix operators to the right.
#[test]
fn grouping() {
    assert_eq!(
        tree("a - b - c"),
        r#"Binary
  Binary
    NameRef
      Ident "a"
    Minus "-"
    NameRef
      Ident "b"
  Minus "-"
  NameRef
    Ident "c"
"#
    );
    assert_eq!(
        tree("-!~a"),
        r#"Prefix
  Minus "-"
  Prefix
    Bang "!"
    Prefix
      Tilde "~"
      NameRef
        Ident "a"
"#
    );
    assert_eq!(
        tree("(a + b) * c"),
        r#"Binary
  Paren
    LParen "("
    Binary
      NameRef
        Ident "a"
      Plus "+"
      NameRef
        Ident "b"
    RParen ")"
  Star "*"
  NameRef
    Ident "c"
"#
    );
}

/// The operators with no level go where decision 6 puts them: `<<` and `>>` below `+`, then
/// `&`, `^`, `|`, ranges and comparisons. So a forbidden mix still gets the usual reading's
/// tree, with its error.
#[test]
fn the_usual_reading() {
    assert_eq!(
        tree("1..n + 1"),
        r#"Range
  Literal
    Int "1"
  DotDot ".."
  Binary
    NameRef
      Ident "n"
    Plus "+"
    Literal
      Int "1"
"#
    );
    assert_eq!(
        tree_with_errors("flags & mask == 0"),
        r#"Binary
  Binary
    NameRef
      Ident "flags"
    Amp "&"
    NameRef
      Ident "mask"
  EqEq "=="
  Literal
    Int "0"
"#
    );
    assert_eq!(
        tree_with_errors("a | b ^ c & d << e"),
        r#"Binary
  NameRef
    Ident "a"
  Pipe "|"
  Binary
    NameRef
      Ident "b"
    Caret "^"
    Binary
      NameRef
        Ident "c"
      Amp "&"
      Binary
        NameRef
          Ident "d"
        LtLt "<<"
        NameRef
          Ident "e"
"#
    );
    assert_eq!(
        tree_with_errors("r == 0..3"),
        r#"Binary
  NameRef
    Ident "r"
  EqEq "=="
  Range
    Literal
      Int "0"
    DotDot ".."
    Literal
      Int "3"
"#
    );
}

/// Decision 5: a `>` and an adjacent `>` or `=` join into one token, `>>` or `>=`. With
/// anything between them they are two tokens, and `a > = b` lacks an expression.
#[test]
fn joined_tokens() {
    assert_eq!(
        tree("a >> b"),
        "Binary\n  NameRef\n    Ident \"a\"\n  GtGt \">>\"\n  NameRef\n    Ident \"b\"\n"
    );
    assert_eq!(
        tree("a >= b"),
        "Binary\n  NameRef\n    Ident \"a\"\n  GtEq \">=\"\n  NameRef\n    Ident \"b\"\n"
    );
    assert_eq!(errors("a > > b"), ["E0016 4..5"]);
    assert_eq!(errors("a > = b"), ["E0016 4..5"]);
    assert_eq!(messages("a > = b"), ["expected an expression, found `=`"]);
    // `>==` is `>` and `==`.
    assert_eq!(errors("a >== b"), ["E0016 3..5"]);
}

/// `r[lex.line-end]`: a line that ends with a binary operator continues, `*` and `>` and the
/// joined `>>` included. A line that starts with one is a new statement.
#[test]
fn a_binary_operator_continues_the_line() {
    for op in ["*", ">", ">>", ">=", "+", "&&"] {
        let text = format!("a {op}\n    b");
        assert_eq!(tree(&text).lines().count(), 6, "{text:?}");
    }
    assert_eq!(
        stmts("  a\n  - b"),
        "NameRef\n  Ident \"a\"\nPrefix\n  Minus \"-\"\n  NameRef\n    Ident \"b\"\n"
    );
    // After `-` a line break is spacing, since `-` can be binary. After `!` it ends the line.
    assert_eq!(tree("-\n    a").lines().count(), 4);
    assert_eq!(errors("!\n  y"), ["E0016 1..2"]);
}

/// E0019 (`r[expr.op.parens-required]`), at the operand that needs parentheses, with the fix
/// that adds them for the usual reading.
#[test]
fn operands_that_need_parentheses() {
    // `r[expr.op.logic.no-mix]`.
    assert_eq!(errors("a || b && c"), ["E0019 5..11 => a || (b && c)"]);
    assert_eq!(errors("a && b || c"), ["E0019 0..6 => (a && b) || c"]);
    // `r[expr.op.bits.parens]`: a bit operator as an operand, or with one as its operand.
    assert_eq!(
        errors("flags & mask == 0"),
        ["E0019 0..12 => (flags & mask) == 0"]
    );
    assert_eq!(errors("1 << n - 1"), ["E0019 5..10 => 1 << (n - 1)"]);
    assert_eq!(errors("a + b & c"), ["E0019 0..5 => (a + b) & c"]);
    // `r[expr.op.bits.chain]`, and `grammar.bits`: a shift doesn't chain.
    assert_eq!(errors("a & b | c"), ["E0019 0..5 => (a & b) | c"]);
    assert_eq!(errors("a << b << c"), ["E0019 0..6 => (a << b) << c"]);
    assert_eq!(errors("a >> b >> c"), ["E0019 0..6 => (a >> b) >> c"]);
    // `r[expr.op.range.parens]`.
    assert_eq!(errors("r == 0..3"), ["E0019 5..9 => r == (0..3)"]);
    assert_eq!(errors("0..3 == r"), ["E0019 0..4 => (0..3) == r"]);
    // Each operand that needs them is its own error.
    assert_eq!(
        errors("a & b == c & d"),
        [
            "E0019 0..5 => (a & b) == c & d",
            "E0019 9..14 => a & b == (c & d)"
        ]
    );
    assert_eq!(
        messages("a || b && c"),
        ["`&&` needs parentheses as an operand of `||`"]
    );
    assert_eq!(
        messages("a < b >> c"),
        ["`>>` needs parentheses as an operand of `<`"]
    );
}

/// What the spec allows needs no parentheses.
#[test]
fn operands_that_need_none() {
    for expr in [
        "a && b && c",
        "a || b || c",
        "a & b & c",
        "a | b | c",
        "a ^ b ^ c",
        "a + 1 < b * 2 && c",
        "(a & b) == 0",
        "a & (b | c)",
        "1 << (n - 1)",
        "-a & ~b",
        "0..n + 1",
        "r == (0..3)",
    ] {
        assert_eq!(errors(expr), Vec::<String>::new(), "{expr}");
    }
}

/// E0020 (`r[expr.op.compare.no-chain]`, `r[expr.op.range.no-chain]`): at the operator that
/// chains, one error per chain. A comparison chain's fix repeats each middle operand.
#[test]
fn chains() {
    assert_eq!(errors("a < b > c"), ["E0020 6..7 => a < b && b > c"]);
    assert_eq!(errors("0 <= i < n"), ["E0020 7..8 => 0 <= i && i < n"]);
    assert_eq!(
        errors("a < b < c < d"),
        ["E0020 6..7 => a < b && b < c && c < d"]
    );
    assert_eq!(
        errors("a < f(b) + 1 >= c"),
        ["E0020 13..15 => a < f(b) + 1 && f(b) + 1 >= c"]
    );
    // A middle operand with a line break in it isn't repeated.
    assert_eq!(errors("a < (b\n  ) < c"), ["E0020 11..12"]);
    assert_eq!(errors("0..n..10"), ["E0020 4..6"]);
    assert_eq!(errors("0..1..2..3"), ["E0020 4..6"]);
    assert_eq!(messages("a < b > c"), ["comparisons don't chain"]);
    assert_eq!(messages("0..n..10"), ["ranges don't chain"]);
    // As an operand of `&&` the fixed chain needs no parentheses, and as one of `||` it does,
    // since `&&` can't be mixed with `||`.
    assert_eq!(
        errors("a < b < c && d"),
        ["E0020 6..7 => a < b && b < c && d"]
    );
    assert_eq!(
        errors("x || a < b < c"),
        ["E0020 11..12 => x || (a < b && b < c)"]
    );
    assert_eq!(
        errors("a < b < c || x"),
        ["E0020 6..7 => (a < b && b < c) || x"]
    );
    // A middle operand that needs parentheses has its own E0019 and fix, so the chain has
    // none.
    assert_eq!(
        errors("a < b..c < d"),
        ["E0019 4..8 => a < (b..c) < d", "E0020 9..10"]
    );
    assert_eq!(
        errors("x < a & b < y"),
        ["E0019 4..9 => x < (a & b) < y", "E0020 10..11"]
    );
}

/// A syntax error in an operand, such as a missing operand, is the one error, so nothing about
/// the operators around it is reported.
#[test]
fn a_syntax_error_is_the_one_error() {
    assert_eq!(errors("a & == b"), ["E0016 4..6"]);
    // `<` continues the line, so what is missing is found at the block's `}`.
    assert_eq!(errors("a < b <"), ["E0016 8..9"]);
    assert_eq!(errors("a ** b"), ["E0016 3..4"]);
    assert_eq!(errors("(a b) & c"), ["E0016 3..4"]);
    assert_eq!(errors("(a b) & c | d"), ["E0016 3..4"]);
    // A frame that closes at the missing operand reports its E0017 instead.
    assert_eq!(errors("g(a && b |"), ["E0017 1..2"]);
    assert_eq!(errors("g(1 << n -"), ["E0017 1..2"]);
    // An error a closing frame silences, such as a missing field name, is still one.
    assert_eq!(errors("g(a && b | c."), ["E0017 1..2"]);
    assert_eq!(errors("g(a < b < c."), ["E0017 1..2"]);
    // A chain whose own operands hold no error is reported, whatever its sibling holds.
    assert_eq!(
        errors("a < b < c && f(x y)"),
        ["E0020 6..7 => a < b && b < c && f(x y)", "E0016 17..18"]
    );
    assert_eq!(errors("a < b < c <"), ["E0016 12..13"]);
}

/// `r[expr.op.type-args]`: type arguments after a name go in its `NameRef`, after a path's
/// last name too.
#[test]
fn type_arguments() {
    assert_eq!(
        tree("json.decode<Order>(body)"),
        r#"Call
  Field
    NameRef
      Ident "json"
    Dot "."
    NameRef
      Ident "decode"
      TypeArgs
        Lt "<"
        TypePath
          Ident "Order"
        Gt ">"
  ArgList
    LParen "("
    NameRef
      Ident "body"
    RParen ")"
"#
    );
    assert_eq!(
        tree("Channel<Job>.new"),
        r#"Field
  NameRef
    Ident "Channel"
    TypeArgs
      Lt "<"
      TypePath
        Ident "Job"
      Gt ">"
  Dot "."
  NameRef
    Ident "new"
"#
    );
    // Case doesn't decide it: a type can be a C name.
    assert_eq!(
        tree("CBuf.new<Ptr<sqlite3>>()"),
        tree("CBuf.new<Ptr<Sqlite3>>()").replace("Sqlite3", "sqlite3")
    );
}

/// The tokens that may follow type arguments' `>`: `(`, `.`, `?`, `,`, `)`, `]`, `}` and a
/// line break. After any other, the `<` is less-than.
#[test]
fn what_may_follow_type_arguments() {
    let has_type_args = |text: &str| {
        let parse = checked(text);
        parse.syntax().descendants().any(|node| {
            node.kind() == SyntaxKind::TypeArgs
                && node.parent().unwrap().kind() == SyntaxKind::NameRef
        })
    };
    for text in [
        "fn f() {\n  g(id<Int>)\n}\n",
        "fn f() {\n  id<Int>.name\n}\n",
        "fn f() {\n  g(id<Int>, 1)\n}\n",
        "fn f() {\n  [id<Int>]\n}\n",
        "fn f() { id<Int> }\n",
        "fn f() {\n  id<Int>\n}\n",
        "fn f() {\n  id<Int>",
        // `?` is a postfix operator, so the tree holds type arguments whatever follows it.
        "fn f() {\n  id<Int>?\n}\n",
    ] {
        assert!(has_type_args(text), "{text:?}");
    }
    for text in [
        "fn f() {\n  a < b > c\n}\n",
        "fn f() {\n  a < b >= c\n}\n",
        "fn f() {\n  a < b >> c\n}\n",
        "fn f() {\n  a < b > -c\n}\n",
        "fn f() {\n  a < b > [c]\n}\n",
        "fn f() {\n  g(a < b, c > d)\n}\n",
    ] {
        assert!(!has_type_args(text), "{text:?}");
    }
}

/// When the `<` is less-than, parsing ahead leaves nothing behind: no error, no node and no
/// closed bracket.
#[test]
fn less_than_after_parsing_ahead() {
    assert_eq!(
        tree("g(a < b, c > d)"),
        r#"Call
  NameRef
    Ident "g"
  ArgList
    LParen "("
    Binary
      NameRef
        Ident "a"
      Lt "<"
      NameRef
        Ident "b"
    Comma ","
    Binary
      NameRef
        Ident "c"
      Gt ">"
      NameRef
        Ident "d"
    RParen ")"
"#
    );
    for expr in [
        "a < b + c",
        "g(a < b)",
        "[a < b, c]",
        "a < (b) && c > d",
        "a < f(b > c)",
    ] {
        assert_eq!(errors(expr), Vec::<String>::new(), "{expr}");
    }
    assert_eq!(errors("a < b > c"), ["E0020 6..7 => a < b && b > c"]);
    assert_eq!(errors("x < Y >= z"), ["E0020 6..8 => x < Y && Y >= z"]);
    assert_eq!(errors("a < b >> c"), ["E0019 4..10 => a < (b >> c)"]);
    // A long chain is one error, and is parsed ahead from each `<` once: the first `<` parses
    // ahead through the rest, and records every `<` it meets.
    let chain = vec!["a"; 200].join(" < ");
    assert_eq!(errors(&chain).len(), 1);
    let text = format!("{PREFIX}{chain}\n}}\n");
    let tokens = 400 + 10;
    assert!(crate::parser::speculated_tokens(&text) <= tokens);
    // Parsing ahead leaves no slip behind, so the operands around it are still checked.
    assert_eq!(
        errors("a < b + 1 < c"),
        ["E0020 10..11 => a < b + 1 && b + 1 < c"]
    );
}

/// `r[lex.lit.int.neg]` and `r[lex.lit.neg-adjacent]`: a `-` in prefix position directly
/// before a number that no postfix operator follows is part of the literal.
#[test]
fn negative_literals() {
    assert_eq!(tree("-1"), "Literal\n  Minus \"-\"\n  Int \"1\"\n");
    assert_eq!(tree("-1.5"), "Literal\n  Minus \"-\"\n  Float \"1.5\"\n");
    assert_eq!(
        tree("-9_223_372_036_854_775_808"),
        "Literal\n  Minus \"-\"\n  Int \"9_223_372_036_854_775_808\"\n"
    );
    assert_eq!(
        tree("- 1"),
        "Prefix\n  Minus \"-\"\n  Literal\n    Int \"1\"\n"
    );
    assert_eq!(
        tree("--1"),
        "Prefix\n  Minus \"-\"\n  Literal\n    Minus \"-\"\n    Int \"1\"\n"
    );
    assert_eq!(
        tree("a - -1"),
        "Binary\n  NameRef\n    Ident \"a\"\n  Minus \"-\"\n  Literal\n    Minus \"-\"\n    Int \"1\"\n"
    );
    // In binary position a `-` is the operator, spaced or not.
    assert_eq!(tree("a -1"), tree("a - 1"));
    // A postfix operator after the number binds tighter, as `-x.abs()` reads.
    assert_eq!(
        tree("-2.pow(2)"),
        r#"Prefix
  Minus "-"
  Call
    Field
      Literal
        Int "2"
      Dot "."
      NameRef
        Ident "pow"
    ArgList
      LParen "("
      Literal
        Int "2"
      RParen ")"
"#
    );
    assert!(tree("-2\n    .pow(2)").starts_with("Prefix\n"));
    // After a `;`, a `.` starts a statement, so it isn't a postfix operator.
    let parse = checked("fn f() {\n  -2;\n  .x\n}\n");
    let block = parse
        .syntax()
        .descendants()
        .find(|node| node.kind() == SyntaxKind::Block)
        .unwrap();
    assert_eq!(block.first_child().unwrap().kind(), SyntaxKind::Literal);
    // A `(` on the next line starts a statement, so it isn't a postfix operator.
    assert_eq!(
        stmts("  -1\n  (2)"),
        "Literal\n  Minus \"-\"\n  Int \"1\"\nParen\n  LParen \"(\"\n  Literal\n    Int \"2\"\n  RParen \")\"\n"
    );
}

/// `grammar.postfix`: indexes and `?` group to the left with calls and fields, and bind
/// tighter than prefix operators (`r[expr.op.levels]`: `-x?` is `-(x?)`).
#[test]
fn indexes_and_try() {
    assert_eq!(
        tree("-xs[i]?"),
        r#"Prefix
  Minus "-"
  Try
    Index
      NameRef
        Ident "xs"
      LBracket "["
      NameRef
        Ident "i"
      RBracket "]"
    Question "?"
"#
    );
    assert_eq!(
        tree("load()?.rows"),
        r#"Field
  Try
    Call
      NameRef
        Ident "load"
      ArgList
        LParen "("
        RParen ")"
    Question "?"
  Dot "."
  NameRef
    Ident "rows"
"#
    );
    // On the next line, `[` starts a statement and `?` can't.
    assert_eq!(
        stmts("  f\n  [x]"),
        "NameRef\n  Ident \"f\"\nListLit\n  LBracket \"[\"\n  NameRef\n    Ident \"x\"\n  RBracket \"]\"\n"
    );
    assert_eq!(errors("xs[]"), ["E0016 3..4"]);
}

/// `grammar.tuple`: `()` and two or more expressions are tuples, and one is in parentheses.
#[test]
fn tuples() {
    assert_eq!(tree("()"), "Tuple\n  LParen \"(\"\n  RParen \")\"\n");
    assert_eq!(
        tree("(a, b,)"),
        r#"Tuple
  LParen "("
  NameRef
    Ident "a"
  Comma ","
  NameRef
    Ident "b"
  Comma ","
  RParen ")"
"#
    );
    assert_eq!(errors("(a,)"), ["E0016 3..4"]);
    assert_eq!(
        messages("(a,)"),
        ["expected a second expression, found `)`"]
    );
}

/// `grammar.map_lit`: entries in braces, and `{:}`.
#[test]
fn maps() {
    assert_eq!(
        tree("{\"a\": 1,}"),
        r#"MapLit
  LBrace "{"
  MapEntry
    Literal
      StringStart "\""
      StringText "a"
      StringEnd "\""
    Colon ":"
    Literal
      Int "1"
  Comma ","
  RBrace "}"
"#
    );
    assert_eq!(
        tree("{:}"),
        "MapLit\n  LBrace \"{\"\n  Colon \":\"\n  RBrace \"}\"\n"
    );
    // `{` that starts a statement is a map too (`r[expr.block.brace-map]`).
    assert_eq!(
        stmts("  {:}"),
        "MapLit\n  LBrace \"{\"\n  Colon \":\"\n  RBrace \"}\"\n"
    );
    assert_eq!(errors("{\"a\" 1}"), ["E0016 5..6"]);
    // With no expression there is no entry node, which would be empty (found by the lexer's
    // mutation test).
    assert_eq!(errors("{= 1}"), ["E0016 1..2"]);
    // A line break in the braces is spacing, before the `:` too.
    assert_eq!(tree("{\"a\"\n    : 1}"), tree("{\"a\": 1}"));
    // Braces whose first item has no `:`, and end or have a line after it, hold statements: a
    // block, which can't go here (`r[expr.block.no-bare]`). On one line it is a map's.
    assert_eq!(errors("{\n    a + 1\n  }"), ["E0016 0..1"]);
    assert_eq!(errors("{ g() }"), ["E0016 0..1"]);
    assert_eq!(errors("{\n    g()\n    h()\n  }"), ["E0016 0..1"]);
    assert_eq!(
        messages("{ g() }"),
        ["expected an expression, found a block"]
    );
    assert_eq!(
        messages("{\"a\"}"),
        ["expected an expression, found a block"]
    );
    // Braces that hold an entry's `:` are a map, whose entry lacks its `:`.
    assert_eq!(messages("{\"a\": 1, \"b\"}"), ["expected `:`, found `}`"]);
    assert_eq!(
        messages("{\n    \"a\"\n    \"b\": 2\n  }"),
        ["expected `:`, found a string"]
    );
}

/// E0025: `{}` where an expression is expected, whose fix writes `{:}`.
#[test]
fn empty_braces() {
    assert_eq!(errors("{}"), ["E0025 0..2 => {:}"]);
    assert_eq!(errors("{ }"), ["E0025 0..3 => {: }"]);
    assert_eq!(errors("f({})"), ["E0025 2..4 => f({:})"]);
    assert_eq!(
        messages("{}"),
        ["`{}` isn't a value: the empty map is `{:}`"]
    );
    // `{}` is whole, so the operators around it are still checked.
    assert_eq!(
        errors("{} & 1 | 2"),
        ["E0025 0..2 => {:} & 1 | 2", "E0019 0..6 => ({} & 1) | 2"]
    );
}

/// `grammar.arg`: a named argument, `name:` alone, and `..base` first
/// (`r[item.struct.update]`).
#[test]
fn arguments() {
    assert_eq!(
        tree("User(..u, id: 1, email:)"),
        r#"Call
  NameRef
    Ident "User"
  ArgList
    LParen "("
    UpdateArg
      DotDot ".."
      NameRef
        Ident "u"
    Comma ","
    NamedArg
      NameRef
        Ident "id"
      Colon ":"
      Literal
        Int "1"
    Comma ","
    NamedArg
      NameRef
        Ident "email"
      Colon ":"
    RParen ")"
"#
    );
    assert_eq!(errors("f(email:\n  )"), Vec::<String>::new());
    assert_eq!(errors("User(id: 1, ..u)"), ["E0016 12..14"]);
    // After the base come one or more named fields (`grammar.struct_lit`).
    assert_eq!(errors("User(..u)"), ["E0016 8..9"]);
    assert_eq!(
        messages("User(..u)"),
        ["expected `,` and `name:`, found `)`"]
    );
    assert_eq!(errors("User(..u,)"), ["E0016 9..10"]);
    assert_eq!(
        messages("User(..u, 1)"),
        ["expected `name:`, found the number `1`"]
    );
    assert_eq!(errors("User(..u, id: 1, 2)"), ["E0016 17..18"]);
    assert_eq!(
        messages("User(id: 1, ..u)"),
        ["expected an expression or `name:`, found `..`"]
    );
    // A line break in the list is spacing, so a value on the line after `name:` is its value.
    assert_eq!(tree("f(email:\n  e)"), tree("f(email: e)"));
    assert_eq!(tree("f(email\n  : e)"), tree("f(email: e)"));
}

/// A block where an expression is expected, which Quince doesn't have
/// (`r[expr.block.no-bare]`), is one E0016 at its `{`, and the parser skips to its `}`.
#[test]
fn a_bare_block_is_one_error() {
    assert_eq!(errors("{\n    let y = 1\n  }"), ["E0016 0..1"]);
    assert_eq!(errors("g({\n    let y = 1\n  }, 2)"), ["E0016 2..3"]);
    assert_eq!(
        messages("{\n    let y = 1\n  }"),
        ["expected an expression, found a block"]
    );
    // Whatever statement comes first, as long as no entry's `:` comes before the statement.
    assert_eq!(errors("{\n    g()\n    let y = 1\n  }"), ["E0016 0..1"]);
    assert_eq!(
        errors("{\n    log(1)\n    var y = {\n      while ok {}\n    }\n  } + 1"),
        ["E0016 0..1"]
    );
    // A bracket left open in the block is its own slip, and the block still skips to its `}`.
    assert_eq!(
        errors("{\n    g(1\n    let y = 1\n  }"),
        ["E0016 0..1", "E0017 7..8"]
    );
    // A map with an entry is a map missing its `}`.
    assert_eq!(errors("{\"a\": 1\n  let y = 1"), ["E0017 0..1"]);
    let text = "fn f() {\n  {\n    var x = 1\n  }\n  g()\n}\n";
    let errors: Vec<_> = checked(text)
        .errors()
        .iter()
        .map(|e| (e.range.start, e.range.end))
        .collect();
    assert_eq!(errors, [(11, 12)]);
}

/// An attribute's arguments may be named too.
#[test]
fn named_attribute_arguments() {
    let parse = checked("@deprecated(\"why\", use: g)\nfn f() {}\n");
    assert_eq!(parse.errors(), []);
    assert!(
        parse
            .syntax()
            .descendants()
            .any(|node| node.kind() == SyntaxKind::NamedArg)
    );
    let errors = checked("@deprecated(..g)\nfn f() {}\n").errors().to_vec();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, "E0016");
}
