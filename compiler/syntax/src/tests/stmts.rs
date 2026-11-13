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
  BindPat
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
    // Braces that hold an entry's `:` were meant as a map: the fix puts them in parentheses,
    // and the block after them is still the block.
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
    // Braces that hold an entry's `:` are a map, parsed as one, so the rest of the condition and
    // the block are as written, on a line that continues the condition too.
    assert_eq!(errors("  if {\"a\": 1}.len() > 0 {}"), ["E0016 5..6"]);
    assert_eq!(errors("  if {\"a\": 1}\n    .len() > 0 {}"), ["E0016 5..6"]);
    assert_eq!(errors("  if {\"a\": 1}[k] {}"), ["E0016 5..6"]);
    assert_eq!(errors("  if m == {\"a\": 1}?.x {}"), ["E0016 10..11"]);
    assert_eq!(errors("  while {\"a\": 1} != m {}"), ["E0016 8..9"]);
    assert_eq!(errors("  if m == {\"a\": 1} && ok {}"), ["E0016 10..11"]);
    assert_eq!(errors("  if m == {\"a\":\n    1} {}"), ["E0016 10..11"]);
    assert_eq!(errors("  if m == {:} {}"), ["E0016 10..11"]);
    assert_eq!(errors("  if m == ({:}) {}"), [] as [String; 0]);
    assert_eq!(errors("  if f({\"a\": 1}) {}"), [] as [String; 0]);
    assert_eq!(
        errors("  if if a { {\"x\": 1} } else { m } == m {}"),
        [] as [String; 0]
    );
    // Braces with no entry's `:` are the block, whatever follows them: one E0016 where the
    // expression is missing, with no fix, then one for what follows the block.
    assert_eq!(errors("  if {x} {y}"), ["E0016 5..6", "E0016 9..10"]);
    assert_eq!(
        messages("  if {x} {y}"),
        [
            "expected an expression, found `{`",
            "expected a line break or `}`, found `{`"
        ]
    );
    assert_eq!(
        errors("  if a > { b } == c {}"),
        ["E0016 9..10", "E0016 20..21"]
    );
    // A block's `let x: T` holds a `:`, but no map holds a `let` line.
    assert_eq!(
        errors("  while n > {\n    let x: Int = 1\n  }"),
        ["E0016 12..13"]
    );
}

/// The block that a `{` in a condition starts is one E0016, wherever the `if` is, and the lines
/// after it parse as written (`r[expr.block.cond-brace]`).
#[test]
fn a_brace_in_a_condition_is_one_error() {
    assert_eq!(
        errors("  let v = if a > { 1 } else { 2 }"),
        ["E0016 17..18"]
    );
    assert_eq!(errors("  g(if a > { 1 })"), ["E0016 11..12"]);
    assert_eq!(errors("  [if a > { 1 }, 2]"), ["E0016 10..11"]);
    assert_eq!(
        errors("  while n > {\n    g()\n  }\n  x = 2"),
        ["E0016 12..13"]
    );
    // A map whose fix would move a comment gets none.
    let fixes = |body: &str| {
        let text = format!("{PREFIX}{body}\n}}\n");
        checked(&text)
            .errors()
            .iter()
            .map(|e| e.fixes.len())
            .collect::<Vec<_>>()
    };
    assert_eq!(fixes("  if m == {\"a\": 1} {}"), [1]);
    assert_eq!(fixes("  if m == {\"a\": 1, // c\n    \"b\": 2} {}"), [0]);
    // Right after an error token, the map is part of that slip.
    assert_eq!(errors("  if a == $ {\"a\": 1} {}"), ["E0001 10..11"]);
    // The map is whole, not a slip, so the operators around it are still checked.
    assert_eq!(
        errors("  if {\"a\": 1} == m == n {}"),
        ["E0016 5..6", "E0020 19..21"]
    );
}

/// `match`, its subject and its arms in braces. An arm is a pattern, a guard if there is one,
/// `=>`, and an expression or a block (`grammar.match_expr`, `grammar.arm`).
#[test]
fn match_and_its_arms() {
    assert_eq!(
        stmts("match x {\n  0 => a,\n  n if n > 0 => { b },\n}"),
        r#"MatchExpr
  MatchKw "match"
  NameRef
    Ident "x"
  MatchArmList
    LBrace "{"
    MatchArm
      LiteralPat
        Literal
          Int "0"
      FatArrow "=>"
      NameRef
        Ident "a"
    Comma ","
    MatchArm
      BindPat
        Name
          Ident "n"
      MatchGuard
        IfKw "if"
        Binary
          NameRef
            Ident "n"
          Gt ">"
          Literal
            Int "0"
      FatArrow "=>"
      Block
        LBrace "{"
        NameRef
          Ident "b"
        RBrace "}"
    Comma ","
    RBrace "}"
"#
    );
    assert_eq!(
        stmts("match x {}"),
        r#"MatchExpr
  MatchKw "match"
  NameRef
    Ident "x"
  MatchArmList
    LBrace "{"
    RBrace "}"
"#
    );
    // The arms are a list, so a line break before a `,` or the `}` is spacing, and a line that
    // starts with `=>` continues the arm, an E0018.
    assert_eq!(
        stmts("match x {\n  0 => a\n  , _ => b\n  ,\n}"),
        stmts("match x { 0 => a, _ => b, }")
    );
    assert_eq!(
        errors("  match x {\n    0\n    => a,\n  }"),
        ["E0018 17..18"]
    );
}

