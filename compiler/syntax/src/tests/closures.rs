//! The parser's trees for closures, `return`, `break` and `continue`, and contracts. Each
//! expected dump is written by hand. Their slips are in `recovery`.

use super::parser::{checked, stmts, valid};
use crate::SyntaxKind;

/// A closure holds its parameters, its return type and effects, and its block
/// (`grammar.closure`). A parameter is a name or `_`, with a type if present
/// (`grammar.cparam`).
#[test]
fn a_closure() {
    assert_eq!(
        stmts("fn(a: Int, _) -> Int uses Io { a }"),
        r#"Closure
  FnKw "fn"
  ParamList
    LParen "("
    Param
      Name
        Ident "a"
      Colon ":"
      TypePath
        Ident "Int"
    Comma ","
    Param
      Underscore "_"
    RParen ")"
  RetType
    Arrow "->"
    TypePath
      Ident "Int"
  Effects
    UsesKw "uses"
    TypePath
      Ident "Io"
  Block
    LBrace "{"
    NameRef
      Ident "a"
    RBrace "}"
"#
    );
}

/// A `uses` after a bare function type in a closure's return type is the closure's
/// (`r[effect.uses-return]`), as in a declaration.
#[test]
fn a_closure_takes_the_uses_after_its_return_type() {
    let parse = checked("fn f() {\n  fn() -> fn(Int) -> Int uses Io { g }\n}\n");
    assert_eq!(parse.errors(), []);
    let closure = parse
        .syntax()
        .descendants()
        .find(|node| node.kind() == SyntaxKind::Closure)
        .expect("a closure");
    let kinds: Vec<_> = closure.children().map(|node| node.kind()).collect();
    use SyntaxKind::*;
    assert_eq!(kinds, [ParamList, RetType, Effects, Block]);
}

/// A closure's header continues on a line that starts with `->`, `uses` or `{`
/// (`r[lex.continue.fn-header]`), and a closure is a primary expression, so it can be called.
#[test]
fn a_closure_header_on_several_lines() {
    assert_eq!(
        stmts("x.map(fn(y)\n  -> Int\n  uses Io\n{ y })"),
        stmts("x.map(fn(y) -> Int uses Io { y })")
    );
    assert_eq!(
        stmts("fn(x) { x }(1)"),
        r#"Call
  Closure
    FnKw "fn"
    ParamList
      LParen "("
      Param
        Name
          Ident "x"
      RParen ")"
    Block
      LBrace "{"
      NameRef
        Ident "x"
      RBrace "}"
  ArgList
    LParen "("
    Literal
      Int "1"
    RParen ")"
"#
    );
}

/// A closure takes no contracts, so after its parameters only `->`, `uses` or `{` can come.
#[test]
fn a_closure_missing_its_body() {
    let parse = checked("fn f() {\n  let g = fn(x)\n}\n");
    let messages: Vec<_> = parse.errors().iter().map(|e| e.message.as_str()).collect();
    assert_eq!(
        messages,
        ["expected `->`, `uses` or `{`, found a line break"]
    );
}

/// At the top of a condition, `{` is the block (`r[expr.block.cond-brace]`), so `return` before
/// it is bare.
#[test]
fn a_bare_return_before_a_block() {
    for body in [
        "if return { a }",
        "match return { _ => 1, }",
        "for x in return { a }",
    ] {
        stmts(body);
    }
}

/// `return` takes everything to its right (`r[expr.op.keyword-forms]`), on its line: `return`
/// on a line of its own is bare, and the next line is a statement of its own. `break` and
/// `continue` take nothing (`grammar.jump`).
#[test]
fn jumps() {
    assert_eq!(
        stmts("return a || b"),
        r#"ReturnExpr
  ReturnKw "return"
  Binary
    NameRef
      Ident "a"
    PipePipe "||"
    NameRef
      Ident "b"
"#
    );
    assert_eq!(
        stmts("return\n-1"),
        r#"ReturnExpr
  ReturnKw "return"
Literal
  Minus "-"
  Int "1"
"#
    );
    assert_eq!(
        stmts("break\ncontinue"),
        r#"BreakExpr
  BreakKw "break"
ContinueExpr
  ContinueKw "continue"
"#
    );
}

