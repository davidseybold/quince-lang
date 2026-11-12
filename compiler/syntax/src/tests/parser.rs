//! The parser's trees: declarations, types, statements and expressions, where a line ends, and
//! where trivia sits. Each expected dump is written by hand. Errors are in `recovery`.

use super::cst::{check_lossless, dump_tree, dump_tree_without_trivia};
use super::repo_root;
use crate::{Parse, SyntaxError, SyntaxKind, decode_source, lex, lex_str, parse, parse_str};

/// The tree without trivia, after checking that the text parses with no error.
pub(super) fn valid(text: &str) -> String {
    let parse = checked(text);
    assert_eq!(parse.errors(), [], "{text:?}");
    dump_tree_without_trivia(&parse.syntax())
}

/// The statements of a function whose body is `body`, each dumped without trivia, after
/// checking that it parses with no error.
pub(super) fn stmts(body: &str) -> String {
    let text = format!("fn f() {{\n{body}\n}}\n");
    let parse = checked(&text);
    assert_eq!(parse.errors(), [], "{text:?}");
    let block = parse
        .syntax()
        .descendants()
        .find(|node| node.kind() == SyntaxKind::Block)
        .expect("a block");
    block
        .children()
        .map(|stmt| dump_tree_without_trivia(&stmt))
        .collect()
}

pub(super) fn checked(text: &str) -> Parse {
    let parse = parse_str(text);
    check_parse(text, &parse);
    parse
}

/// The tree is lossless, and every error range lies inside the text on character boundaries,
/// in source order. Only an error at the end of the text may be empty.
pub(super) fn check_parse(text: &str, parse: &Parse) {
    check_lossless(text, &lex_str(text), &parse.syntax());
    check_error_ranges(text, parse.errors());
}

pub(super) fn check_error_ranges(text: &str, errors: &[SyntaxError]) {
    for e in errors {
        let (start, end) = (e.range.start as usize, e.range.end as usize);
        assert!(
            (start < end || start == text.len()) && end <= text.len(),
            "{text:?}: {e:?}"
        );
        assert!(
            text.is_char_boundary(start) && text.is_char_boundary(end),
            "{text:?}: {e:?}"
        );
    }
    assert!(
        errors.is_sorted_by_key(|e| e.range.start),
        "{text:?}: errors out of order: {errors:?}"
    );
}

/// The `lex/` fixtures are valid syntax: the parser adds no error to the lexer's.
#[test]
fn the_lex_fixtures_parse() {
    let dir = repo_root().join("tests/conformance/testdata/lex");
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    paths.sort();
    for path in paths {
        let decoded = decode_source(&std::fs::read(&path).unwrap()).unwrap();
        let lexed = lex(&decoded);
        let parse = parse(&decoded);
        let mut expected = lexed.errors.clone();
        expected.sort_by_key(|e| e.range.start);
        assert_eq!(parse.errors(), expected, "{}", path.display());
        check_lossless(&decoded.text, &lexed, &parse.syntax());
    }
}

#[test]
fn an_empty_file() {
    assert_eq!(valid(""), "File\n");
    assert_eq!(valid("\n// c\n"), "File\n");
}

#[test]
fn a_function_with_parameters() {
    assert_eq!(
        valid("fn f(self, var n: Int, _: List<List<Int>>, d: Int = 1,) -> Self {}"),
        r#"File
  FnDecl
    FnKw "fn"
    Name
      Ident "f"
    ParamList
      LParen "("
      Param
        SelfKw "self"
      Comma ","
      Param
        VarKw "var"
        Name
          Ident "n"
        Colon ":"
        TypePath
          Ident "Int"
      Comma ","
      Param
        Underscore "_"
        Colon ":"
        TypePath
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
      Comma ","
      Param
        Name
          Ident "d"
        Colon ":"
        TypePath
          Ident "Int"
        Eq "="
        Literal
          Int "1"
      Comma ","
      RParen ")"
    RetType
      Arrow "->"
      TypePath
        SelfTypeKw "Self"
    Block
      LBrace "{"
      RBrace "}"
"#
    );
}

#[test]
fn constants_impls_and_tests() {
    assert_eq!(
        valid("pub const MAX: Int = 3\nimpl a.User {\n  pub fn new() {}\n}\ntest \"t\" {\n}\n"),
        r#"File
  ConstDecl
    PubKw "pub"
    ConstKw "const"
    Name
      Ident "MAX"
    Colon ":"
    TypePath
      Ident "Int"
    Eq "="
    Literal
      Int "3"
  ImplDecl
    ImplKw "impl"
    TypePath
      Ident "a"
      Dot "."
      Ident "User"
    LBrace "{"
    FnDecl
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
  TestDecl
    TestKw "test"
    Literal
      StringStart "\""
      StringText "t"
      StringEnd "\""
    Block
      LBrace "{"
      RBrace "}"
"#
    );
}

/// `true` and `false` are literals, like numbers and chars (`r[lex.lit.bool]`).
#[test]
fn literals_and_list_literals() {
    assert_eq!(
        stmts("let a: Map<String, Int> = [1, 2.5, 'c', true, false,]\n[]"),
        r#"LetStmt
  LetKw "let"
  Name
    Ident "a"
  Colon ":"
  TypePath
    Ident "Map"
    TypeArgs
      Lt "<"
      TypePath
        Ident "String"
      Comma ","
      TypePath
        Ident "Int"
      Gt ">"
  Eq "="
  ListLit
    LBracket "["
    Literal
      Int "1"
    Comma ","
    Literal
      Float "2.5"
    Comma ","
    Literal
      Char "'c'"
    Comma ","
    Literal
      TrueKw "true"
    Comma ","
    Literal
      FalseKw "false"
    Comma ","
    RBracket "]"
ListLit
  LBracket "["
  RBracket "]"
"#
    );
}

