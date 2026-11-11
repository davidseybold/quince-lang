//! `spec/grammar.ebnf`: every `ebnf` code block in the spec, copied verbatim under a header.
//!
//! The header is the leading `//` lines, and is the only hand-edited part of the file. One of
//! them, `// terminals: ...`, lists the names the spec defines in prose instead of as
//! productions. A blank line separates the header from the grammar.

use crate::blocks::code_blocks;
use crate::{ChapterFile, FileError, SPEC_DIR};
use std::collections::{HashMap, HashSet};

/// The info string of a fenced code block that holds grammar. Any other block is an example.
pub const GRAMMAR_INFO: &str = "ebnf";

/// The production every other production is reachable from, so nothing needs to reference it.
pub const ROOT_PRODUCTION: &str = "file";

/// The header `regenerate` uses when the file doesn't exist yet.
pub const DEFAULT_HEADER: &str = "\
// The Quince grammar, copied verbatim from the `ebnf` blocks of the spec's chapters (spec/chapters.txt).
// Don't edit below this header by hand: change the spec, then run `cargo xtask spec-grammar --write`.
// `cargo xtask spec-grammar` fails when this file and the spec disagree.
//
// Notation: `{ x }` is zero or more, `[ x ]` is optional. The names below are defined in the
// spec's prose, not as productions. `path` is `name { \".\" name }`, a package path.
// terminals: name Name NAME c_name path literal string int doc_comment NEWLINE
";

const TERMINALS_PREFIX: &str = "// terminals:";

/// The grammar as it appears in the spec, with the chapter file and line each grammar line came
/// from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extracted {
    /// Every `ebnf` block, separated by a blank line, ending with a newline.
    pub text: String,
    /// The chapter files the blocks came from, relative to the repo root.
    paths: Vec<String>,
    /// `origins[i]` is the source of grammar line `i + 1`: an index into `paths` and a 1-based
    /// line in that file.
    origins: Vec<(usize, usize)>,
}

impl Extracted {
    /// The chapter file and 1-based line that 1-based grammar line `line` was copied from, or
    /// `("spec", 0)` for a line past the end.
    pub fn origin(&self, line: usize) -> (&str, usize) {
        self.origins
            .get(line.wrapping_sub(1))
            .map_or((SPEC_DIR, 0), |&(path, line)| {
                (self.paths[path].as_str(), line)
            })
    }
}

/// Collects every code block whose info string is `ebnf`, in the order of `files` and then of
/// each file, separated by one blank line.
pub fn extract(files: &[ChapterFile]) -> Result<Extracted, FileError> {
    let mut text_lines: Vec<&str> = Vec::new();
    let mut paths = Vec::new();
    let mut origins = Vec::new();
    for file in files {
        let path = paths.len();
        paths.push(file.path.clone());
        for block in code_blocks(&file.text) {
            if block.label != GRAMMAR_INFO {
                continue;
            }
            if !block.closed {
                return Err(FileError {
                    path: file.path.clone(),
                    line: block.line,
                    message: format!("this `{GRAMMAR_INFO}` code block is never closed"),
                });
            }
            if !origins.is_empty() {
                text_lines.push("");
                origins.push((path, block.line));
            }
            for (index, line) in block.lines.iter().enumerate() {
                text_lines.push(line);
                origins.push((path, block.line + 1 + index));
            }
        }
    }
    if origins.is_empty() {
        return Err(FileError {
            path: SPEC_DIR.to_string(),
            line: 0,
            message: format!(
                "no chapter has a code block marked `{GRAMMAR_INFO}`; grammar goes in fenced \
                 blocks whose info string is `{GRAMMAR_INFO}`"
            ),
        });
    }
    Ok(Extracted {
        text: format!("{}\n", text_lines.join("\n")),
        paths,
        origins,
    })
}

/// Splits the file into its header (leading `//` lines, each with its newline) and the grammar
/// after the blank line that follows the header.
pub fn split_header(file: &str) -> (&str, &str) {
    let mut header_len = 0;
    for line in file.split_inclusive('\n') {
        if line.starts_with("//") {
            header_len += line.len();
        } else {
            break;
        }
    }
    let (header, rest) = file.split_at(header_len);
    (header, rest.strip_prefix('\n').unwrap_or(rest))
}

