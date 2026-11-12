//! The parser's trees for imports, structs, newtypes, enums and type aliases, their doc
//! comments and attributes, and their errors. Each expected dump is written by hand.

use super::errors::describe;
use super::parser::{checked, valid};
use crate::SyntaxKind::*;

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
fn a_struct_with_doc_comments_and_attributes() {
    let text = "/// A user.\n@opaque\npub struct User {\n  /// Its id.\n  pub id: Int,\n}\n";
    assert_eq!(
        valid(text),
        r#"File
  StructDecl
    DocComments
      DocComment "/// A user."
    Attribute
      At "@"
      NameRef
        Ident "opaque"
    PubKw "pub"
    StructKw "struct"
    Name
      Ident "User"
    FieldList
      LBrace "{"
      FieldDecl
        DocComments
          DocComment "/// Its id."
        PubKw "pub"
        Name
          Ident "id"
        Colon ":"
        TypePath
          Ident "Int"
      Comma ","
      RBrace "}"
"#
    );
}

#[test]
fn an_enum() {
    assert_eq!(
        valid("enum Shape { Circle(radius: Float), Point }"),
        r#"File
  EnumDecl
    EnumKw "enum"
    Name
      Ident "Shape"
    VariantList
      LBrace "{"
      Variant
        Name
          Ident "Circle"
        VariantFields
          LParen "("
          FieldDecl
            Name
              Ident "radius"
            Colon ":"
            TypePath
              Ident "Float"
          RParen ")"
      Comma ","
      Variant
        Name
          Ident "Point"
      RBrace "}"
"#
    );
}

#[test]
fn imports() {
    assert_eq!(
        valid("import std.log.{self, Log}\nimport json as j\n"),
        r#"File
  Import
    ImportKw "import"
    Ident "std"
    Dot "."
    Ident "log"
    Dot "."
    ImportNames
      LBrace "{"
      NameRef
        SelfKw "self"
      Comma ","
      NameRef
        Ident "Log"
      RBrace "}"
  Import
    ImportKw "import"
    Ident "json"
    AsKw "as"
    Name
      Ident "j"
"#
    );
}

#[test]
fn a_newtype_and_an_alias() {
    assert_eq!(
        valid("struct Id(pub Int)\ntype Ids = List<Id>\n"),
        r#"File
  NewtypeDecl
    StructKw "struct"
    Name
      Ident "Id"
    LParen "("
    PubKw "pub"
    TypePath
      Ident "Int"
    RParen ")"
  AliasDecl
    TypeKw "type"
    Name
      Ident "Ids"
    Eq "="
    TypePath
      Ident "List"
      TypeArgs
        Lt "<"
        TypePath
          Ident "Id"
        Gt ">"
"#
    );
}

/// A doc comment that documents nothing stays a comment until E0023 arrives (layer 7): in a
/// function body, after an attribute, and in an `impl` body, whose members take theirs in
/// layer 7.
#[test]
fn doc_comments_that_document_nothing_stay_comments() {
    for text in [
        "fn f() {\n  /// stray\n}\n",
        "@test_only\n/// after the attribute\nfn f() {}\n",
        "impl S {\n  /// a method\n  fn m(self) {}\n}\n",
        "fn f() {}\n/// at the end\n",
    ] {
        let parse = checked(text);
        assert_eq!(parse.errors(), [], "{text:?}");
        let docs = parse
            .syntax()
            .descendants()
            .any(|n| n.kind() == DocComments);
        assert!(!docs, "{text:?}");
    }
}

/// An ordinary comment above the doc comments goes in the declaration with them.
#[test]
fn comments_above_doc_comments_attach() {
    let parse = checked("fn a() {}\n\n// note\n/// doc\nfn b() {}\n");
    let b = parse
        .syntax()
        .children()
        .nth(1)
        .expect("a second declaration");
    assert_eq!(b.text().to_string(), "// note\n/// doc\nfn b() {}");
}

/// Decision 9: a member start after attribute lines closes what it closes, with no E0016 at
/// the `@`.
#[test]
fn attribute_lines_are_looked_past() {
    assert_eq!(
        errors("fn f() {\n  g(1,\n@test_only\nfn h() {}\n"),
        ["E0017 7..8", "E0017 12..13"]
    );
    let text = "struct S {\n  a: Int,\n  @wire(\"b\")\n  pub b: Int,\n}\n";
    assert_eq!(errors(text), Vec::<String>::new());
}

/// The slips of decision 9's table for struct braces.
#[test]
fn slips_in_struct_braces() {
    assert_eq!(
        errors("struct S {\n  pub a: Int,\n  pub b: Int,\n}\n"),
        Vec::<String>::new()
    );
    assert_eq!(
        errors("struct S {\n  a: Int,\npub fn g() {}\n"),
        ["E0017 9..10"]
    );
    // A `pub` field on its own line closes a list inside the braces.
    assert_eq!(
        errors("struct S {\n  a: fn(Int,\n  pub b: Int,\n}\n"),
        ["E0017 18..19"]
    );
    assert_eq!(
        messages("enum E { A(pub x: Int) }"),
        ["expected a field name, found the keyword `pub`"]
    );
}

#[test]
fn empty_struct_braces() {
    assert_eq!(errors("struct S {}"), [r#"E0024 9..11 => "struct S""#]);
    assert_eq!(
        errors("struct S {\n}\n"),
        [r#"E0024 9..12 => "struct S\n""#]
    );
    // A `requires` needs fields, so removing the braces alone wouldn't fix it.
    assert_eq!(errors("struct S {} requires x"), ["E0024 9..11"]);
}

#[test]
fn an_attribute_on_its_declarations_line() {
    assert_eq!(
        errors("@verify fn f() {}"),
        [r#"E0016 8..10 => "@verify\nfn f() {}""#]
    );
    let text = "struct S {\n  @wire(\"a\") a: Int,\n}\n";
    assert_eq!(
        errors(text),
        [r#"E0016 24..25 => "struct S {\n  @wire(\"a\")\n  a: Int,\n}\n""#]
    );
    assert_eq!(
        messages("@\nfn f() {}"),
        ["expected an attribute name, found a line break"]
    );
}

#[test]
fn errors_in_declarations() {
    assert_eq!(
        messages("import a.b\nfn f() {}\nimport c\n"),
        ["expected a declaration, found the keyword `import`"]
    );
    assert_eq!(
        messages("import std.http.*"),
        ["expected a name or `{`, found `*`"]
    );
    assert_eq!(messages("struct P(Int, Int)"), ["expected `)`, found `,`"]);
    assert_eq!(
        messages("enum E"),
        ["expected `{`, found the end of the file"]
    );
    assert_eq!(
        messages("type T Int"),
        ["expected `=`, found the name `Int`"]
    );
    assert_eq!(
        messages("struct S { a: Int b: Int }"),
        ["expected `,` or `}`, found the name `b`"]
    );
}
