//! `spec/grammar.ebnf`: the two grammar appendices of the spec, copied verbatim under a header.
//!
//! The header is the leading `//` lines, and is the only hand-edited part of the file. One of
//! them, `// terminals: ...`, lists the names the spec defines in prose instead of as
//! productions. A blank line separates the header from the grammar.

use crate::SpecError;
use std::collections::{HashMap, HashSet};

/// The headings whose code block holds each half of the grammar, in file order.
pub const APPENDIX_HEADINGS: [&str; 2] = [
    "## Appendix: Expression grammar",
    "### Declarations, types and patterns",
];

/// The production every other production is reachable from, so nothing needs to reference it.
pub const ROOT_PRODUCTION: &str = "file";

/// The header `regenerate` uses when the file doesn't exist yet.
pub const DEFAULT_HEADER: &str = "\
// The Quince grammar, copied verbatim from the two grammar appendices of spec/quince-spec.md.
// Don't edit below this header by hand: change the spec, then run `cargo xtask spec-grammar --write`.
// `cargo xtask spec-grammar` fails when this file and the spec disagree.
//
// Notation: `{ x }` is zero or more, `[ x ]` is optional. The names below are defined in the
// spec's prose, not as productions. `path` is `name { \".\" name }`, a package path.
// terminals: name Name NAME c_name path literal string int doc_comment NEWLINE
";

const TERMINALS_PREFIX: &str = "// terminals:";

/// The grammar as it appears in the spec, with the spec line each grammar line came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extracted {
    /// Both appendix code blocks, separated by a blank line, ending with a newline.
    pub text: String,
    /// `spec_lines[i]` is the 1-based spec line of grammar line `i + 1`.
    spec_lines: Vec<usize>,
}

impl Extracted {
    /// The 1-based spec line that 1-based grammar line `line` was copied from.
    pub fn spec_line(&self, line: usize) -> usize {
        self.spec_lines
            .get(line.wrapping_sub(1))
            .copied()
            .unwrap_or(0)
    }
}

