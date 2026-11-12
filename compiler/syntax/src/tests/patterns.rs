//! The parser's trees for patterns, and their errors. Each expected dump is written by hand.

use super::parser::{checked, stmts};

/// The tree of `pattern`, a `let`'s pattern, after checking that it parses with no error.
fn tree(pattern: &str) -> String {
    let dump = stmts(&format!("  let {pattern} = v"));
    let start = dump.find("  LetKw \"let\"\n").expect("a `let`") + "  LetKw \"let\"\n".len();
    let end = dump.find("  Eq \"=\"\n").expect("a `=`");
    dump[start..end]
        .lines()
        .map(|line| format!("{}\n", &line[2..]))
        .collect()
}

const PREFIX: &str = "fn f() {\n  let ";
const SUFFIX: &str = " = v\n}\n";

/// The errors of `pattern`, a `let`'s pattern: each as `code start..end` in `pattern`, then the
/// fixed pattern when it has one fix.
fn errors(pattern: &str) -> Vec<String> {
    let text = format!("{PREFIX}{pattern}{SUFFIX}");
    let parse = checked(&text);
    parse
        .errors()
        .iter()
        .map(|e| {
            let at = |offset: u32| offset as usize - PREFIX.len();
            let mut out = format!("{} {:?}", e.code, at(e.range.start)..at(e.range.end));
            if let [fix] = e.fixes.as_slice() {
                let mut fixed = text.clone();
                for (range, with) in fix.edits.iter().rev() {
                    fixed.replace_range(range.start as usize..range.end as usize, with);
                }
                out += &format!(" => {}", &fixed[PREFIX.len()..fixed.len() - SUFFIX.len()]);
            }
            out
        })
        .collect()
}

/// A name that stands alone binds, unless it starts with an uppercase letter
/// (`r[pat.const-exhaustive]`). Any other name or path is a path, whose meaning name
/// resolution decides (`r[pat.const]`, decision 7).
#[test]
fn names_and_paths() {
    assert_eq!(tree("n"), "BindPat\n  Name\n    Ident \"n\"\n");
    assert_eq!(tree("_rest"), "BindPat\n  Name\n    Ident \"_rest\"\n");
    assert_eq!(tree("MAX"), "PathPat\n  TypePath\n    Ident \"MAX\"\n");
    assert_eq!(tree("None"), "PathPat\n  TypePath\n    Ident \"None\"\n");
    assert_eq!(
        tree("Status.OK"),
        r#"PathPat
  TypePath
    Ident "Status"
    Dot "."
    Ident "OK"
"#
    );
    assert_eq!(
        tree("geo.Shape.Point"),
        r#"PathPat
  TypePath
    Ident "geo"
    Dot "."
    Ident "Shape"
    Dot "."
    Ident "Point"
"#
    );
    // A `.` on the next line continues the path, as it continues a line.
    assert_eq!(tree("Status\n    .OK"), tree("Status.OK"));
    assert_eq!(tree("_"), "WildcardPat\n  Underscore \"_\"\n");
}

/// Literals, negative numbers and ranges of literals (`r[pat.negative]`,
/// `r[expr.range.pattern]`). A negative number is one `Literal`, as in an expression.
#[test]
fn literals_and_ranges() {
    assert_eq!(tree("0"), "LiteralPat\n  Literal\n    Int \"0\"\n");
    assert_eq!(
        tree("-5"),
        "LiteralPat\n  Literal\n    Minus \"-\"\n    Int \"5\"\n"
    );
    assert_eq!(
        tree("\"GET\""),
        r#"LiteralPat
  Literal
    StringStart "\""
    StringText "GET"
    StringEnd "\""
"#
    );
    assert_eq!(
        tree("-10..=-1"),
        r#"RangePat
  Literal
    Minus "-"
    Int "10"
  DotDotEq "..="
  Literal
    Minus "-"
    Int "1"
"#
    );
    assert_eq!(
        tree("'a'..'z'"),
        r#"RangePat
  Literal
    Char "'a'"
  DotDot ".."
  Literal
    Char "'z'"
"#
    );
}

