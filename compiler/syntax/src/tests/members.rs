//! The parser's trees for `impl`, `interface`, `effect` and `foreign` declarations and their
//! members, and their errors. Each expected dump is written by hand.

use super::cst::dump_tree_without_trivia;
use super::parser::{checked, valid};
use quince_diagnostics::Applicability;

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

/// An `effect` with the effects it is built on and an operation, and one that names another
/// (`grammar.effect_decl`).
#[test]
fn an_effect() {
    assert_eq!(
        valid(
            "effect Db uses Net {\n  fn query(sql: String) -> Rows\n}\neffect Ticks = time.Clock\n"
        ),
        r#"File
  EffectDecl
    EffectKw "effect"
    Name
      Ident "Db"
    Effects
      UsesKw "uses"
      TypePath
        Ident "Net"
    LBrace "{"
    FnDecl
      FnKw "fn"
      Name
        Ident "query"
      ParamList
        LParen "("
        Param
          Name
            Ident "sql"
          Colon ":"
          TypePath
            Ident "String"
        RParen ")"
      RetType
        Arrow "->"
        TypePath
          Ident "Rows"
    RBrace "}"
  EffectDecl
    EffectKw "effect"
    Name
      Ident "Ticks"
    Eq "="
    TypePath
      Ident "time"
      Dot "."
      Ident "Clock"
"#
    );
}

/// A `foreign` block with its effects, a C type and a C function (`grammar.foreign_decl`,
/// `grammar.c_fn`).
#[test]
fn a_foreign_block() {
    assert_eq!(
        valid("foreign \"m\" uses Fs {\n  type FILE\n  fn fclose(f: Ptr<FILE>) -> CInt\n}\n"),
        r#"File
  ForeignDecl
    ForeignKw "foreign"
    Literal
      StringStart "\""
      StringText "m"
      StringEnd "\""
    Effects
      UsesKw "uses"
      TypePath
        Ident "Fs"
    LBrace "{"
    ForeignType
      TypeKw "type"
      Name
        Ident "FILE"
    FnDecl
      FnKw "fn"
      Name
        Ident "fclose"
      ParamList
        LParen "("
        Param
          Name
            Ident "f"
          Colon ":"
          TypePath
            Ident "Ptr"
            TypeArgs
              Lt "<"
              TypePath
                Ident "FILE"
              Gt ">"
        RParen ")"
      RetType
        Arrow "->"
        TypePath
          Ident "CInt"
    RBrace "}"
"#
    );
    valid("foreign \"c\" { fn abort() }\n");
}

/// What the grammar rejects in an `effect` is an E0016: a body, a contract, `pub`, and any
/// member but an operation.
#[test]
fn effect_errors() {
    assert_eq!(
        errors("effect E {\n  fn f() -> Int { 1 }\n}\n"),
        ["E0016 27..28: expected `uses` or a line break, found `{`"]
    );
    assert_eq!(
        errors("effect E {\n  fn f() requires ok\n}\n"),
        ["E0016 20..28: expected `->`, `uses` or a line break, found the keyword `requires`"]
    );
    assert_eq!(
        errors("effect E {\n  pub fn f()\n}\n"),
        ["E0016 13..16: expected `fn`, found the keyword `pub`"]
    );
    assert_eq!(
        errors("effect E {\n  type T\n}\n"),
        ["E0016 13..17: expected `fn` or `}`, found the keyword `type`"]
    );
    assert_eq!(
        errors("effect E X {}\n"),
        ["E0016 9..10: expected `uses`, `{` or `=`, found the name `X`"]
    );
    assert_eq!(
        errors("effect E = {}\n"),
        ["E0016 11..12: expected an effect, found `{`"]
    );
    assert_eq!(codes("effect E uses Net\n{\n}\n"), ["E0018"]);
    assert_eq!(codes("effect E\n= Other\n"), ["E0018"]);
}