/// A string is one `Literal`, and an interpolation in it is an `Interp` of its path's tokens.
#[test]
fn strings_and_interpolation() {
    assert_eq!(
        stmts(r#""hi {user.name}!""#),
        r#"Literal
  StringStart "\""
  StringText "hi "
  Interp
    InterpStart "{"
    Ident "user"
    Dot "."
    Ident "name"
    InterpEnd "}"
  StringText "!"
  StringEnd "\""
"#
    );
    assert_eq!(
        stmts("\"\"\"\n  a\n  \"\"\""),
        r#"Literal
  StringStart "\"\"\""
  StringNewline "\n"
  StringIndent "  "
  StringText "a"
  StringNewline "\n"
  StringIndent "  "
  StringEnd "\"\"\""
"#
    );
}

/// Calls, method calls and fields wrap the expression before them, so a chain nests to the
/// left. A method call is a call whose callee is a field.
#[test]
fn calls_method_calls_and_fields() {
    assert_eq!(
        stmts("3.14.to_fixed(2).x.0\nself.f(a, g())"),
        r#"Field
  Field
    Call
      Field
        Literal
          Float "3.14"
        Dot "."
        NameRef
          Ident "to_fixed"
      ArgList
        LParen "("
        Literal
          Int "2"
        RParen ")"
    Dot "."
    NameRef
      Ident "x"
  Dot "."
  Int "0"
Call
  Field
    NameRef
      SelfKw "self"
    Dot "."
    NameRef
      Ident "f"
  ArgList
    LParen "("
    NameRef
      Ident "a"
    Comma ","
    Call
      NameRef
        Ident "g"
      ArgList
        LParen "("
        RParen ")"
    RParen ")"
"#
    );
}

#[test]
fn ranges() {
    assert_eq!(
        stmts("0..=n.len()\n1..10"),
        r#"Range
  Literal
    Int "0"
  DotDotEq "..="
  Call
    Field
      NameRef
        Ident "n"
      Dot "."
      NameRef
        Ident "len"
    ArgList
      LParen "("
      RParen ")"
Range
  Literal
    Int "1"
  DotDot ".."
  Literal
    Int "10"
"#
    );
}

/// A line break ends a statement, unless the line ends with a token that continues it, or the
/// next line starts with `.` (`r[lex.line-end]`, `r[lex.continue.more]`).
#[test]
fn where_a_statement_ends() {
    assert_eq!(
        stmts("f\ng"),
        "NameRef\n  Ident \"f\"\nNameRef\n  Ident \"g\"\n"
    );
    assert_eq!(
        stmts("f\n[x]"),
        "NameRef\n  Ident \"f\"\nListLit\n  LBracket \"[\"\n  NameRef\n    Ident \"x\"\n  RBracket \"]\"\n"
    );
    assert_eq!(
        stmts("a\n  .b\n  .c()"),
        r#"Call
  Field
    Field
      NameRef
        Ident "a"
      Dot "."
      NameRef
        Ident "b"
    Dot "."
    NameRef
      Ident "c"
  ArgList
    LParen "("
    RParen ")"
"#
    );
    assert_eq!(
        stmts("let r = 1..\n  10\ng(\n  a,\n  b,\n)"),
        stmts("let r = 1..10\ng(a, b,)")
    );
    assert_eq!(stmts("g(\n  a,\n  b,\n)"), stmts("g(a, b,)"));
    // A `(` on the next line doesn't call `f`: it starts a statement.
    assert_eq!(
        stmts("f\n(x)"),
        "NameRef\n  Ident \"f\"\nParen\n  LParen \"(\"\n  NameRef\n    Ident \"x\"\n  RParen \")\"\n"
    );
}

/// In a `fn` header, a line that starts with `->` or `{` continues it
/// (`r[lex.continue.fn-header]`). Indentation means nothing (`r[lex.indentation]`).
#[test]
fn a_fn_header_continues_on_the_next_line() {
    assert_eq!(
        valid("fn f()\n    -> Int\n{\n1\n}"),
        valid("fn f() -> Int {\n  1\n}")
    );
    assert_eq!(valid("fn f()\n{}"), valid("fn f() {}"));
    // After a token that continues the line, a type may start the next one.
    assert_eq!(
        valid("fn f() ->\n  Map<\n    String,\n    Int\n  > {}"),
        valid("fn f() -> Map<String, Int> {}")
    );
}

/// Trivia goes outside nodes, comments above a declaration go inside it, and a doc comment in
/// a body is trivia.
#[test]
fn where_trivia_sits() {
    let text = "// lead\nfn f() {\n  /// stray\n  x // end\n}\n";
    assert_eq!(
        dump_tree(&checked(text).syntax()),
        r#"File
  FnDecl
    Comment "// lead"
    Newline "\n"
    FnKw "fn"
    Whitespace " "
    Name
      Ident "f"
    ParamList
      LParen "("
      RParen ")"
    Whitespace " "
    Block
      LBrace "{"
      Newline "\n"
      Whitespace "  "
      DocComment "/// stray"
      Newline "\n"
      Whitespace "  "
      NameRef
        Ident "x"
      Whitespace " "
      Comment "// end"
      Newline "\n"
      RBrace "}"
  Newline "\n"
"#
    );
}

/// The skipped tokens of an error go in a `Skipped` node, a bracket and what it holds
/// included.
#[test]
fn skipped_tokens() {
    let parse = checked("fn match() {}");
    assert_eq!(
        dump_tree_without_trivia(&parse.syntax()),
        r#"File
  FnDecl
    FnKw "fn"
  Skipped
    MatchKw "match"
    LParen "("
    RParen ")"
    LBrace "{"
    RBrace "}"
"#
    );
}
