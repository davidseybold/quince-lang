//! The parser's trees for statements and `if`, and their errors. Each expected dump is written
//! by hand.

use super::errors::describe;
use super::parser::{checked, stmts};

const PREFIX: &str = "fn f() {\n";

/// The errors of a function body, each as `code start..end` in `body`, then the fixed text when
/// it has one fix.
fn errors(body: &str) -> Vec<String> {
    let text = format!("{PREFIX}{body}\n}}\n");
    let parse = checked(&text);
    parse
        .errors()
        .iter()
        .map(|e| {
            let mut e = e.clone();
            e.range = e.range.start - PREFIX.len() as u32..e.range.end - PREFIX.len() as u32;
            e.fixes.clear();
            describe(body, &e)
        })
        .collect()
}

fn codes(parse: &crate::Parse) -> Vec<&'static str> {
    parse.errors().iter().map(|e| e.code).collect()
}

fn messages(body: &str) -> Vec<String> {
    let text = format!("{PREFIX}{body}\n}}\n");
    let parse = checked(&text);
    parse.errors().iter().map(|e| e.message.clone()).collect()
}

/// `var` takes a name, an optional type, `=` and a value, as `let` does (`grammar.stmt`).
#[test]
fn var() {
    assert_eq!(
        stmts("var n: Int = 1"),
        r#"VarStmt
  VarKw "var"
  Name
    Ident "n"
  Colon ":"
  TypePath
    Ident "Int"
  Eq "="
  Literal
    Int "1"
"#
    );
    assert_eq!(stmts("var n =\n  1"), stmts("var n = 1"));
}

/// An assignment wraps its place and its value (`r[stmt.assign.stmt]`). A place may hold
/// fields, tuple positions and indexes (`grammar.place`).
#[test]
fn assignments() {
    assert_eq!(
        stmts("total += 1"),
        r#"AssignStmt
  NameRef
    Ident "total"
  PlusEq "+="
  Literal
    Int "1"
"#
    );
    assert_eq!(
        stmts("pair.1.0 = 5"),
        r#"AssignStmt
  Field
    Field
      NameRef
        Ident "pair"
      Dot "."
      Int "1"
    Dot "."
    Int "0"
  Eq "="
  Literal
    Int "5"
"#
    );
    assert_eq!(
        stmts("xs[i] = v"),
        r#"AssignStmt
  Index
    NameRef
      Ident "xs"
    LBracket "["
    NameRef
      Ident "i"
    RBracket "]"
  Eq "="
  NameRef
    Ident "v"
"#
    );
    for op in ["=", "+=", "-=", "*=", "/=", "%="] {
        assert_eq!(
            stmts(&format!("self.items[f(i)].count {op}\n  2")),
            stmts(&format!("self.items[f(i)].count {op} 2"))
        );
    }
}

#[test]
fn while_loops() {
    assert_eq!(
        stmts("while n > 0 { n -= 1 }"),
        r#"WhileStmt
  WhileKw "while"
  Binary
    NameRef
      Ident "n"
    Gt ">"
    Literal
      Int "0"
  Block
    LBrace "{"
    AssignStmt
      NameRef
        Ident "n"
      MinusEq "-="
      Literal
        Int "1"
    RBrace "}"
"#
    );
    assert_eq!(
        stmts("while n > 0 {\n  n -= 1\n}"),
        stmts("while n > 0 { n -= 1 }")
    );
}

/// `else` takes another `if` or a block, so an `else if` chain nests (`grammar.if_expr`).
#[test]
fn if_and_else() {
    assert_eq!(
        stmts("if a { 1 } else if b { 2 } else { 3 }"),
        r#"IfExpr
  IfKw "if"
  NameRef
    Ident "a"
  Block
    LBrace "{"
    Literal
      Int "1"
    RBrace "}"
  ElseKw "else"
  IfExpr
    IfKw "if"
    NameRef
      Ident "b"
    Block
      LBrace "{"
      Literal
        Int "2"
      RBrace "}"
    ElseKw "else"
    Block
      LBrace "{"
      Literal
        Int "3"
      RBrace "}"
"#
    );
    assert_eq!(
        stmts("if a {\n  1\n} else if b {\n  2\n} else {\n  3\n}"),
        stmts("if a { 1 } else if b { 2 } else { 3 }")
    );
    assert_eq!(
        stmts("let x = if a { 1 } else { 2 }"),
        r#"LetStmt
  LetKw "let"
  Name
    Ident "x"
  Eq "="
  IfExpr
    IfKw "if"
    NameRef
      Ident "a"
    Block
      LBrace "{"
      Literal
        Int "1"
      RBrace "}"
    ElseKw "else"
    Block
      LBrace "{"
      Literal
        Int "2"
      RBrace "}"
"#
    );
}

/// A condition's `<` is less-than, since `{` can't follow type arguments, and the block's `{`
/// isn't a map. A line after an `if`'s `}` starts a statement.
#[test]
fn conditions_and_the_line_after() {
    assert_eq!(
        stmts("if a < b {}"),
        r#"IfExpr
  IfKw "if"
  Binary
    NameRef
      Ident "a"
    Lt "<"
    NameRef
      Ident "b"
  Block
    LBrace "{"
    RBrace "}"
"#
    );
    assert_eq!(
        stmts("if a {}\n(x)"),
        r#"IfExpr
  IfKw "if"
  NameRef
    Ident "a"
  Block
    LBrace "{"
    RBrace "}"
Paren
  LParen "("
  NameRef
    Ident "x"
  RParen ")"
"#
    );
}

