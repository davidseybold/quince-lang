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
    // A chain that is itself an operand is reported with that operand's own error.
    assert_eq!(
        errors("a < b < c && d"),
        ["E0020 6..7 => a < b && b < c && d"]
    );
}

/// A missing operand is the one error, so nothing about the operators around it is reported.
#[test]
fn a_missing_operand_is_the_error() {
    assert_eq!(errors("a & == b"), ["E0016 4..6"]);
    // `<` continues the line, so what is missing is found at the block's `}`.
    assert_eq!(errors("a < b <"), ["E0016 8..9"]);
    assert_eq!(errors("a ** b"), ["E0016 3..4"]);
    assert_eq!(errors("(a b) & c"), ["E0016 3..4"]);
}
