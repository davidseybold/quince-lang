//! The parser's trees for types, generics and `uses` lists, and their errors. Each expected
//! dump is written by hand.

use super::cst::dump_tree_without_trivia;
use super::errors::describe;
use super::parser::{checked, stmts, valid};
use crate::SyntaxKind::{self, *};

/// The tree of `type`, a parameter's type, after checking that it parses with no error.
fn ty(type_text: &str) -> String {
    node(&format!("fn f(a: {type_text}) {{}}"), TypePath, |kind| {
        matches!(kind, TypePath | TupleType | ParenType | FnType)
    })
}

/// The first node of `kind`, or the first that `matches` after the declaration's name, dumped.
fn node(text: &str, kind: SyntaxKind, matches: fn(SyntaxKind) -> bool) -> String {
    let parse = checked(text);
    assert_eq!(parse.errors(), [], "{text:?}");
    let found = parse
        .syntax()
        .descendants()
        .find(|node| node.kind() == kind || matches(node.kind()))
        .unwrap_or_else(|| panic!("no {kind:?} in {text:?}"));
    dump_tree_without_trivia(&found)
}

fn generics_of(text: &str) -> String {
    node(text, Generics, |_| false)
}

/// Each error as `code start..end`, then its fixed text when it has one fix.
fn errors(text: &str) -> Vec<String> {
    let parse = checked(text);
    parse.errors().iter().map(|e| describe(text, e)).collect()
}

fn messages(text: &str) -> Vec<String> {
    let parse = checked(text);
    parse.errors().iter().map(|e| e.message.clone()).collect()
}

#[test]
fn unit_and_tuples() {
    assert_eq!(ty("()"), "TupleType\n  LParen \"(\"\n  RParen \")\"\n");
    assert_eq!(
        ty("(Int, String,)"),
        r#"TupleType
  LParen "("
  TypePath
    Ident "Int"
  Comma ","
  TypePath
    Ident "String"
  Comma ","
  RParen ")"
"#
    );
}

#[test]
fn a_function_type() {
    assert_eq!(
        ty("fn(Int) -> Int uses Io, db.Db"),
        r#"FnType
  FnKw "fn"
  LParen "("
  TypePath
    Ident "Int"
  RParen ")"
  RetType
    Arrow "->"
    TypePath
      Ident "Int"
  Effects
    UsesKw "uses"
    TypePath
      Ident "Io"
    Comma ","
    TypePath
      Ident "db"
      Dot "."
      Ident "Db"
"#
    );
    assert_eq!(
        ty("fn() uses *"),
        r#"FnType
  FnKw "fn"
  LParen "("
  RParen ")"
  Effects
    UsesKw "uses"
    Star "*"
"#
    );
}

/// `r[effect.uses-return]`: a `uses` after a bare returned function type is the outer
/// function's.
#[test]
fn a_returned_function_type_leaves_its_uses() {
    assert_eq!(
        valid("fn make() -> fn(Int) -> Int uses Net {}"),
        r#"File
  FnDecl
    FnKw "fn"
    Name
      Ident "make"
    ParamList
      LParen "("
      RParen ")"
    RetType
      Arrow "->"
      FnType
        FnKw "fn"
        LParen "("
        TypePath
          Ident "Int"
        RParen ")"
        RetType
          Arrow "->"
          TypePath
            Ident "Int"
    Effects
      UsesKw "uses"
      TypePath
        Ident "Net"
    Block
      LBrace "{"
      RBrace "}"
"#
    );
    // The same for a function type whose return type is a function type.
    assert_eq!(
        ty("fn(Int) -> fn() -> Int uses Io"),
        r#"FnType
  FnKw "fn"
  LParen "("
  TypePath
    Ident "Int"
  RParen ")"
  RetType
    Arrow "->"
    FnType
      FnKw "fn"
      LParen "("
      RParen ")"
      RetType
        Arrow "->"
        TypePath
          Ident "Int"
  Effects
    UsesKw "uses"
    TypePath
      Ident "Io"
"#
    );
}