/// The left side of an assignment that isn't a place is an E0026 over it, with no fix
/// (`r[stmt.assign.place]`). One that holds a syntax error is that error alone.
#[test]
fn not_a_place() {
    assert_eq!(errors("  xs.len() = 0"), ["E0026 2..10"]);
    assert_eq!(errors("  (x) += 1"), ["E0026 2..5"]);
    assert_eq!(errors("  x.f<T>.y = 1"), ["E0026 2..10"]);
    assert_eq!(errors("  x? = 1"), ["E0026 2..4"]);
    assert_eq!(errors("  Self.x = 1"), ["E0026 2..8"]);
    assert_eq!(errors("  -x = 1"), ["E0026 2..4"]);
    assert_eq!(errors("  if a { x } else { y } = 1"), ["E0026 2..23"]);
    assert_eq!(errors("  g(a b) = 1"), ["E0016 6..7"]);
    assert_eq!(
        messages("  xs.len() = 0"),
        [
            "the left side of an assignment must be a place: a name or `self`, followed by \
             fields, tuple positions and indexes"
        ]
    );
}

/// Assignment is a statement: a second `=` ends nothing, and `=` can't go in an expression
/// (`r[stmt.assign.no-chain]`). Each is one E0016.
#[test]
fn assignment_is_a_statement() {
    assert_eq!(errors("  a = b = 1"), ["E0016 8..9"]);
    assert_eq!(
        messages("  a = b = 1"),
        ["expected a line break or `}`, found `=`"]
    );
    assert_eq!(errors("  if a = 1 { b = 2 }"), ["E0016 7..8"]);
    assert_eq!(
        messages("  if a = 1 { b = 2 }"),
        ["expected `{`, found `=`"]
    );
    assert_eq!(errors("  g(a = 1)"), ["E0016 6..7"]);
    assert_eq!(errors("  let c = (a = 2)"), ["E0016 13..14"]);
}

/// A slip in a condition is one error: the parser skips to the block on its line, or takes a
/// condition missing from its line from the next one.
#[test]
fn a_missing_condition_block_or_else_branch() {
    assert_eq!(errors("  if < 0 { x }"), ["E0016 5..6"]);
    assert_eq!(
        errors("  let s = if < 0 { 1 } else { 2 }"),
        ["E0016 13..14"]
    );
    assert_eq!(errors("  if a = 1 { b = 2 }"), ["E0016 7..8"]);
    assert_eq!(errors("  if\n  a {}"), ["E0016 4..5"]);
    assert_eq!(errors("  while\n  n > 0 { n -= 1 }"), ["E0016 7..8"]);
    assert_eq!(errors("  if\n  {}"), ["E0016 4..5"]);
    // A frame that closes at the next line keeps its E0017.
    for keyword in ["if", "while"] {
        let text = format!("fn f() {{\n  {keyword}\n");
        assert_eq!(codes(&checked(&text)), ["E0017"], "{text:?}");
        let text = format!("fn f() {{\n  {keyword}\nfn g() {{}}\n");
        assert_eq!(codes(&checked(&text)), ["E0017"], "{text:?}");
    }
    assert_eq!(errors("  if\n  g()"), ["E0016 4..5"]);
    assert_eq!(
        messages("  if\n  g()"),
        ["expected an expression, found a line break"]
    );
    assert_eq!(errors("  while a b {}"), ["E0016 10..11"]);
    assert_eq!(errors("  if a {} else b"), ["E0016 15..16"]);
    assert_eq!(
        messages("  if a {} else b"),
        ["expected `if` or `{`, found the name `b`"]
    );
    assert_eq!(errors("  var (a, b) = c"), ["E0016 6..7"]);
    assert_eq!(messages("  var (a, b) = c"), ["expected a name, found `(`"]);
}

/// At the top of a condition, `{` is the block, never a map: one E0016 where an expression is
/// missing (`r[expr.block.cond-brace]`). In parentheses it is a map.
#[test]
fn a_brace_in_a_condition_is_the_block() {
    // Braces another `{` follows on their line were meant as a map: the fix puts them in
    // parentheses, and the block after them is still the block.
    let text = "fn f() {\n  if m == {:} { g() }\n}\n";
    let found: Vec<_> = checked(text)
        .errors()
        .iter()
        .map(|e| describe(text, e))
        .collect();
    assert_eq!(
        found,
        [r#"E0016 19..20 => "fn f() {\n  if m == ({:}) { g() }\n}\n""#]
    );
    assert_eq!(errors("  while n > { n -= 1 }"), ["E0016 12..13"]);
    assert_eq!(
        messages("  while n > { n -= 1 }"),
        ["expected an expression, found `{`"]
    );
    assert_eq!(errors("  if { x }"), ["E0016 5..6"]);
    assert_eq!(errors("  if m == {:} {}"), ["E0016 10..11"]);
    assert_eq!(errors("  if m == ({:}) {}"), [] as [String; 0]);
    assert_eq!(errors("  if f({\"a\": 1}) {}"), [] as [String; 0]);
    assert_eq!(
        errors("  if if a { {\"x\": 1} } else { m } == m {}"),
        [] as [String; 0]
    );
}

/// Blocks nest, and a missing `}` is one E0017 at the `{` the next declaration closes.
#[test]
fn nested_blocks_close_at_a_declaration() {
    let text = "fn f() {\n  if a {\n    while b {\n      g()\n    }\n}\nfn h() {}\n";
    let parse = checked(text);
    let found: Vec<_> = parse.errors().iter().map(|e| describe(text, e)).collect();
    assert_eq!(found, ["E0017 7..8"]);
}
