//! `spec/grammar.ebnf`: the two grammar appendices of the spec, copied verbatim under a header.
//!
//! The header is the leading `//` lines. One of them, `// terminals: ...`, lists the names the
//! spec defines in prose instead of as productions. A blank line separates the header from the
//! grammar.

use crate::SpecError;
use std::collections::{HashMap, HashSet};

/// The headings whose first code block holds each half of the grammar, in file order.
pub const APPENDIX_HEADINGS: [&str; 2] = [
    "## Appendix: Expression grammar",
    "### Declarations, types and patterns",
];

const TERMINALS_PREFIX: &str = "// terminals:";

/// The grammar as it appears in the spec: both appendix code blocks, separated by a blank line.
pub fn extract(spec: &str) -> Result<String, SpecError> {
    let lines: Vec<&str> = spec.lines().collect();
    let mut blocks = Vec::new();
    for heading in APPENDIX_HEADINGS {
        let start = lines
            .iter()
            .position(|line| line.trim_end() == heading)
            .ok_or_else(|| SpecError {
                line: 0,
                message: format!("heading `{heading}` not found in the spec"),
            })?;
        let open = (start + 1..lines.len())
            .find(|&i| lines[i].trim_start().starts_with("```"))
            .ok_or_else(|| SpecError {
                line: start + 1,
                message: format!("no code block after `{heading}`"),
            })?;
        let close = (open + 1..lines.len())
            .find(|&i| lines[i].trim_start().starts_with("```"))
            .ok_or_else(|| SpecError {
                line: open + 1,
                message: format!("the code block after `{heading}` is never closed"),
            })?;
        blocks.push(lines[open + 1..close].join("\n"));
    }
    Ok(format!("{}\n", blocks.join("\n\n")))
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

/// Checks that no production is defined twice and that every referenced name is a production
/// or a declared terminal.
pub fn check_closure(grammar: &str, terminals: &HashSet<String>) -> Vec<SpecError> {
    let productions = productions(grammar);
    let mut errors = Vec::new();
    let mut defined: HashMap<&str, usize> = HashMap::new();
    for p in &productions {
        if let Some(first) = defined.get(p.name.as_str()) {
            errors.push(SpecError {
                line: p.line,
                message: format!(
                    "production `{}` is defined twice; first on line {first}",
                    p.name
                ),
            });
        } else {
            defined.insert(&p.name, p.line);
        }
    }
    for p in &productions {
        let mut reported = HashSet::new();
        for name in &p.references {
            let known = defined.contains_key(name.as_str()) || terminals.contains(name);
            if !known && reported.insert(name) {
                errors.push(SpecError {
                    line: p.line,
                    message: format!(
                        "`{name}`, used in production `{}`, is neither a production nor a terminal in the header",
                        p.name
                    ),
                });
            }
        }
    }
    errors
}

/// A line diff of `expected` against `actual`, with `-` for lines only in `actual` and `+` for
/// lines only in `expected`, and up to two lines of context around each change.
pub fn diff(actual: &str, expected: &str) -> String {
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
    let mut ops = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < a.len() || j < b.len() {
        if i < a.len() && j < b.len() && a[i] == b[j] {
            ops.push((' ', a[i]));
            i += 1;
            j += 1;
        } else if j < b.len() && (i == a.len() || lcs[i][j + 1] >= lcs[i + 1][j]) {
            ops.push(('+', b[j]));
            j += 1;
        } else {
            ops.push(('-', a[i]));
            i += 1;
        }
    }
    let changed: Vec<usize> = (0..ops.len()).filter(|&k| ops[k].0 != ' ').collect();
    let mut out = String::new();
    let mut last_printed: Option<usize> = None;
    for (k, (kind, text)) in ops.iter().enumerate() {
        let near_change = changed.iter().any(|&c| c.abs_diff(k) <= 2);
        if !near_change {
            continue;
        }
        if last_printed.is_some_and(|p| p + 1 != k) {
            out.push_str("...\n");
        }
        out.push_str(&format!("{kind} {text}\n"));
        last_printed = Some(k);
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

    #[test]
    fn extracts_both_blocks() {
        assert_eq!(
            extract(SPEC).unwrap(),
            "expr = term { \"+\" term }   // a comment with words\nterm = \"(\" expr \")\" | name\n\nfile = { expr NEWLINE }\n"
        );
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
    fn header_split_and_terminals() {
        let file = "// about\n// terminals: name NEWLINE\n\nexpr = name\n";
        let (header, grammar) = split_header(file);
        assert_eq!(header, "// about\n// terminals: name NEWLINE\n");
        assert_eq!(grammar, "expr = name\n");
        assert_eq!(terminals(header), terms(&["name", "NEWLINE"]));
    }

    #[test]
    fn productions_ignore_strings_and_comments() {
        let grammar = extract(SPEC).unwrap();
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
        let grammar = extract(SPEC).unwrap();
        assert_eq!(
            check_closure(&grammar, &terms(&["name", "NEWLINE"])),
            Vec::new()
        );
    }

    #[test]
    fn undefined_name() {
        let errors = check_closure("a = type_list b\nb = name\n", &terms(&["name"]));
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].line, 1);
        assert!(
            errors[0]
                .message
                .starts_with("`type_list`, used in production `a`")
        );
    }

    #[test]
    fn duplicate_production() {
        let errors = check_closure("arg = name\narg = name\n", &terms(&["name"]));
        assert_eq!(
            errors[0].to_string(),
            "line 2: production `arg` is defined twice; first on line 1"
        );
    }

    #[test]
    fn diff_shows_changes() {
        let d = diff("a\nb\nc\n", "a\nB\nc\n");
        assert!(d.contains("- b\n"), "{d}");
        assert!(d.contains("+ B\n"), "{d}");
        assert_eq!(diff("same\n", "same\n"), "");
    }
}