/// `r[type.fn-parens]`: parentheses end a function type, so it keeps its own `uses`.
#[test]
fn parentheses_end_a_function_type() {
    assert_eq!(
        ty("(fn() uses Io)"),
        r#"ParenType
  LParen "("
  FnType
    FnKw "fn"
    LParen "("
    RParen ")"
    Effects
      UsesKw "uses"
      TypePath
        Ident "Io"
  RParen ")"
"#
    );
    let text = "fn make() -> (fn() uses Io) uses Net {}";
    let parse = checked(text);
    assert_eq!(parse.errors(), []);
    let effects = parse.syntax().descendants().filter(|n| n.kind() == Effects);
    assert_eq!(effects.count(), 2);
}

/// `r[effect.uses-list]`: in a list of types, the effects take every type path after them,
/// but a parameter or a named bound argument ends them.
#[test]
fn where_a_uses_list_ends() {
    assert_eq!(
        ty("Map<String, fn() uses Io, Db>"),
        r#"TypePath
  Ident "Map"
  TypeArgs
    Lt "<"
    TypePath
      Ident "String"
    Comma ","
    FnType
      FnKw "fn"
      LParen "("
      RParen ")"
      Effects
        UsesKw "uses"
        TypePath
          Ident "Io"
        Comma ","
        TypePath
          Ident "Db"
    Gt ">"
"#
    );
    let text = "fn serve(h: fn() uses E, host: String, var x: Int, _: Int) {}";
    let parse = checked(text);
    assert_eq!(parse.errors(), []);
    let params = parse.syntax().descendants().filter(|n| n.kind() == Param);
    assert_eq!(params.count(), 4);
    assert_eq!(
        generics_of("fn f<I: Iterator<Item = fn() uses Io, Output = Int>>() {}"),
        r#"Generics
  Lt "<"
  GenericParam
    Name
      Ident "I"
    Colon ":"
    TypePath
      Ident "Iterator"
      TypeArgs
        Lt "<"
        NamedTypeArg
          NameRef
            Ident "Item"
          Eq "="
          FnType
            FnKw "fn"
            LParen "("
            RParen ")"
            Effects
              UsesKw "uses"
              TypePath
                Ident "Io"
        Comma ","
        NamedTypeArg
          NameRef
            Ident "Output"
          Eq "="
          TypePath
            Ident "Int"
        Gt ">"
  Gt ">"
"#
    );
}

#[test]
fn generics_and_bounds() {
    assert_eq!(
        generics_of("fn f<T: Eq + Hash, uses E>() {}"),
        r#"Generics
  Lt "<"
  GenericParam
    Name
      Ident "T"
    Colon ":"
    TypePath
      Ident "Eq"
    Plus "+"
    TypePath
      Ident "Hash"
  Comma ","
  GenericParam
    UsesKw "uses"
    Name
      Ident "E"
  Gt ">"
"#
    );
    // A `uses` list in type arguments comes last.
    assert_eq!(
        ty("Route<uses Db, Net>"),
        r#"TypePath
  Ident "Route"
  TypeArgs
    Lt "<"
    Effects
      UsesKw "uses"
      TypePath
        Ident "Db"
      Comma ","
      TypePath
        Ident "Net"
    Gt ">"
"#
    );
}

/// `r[lex.no-shift-token]`: `>>` and `>=` after type arguments are two tokens, and the first
/// `>` closes them.
#[test]
fn a_closing_angle_is_never_joined() {
    assert_eq!(
        ty("List<List<Int>>"),
        r#"TypePath
  Ident "List"
  TypeArgs
    Lt "<"
    TypePath
      Ident "List"
      TypeArgs
        Lt "<"
        TypePath
          Ident "Int"
        Gt ">"
    Gt ">"
"#
    );
    assert_eq!(
        stmts("  let x: Option<Int>= y"),
        r#"LetStmt
  LetKw "let"
  Name
    Ident "x"
  Colon ":"
  TypePath
    Ident "Option"
    TypeArgs
      Lt "<"
      TypePath
        Ident "Int"
      Gt ">"
  Eq "="
  NameRef
    Ident "y"
"#
    );
}