/// Tuples, and variants with fields by position or by name (`grammar.tuple_pat`,
/// `grammar.variant_pat`, `grammar.pfields`). A lowercase name followed by `(` is a variant
/// too, since a path may be a C name.
#[test]
fn tuples_and_variants() {
    assert_eq!(
        tree("(a, _,)"),
        r#"TuplePat
  LParen "("
  BindPat
    Name
      Ident "a"
  Comma ","
  WildcardPat
    Underscore "_"
  Comma ","
  RParen ")"
"#
    );
    assert_eq!(
        tree("Some(x)"),
        r#"VariantPat
  TypePath
    Ident "Some"
  LParen "("
  BindPat
    Name
      Ident "x"
  RParen ")"
"#
    );
    assert_eq!(
        tree("Shape.Rect(width: w, height:, ..)"),
        r#"VariantPat
  TypePath
    Ident "Shape"
    Dot "."
    Ident "Rect"
  LParen "("
  FieldPat
    NameRef
      Ident "width"
    Colon ":"
    BindPat
      Name
        Ident "w"
  Comma ","
  FieldPat
    NameRef
      Ident "height"
    Colon ":"
  Comma ","
  DotDot ".."
  RParen ")"
"#
    );
    assert_eq!(
        tree("Shape.Circle(..)"),
        r#"VariantPat
  TypePath
    Ident "Shape"
    Dot "."
    Ident "Circle"
  LParen "("
  DotDot ".."
  RParen ")"
"#
    );
    assert_eq!(
        tree("point(x)"),
        r#"VariantPat
  TypePath
    Ident "point"
  LParen "("
  BindPat
    Name
      Ident "x"
  RParen ")"
"#
    );
    // Line breaks in the parentheses are spacing (`r[lex.continue.lists]`).
    assert_eq!(
        tree("Shape.Rect(\n    width:,\n    ..\n  )"),
        tree("Shape.Rect(width:, ..)")
    );
}

/// Alternatives join with `|`, which in a pattern is never the bit operator
/// (`r[lex.grammar-rules]`). A line that starts with `|` continues the pattern, an E0018.
#[test]
fn alternatives() {
    assert_eq!(
        tree("A | b | _"),
        r#"OrPat
  PathPat
    TypePath
      Ident "A"
  Pipe "|"
  BindPat
    Name
      Ident "b"
  Pipe "|"
  WildcardPat
    Underscore "_"
"#
    );
    assert_eq!(tree("Some(1 | 2)").matches("OrPat").count(), 1);
    assert_eq!(errors("A\n    | B"), ["E0018 1..2 => A | B"]);
}

/// What the grammar doesn't allow: one pattern or none in parentheses, a variant with no
/// fields in its parentheses, `..` anywhere but last, fields by position and by name mixed,
/// and a `-` with a space after it, whose fix deletes the space.
#[test]
fn slips() {
    assert_eq!(errors("(a)"), ["E0016 2..3"]);
    assert_eq!(errors("(a,)"), ["E0016 3..4"]);
    assert_eq!(errors("()"), ["E0016 1..2"]);
    assert_eq!(errors("Shape.Rect()"), ["E0016 11..12"]);
    assert_eq!(errors("Shape.Rect(.., w:)"), ["E0016 15..16"]);
    assert_eq!(errors("Shape.Rect(w, h:)"), ["E0016 15..16"]);
    assert_eq!(errors("Shape.Rect(w:, h)"), ["E0016 15..16"]);
    assert_eq!(errors("Shape.(x)"), ["E0016 6..7"]);
    assert_eq!(errors("- 5"), ["E0016 1..2 => -5"]);
    assert_eq!(errors("-\n    5"), ["E0016 1..6"]);
    assert_eq!(errors("-x"), ["E0016 1..2"]);
    assert_eq!(errors("1..x"), ["E0016 3..4"]);
    // A pattern missing from the line of its `let` is taken from the next line.
    assert_eq!(errors("\n    (a, b)"), ["E0016 0..1"]);
    assert_eq!(errors("\n    x"), ["E0016 0..1"]);
    // A slip inside the parentheses is the one error, and the `let` goes on after them.
    assert_eq!(errors("(a b, c)"), ["E0016 3..4"]);
    assert_eq!(errors("Some(a b)"), ["E0016 7..8"]);
}