/// What the grammar rejects in a `foreign` block is an E0016: a keyword as a C name
/// (`r[ffi.cbind.keyword-name]`), effects, a body, `pub`, and a constant.
#[test]
fn foreign_errors() {
    assert_eq!(
        errors("foreign \"c\" {\n  fn select()\n}\n"),
        ["E0016 19..25: expected a function name, found the keyword `select`"]
    );
    assert_eq!(
        errors("foreign \"c\" {\n  type match\n}\n"),
        ["E0016 21..26: expected a type name, found the keyword `match`"]
    );
    assert_eq!(
        errors("foreign \"c\" {\n  fn f(fn: CInt)\n}\n"),
        ["E0016 21..23: expected a parameter, found the keyword `fn`"]
    );
    assert_eq!(
        errors("foreign \"c\" {\n  fn f() -> CInt uses Fs\n}\n"),
        ["E0016 31..35: expected a line break, found the keyword `uses`"]
    );
    assert_eq!(
        errors("foreign \"c\" {\n  fn f() {}\n}\n"),
        ["E0016 23..24: expected `->` or a line break, found `{`"]
    );
    assert_eq!(
        errors("foreign \"c\" {\n  pub fn f()\n}\n"),
        ["E0016 16..19: expected `fn` or `type`, found the keyword `pub`"]
    );
    assert_eq!(
        errors("foreign \"c\" {\n  const N: CInt = 1\n}\n"),
        ["E0016 16..21: expected `fn`, `type` or `}`, found the keyword `const`"]
    );
    // A slip in the header keeps the body.
    assert_eq!(
        errors("foreign c {\n  fn f()\n}\n"),
        ["E0016 8..9: expected the library's name, a string, found the name `c`"]
    );
    assert_eq!(codes("foreign \"c\"\n{\n}\n"), ["E0018"]);
    // A `uses` after a returned function type is the C function's (`r[effect.uses-return]`).
    assert_eq!(
        errors("foreign \"c\" {\n  fn f() -> fn(Int) -> Int uses Io\n}\n"),
        ["E0016 41..45: expected a line break, found the keyword `uses`"]
    );
}

/// The text after applying each error's fix, with its applicability, for errors that have
/// one.
fn fixed(text: &str) -> Vec<(String, Applicability)> {
    checked(text)
        .errors()
        .iter()
        .flat_map(|e| &e.fixes)
        .map(|fix| {
            let mut fixed = text.to_string();
            for (range, with) in fix.edits.iter().rev() {
                fixed.replace_range(range.start as usize..range.end as usize, with);
            }
            (fixed, fix.applicability)
        })
        .collect()
}

/// A header that ends its line where its body's `{` belongs is one E0016 at the line break,
/// and the lines after it are parsed as the body's members up to its `}`. The fix inserts the
/// `{` when a `}` closes the body (decision 9).
#[test]
fn a_missing_brace_is_assumed() {
    let text = "interface I\n  fn f() -> Int\n  fn g(x: Int)\n  type T\n}\n";
    assert_eq!(
        errors(text),
        ["E0016 11..12: expected `{`, found a line break"]
    );
    assert_eq!(
        fixed(text),
        [(
            "interface I {\n  fn f() -> Int\n  fn g(x: Int)\n  type T\n}\n".to_string(),
            Applicability::Safe
        )]
    );
    // The members are the interface's.
    let tree = dump_tree_without_trivia(&checked(text).syntax());
    assert_eq!(tree.matches("InterfaceDecl").count(), 1, "{tree}");
    assert!(tree.trim_end().ends_with("RBrace \"}\""), "{tree}");
    // A comment after the header stays after the `{`.
    assert_eq!(
        fixed("impl S for T // c\n  fn f() {}\n}\n"),
        [(
            "impl S for T { // c\n  fn f() {}\n}\n".to_string(),
            Applicability::Safe
        )]
    );
    assert_eq!(
        errors("impl S\n  fn f() {}\n  type A = Int\n}\n"),
        ["E0016 6..7: expected `for` or `{`, found a line break"]
    );
    assert_eq!(
        errors("effect E\n  fn f()\n  fn g() -> Int\n}\n"),
        ["E0016 8..9: expected `uses`, `{` or `=`, found a line break"]
    );
    assert_eq!(
        errors("effect E uses Fs\n  fn f()\n}\n"),
        ["E0016 16..17: expected `{`, found a line break"]
    );
    assert_eq!(
        errors("foreign \"c\"\n  type FILE\n  fn f() -> CInt\n}\n"),
        ["E0016 11..12: expected `uses` or `{`, found a line break"]
    );
    // With no `}`, the body ends at a line it can't hold, or at the end of the file, with no
    // error of its own and no fix: inserting the `{` would leave it unclosed.
    for text in [
        "interface I\n  fn f(self)\nstruct S\n",
        "interface I\n  fn f(self)\n",
        "interface I\n",
    ] {
        assert_eq!(codes(text), ["E0016"], "{text:?}");
        assert_eq!(fixed(text), [], "{text:?}");
    }
    // The first member reports its own slip, as after a real `{`, and a body with a slip
    // gets no fix: the `{` isn't all that's missing.
    for text in [
        "interface I\n  x\n  fn f(self)\n}\n",
        "interface I\n  fn f(\n}\n",
    ] {
        assert_eq!(codes(text)[0], "E0016", "{text:?}");
        assert_eq!(codes(text).len(), 2, "{text:?}");
        assert_eq!(fixed(text), [], "{text:?}");
    }
    assert_eq!(
        codes("interface I /// d\n  fn f(self)\n}\n"),
        ["E0023", "E0016"]
    );
    // After a `;`, the lexer's E0002 is the slip, and covers the first member's.
    assert_eq!(codes("interface I;\n  x\n  fn f(self)\n}\n"), ["E0002"]);
    // Generics that the break closes, with an E0017, are a header that parsed.
    assert_eq!(
        errors("interface I<T\n  fn f()\n  fn g()\n}\nfn h() {}\n"),
        ["E0017 11..12: this `<` has no matching `>`"]
    );
    // After a slip on the header's line, no body is assumed: the header may not take one.
    assert_eq!(
        errors("effect E -> s = time.Clock\nfn g() {}\n"),
        ["E0016 9..11: expected `uses`, `{` or `=`, found `->`"]
    );
    // A `{` that an unclosed string swallowed: the lexer's errors are the slip.
    assert_eq!(
        codes("foreign \"c {\n  fn f()\n  fn g()\n}\n"),
        ["E0004", "E0007"]
    );
}