/// The full file for `grammar`: the existing file's header, or `DEFAULT_HEADER` when there is
/// no file or it has no header, then a blank line and the grammar.
pub fn regenerate(existing: Option<&str>, grammar: &str) -> String {
    let header = existing
        .map(|file| split_header(file).0)
        .filter(|header| !header.is_empty())
        .unwrap_or(DEFAULT_HEADER);
    format!("{header}\n{grammar}")
}

/// The terminals declared by the header's `// terminals:` line.
pub fn terminals(header: &str) -> HashSet<String> {
    header
        .lines()
        .filter_map(|line| line.strip_prefix(TERMINALS_PREFIX))
        .flat_map(|names| names.split_whitespace().map(str::to_string))
        .collect()
}

/// One production: its name, its 1-based line within the grammar text, and its right-hand side
/// with comments and string literals removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Production {
    pub name: String,
    pub line: usize,
    pub references: Vec<String>,
}

/// Parses productions: a line that starts with `name =` begins one, and indented lines continue
/// it.
pub fn productions(grammar: &str) -> Vec<Production> {
    let mut out: Vec<Production> = Vec::new();
    for (index, raw) in grammar.lines().enumerate() {
        let code = strip_strings_and_comments(raw);
        let starts_production = !raw.starts_with(char::is_whitespace)
            && code
                .split_once('=')
                .is_some_and(|(lhs, _)| is_identifier(lhs.trim()));
        if starts_production {
            let (lhs, rhs) = code.split_once('=').expect("checked above");
            out.push(Production {
                name: lhs.trim().to_string(),
                line: index + 1,
                references: identifiers(rhs),
            });
        } else if let Some(last) = out.last_mut() {
            last.references.extend(identifiers(&code));
        }
    }
    out
}

/// Maps a 1-based grammar line to the file and line reported, so errors point at the spec.
pub type Locate<'a> = &'a dyn Fn(usize) -> (String, usize);

/// Errors keyed by grammar line, sorted, then located. Grammar lines follow manifest order and
/// then document order, so the errors come out in file order.
fn located(mut errors: Vec<(usize, String)>, locate: Locate) -> Vec<FileError> {
    errors.sort_by_key(|(line, _)| *line);
    errors
        .into_iter()
        .map(|(line, message)| {
            let (path, line) = locate(line);
            FileError {
                path,
                line,
                message,
            }
        })
        .collect()
}

/// Checks that no production is defined twice, that every referenced name is a production or
/// a declared terminal, and that every production except `file` is used by another one.
pub fn check_closure(grammar: &str, terminals: &HashSet<String>, locate: Locate) -> Vec<FileError> {
    let productions = productions(grammar);
    let mut errors = Vec::new();
    let mut defined: HashMap<&str, usize> = HashMap::new();
    for p in &productions {
        if let Some(&first) = defined.get(p.name.as_str()) {
            let (first_path, first_line) = locate(first);
            errors.push((
                p.line,
                format!(
                    "production `{}` is defined twice; first at {first_path}:{first_line}",
                    p.name
                ),
            ));
        } else {
            defined.insert(&p.name, p.line);
        }
    }
    let mut used = HashSet::new();
    for p in &productions {
        let mut reported = HashSet::new();
        for name in &p.references {
            if name != &p.name {
                used.insert(name.as_str());
            }
            let known = defined.contains_key(name.as_str()) || terminals.contains(name);
            if !known && reported.insert(name) {
                errors.push((
                    p.line,
                    format!(
                        "`{name}`, used in production `{}` (which starts on this line), is \
                         neither a production nor a terminal in the header",
                        p.name
                    ),
                ));
            }
        }
    }
    for p in &productions {
        if p.name != ROOT_PRODUCTION && !used.contains(p.name.as_str()) {
            errors.push((
                p.line,
                format!(
                    "production `{}` is never used by another production; remove it or reference it",
                    p.name
                ),
            ));
        }
    }
    located(errors, locate)
}

/// Productions whose derived ID `grammar.<name>` is retired: a removed production name can't
/// come back with a new meaning.
pub fn check_retired(grammar: &str, retired: &HashSet<String>, locate: Locate) -> Vec<FileError> {
    let errors = productions(grammar)
        .into_iter()
        .filter_map(|p| {
            let id = format!("grammar.{}", p.name);
            retired.contains(&id).then(|| {
                (
                    p.line,
                    format!(
                        "production `{}` has the retired rule ID `{id}` (spec/retired-rules.txt); give it a new name",
                        p.name
                    ),
                )
            })
        })
        .collect();
    located(errors, locate)
}