/// `return`, `break` and `continue` are expressions, so they can be an arm's body
/// (`r[expr.jump.expr]`). A bare `return` ends at the arm's `,`.
#[test]
fn a_jump_in_an_arm() {
    assert_eq!(
        stmts("match x { None => return, _ => break, }"),
        r#"MatchExpr
  MatchKw "match"
  NameRef
    Ident "x"
  MatchArmList
    LBrace "{"
    MatchArm
      PathPat
        TypePath
          Ident "None"
      FatArrow "=>"
      ReturnExpr
        ReturnKw "return"
    Comma ","
    MatchArm
      WildcardPat
        Underscore "_"
      FatArrow "=>"
      BreakExpr
        BreakKw "break"
    Comma ","
    RBrace "}"
"#
    );
}

/// Contracts go between a function's signature and its body (`grammar.fn_decl`,
/// `grammar.contract`). `ensures` followed by a name in parentheses names the return value
/// (`r[contract.ensures-binder]`), so in `ensures (x) - 1 > 0` the condition is `-1 > 0`.
#[test]
fn contracts() {
    assert_eq!(
        valid("fn f(x: Int) -> Int requires ok ensures (x) - 1 > 0 {}"),
        r#"File
  FnDecl
    FnKw "fn"
    Name
      Ident "f"
    ParamList
      LParen "("
      Param
        Name
          Ident "x"
        Colon ":"
        TypePath
          Ident "Int"
      RParen ")"
    RetType
      Arrow "->"
      TypePath
        Ident "Int"
    Contract
      RequiresKw "requires"
      NameRef
        Ident "ok"
    Contract
      EnsuresKw "ensures"
      LParen "("
      Name
        Ident "x"
      RParen ")"
      Binary
        Literal
          Minus "-"
          Int "1"
        Gt ">"
        Literal
          Int "0"
    Block
      LBrace "{"
      RBrace "}"
"#
    );
    // Only a name alone in the parentheses is the binder.
    assert_eq!(
        valid("fn f() ensures (x, y) == z {}"),
        r#"File
  FnDecl
    FnKw "fn"
    Name
      Ident "f"
    ParamList
      LParen "("
      RParen ")"
    Contract
      EnsuresKw "ensures"
      Binary
        Tuple
          LParen "("
          NameRef
            Ident "x"
          Comma ","
          NameRef
            Ident "y"
          RParen ")"
        EqEq "=="
        NameRef
          Ident "z"
    Block
      LBrace "{"
      RBrace "}"
"#
    );
    // A line that starts with `requires`, `ensures` or `{` continues the header.
    assert_eq!(
        valid("fn f() -> Int\n  requires a\n  ensures(r) r > 0\n{\n}\n"),
        valid("fn f() -> Int requires a ensures(r) r > 0 {}")
    );
    assert_eq!(
        valid("impl A {\n  fn f(var self) uses Io requires ok {}\n}\n"),
        r#"File
  ImplDecl
    ImplKw "impl"
    TypePath
      Ident "A"
    LBrace "{"
    FnDecl
      FnKw "fn"
      Name
        Ident "f"
      ParamList
        LParen "("
        Param
          VarKw "var"
          SelfKw "self"
        RParen ")"
      Effects
        UsesKw "uses"
        TypePath
          Ident "Io"
      Contract
        RequiresKw "requires"
        NameRef
          Ident "ok"
      Block
        LBrace "{"
        RBrace "}"
    RBrace "}"
"#
    );
}

/// A block in a contract is a line context of its own: the header's rules don't reach its
/// lines, so a statement in it may start with `{`.
#[test]
fn a_block_in_a_contract() {
    let text = "fn f() -> Int\n  ensures(r) if r > 0 {\n    let m = 1\n    {\"a\": m}.len() > 0\n  } else { true }\n{\n  1\n}\n";
    valid(text);
}
