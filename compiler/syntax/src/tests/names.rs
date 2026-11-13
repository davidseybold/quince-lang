//! The naming-case check (`validate`): the classes, the conversion its messages give, and
//! which declared names it checks.

use super::parser::checked;
use super::repo_root;
use crate::SyntaxKind;
use crate::validate::{Case, class_of};

/// Each error as `code start..end: message`.
fn errors(text: &str) -> Vec<String> {
    checked(text)
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

/// The class each name fits, if any (`r[lex.name.snake]`, `r[lex.name.pascal]`,
/// `r[lex.name.screaming]`). No name fits two.
#[test]
fn the_classes() {
    let cases: &[(&str, Option<Case>)] = &[
        ("x", Some(Case::Snake)),
        ("x2", Some(Case::Snake)),
        ("utf8_len", Some(Case::Snake)),
        ("a_1", Some(Case::Snake)),
        ("T", Some(Case::Pascal)),
        ("T2", Some(Case::Pascal)),
        ("Url", Some(Case::Pascal)),
        ("HTTPServer", Some(Case::Pascal)),
        ("Utf8Reader", Some(Case::Pascal)),
        ("ID", Some(Case::Screaming)),
        ("HTTP2", Some(Case::Screaming)),
        ("MAX_RETRIES", Some(Case::Screaming)),
        ("A_1", None),
        ("_unused", None),
        ("trailing_", None),
        ("a__b", None),
        ("camelCase", None),
        ("Sdl_init", None),
        ("MAX__RETRIES", None),
        ("MAX_", None),
        ("__", None),
        ("2x", None),
    ];
    for &(name, class) in cases {
        let fits: Vec<Case> = [Case::Snake, Case::Pascal, Case::Screaming]
            .into_iter()
            .filter(|case| case.fits(name))
            .collect();
        assert_eq!(fits, class.into_iter().collect::<Vec<_>>(), "{name}");
    }
}

/// A name written in another class, word by word, or nothing when that gives no name of the
/// class or gives a keyword.
#[test]
fn the_conversion() {
    let cases: &[(&str, Case, Option<&str>)] = &[
        ("AddTwo", Case::Snake, Some("add_two")),
        ("camelCase", Case::Snake, Some("camel_case")),
        ("HTTPServer", Case::Snake, Some("http_server")),
        ("utf8Len", Case::Snake, Some("utf8_len")),
        ("_unused", Case::Snake, Some("unused")),
        ("a__b", Case::Snake, Some("a_b")),
        ("MAX_RETRIES", Case::Pascal, Some("MaxRetries")),
        ("URL", Case::Pascal, Some("Url")),
        ("http_server", Case::Pascal, Some("HttpServer")),
        ("Point_2d", Case::Pascal, Some("Point2d")),
        ("maxRetries", Case::Screaming, Some("MAX_RETRIES")),
        ("HttpServer", Case::Screaming, Some("HTTP_SERVER")),
        ("N", Case::Screaming, None),
        ("Fn", Case::Snake, None),
        ("__", Case::Snake, None),
    ];
    for &(name, case, converted) in cases {
        assert_eq!(
            case.convert(name).as_deref(),
            converted,
            "{name} as {case:?}"
        );
    }
}

/// E0021 is over the name, and its message names the class and the name converted to it.
#[test]
fn the_messages() {
    assert_eq!(
        errors("fn AddTwo() {}\n"),
        ["E0021 3..9: `AddTwo` isn't snake_case, the case of a function's name: write `add_two`"]
    );
    assert_eq!(
        errors("const N: Int = 8\n"),
        ["E0021 6..7: `N` isn't SCREAMING_SNAKE_CASE, the case of a constant's name"]
    );
    assert_eq!(
        errors("fn f<uses e>() {}\n"),
        ["E0021 10..11: `e` isn't PascalCase, the case of an effect variable's name: write `E`"]
    );
    // A pattern binds a name that doesn't start with an uppercase letter, so `_rest` is a
    // binding, and fits no class.
    assert_eq!(
        errors("fn f() {\n  let _rest = v\n}\n"),
        ["E0021 15..20: `_rest` isn't snake_case, the case of a binding's name: write `rest`"]
    );
}

/// A name glued to a character the lexer rejects is part of one identifier the lexer split,
/// so its E0001 is the one error.
#[test]
fn a_name_split_by_the_lexer() {
    for text in [
        "struct \u{c9}bauche\n",
        "struct foo\u{e9}\n",
        "fn f(\u{e9}a: Int) {}\n",
    ] {
        assert_eq!(
            checked(text)
                .errors()
                .iter()
                .map(|e| e.code)
                .collect::<Vec<_>>(),
            ["E0001"],
            "{text:?}"
        );
    }
}

/// Every `Name` outside a foreign block has a class, and none inside one does
/// (`r[lex.name.foreign-exempt]`).
#[test]
fn every_declared_name_has_a_class() {
    let text = "import std.json as fast_json
const SIZE: Int = 1
struct Pair<T> { first: T, second: T }
struct UserId(Int)
enum Shape { Circle(radius: Float) }
type Meters = Float
interface Show { type Output\n fn show(self) -> String ensures (text) text != \"\" }
impl<T: Show> Show for List<T> { type Output = String\n const LIMIT: Int = 1\n fn show(self) -> String { \"\" } }
effect Clock { fn now() -> Instant }
fn run<uses E>(n: Int) uses E {
  let (a, b) = (1, 2)
  var count = 0
  for item in [1] {}
  let f = fn(x) { x }
  let r = background(scope: TaskScope) {}
  match n { other => other, }
}
";
    let parse = checked(text);
    assert_eq!(parse.errors(), [], "{text:?}");
    let names: Vec<_> = parse
        .syntax()
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::Name)
        .collect();
    assert_eq!(names.len(), 33);
    for name in &names {
        assert!(class_of(name).is_some(), "{name:?} in {:?}", name.parent());
    }

    let foreign =
        checked("foreign \"c\" {\n  type sqlite3\n  fn SDL_Init(Flags: CUInt) -> CInt\n}\n");
    assert_eq!(foreign.errors(), []);
    assert!(
        foreign
            .syntax()
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::Name)
            .all(|name| class_of(&name).is_none())
    );
}

/// Every `Name` outside a foreign block in the parse fixtures has a class, so a new place that
/// declares a name can't slip past the check.
#[test]
fn every_fixture_name_has_a_class() {
    let mut dirs = vec![repo_root().join("tests/conformance/testdata/parse")];
    let mut seen = 0;
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                dirs.push(path);
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            for name in checked(&text)
                .syntax()
                .descendants()
                .filter(|node| node.kind() == SyntaxKind::Name)
            {
                let foreign = name
                    .ancestors()
                    .any(|node| node.kind() == SyntaxKind::ForeignDecl);
                assert_eq!(
                    class_of(&name).is_some(),
                    !foreign,
                    "{name:?} in {:?}, {}",
                    name.parent(),
                    path.display()
                );
                seen += 1;
            }
        }
    }
    assert!(seen > 0, "no names in the parse fixtures");
}