/// `uses` can't start a line, so after an `effect` or `foreign` header it continues the
/// header: an E0018 (decision 4).
#[test]
fn uses_continues_a_header() {
    let text = "effect E\nuses Fs {\n  fn f()\n}\n";
    assert_eq!(
        errors(text),
        ["E0018 8..9: a line break can't come before `uses`, which continues the line above"]
    );
    assert_eq!(
        fixed(text),
        [(
            "effect E uses Fs {\n  fn f()\n}\n".to_string(),
            Applicability::Safe
        )]
    );
    assert_eq!(
        codes("foreign \"c\"\n  uses Fs {\n  fn f()\n}\n"),
        ["E0018"]
    );
    // So does `for` after an `impl`'s interface.
    let text = "impl<K, V> Show<K, V>\n    for Map<K, V> {\n  fn show(self) -> String {\n    \"\"\n  }\n}\n";
    assert_eq!(codes(text), ["E0018"]);
    assert_eq!(
        fixed(text)[0].0,
        "impl<K, V> Show<K, V> for Map<K, V> {\n  fn show(self) -> String {\n    \"\"\n  }\n}\n"
    );
    // A `fn` header continues at `uses` (`r[lex.continue.fn-header]`).
    valid("fn f()\n  uses Io {\n}\n");
}

/// A line that starts no declaration or member is one E0016, and a block it opens is skipped
/// to its `}`, the members in it included (decision 9).
#[test]
fn a_line_that_is_no_declaration_skips_its_block() {
    assert_eq!(
        errors("interfce I {\n  fn f(self)\n  fn g(self)\n}\nfn h() {}\n"),
        ["E0016 0..8: expected a declaration, found the name `interfce`"]
    );
    assert_eq!(codes("{\n  fn f(self)\n  type T\n}\n"), ["E0016"]);
    // With no `}`, the skip ends at a line its block can't hold, or at the end of the file.
    assert_eq!(
        codes("imlp S {\n  fn f() {}\n  type A = Int\nstruct T\nfn g() x\n"),
        ["E0016", "E0016"]
    );
    assert_eq!(codes("imlp S {\n  fn f() {}\n"), ["E0016"]);
    // In a body, the block is a method's, so a member start ends the skip.
    assert_eq!(
        errors("impl S {\n  fnn f() {\n    let x = 1\n    while x {}\n  }\n  fn h() {}\n}\n"),
        ["E0016 11..14: expected `fn`, `const`, `type` or `}`, found the name `fnn`"]
    );
    assert_eq!(
        codes("impl S {\n  t{ype A = Int\n  fn f() {}\n}\nfn g() {}\n"),
        ["E0016"]
    );
}
