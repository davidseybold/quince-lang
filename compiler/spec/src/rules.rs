//! Rule tags: `r[<id>]`, written directly before the sentence or sentences a rule covers.
//!
//! A tag is `r[` at the start of a line or after whitespace, outside code blocks and inline
//! code spans, closed by `]` and followed by one space. Tags never appear in headings.

use crate::SpecError;
use std::collections::{HashMap, HashSet};

/// One tagged rule in the spec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub id: String,
    /// 1-based line of the tag.
    pub line: usize,
    /// The `## ` heading the tag sits under, such as `2. Lexical structure and syntax`.
    pub section: String,
}

/// An ID is 2 to 4 dot-separated segments, each lowercase letters and digits joined by
/// single hyphens: `bind.shadow.same-scope`.
pub fn is_valid_id(id: &str) -> bool {
    let segments: Vec<&str> = id.split('.').collect();
    (2..=4).contains(&segments.len()) && segments.iter().all(|s| is_valid_segment(s))
}

fn is_valid_segment(segment: &str) -> bool {
    segment.split('-').all(|word| {
        !word.is_empty()
            && word
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    })
}

/// Parses every tag in the spec and checks format, placement and uniqueness. Returns every
/// rule, or every problem found.
pub fn check(spec: &str, retired: &HashSet<String>) -> Result<Vec<Rule>, Vec<SpecError>> {
    let (rules, mut errors) = parse(spec);
    let mut first_line: HashMap<&str, usize> = HashMap::new();
    for rule in &rules {
        if let Some(first) = first_line.get(rule.id.as_str()) {
            errors.push(SpecError {
                line: rule.line,
                message: format!(
                    "duplicate rule ID `{}`; it is first used on line {first}",
                    rule.id
                ),
            });
        } else {
            first_line.insert(&rule.id, rule.line);
        }
        if retired.contains(&rule.id) {
            errors.push(SpecError {
                line: rule.line,
                message: format!(
                    "rule ID `{}` is retired (spec/retired-rules.txt) and can't be reused; pick a new ID",
                    rule.id
                ),
            });
        }
    }
    if errors.is_empty() {
        Ok(rules)
    } else {
        errors.sort_by_key(|e| e.line);
        Err(errors)
    }
}

/// Parses `spec/retired-rules.txt`: one ID per line; blank lines and `#` comments are ignored.
pub fn parse_retired(text: &str) -> Result<HashSet<String>, Vec<SpecError>> {
    let mut ids = HashSet::new();
    let mut errors = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let entry = line.trim();
        if entry.is_empty() || entry.starts_with('#') {
            continue;
        }
        if is_valid_id(entry) {
            ids.insert(entry.to_string());
        } else {
            errors.push(SpecError {
                line: index + 1,
                message: format!("`{entry}` is not a valid rule ID"),
            });
        }
    }
    if errors.is_empty() {
        Ok(ids)
    } else {
        Err(errors)
    }
}

/// The spec with every tag and the space after it removed.
pub fn strip(spec: &str) -> String {
    let mut out = String::with_capacity(spec.len());
    let mut in_code_block = false;
    for chunk in spec.split_inclusive('\n') {
        if chunk.trim_start().starts_with("```") {
            in_code_block = !in_code_block;
        }
        if in_code_block {
            out.push_str(chunk);
            continue;
        }
        let mut copied_to = 0;
        for tag in find_tags(chunk, true) {
            if !tag.terminated || !is_valid_id(&tag.id) {
                continue;
            }
            out.push_str(&chunk[copied_to..tag.start]);
            copied_to = if chunk[tag.end..].starts_with(' ') {
                tag.end + 1
            } else {
                tag.end
            };
        }
        out.push_str(&chunk[copied_to..]);
    }
    out
}