/// Finds both appendix code blocks. Each heading must be followed by exactly one code block
/// before the next heading, so a second block can't be silently skipped.
pub fn extract(spec: &str) -> Result<Extracted, SpecError> {
    let lines: Vec<&str> = spec.lines().collect();
    let is_fence = |i: usize| lines[i].trim_start().starts_with("```");
    let mut text_lines: Vec<&str> = Vec::new();
    let mut spec_lines = Vec::new();
    for (n, heading) in APPENDIX_HEADINGS.into_iter().enumerate() {
        let start = lines
            .iter()
            .position(|line| line.trim_end() == heading)
            .ok_or_else(|| SpecError {
                line: 0,
                message: format!("heading `{heading}` not found in the spec"),
            })?;
        let open = (start + 1..lines.len())
            .find(|&i| is_fence(i))
            .ok_or_else(|| SpecError {
                line: start + 1,
                message: format!("no code block after `{heading}`"),
            })?;
        let close = (open + 1..lines.len())
            .find(|&i| is_fence(i))
            .ok_or_else(|| SpecError {
                line: open + 1,
                message: format!("the code block after `{heading}` is never closed"),
            })?;
        let next_heading = (close + 1..lines.len())
            .find(|&i| lines[i].starts_with('#'))
            .unwrap_or(lines.len());
        if let Some(extra) = (close + 1..next_heading).find(|&i| is_fence(i)) {
            return Err(SpecError {
                line: extra + 1,
                message: format!(
                    "`{heading}` has a second code block; the grammar check reads exactly one \
                     block per appendix heading, so merge them or move this one under its own heading"
                ),
            });
        }
        if n > 0 {
            text_lines.push("");
            spec_lines.push(open);
        }
        text_lines.extend(&lines[open + 1..close]);
        spec_lines.extend(open + 2..close + 1);
    }
    Ok(Extracted {
        text: format!("{}\n", text_lines.join("\n")),
        spec_lines,
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

/// Checks that no production is defined twice, that every referenced name is a production or
/// a declared terminal, and that every production except `file` is used by another one.
/// `spec_line` maps a grammar line to the line reported, so errors point at the spec.
pub fn check_closure(
    grammar: &str,
    terminals: &HashSet<String>,
    spec_line: &dyn Fn(usize) -> usize,
) -> Vec<SpecError> {
    let productions = productions(grammar);
    let mut errors = Vec::new();
    let mut defined: HashMap<&str, usize> = HashMap::new();
    for p in &productions {
        if let Some(&first) = defined.get(p.name.as_str()) {
            errors.push(SpecError {
                line: spec_line(p.line),
                message: format!(
                    "production `{}` is defined twice; first on line {}",
                    p.name,
                    spec_line(first)
                ),
            });
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
                errors.push(SpecError {
                    line: spec_line(p.line),
                    message: format!(
                        "`{name}`, used in production `{}` (which starts on this line), is \
                         neither a production nor a terminal in the header",
                        p.name
                    ),
                });
            }
        }
    }
    for p in &productions {
        if p.name != ROOT_PRODUCTION && !used.contains(p.name.as_str()) {
            errors.push(SpecError {
                line: spec_line(p.line),
                message: format!(
                    "production `{}` is never used by another production; remove it or reference it",
                    p.name
                ),
            });
        }
    }
    errors.sort_by_key(|e| e.line);
    errors
}

/// Productions whose derived ID `grammar.<name>` is retired: a removed production name can't
/// come back with a new meaning.
pub fn check_retired(
    grammar: &str,
    retired: &HashSet<String>,
    spec_line: &dyn Fn(usize) -> usize,
) -> Vec<SpecError> {
    productions(grammar)
        .into_iter()
        .filter_map(|p| {
            let id = format!("grammar.{}", p.name);
            retired.contains(&id).then(|| SpecError {
                line: spec_line(p.line),
                message: format!(
                    "production `{}` has the retired rule ID `{id}` (spec/retired-rules.txt); give it a new name",
                    p.name
                ),
            })
        })
        .collect()
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

## Appendix: Expression grammar

Prose with r[lex.x] a tag.

```
expr = term { \"+\" term }   // a comment with words
term = \"(\" expr \")\" | name
```

### Declarations, types and patterns

```
file = { expr NEWLINE }
```
";

    fn terms(names: &[&str]) -> HashSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    fn same_line(line: usize) -> usize {
        line
    }

    #[test]
    fn extracts_both_blocks() {
        let extracted = extract(SPEC).unwrap();
        assert_eq!(
            extracted.text,
            "expr = term { \"+\" term }   // a comment with words\nterm = \"(\" expr \")\" | name\n\nfile = { expr NEWLINE }\n"
        );
        assert_eq!([1, 2, 4].map(|line| extracted.spec_line(line)), [8, 9, 15]);
    }

    #[test]
    fn missing_heading() {
        let err =
            extract("# Spec\n\n## Appendix: Expression grammar\n\n```\nx = y\n```\n").unwrap_err();
        assert_eq!(
            err.message,
            "heading `### Declarations, types and patterns` not found in the spec"
        );
    }

    #[test]
    fn second_code_block_is_an_error() {
        let spec = SPEC.replace(
            "### Declarations",
            "More prose.\n\n```\nextra = name\n```\n\n### Declarations",
        );
        let err = extract(&spec).unwrap_err();
        assert_eq!(err.line, 14);
        assert!(
            err.message
                .starts_with("`## Appendix: Expression grammar` has a second code block"),
            "{}",
            err.message
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
        let grammar = extract(SPEC).unwrap().text;
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
        let extracted = extract(SPEC).unwrap();
        assert_eq!(
            check_closure(&extracted.text, &terms(&["name", "NEWLINE"]), &|line| {
                extracted.spec_line(line)
            }),
            Vec::new()
        );
    }

    #[test]
    fn undefined_name() {
        let errors = check_closure(
            "file = a\na = type_list b\nb = name\n",
            &terms(&["name"]),
            &same_line,
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
            &|line| line + 100,
        );
        assert_eq!(
            errors[0].to_string(),
            "line 103: production `arg` is defined twice; first on line 102"
        );
    }

    #[test]
    fn unused_production() {
        let errors = check_closure(
            "file = a\na = name | a\nold_arg = name\n",
            &terms(&["name"]),
            &same_line,
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
        let errors = check_retired("file = old_arg\nold_arg = name\n", &retired, &same_line);
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
