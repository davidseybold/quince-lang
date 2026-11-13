//! The parser's trees for `impl` and `interface` declarations and their members, and their
//! errors. Each expected dump is written by hand.

use super::parser::{checked, valid};

/// Each error as `code start..end: message`.
fn errors(text: &str) -> Vec<String> {
    let parse = checked(text);
    parse
        .errors()
        .iter()
        .map(|e| {
            format!(
                "{} {}..{}: {}",
                e.code, e.range.start, e.range.end, e.message
            )
        })
        .collect()
}

fn codes(text: &str) -> Vec<&'static str> {
    checked(text).errors().iter().map(|e| e.code).collect()
}

/// An `impl` with generics, an interface and `for`, an associated type and a constant
/// (`grammar.impl_decl`, `grammar.impl_item`).
#[test]
fn an_impl_of_an_interface() {
    assert_eq!(
        valid("impl<T> Sum<T> for List<T> {\n  type Output = T\n  pub const N: Int = 1\n}\n"),
        r#"File
  ImplDecl
    ImplKw "impl"
    Generics
      Lt "<"
      GenericParam
        Name
          Ident "T"
      Gt ">"
    TypePath
      Ident "Sum"
      TypeArgs
        Lt "<"
        TypePath
          Ident "T"
        Gt ">"
    ForKw "for"
    TypePath
      Ident "List"
      TypeArgs
        Lt "<"
        TypePath
          Ident "T"
        Gt ">"
    LBrace "{"
    AssocType
      TypeKw "type"
      Name
        Ident "Output"
      Eq "="
      TypePath
        Ident "T"
    ConstDecl
      PubKw "pub"
      ConstKw "const"
      Name
        Ident "N"
      Colon ":"
      TypePath
        Ident "Int"
      Eq "="
      Literal
        Int "1"
    RBrace "}"
"#
    );
}

/// An `interface` with generics, an associated type, and a method signature with a contract
/// and no body (`grammar.interface_decl`).
#[test]
fn an_interface() {
    assert_eq!(
        valid("interface I<T> {\n  type Item\n  fn f(self) -> T requires ok\n}\n"),
        r#"File
  InterfaceDecl
    InterfaceKw "interface"
    Name
      Ident "I"
    Generics
      Lt "<"
      GenericParam
        Name
          Ident "T"
      Gt ">"
    LBrace "{"
    AssocType
      TypeKw "type"
      Name
        Ident "Item"
    FnDecl
      FnKw "fn"
      Name
        Ident "f"
      ParamList
        LParen "("
        Param
          SelfKw "self"
        RParen ")"
      RetType
        Arrow "->"
        TypePath
          Ident "T"
      Contract
        RequiresKw "requires"
        NameRef
          Ident "ok"
    RBrace "}"
"#
    );
    // Members on the line of the braces.
    valid("interface From<T> { fn from(value: T) -> Self }\n");
    valid("impl<T> Iterator for Counter<T> { type Item = T }\n");
}

/// What the grammar rejects in an `impl` header or body is an E0016, and the body is still
/// parsed.
#[test]
fn impl_errors() {
    assert_eq!(
        errors("impl S {\n  pub type A = Int\n}\n"),
        ["E0016 11..14: expected `type`, found the keyword `pub`"]
    );
    // Only a type path names an interface.
    assert_eq!(
        errors("impl (Int, Int) for P {\n  fn f() { x }\n}\n"),
        ["E0016 16..19: expected `{`, found the keyword `for`"]
    );
    assert_eq!(
        errors("impl S X {\n  fn f() {}\n}\n"),
        ["E0016 7..8: expected `for` or `{`, found the name `X`"]
    );
    assert_eq!(
        errors("impl S {\n  let x = 1\n}\n"),
        ["E0016 11..14: expected `fn`, `const`, `type` or `}`, found the keyword `let`"]
    );
    // A file's member start closes the body (decision 9).
    assert_eq!(codes("impl S {\n  test \"t\" {}\n}\n"), ["E0017", "E0016"]);
    assert_eq!(codes("impl S for T\n{\n}\n"), ["E0018"]);
    // `Self` is a type path.
    valid("impl Self for T {}\n");
    // A member start closes the method's block.
    assert_eq!(
        codes("impl S {\n  fn f() {\n  const N: Int = 1\n}\n"),
        ["E0017"]
    );
}

/// What the grammar rejects in an `interface` is an E0016: a constant
/// (`r[item.const.assoc.no-interface]`), a method body (`r[item.iface.no-default]`), a value
/// for an associated type, and `pub`.
#[test]
fn interface_errors() {
    assert_eq!(
        errors("interface I {\n  const M: Int = 1\n}\n"),
        ["E0016 16..21: expected `fn`, `type` or `}`, found the keyword `const`"]
    );
    assert_eq!(
        errors("interface I {\n  fn f(self) -> Int { 1 }\n}\n"),
        ["E0016 34..35: expected `uses`, `requires`, `ensures` or a line break, found `{`"]
    );
    // A body on the next line continues the header, and is skipped whole.
    assert_eq!(
        errors("interface I {\n  fn f(self)\n  {\n    x\n  }\n}\n"),
        ["E0016 29..30: expected `->`, `uses`, `requires`, `ensures` or a line break, found `{`"]
    );
    assert_eq!(
        errors("interface I {\n  type T = Int\n}\n"),
        ["E0016 23..24: expected a line break or `}`, found `=`"]
    );
    assert_eq!(
        errors("interface I {\n  pub fn f(self)\n}\n"),
        ["E0016 16..19: expected `fn` or `type`, found the keyword `pub`"]
    );
    // A slip on the line before doesn't hide the member's own.
    assert_eq!(
        codes("interface I {\n  type T = Int\n  pub fn f(self)\n}\n"),
        ["E0016", "E0016"]
    );
    // A line that is no member stays part of that slip.
    assert_eq!(
        codes("interface I {\n  type T = Int\n  pub 1\n}\n"),
        ["E0016"]
    );
    assert_eq!(codes("interface I\n{\n}\n"), ["E0018"]);
    // A body missing after a contract that slipped is part of that slip.
    assert_eq!(
        codes("interface I {\n  fn f(self) requires\n  ensures x\n  { 1 }\n}\n"),
        ["E0016"]
    );
    assert_eq!(
        codes("interface I {\n  fn f(self) requires\n  fn g(self)\n}\n"),
        ["E0016"]
    );
}

/// No type holds braces, so a `{` closes the generics or type arguments open around it, and
/// the header still finds its body: one error, however many members follow.
#[test]
fn a_brace_closes_generics() {
    assert_eq!(
        errors("interface I<T {\n  fn f(self)\n  fn g(self)\n}\n"),
        ["E0017 11..12: this `<` has no matching `>`"]
    );
    assert_eq!(
        codes("impl<T S {\n  fn f() {}\n  fn g() {}\n}\n"),
        ["E0016"]
    );
    assert_eq!(
        codes("impl<T: Ord<Int {\n  fn f() {}\n}\n"),
        ["E0017", "E0017"]
    );
    assert_eq!(codes("fn f() -> List<Int {\n  x\n}\n"), ["E0017"]);
}