/// A line that starts with `uses` continues a `fn` header, and line breaks inside angle
/// brackets are spacing.
#[test]
fn line_breaks_in_types() {
    let text = "fn f<\n  T\n>(m: Map<\n  String,\n  Int\n>) -> Int\n  uses Io {}\n";
    let tree = valid(text);
    assert!(tree.contains("Effects"), "{tree}");
}

#[test]
fn errors_in_types() {
    // One type in parentheses, which a fix removes. A function type may have them.
    assert_eq!(
        errors("fn f(a: (Int)) {}"),
        [r#"E0016 12..13 => "fn f(a: Int) {}""#]
    );
    assert_eq!(
        messages("fn f(a: (List<Int>)) {}"),
        ["expected `,` and a second type, found `)`"]
    );
    assert_eq!(
        messages("fn f(a: (Int,)) {}"),
        ["expected a second type, found `)`"]
    );
    // Before `->`, removing them wouldn't fix it.
    assert_eq!(errors("fn f(a: fn((Int) -> Int)) {}"), ["E0016 15..16"]);
    // A slip inside one pair of parentheses leaves a `ParenType`.
    let parse = checked("fn f(a: (1)) {}");
    assert_eq!(parse.errors().len(), 1);
    assert!(parse.syntax().descendants().any(|n| n.kind() == ParenType));
    // A trailing comma may end any of the lists, but a comma alone isn't one.
    for ok in [
        "fn f(a: fn(Int,)) {}",
        "fn f(a: Map<A, B,>) {}",
        "fn f<T,>() {}",
        "fn f<I: Iterator<Item = Int,>>() {}",
    ] {
        assert_eq!(errors(ok), Vec::<String>::new(), "{ok}");
    }
    assert_eq!(
        messages("fn f(a: fn(,)) {}"),
        ["expected a type, found `,`"]
    );
    assert_eq!(
        messages("fn f(a: List<>) {}"),
        ["expected a type, found `>`"]
    );
    assert_eq!(
        messages("fn f<>() {}"),
        ["expected a type parameter, found `>`"]
    );
    assert_eq!(
        messages("fn f(a: Pool<uses Net, (Int, Int)>) {}"),
        ["expected `>`, found `,`"]
    );
    assert_eq!(
        messages("fn f() uses {}"),
        ["expected an effect, found `{`"]
    );
    assert_eq!(
        messages("fn f() -> Int Int {}"),
        ["expected `uses` or `{`, found the name `Int`"]
    );
    assert_eq!(
        messages("fn f() Int {}"),
        ["expected `->`, `uses` or `{`, found the name `Int`"]
    );
    assert_eq!(
        messages("fn f(a: fn -> Int) {}"),
        ["expected `(`, found `->`"]
    );
}

/// An error inside angle brackets stays inside them: the list around them goes on.
#[test]
fn errors_in_angle_brackets_stay_inside() {
    assert_eq!(messages("fn f(a: Map<A, B C, D>, b: Int) {}").len(), 1);
    assert_eq!(
        messages("fn f(a: Pool<uses Net, (Int, Int)>, b: Int) {}").len(),
        1
    );
    assert_eq!(messages("fn f<T: Eq +, U>(a: T) {}").len(), 1);
    // A skip after an error before them doesn't stop at a comma inside them.
    assert_eq!(messages("fn f(a Map<String, Int>) {}").len(), 1);
    assert_eq!(messages("fn f(a: Int b: Map<K, V>) {}").len(), 1);
    assert_eq!(
        messages("fn f<I Iterator<Item = fn() uses Io, Output = Int>>() {}").len(),
        1
    );
    // A closing bracket further out closes them, with an E0017 at the `<`.
    assert_eq!(errors("fn f(a: List<Int) {}"), ["E0017 12..13"]);
}