/// Finds every tag and reports misplaced or malformed ones, without the uniqueness check.
pub fn parse(spec: &str) -> (Vec<Rule>, Vec<SpecError>) {
    let mut rules = Vec::new();
    let mut errors = Vec::new();
    let mut in_code_block = false;
    let mut section = String::new();
    for (index, line) in spec.lines().enumerate() {
        let line_no = index + 1;
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }
        let mut error = |message: String| {
            errors.push(SpecError {
                line: line_no,
                message,
            })
        };
        if in_code_block {
            // Code such as `xs[i]` is not a tag; only a well-formed ID is reported here.
            for tag in find_tags(line, false) {
                if tag.terminated && is_valid_id(&tag.id) {
                    error(format!(
                        "rule tag `r[{}]` is inside a code block; tags go before prose, never in code",
                        tag.id
                    ));
                }
            }
            continue;
        }
        if let Some(heading) = trimmed.strip_prefix("## ") {
            section = heading.trim().to_string();
        }
        let is_heading = trimmed.starts_with('#');
        for tag in find_tags(line, true) {
            if !tag.terminated {
                error("unterminated rule tag: `r[` has no closing `]`".to_string());
            } else if is_heading {
                error(format!(
                    "rule tag `r[{}]` is in a heading; put it before the sentence it covers",
                    tag.id
                ));
            } else if !is_valid_id(&tag.id) {
                error(format!(
                    "invalid rule ID `{}`: use 2 to 4 dot-separated segments of lowercase letters, \
                     digits and hyphens, such as `bind.shadow.same-scope`",
                    tag.id
                ));
            } else if !line[tag.end..].starts_with(' ') {
                error(format!(
                    "rule tag `r[{}]` must be followed by a space and the text it covers",
                    tag.id
                ));
            } else {
                rules.push(Rule {
                    id: tag.id,
                    line: line_no,
                    section: section.clone(),
                });
            }
        }
    }
    (rules, errors)
}

struct Tag {
    /// Byte offset of the `r`.
    start: usize,
    /// Byte offset just past the `]`, or the end of the line when unterminated.
    end: usize,
    id: String,
    terminated: bool,
}

/// Tag candidates on one line. With `skip_inline_code`, text inside backtick spans is ignored.
fn find_tags(line: &str, skip_inline_code: bool) -> Vec<Tag> {
    let bytes = line.as_bytes();
    let mut tags = Vec::new();
    let mut open_backticks = 0;
    let mut i = 0;
    while i < bytes.len() {
        if skip_inline_code && bytes[i] == b'`' {
            let run = bytes[i..].iter().take_while(|&&b| b == b'`').count();
            if open_backticks == 0 {
                open_backticks = run;
            } else if open_backticks == run {
                open_backticks = 0;
            }
            i += run;
            continue;
        }
        let starts_tag = open_backticks == 0
            && bytes[i..].starts_with(b"r[")
            && (i == 0 || bytes[i - 1].is_ascii_whitespace());
        if !starts_tag {
            i += 1;
            continue;
        }
        let id_start = i + 2;
        match line[id_start..].find(']') {
            Some(offset) => {
                let end = id_start + offset + 1;
                tags.push(Tag {
                    start: i,
                    end,
                    id: line[id_start..end - 1].to_string(),
                    terminated: true,
                });
                i = end;
            }
            None => {
                tags.push(Tag {
                    start: i,
                    end: line.len(),
                    id: line[id_start..].to_string(),
                    terminated: false,
                });
                i = line.len();
            }
        }
    }
    tags
}

#[cfg(test)]
mod tests {
    use super::*;

    fn messages(spec: &str) -> Vec<String> {
        match check(spec, &HashSet::new()) {
            Ok(_) => Vec::new(),
            Err(errors) => errors.into_iter().map(|e| e.to_string()).collect(),
        }
    }

    #[test]
    fn id_format() {
        for ok in [
            "bind.let",
            "bind.shadow.same-scope",
            "lit.int.neg",
            "a.b.c.d",
            "op.2x",
        ] {
            assert!(is_valid_id(ok), "{ok}");
        }
        for bad in [
            "bind",
            "Bind_Shadow",
            "bind.Shadow",
            "a.b.c.d.e",
            "bind..let",
            "bind.-let",
            "bind.let-",
            "bind.a--b",
            "",
        ] {
            assert!(!is_valid_id(bad), "{bad}");
        }
    }

