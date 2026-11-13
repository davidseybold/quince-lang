//! The parser's trees for `concurrent`, `spawn`, `background`, `select` and `with`. Each
//! expected dump is written by hand. Their slips are in `recovery`.

use super::parser::{checked, stmts};
use crate::SyntaxKind::{self, *};

/// The kinds of the child nodes of the first `kind` node in a function whose body is `body`,
/// after checking that it parses with no error.
fn children(body: &str, kind: SyntaxKind) -> Vec<SyntaxKind> {
    let parse = checked(&format!("fn f() {{\n{body}\n}}\n"));
    assert_eq!(parse.errors(), [], "{body:?}");
    let node = parse
        .syntax()
        .descendants()
        .find(|node| node.kind() == kind)
        .expect("the node");
    node.children().map(|child| child.kind()).collect()
}

/// `concurrent`, its arguments, and its spawns in braces, with a trailing `,`
/// (`grammar.concurrent`, `grammar.spawn`).
#[test]
fn a_concurrent_block() {
    assert_eq!(
        stmts("concurrent(settle: true,) { spawn a, spawn b, }"),
        r#"ConcurrentExpr
  ConcurrentKw "concurrent"
  ArgList
    LParen "("
    NamedArg
      NameRef
        Ident "settle"
      Colon ":"
      Literal
        TrueKw "true"
    Comma ","
    RParen ")"
  SpawnList
    LBrace "{"
    SpawnExpr
      SpawnKw "spawn"
      NameRef
        Ident "a"
    Comma ","
    SpawnExpr
      SpawnKw "spawn"
      NameRef
        Ident "b"
    Comma ","
    RBrace "}"
"#
    );
    // A fan-out's `for` is a loop inside the node.
    assert_eq!(
        children(
            "concurrent(limit: 2) for x in xs { spawn g(x) }",
            ConcurrentExpr
        ),
        [ArgList, ForStmt]
    );
    // The spawns are a list: line breaks before a `,` or the `}` are spacing.
    assert_eq!(
        stmts("concurrent {\n  spawn a\n  , spawn b\n}"),
        stmts("concurrent { spawn a, spawn b }")
    );
}

/// `spawn in` takes a scope, a postfix expression, then the expression to spawn
/// (`grammar.jump`, `r[conc.background.spawn-in]`). The scope takes a `(` after it, so in
/// `spawn in bg(x).run() y` the scope is `bg(x).run()`.
#[test]
fn spawn_in_a_scope() {
    assert_eq!(
        stmts("spawn in self.bg write(line)"),
        r#"SpawnExpr
  SpawnKw "spawn"
  InKw "in"
  Field
    NameRef
      SelfKw "self"
    Dot "."
    NameRef
      Ident "bg"
  Call
    NameRef
      Ident "write"
    ArgList
      LParen "("
      NameRef
        Ident "line"
      RParen ")"
"#
    );
    assert_eq!(
        children("spawn in bg(x).run() y", SpawnExpr),
        [Call, NameRef]
    );
}

/// `background`, the scope's name and `TaskScope`, an optional argument, and a block
/// (`grammar.background`).
#[test]
fn a_background_scope() {
    assert_eq!(
        stmts("background(bg: TaskScope, on_error: p) { a }"),
        r#"BackgroundExpr
  BackgroundKw "background"
  LParen "("
  Param
    Name
      Ident "bg"
    Colon ":"
    TypePath
      Ident "TaskScope"
  Comma ","
  NamedArg
    NameRef
      Ident "on_error"
    Colon ":"
    NameRef
      Ident "p"
  RParen ")"
  Block
    LBrace "{"
    NameRef
      Ident "a"
    RBrace "}"
"#
    );
}

/// `select` and its arms: a pattern, `=`, an operation, `=>` and a body (`grammar.select`).
#[test]
fn a_select() {
    assert_eq!(
        stmts("select { job = jobs.receive() => job, }"),
        r#"SelectExpr
  SelectKw "select"
  SelectArmList
    LBrace "{"
    SelectArm
      BindPat
        Name
          Ident "job"
      Eq "="
      Call
        Field
          NameRef
            Ident "jobs"
          Dot "."
          NameRef
            Ident "receive"
        ArgList
          LParen "("
          RParen ")"
      FatArrow "=>"
      NameRef
        Ident "job"
    Comma ","
    RBrace "}"
"#
    );
}

/// Comment lines directly above a select arm go inside it, as they do above a match arm.
#[test]
fn comments_above_a_select_arm() {
    let text = "fn f() {\n  select {\n    // first\n    x = c.r() => 1,\n  }\n}\n";
    let arm = checked(text)
        .syntax()
        .descendants()
        .find(|node| node.kind() == SelectArm)
        .expect("an arm");
    assert_eq!(arm.text().to_string(), "// first\n    x = c.r() => 1");
}

/// A `select` in a match arm has arms of its own: their `=>` belongs to the `select`, and
/// closes nothing of the `match`'s.
#[test]
fn a_select_in_a_match_arm() {
    assert_eq!(
        children(
            "match s { 0 => select { x = c.recv() => 1, }, 1 => 2, }",
            MatchArmList
        ),
        [MatchArm, MatchArm]
    );
    assert_eq!(
        children(
            "match s { 0 => select { x = c.recv() => 1, }, 1 => 2, }",
            SelectArmList
        ),
        [SelectArm]
    );
}

/// `with`, its handlers separated by commas, and a block (`grammar.with_expr`,
/// `grammar.handler`). A line break after a `,` is spacing.
#[test]
fn a_with() {
    assert_eq!(
        stmts("with app.Db = db, Log = l { run() }"),
        r#"WithExpr
  WithKw "with"
  Handler
    TypePath
      Ident "app"
      Dot "."
      Ident "Db"
    Eq "="
    NameRef
      Ident "db"
  Comma ","
  Handler
    TypePath
      Ident "Log"
    Eq "="
    NameRef
      Ident "l"
  Block
    LBrace "{"
    Call
      NameRef
        Ident "run"
      ArgList
        LParen "("
        RParen ")"
    RBrace "}"
"#
    );
    assert_eq!(
        stmts("with Db = db,\n  Log = l {\n  run()\n}"),
        stmts("with Db = db, Log = l { run() }")
    );
}