/// A unified line diff of `expected` against `actual`: `-` for lines only in `actual`, `+`
/// for lines only in `expected`, two lines of context, and an `@@ -a,n +b,m @@` header on each
/// hunk.
pub fn diff(actual: &str, expected: &str) -> String {
    const CONTEXT: usize = 2;
    let a: Vec<&str> = actual.lines().collect();
    let b: Vec<&str> = expected.lines().collect();
    // Longest common subsequence table, filled from the end.
    let mut lcs = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    // Each op is (kind, text, 0-based index into `a`, 0-based index into `b`); the index is
    // that of the next line on a side the op doesn't consume.
    let mut ops = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < a.len() || j < b.len() {
        if i < a.len() && j < b.len() && a[i] == b[j] {
            ops.push((' ', a[i], i, j));
            i += 1;
            j += 1;
        } else if i < a.len() && (j == b.len() || lcs[i + 1][j] >= lcs[i][j + 1]) {
            ops.push(('-', a[i], i, j));
            i += 1;
        } else {
            ops.push(('+', b[j], i, j));
            j += 1;
        }
    }
    let changed: Vec<usize> = (0..ops.len()).filter(|&k| ops[k].0 != ' ').collect();
    let shown: Vec<usize> = (0..ops.len())
        .filter(|&k| changed.iter().any(|&c| c.abs_diff(k) <= CONTEXT))
        .collect();
    let mut out = String::new();
    let mut hunk_start = 0;
    while hunk_start < shown.len() {
        let mut hunk_end = hunk_start + 1;
        while hunk_end < shown.len() && shown[hunk_end] == shown[hunk_end - 1] + 1 {
            hunk_end += 1;
        }
        let hunk = &ops[shown[hunk_start]..=shown[hunk_end - 1]];
        let a_count = hunk.iter().filter(|op| op.0 != '+').count();
        let b_count = hunk.iter().filter(|op| op.0 != '-').count();
        let (_, _, a_at, b_at) = hunk[0];
        out.push_str(&format!(
            "@@ -{},{a_count} +{},{b_count} @@\n",
            a_at + 1,
            b_at + 1
        ));
        for (kind, text, _, _) in hunk {
            out.push_str(&format!("{kind} {text}\n"));
        }
        hunk_start = hunk_end;
    }
    out
}

fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn identifiers(code: &str) -> Vec<String> {
    code.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|word| is_identifier(word))
        .map(str::to_string)
        .collect()
}

/// Replaces string literals with `""` and drops a `//` comment, so only grammar symbols remain.
fn strip_strings_and_comments(line: &str) -> String {
    let mut out = String::new();
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                while let Some(s) = chars.next() {
                    match s {
                        '\\' => {
                            chars.next();
                        }
                        '"' => break,
                        _ => {}
                    }
                }
                out.push_str("\"\"");
            }
            '/' if chars.peek() == Some(&'/') => break,
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC: &str = "\
# Spec

## Expressions

Prose with r[lex.x] a tag.

```ebnf
expr = term { \"+\" term }   // a comment with words
term = \"(\" expr \")\" | name
```

```
example = not grammar
```
";

    const ITEMS: &str = "\
# Items