/// Comment lines directly above an arm go inside it, as they do above a declaration.
#[test]
fn comments_above_an_arm() {
    let text = "fn f() {\n  match x {\n    // zero\n    0 => a,\n  }\n}\n";
    let arm = checked(text)
        .syntax()
        .descendants()
        .find(|node| node.kind() == crate::SyntaxKind::MatchArm)
        .expect("an arm");
    assert_eq!(arm.text().to_string(), "// zero\n    0 => a");
}

/// `for`, a pattern, `in`, an expression and a block (`grammar.stmt`).
#[test]
fn for_loops() {
    assert_eq!(
        stmts("for (k, v) in m { a }"),
        r#"ForStmt
  ForKw "for"
  TuplePat
    LParen "("
    BindPat
      Name
        Ident "k"
    Comma ","
    BindPat
      Name
        Ident "v"
    RParen ")"
  InKw "in"
  NameRef
    Ident "m"
  Block
    LBrace "{"
    NameRef
      Ident "a"
    RBrace "}"
"#
    );
    // `for` is a statement, so it isn't a value (`r[stmt.loop.stmt]`).
    assert_eq!(errors("  let v = for x in xs { a }"), ["E0016 10..13"]);
}

/// Every arm ends with a `,` (`r[expr.match.arm-comma]`), so a `}` right after one is an
/// E0016 whose fix inserts it. Each slip in a `match` or a `for` is one error.
#[test]
fn match_and_for_slips() {
    let fixed = |body: &str| {
        let text = format!("{PREFIX}{body}\n}}\n");
        let parse = checked(&text);
        parse
            .errors()
            .iter()
            .filter_map(|e| {
                let [fix] = e.fixes.as_slice() else {
                    return None;
                };
                let mut fixed = text.clone();
                for (range, with) in fix.edits.iter().rev() {
                    fixed.replace_range(range.start as usize..range.end as usize, with);
                }
                Some(fixed[PREFIX.len()..fixed.len() - "\n}\n".len()].to_string())
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(errors("  match x { 0 => a }"), ["E0016 19..20"]);
    assert_eq!(fixed("  match x { 0 => a }"), ["  match x { 0 => a, }"]);
    assert_eq!(
        fixed("  match x {\n    _ => { a }\n  }"),
        ["  match x {\n    _ => { a },\n  }"]
    );
    assert_eq!(
        errors("  match x {\n    0 => a\n    _ => b,\n  }"),
        ["E0016 27..28"]
    );
    assert_eq!(errors("  match x { 0 a, }"), ["E0016 14..15"]);
    assert_eq!(errors("  match x { => a, }"), ["E0016 12..14"]);
    assert_eq!(errors("  match x { a if => b, }"), ["E0016 17..19"]);
    // A guard with a slip in it has no `=>` to report missing.
    assert_eq!(errors("  match x { a if => , }"), ["E0016 17..19"]);
    // After an arm whose string is unclosed, the missing `,` is part of that slip.
    assert_eq!(
        codes(&checked("fn f() {\n  match x {\n    0 => \"abc\n  }\n}\n")),
        ["E0004"]
    );
    assert_eq!(errors("  match { _ => a, }"), ["E0016 8..9"]);
    assert_eq!(
        errors("  match x {\n    0 => a,\n  let y = 1"),
        ["E0017 10..11"]
    );
    // A line that isn't a member start reads as an arm, so the missing `}` cascades, as a
    // map's does (decision 9, accepted by David, 2026-11-12).
    let text = "fn f() {\n  let a = match s {\n    0 => 1,\n  log(a)\n}\n";
    let found: Vec<_> = checked(text)
        .errors()
        .iter()
        .map(|e| describe(text, e))
        .collect();
    assert_eq!(found, ["E0017 7..8", "E0016 49..50"]);
    assert_eq!(errors("  for x xs { a }"), ["E0016 8..10"]);
    // The block after the slip is still the block, so a slip in it is reported too.
    assert_eq!(
        errors("  for x xs {\n    a b\n  }"),
        ["E0016 8..10", "E0016 19..20"]
    );
    assert_eq!(errors("  for in xs { a }"), ["E0016 6..8"]);
    assert_eq!(errors("  for x in { a }"), ["E0016 11..12"]);
    // A pattern missing from the line of its `let` or `for` is taken from the next line.
    assert_eq!(errors("  for\n    x in xs { a }"), ["E0016 5..6"]);
    assert_eq!(errors("  let\n    (a, b) = p"), ["E0016 5..6"]);
}

/// An arm's `=>` closes the brackets the arm left open, so the next arms parse and the slip is
/// one E0017 at the open bracket. A block in the arm is statement lines again, so a `=>` in it
/// is an ordinary slip, and a missing `}` before the next arm cascades.
#[test]
fn an_arms_fat_arrow_closes_its_brackets() {
    assert_eq!(
        errors("  match s {\n    A(x => 1,\n    B(y) => 2,\n    C(z) => 3,\n    _ => 4,\n  }"),
        ["E0017 17..18"]
    );
    assert_eq!(
        errors("  match s {\n    A(x) if g(x => 1,\n    _ => 4,\n  }"),
        ["E0017 25..26"]
    );
    assert_eq!(
        errors("  match s {\n    Shape.Circle(radius: => 1,\n    _ => 4,\n  }"),
        ["E0017 28..29"]
    );
    assert_eq!(
        errors("  match s {\n    A(f(x => 1,\n    _ => 4,\n  }"),
        ["E0017 17..18", "E0017 19..20"]
    );
    // In the body the next arm's pattern is the slip when it can't be an argument, and its
    // `=>` closes the `(` silently. When it can, the `=>` closes the `(` with an E0017.
    assert_eq!(
        errors("  match s {\n    0 => f(a,\n    _ => 4,\n  }"),
        ["E0016 30..31"]
    );
    assert_eq!(
        errors("  match s {\n    0 => f(a,\n    B => 4,\n  }"),
        ["E0017 22..23"]
    );
    // The innermost match's arm closes its own brackets, and the outer arms go on.
    assert_eq!(
        errors("  match s {\n    0 => match t { A(x => 1, _ => 2, },\n    _ => 3,\n  }"),
        ["E0017 32..33"]
    );
    assert_eq!(
        errors("  match s {\n    0 => f(match t { 1 => 2, }),\n    _ => 3,\n  }"),
        Vec::<String>::new()
    );
    // While the parser skips after a slip in an arm, the arm's `=>` stops the skip too, so
    // the next arm is parsed, and its own slip reported.
    assert_eq!(
        errors("  match s {\n    0 => a b(c => 1,\n    _ => x y,\n  }"),
        ["E0016 23..24", "E0016 44..45"]
    );
    assert_eq!(
        errors("  match s {\n    [a => 1,\n    _ => x y,\n  }"),
        ["E0016 16..17", "E0016 36..37"]
    );
    assert_eq!(
        errors("  match s {\n    A(x y(z => 1,\n    _ => x y,\n  }"),
        ["E0016 20..21", "E0016 41..42"]
    );
    assert_eq!(
        errors("  match s {\n    0 => {\n      x => 1\n    },\n    _ => 3,\n  }"),
        ["E0016 31..33"]
    );
    // A missing `}` in a block arm: the block runs on over the next arms (decision 9).
    let text =
        format!("{PREFIX}  match s {{\n    0 => {{\n      a\n    1 => 2,\n    _ => 3,\n  }}\n}}\n");
    let found: Vec<_> = checked(&text).errors().iter().map(|e| e.code).collect();
    assert_eq!(found, ["E0017", "E0016", "E0016"]);
}

/// Blocks nest, and a missing `}` is one E0017 at the `{` the next declaration closes.
#[test]
fn nested_blocks_close_at_a_declaration() {
    let text = "fn f() {\n  if a {\n    while b {\n      g()\n    }\n}\nfn h() {}\n";
    let parse = checked(text);
    let found: Vec<_> = parse.errors().iter().map(|e| describe(text, e)).collect();
    assert_eq!(found, ["E0017 7..8"]);
}

/// Each open block is a frame, and the member-start check at each line runs over the open
/// frames. It peeks at the line's first tokens once, so the looks it takes between two tokens
/// don't grow with the depth. More open frames than `LOOKAHEAD_LIMIT` allows looks would trip
/// the loop guard if they did, so the depth is past it. The thread has room for the recursion
/// in a debug build.
#[test]
fn deeply_nested_blocks() {
    const DEPTH: usize = 300;
    let text = format!(
        "fn f() {{\n{}g()\n{}}}\n",
        "if a {\n".repeat(DEPTH),
        "}\n".repeat(DEPTH)
    );
    let errors = std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(move || checked(&text).errors().len())
        .expect("a thread")
        .join()
        .expect("no panic");
    assert_eq!(errors, 0);
}