    #[test]
    fn finds_tags_with_their_sections() {
        let spec = "## 2. Syntax\n\n- r[bind.let] `let` binds. r[bind.var] `var` too.\n\n## 3. Types\n\nr[ty.int] Int.\n";
        let rules = check(spec, &HashSet::new()).unwrap();
        let found: Vec<_> = rules
            .iter()
            .map(|r| (r.id.as_str(), r.line, r.section.as_str()))
            .collect();
        assert_eq!(
            found,
            [
                ("bind.let", 3, "2. Syntax"),
                ("bind.var", 3, "2. Syntax"),
                ("ty.int", 7, "3. Types"),
            ]
        );
    }

    #[test]
    fn table_cell_tags() {
        let spec = "| Integer | `42` | r[lit.int.default] Default type `Int`. |\n";
        assert_eq!(
            check(spec, &HashSet::new()).unwrap()[0].id,
            "lit.int.default"
        );
    }

    #[test]
    fn code_is_not_a_tag() {
        let spec = "Index with `r[i]` or `xs[i]`.\n```\nlet v = r[0]\nxs[i] = v\n```\n";
        assert_eq!(check(spec, &HashSet::new()).unwrap(), Vec::new());
    }

    #[test]
    fn tag_in_code_block() {
        assert_eq!(
            messages("```\n// r[bind.let] no\n```\n"),
            [
                "line 2: rule tag `r[bind.let]` is inside a code block; tags go before prose, never in code"
            ]
        );
    }

    #[test]
    fn tag_in_heading() {
        assert_eq!(
            messages("### r[bind.let] Bindings\n"),
            [
                "line 1: rule tag `r[bind.let]` is in a heading; put it before the sentence it covers"
            ]
        );
    }

    #[test]
    fn invalid_and_unterminated() {
        let errors = messages("r[Bind_Shadow] x\nr[bind] y\nr[bind.let\n");
        assert_eq!(errors.len(), 3, "{errors:?}");
        assert!(errors[0].starts_with("line 1: invalid rule ID `Bind_Shadow`"));
        assert!(errors[1].starts_with("line 2: invalid rule ID `bind`"));
        assert!(errors[2].starts_with("line 3: unterminated rule tag"));
    }

    #[test]
    fn tag_needs_a_space_after_it() {
        assert_eq!(
            messages("r[bind.let]`let` binds.\n"),
            ["line 1: rule tag `r[bind.let]` must be followed by a space and the text it covers"]
        );
    }

    #[test]
    fn duplicate_ids() {
        assert_eq!(
            messages("r[bind.let] a\n\nr[bind.let] b\n"),
            ["line 3: duplicate rule ID `bind.let`; it is first used on line 1"]
        );
    }

    #[test]
    fn retired_ids() {
        let retired = parse_retired("# retired\n\nbind.old\n").unwrap();
        let errors = check("r[bind.old] a\n", &retired).unwrap_err();
        assert_eq!(errors[0].line, 1);
        assert!(errors[0].message.contains("is retired"));
        let bad = parse_retired("ok.id\nNot An Id\n").unwrap_err();
        assert_eq!(bad[0].line, 2);
    }

    #[test]
    fn strip_removes_only_tags() {
        let untagged = "## 2. Syntax\n\n- `let` binds. `var` too.\n| A | B |\n```\nxs[i]\n```\nEnd";
        let tagged = "## 2. Syntax\n\n- r[bind.let] `let` binds. r[bind.var] `var` too.\n| A | r[lit.a] B |\n```\nxs[i]\n```\nEnd";
        assert_eq!(strip(tagged), untagged);
        assert_eq!(strip(untagged), untagged);
    }
}