```ebnf
file = { expr NEWLINE }
```
";

    fn file(path: &str, text: &str) -> ChapterFile {
        ChapterFile {
            prefix: "x".into(),
            title: "X".into(),
            path: path.into(),
            file: path.into(),
            text: text.into(),
        }
    }

    fn spec() -> Vec<ChapterFile> {
        vec![file("spec/a.md", SPEC), file("spec/b.md", ITEMS)]
    }

    fn terms(names: &[&str]) -> HashSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    fn in_g(line: usize) -> (String, usize) {
        ("g".to_string(), line)
    }

    #[test]
    fn extracts_every_ebnf_block_in_order() {
        let extracted = extract(&spec()).unwrap();
        assert_eq!(
            extracted.text,
            "expr = term { \"+\" term }   // a comment with words\nterm = \"(\" expr \")\" | name\n\nfile = { expr NEWLINE }\n"
        );
        assert_eq!(
            [1, 2, 3, 4, 5].map(|line| extracted.origin(line)),
            [
                ("spec/a.md", 8),
                ("spec/a.md", 9),
                ("spec/b.md", 3),
                ("spec/b.md", 4),
                ("spec", 0)
            ]
        );
    }

    #[test]
    fn several_blocks_in_one_file() {
        let text = "```ebnf\na = b\n```\n\n```  ebnf  \nb = name\n```\n";
        let extracted = extract(&[file("spec/a.md", text)]).unwrap();
        assert_eq!(extracted.text, "a = b\n\nb = name\n");
        assert_eq!(extracted.origin(3), ("spec/a.md", 6));
    }

    #[test]
    fn no_grammar_blocks() {
        let err = extract(&[file("spec/a.md", "# Spec\n\n```\nx = y\n```\n")]).unwrap_err();
        assert_eq!(err.path, "spec");
        assert!(
            err.message
                .starts_with("no chapter has a code block marked `ebnf`"),
            "{}",
            err.message
        );
    }

    #[test]
    fn unclosed_grammar_block() {
        let err = extract(&[file("spec/a.md", "# Spec\n\n```ebnf\nx = y\n")]).unwrap_err();
        assert_eq!(
            err.to_string(),
            "spec/a.md:3: this `ebnf` code block is never closed"
        );
    }

    #[test]
    fn header_split_and_terminals() {
        let file = "// about\n// terminals: name NEWLINE\n\nexpr = name\n";
        let (header, grammar) = split_header(file);
        assert_eq!(header, "// about\n// terminals: name NEWLINE\n");
        assert_eq!(grammar, "expr = name\n");
        assert_eq!(terminals(header), terms(&["name", "NEWLINE"]));
    }

    #[test]
    fn regenerate_keeps_or_defaults_the_header() {
        let kept = regenerate(Some("// mine\n\nold = name\n"), "new = name\n");
        assert_eq!(kept, "// mine\n\nnew = name\n");
        let fresh = regenerate(None, "new = name\n");
        assert_eq!(fresh, format!("{DEFAULT_HEADER}\nnew = name\n"));
        assert_eq!(regenerate(Some("old = name\n"), "new = name\n"), fresh);
    }

    #[test]
    fn productions_ignore_strings_and_comments() {
        let grammar = extract(&spec()).unwrap().text;
        let found = productions(&grammar);
        let names: Vec<_> = found.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["expr", "term", "file"]);
        assert_eq!(found[0].references, ["term", "term"]);
    }

    #[test]
    fn continuation_lines_belong_to_the_production() {
        let found = productions("a = b\n    | c   // d\n");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].references, ["b", "c"]);
    }

    #[test]
    fn closure_passes() {
        let extracted = extract(&spec()).unwrap();
        let locate = |line| {
            let (path, line) = extracted.origin(line);
            (path.to_string(), line)
        };
        assert_eq!(
            check_closure(&extracted.text, &terms(&["name", "NEWLINE"]), &locate),
            Vec::new()
        );
    }

    #[test]
    fn undefined_name() {
        let errors = check_closure(
            "file = a\na = type_list b\nb = name\n",
            &terms(&["name"]),
            &in_g,
        );
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert_eq!(errors[0].line, 2);
        assert!(
            errors[0]
                .message
                .starts_with("`type_list`, used in production `a` (which starts on this line)")
        );
    }

    #[test]
    fn duplicate_production_reports_spec_lines() {
        let errors = check_closure(
            "file = arg\narg = name\narg = name\n",
            &terms(&["name"]),
            &|line| ("spec/a.md".to_string(), line + 100),
        );
        assert_eq!(
            errors[0].to_string(),
            "spec/a.md:103: production `arg` is defined twice; first at spec/a.md:102"
        );
    }

    #[test]
    fn unused_production() {
        let errors = check_closure(
            "file = a\na = name | a\nold_arg = name\n",
            &terms(&["name"]),
            &in_g,
        );
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert_eq!(errors[0].line, 3);
        assert!(
            errors[0]
                .message
                .starts_with("production `old_arg` is never used")
        );
    }

    #[test]
    fn retired_production_name() {
        let retired = terms(&["grammar.old_arg"]);
        let errors = check_retired("file = old_arg\nold_arg = name\n", &retired, &in_g);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].line, 2);
        assert!(
            errors[0]
                .message
                .contains("retired rule ID `grammar.old_arg`")
        );
    }

    #[test]
    fn diff_has_hunk_headers() {
        let d = diff("a\nb\nc\nd\ne\nf\ng\nh\n", "a\nB\nc\nd\ne\nf\ng\nH\n");
        assert_eq!(
            d,
            "@@ -1,4 +1,4 @@\n  a\n- b\n+ B\n  c\n  d\n@@ -6,3 +6,3 @@\n  f\n  g\n- h\n+ H\n"
        );
        assert_eq!(diff("same\n", "same\n"), "");
    }
}
