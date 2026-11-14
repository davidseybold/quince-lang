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

fn codes(text: &str) -> Vec<&'static str> {
    checked(text).errors().iter().map(|e| e.code).collect()
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

/// Doc comments on a variant and on a variant's field.
#[test]
fn doc_comments_on_variants() {
    let text = "enum E {\n  /// A.\n  A(\n    /// X.\n    x: Int,\n  ),\n}\n";
    assert_eq!(
        valid(text),
        r#"File
  EnumDecl
    EnumKw "enum"
    Name
      Ident "E"
    VariantList
      LBrace "{"
      Variant
        DocComments
          DocComment "/// A."
        Name
          Ident "A"
        VariantFields
          LParen "("
          FieldDecl
            DocComments
              DocComment "/// X."
            Name
              Ident "x"
            Colon ":"
            TypePath
              Ident "Int"
          Comma ","
          RParen ")"
      Comma ","
      RBrace "}"
"#
    );
}

/// A slip in an attribute is one error, and the declaration after it is still parsed.
#[test]
fn slips_in_attributes() {
    for text in [
        "@test fn f() {}",
        "@test\nfn f() {}",
        "@\nfoo\nfn f() {}",
        "@pub\nfn f() {}",
        "@ fn f() {}",
        "@x\n@enum",
        "@enum",
        "@fn",
        "@type X",
    ] {
        assert_eq!(checked(text).errors().len(), 1, "{text:?}");
    }
    let parse = checked("@test fn f() {}");
    assert!(parse.syntax().descendants().any(|n| n.kind() == FnDecl));
    // A missing `)` in an attribute's arguments.
    let text = "struct S {\n  @wire(\"a\"\n  a: Int,\n}\nfn g() {}\n";
    // A field line looks like the next argument: an E0016 with a fix that inserts `,`, then
    // the struct's `}` closes the `(`.
    assert_eq!(codes(text), ["E0017", "E0016"]);
    assert_eq!(codes("@deprecated(\"use g\"\nfn f() {}\n"), ["E0017"]);
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

/// A doc comment that documents nothing is an E0023, whose fix makes it an ordinary comment,
/// and stays trivia (`r[lex.comment.doc-placement]`): in a function body, after an attribute,
/// before an import, before a closing brace, and at the end of the file.
#[test]
fn doc_comments_that_document_nothing() {
    for (text, expected) in [
        (
            "fn f() {\n  /// stray\n}\n",
            r#"E0023 11..20 => "fn f() {\n  // stray\n}\n""#,
        ),
        (
            "@test_only\n/// after the attribute\nfn f() {}\n",
            r#"E0023 11..34 => "@test_only\n// after the attribute\nfn f() {}\n""#,
        ),
        ("/// d\nimport a\n", r#"E0023 0..5 => "// d\nimport a\n""#),
        (
            "struct S {\n  a: Int,\n  /// d\n}\n",
            r#"E0023 23..28 => "struct S {\n  a: Int,\n  // d\n}\n""#,
        ),
        (
            "fn f() {}\n/// at the end\n",
            r#"E0023 10..24 => "fn f() {}\n// at the end\n""#,
        ),
    ] {
        assert_eq!(errors(text), [expected], "{text:?}");
        let docs = checked(text)
            .syntax()
            .descendants()
            .any(|n| n.kind() == DocComments);
        assert!(!docs, "{text:?}");
    }
}

/// The doc comments before one token that document nothing are one E0023, whose fix makes
/// each an ordinary comment, ordinary comments and blank lines between them included.
#[test]
fn doc_comments_together_are_one_error() {
    assert_eq!(
        errors("fn f() {\n  /// one\n  // note\n\n  /// two\n  x\n}\n"),
        [r#"E0023 11..39 => "fn f() {\n  // one\n  // note\n\n  // two\n  x\n}\n""#]
    );
}

/// A doc comment after code on its line documents nothing (`r[lex.comment.doc]`), and the
/// ones on the lines below it still document the declaration after them.
#[test]
fn a_doc_comment_after_code() {
    let text = "const AB: Int = 1 /// a\n/// b\nfn g() {}\n";
    assert_eq!(
        errors(text),
        [r#"E0023 18..23 => "const AB: Int = 1 // a\n/// b\nfn g() {}\n""#]
    );
    let g = checked(text).syntax().children().nth(1).expect("`g`");
    assert_eq!(g.text().to_string(), "/// b\nfn g() {}");
    assert_eq!(codes("fn f() {} /// a\n"), ["E0023"]);
    // At the start of the file, nothing comes before it.
    assert_eq!(codes("/// a\nfn f() {}\n"), [""; 0]);
}

/// A doc comment in tokens skipped after an error is part of that slip.
#[test]
fn a_doc_comment_in_a_skip() {
    assert_eq!(codes("fn f() {\n  let x = ) /// a\n  y\n}\n"), ["E0016"]);
    assert_eq!(codes("fn f() {\n  g(1 2 /// a\n  3)\n}\n"), ["E0016"]);
    // After code on its line, before a declaration that starts while the parser skips.
    assert_eq!(codes("const AB: Int = ) /// a\nfn g() {}\n"), ["E0016"]);
    assert_eq!(codes("const AB: Int = 1 /// a\nfn g() {}\n"), ["E0023"]);
}

/// An `impl`'s members take doc comments and attributes, as declarations do
/// (`grammar.impl_item`).
#[test]
fn an_impl_member_with_doc_comments_and_attributes() {
    let text = "impl S {\n  /// Make one.\n  @verify\n  pub fn new() {}\n}\n";
    assert_eq!(
        valid(text),
        r#"File
  ImplDecl
    ImplKw "impl"
    TypePath
      Ident "S"
    LBrace "{"
    FnDecl
      DocComments
        DocComment "/// Make one."
      Attribute
        At "@"
        NameRef
          Ident "verify"
      PubKw "pub"
      FnKw "fn"
      Name
        Ident "new"
      ParamList
        LParen "("
        RParen ")"
      Block
        LBrace "{"
        RBrace "}"
    RBrace "}"
"#
    );
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
    // A blank line may follow an attribute.
    assert_eq!(
        errors("fn f() {\n  g(1,\n@test_only\n\nfn h() {}\n"),
        ["E0017 7..8", "E0017 12..13"]
    );
    assert_eq!(errors("@test_only\n\n\nfn h() {}\n"), Vec::<String>::new());
}

/// A line that turns out to be no item after attributes or `pub` stays part of the slip
/// before it, and only an item ends the imports.
#[test]
fn one_slip_one_error_around_items() {
    for text in [
        "struct User {\n  a: Int)\n  pub id: UserId,\n  name: String,\n}",
        "struct User {\n  @wire(\"i\"))\n  pub id: UserId,\n  name: String,\n}",
        "struct User {\n  a: Int)\n  @wire(\"i\")\n  name: String,\n}",
        "import a\nfoo()\nimport b\nimport c",
        "@x\nimport a\nimport b\nimport c",
    ] {
        assert_eq!(errors(text).len(), 1, "{text}: {:?}", errors(text));
    }
    assert_eq!(codes("fn a() {}\nimport b"), ["E0016"]);
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
    // Removing them would delete a comment.
    assert_eq!(errors("struct S { // TODO\n}\n"), ["E0024 9..20"]);
    // The doc comment documents nothing, an error of its own.
    assert_eq!(codes("struct S {\n  /// doc\n}\n"), ["E0024", "E0023"]);
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
    // No fix when what follows can't be what the attribute applies to.
    assert_eq!(errors("enum E {\n  @x,\n  A,\n}\n"), ["E0016 13..14"]);
    // The fix keeps the file's line endings.
    assert_eq!(
        errors("fn a() {}\r\n@x fn f() {}"),
        [r#"E0016 14..16 => "fn a() {}\r\n@x\r\nfn f() {}""#]
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
    // Forms the grammar rejects.
    assert_eq!(messages("enum E { A() }"), ["expected a field, found `)`"]);
    assert_eq!(messages("import a.{}"), ["expected a name, found `}`"]);
    assert_eq!(
        messages("import a.{b} as c"),
        ["expected a line break, found the keyword `as`"]
    );
    assert_eq!(
        messages("@deprecated()\nfn f() {}"),
        ["expected an expression, found `)`"]
    );
    assert_eq!(errors("@x(1,)\nfn f() {}"), Vec::<String>::new());
    assert_eq!(
        messages("@x(,)\nfn f() {}"),
        ["expected an expression, found `,`"]
    );
    // A skip in struct braces stays out of type arguments.
    assert_eq!(codes("struct S { a: Int b: Map<K, V> }"), ["E0016"]);
}

/// A test's `forall` parameters and `uses` effects come in that order, and `forall` takes one
/// parameter or more (`grammar.test_decl`). A line that starts with `forall`, `uses` or `{`
/// continues the header (`r[lex.continue.test-header]`), and one that starts with a word that
/// continues only a `fn` header ends it.
#[test]
fn a_test_header_errors() {
    assert_eq!(
        errors("test \"t\" forall (seed: Int, var n: Int,) uses Net, Clock {}\n"),
        Vec::<String>::new()
    );
    assert_eq!(
        messages("test \"t\" forall () {}\n"),
        ["expected a parameter, found `)`"]
    );
    assert_eq!(errors("test \"t\" forall () {}\n"), ["E0016 17..18"]);
    assert_eq!(
        messages("test \"t\" forall {}\n"),
        ["expected `(`, found `{`"]
    );
    assert_eq!(
        messages("test \"t\" uses {}\n"),
        ["expected an effect, found `{`"]
    );
    assert_eq!(
        messages("test \"t\" uses Net forall (n: Int) {}\n"),
        ["expected `{`, found the keyword `forall`"]
    );
    assert_eq!(
        messages("test \"t\" 1 {}\n"),
        ["expected `forall`, `uses` or `{`, found the number `1`"]
    );
    assert_eq!(
        messages("test \"t\" forall (n: Int) 1 {}\n"),
        ["expected `uses` or `{`, found the number `1`"]
    );
    let one_line = valid("test \"t\" forall (n: Int) uses Net {}\n");
    for broken in [
        "test \"t\"\n  forall (n: Int) uses Net {}\n",
        "test \"t\" forall (n: Int)\n  uses Net {}\n",
        "test \"t\" forall (n: Int) uses Net\n{}\n",
        "test \"t\"\n  forall (n: Int)\n  uses Net\n{}\n",
    ] {
        assert_eq!(valid(broken), one_line, "{broken:?}");
    }
    assert_eq!(
        messages("test \"t\"\n  requires true {}\n"),
        ["expected `forall`, `uses` or `{`, found a line break"]
    );
    // A function type in the parameters has a `fn` header of its own.
    valid("test \"t\" forall (f: fn(Int)\n  -> Int) {}\n");
    // An unclosed parameter list closes at the next declaration.
    assert_eq!(
        errors("test \"t\" forall (n: Int\nfn f() {}\n"),
        ["E0017 16..17"]
    );
}
