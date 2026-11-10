//! Rule tags: `r[<id>]`, written directly before the sentence or sentences a rule covers.
//!
//! A tag is `r[` at the start of a line or after whitespace, outside code blocks and inline
//! code spans, closed by `]` and followed by one space. Tags never appear in headings. Text
//! shaped like a tag anywhere else is an error, so a misplaced tag can't silently drop out of
//! coverage. A backtick run with no closing run on the same line is literal text, as in
//! CommonMark.

use crate::SpecError;
use std::collections::{HashMap, HashSet};

/// The topic reserved for IDs derived from grammar productions, such as `grammar.fn_decl`.
pub const GRAMMAR_TOPIC: &str = "grammar";

/// One tagged rule in the spec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub id: String,
    /// 1-based line of the tag.
    pub line: usize,
    /// The `## ` heading the tag sits under, such as `2. Lexical structure and syntax`.
    pub section: String,
}

/// The topics listed in `spec/rule-topics.txt`, each with its 1-based line in that file.
pub type Topics = HashMap<String, usize>;

/// An ID is 2 to 4 dot-separated segments, each lowercase letters and digits joined by
/// single hyphens: `bind.shadow.same-scope`.
pub fn is_valid_id(id: &str) -> bool {
    let segments: Vec<&str> = id.split('.').collect();
    (2..=4).contains(&segments.len()) && segments.iter().all(|s| is_valid_segment(s))
}

/// `grammar.<production>`, where the production name is a grammar identifier such as
/// `fn_decl`.
pub fn is_grammar_id(id: &str) -> bool {
    id.strip_prefix("grammar.").is_some_and(|name| {
        name.starts_with(|c: char| c.is_ascii_alphabetic())
            && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
    })
}

/// The first segment of an ID.
pub fn topic(id: &str) -> &str {
    id.split('.').next().unwrap_or(id)
}

fn is_valid_segment(segment: &str) -> bool {
    segment.split('-').all(|word| {
        !word.is_empty()
            && word
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    })
}

/// Parses every tag in the spec and checks format, placement, topics and uniqueness. Returns
/// every rule, or every problem found.
pub fn check(
    spec: &str,
    retired: &HashSet<String>,
    topics: &Topics,
) -> Result<Vec<Rule>, Vec<SpecError>> {
    let (rules, mut errors) = parse(spec);
    let mut first_line: HashMap<&str, usize> = HashMap::new();
    for rule in &rules {
        let mut error = |message: String| {
            errors.push(SpecError {
                line: rule.line,
                message,
            })
        };
        if let Some(first) = first_line.get(rule.id.as_str()) {
            error(format!(
                "duplicate rule ID `{}`; it is first used on line {first}",
                rule.id
            ));
        } else {
            first_line.insert(&rule.id, rule.line);
        }
        if retired.contains(&rule.id) {
            error(format!(
                "rule ID `{}` is retired (spec/retired-rules.txt) and can't be reused; pick a new ID",
                rule.id
            ));
        }
        let topic = topic(&rule.id);
        if topic == GRAMMAR_TOPIC {
            error(format!(
                "rule ID `{}` uses the reserved topic `grammar`, which belongs to IDs derived \
                 from grammar productions; pick another topic",
                rule.id
            ));
        } else if !topics.contains_key(topic) {
            error(format!(
                "rule ID `{}` has the topic `{topic}`, which spec/rule-topics.txt doesn't list; \
                 reuse a listed topic, or add this one there with the subject it covers",
                rule.id
            ));
        }
    }
    if errors.is_empty() {
        Ok(rules)
    } else {
        errors.sort_by_key(|e| e.line);
        Err(errors)
    }
}

/// Listed topics that no rule uses, as errors on their lines in `spec/rule-topics.txt`.
pub fn unused_topics(rules: &[Rule], topics: &Topics) -> Vec<SpecError> {
    let used: HashSet<&str> = rules.iter().map(|rule| topic(&rule.id)).collect();
    let mut errors: Vec<SpecError> = topics
        .iter()
        .filter(|(topic, _)| !used.contains(topic.as_str()))
        .map(|(topic, &line)| SpecError {
            line,
            message: format!("topic `{topic}` is listed, but no rule tag uses it; remove it"),
        })
        .collect();
    errors.sort_by_key(|e| e.line);
    errors
}

/// Parses `spec/retired-rules.txt`: one ID per line; blank lines and `#` comments are ignored.
/// Grammar IDs such as `grammar.old_arg` are allowed. Returns the valid IDs and an error for
/// each invalid line.
pub fn parse_retired(text: &str) -> (HashSet<String>, Vec<SpecError>) {
    let mut ids = HashSet::new();
    let mut errors = Vec::new();
    for (line, entry) in entries(text) {
        if is_valid_id(entry) || is_grammar_id(entry) {
            ids.insert(entry.to_string());
        } else {
            errors.push(SpecError {
                line,
                message: format!("`{entry}` is not a valid rule ID"),
            });
        }
    }
    (ids, errors)
}

/// Parses `spec/rule-topics.txt`: a topic and the subject it covers on each line; blank lines
/// and `#` comments are ignored. Returns the valid topics and an error for each bad line.
pub fn parse_topics(text: &str) -> (Topics, Vec<SpecError>) {
    let mut topics = Topics::new();
    let mut errors = Vec::new();
    for (line, entry) in entries(text) {
        let (topic, subject) = entry
            .split_once(char::is_whitespace)
            .map_or((entry, ""), |(topic, subject)| (topic, subject.trim()));
        let problem = if !is_valid_segment(topic) {
            Some(format!(
                "`{topic}` is not a valid topic: use lowercase letters and digits joined by single hyphens"
            ))
        } else if topic == GRAMMAR_TOPIC {
            Some("`grammar` is reserved for IDs derived from grammar productions".to_string())
        } else if subject.is_empty() {
            Some(format!(
                "topic `{topic}` needs the subject it covers after it, such as `bind  Bindings`"
            ))
        } else if let Some(first) = topics.get(topic) {
            Some(format!(
                "topic `{topic}` is listed twice; it is first listed on line {first}"
            ))
        } else {
            topics.insert(topic.to_string(), line);
            None
        };
        if let Some(message) = problem {
            errors.push(SpecError { line, message });
        }
    }
    (topics, errors)
}

/// The non-blank, non-comment lines of a list file, trimmed, with their 1-based line numbers.
fn entries(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.lines()
        .enumerate()
        .map(|(index, line)| (index + 1, line.trim()))
        .filter(|(_, entry)| !entry.is_empty() && !entry.starts_with('#'))
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
            if tag.glued || !tag.terminated || !is_valid_id(&tag.id) {
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

/// Finds every tag and reports misplaced or malformed ones, without the uniqueness and topic
/// checks.
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
                // Indexing such as `counter[user.id]` is glued, so it isn't a tag either.
                if !tag.glued && tag.terminated && is_valid_id(&tag.id) {
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
            if tag.glued {
                error(format!(
                    "rule tag `r[{}]` must start a line or follow a space; move it out of any \
                     markup or punctuation, directly before the sentence it covers (or, if this \
                     is code, put it in backticks)",
                    tag.id
                ));
            } else if !tag.terminated {
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
    /// Preceded by something other than whitespace, as in `(r[x.y]`. Only reported when the
    /// text inside is a valid ID, so code-like prose such as `arr[i]` isn't flagged.
    glued: bool,
}

/// Tag candidates on one line. With `skip_inline_code`, text inside backtick spans is ignored.
fn find_tags(line: &str, skip_inline_code: bool) -> Vec<Tag> {
    let bytes = line.as_bytes();
    let in_code = if skip_inline_code {
        inline_code_mask(line)
    } else {
        vec![false; bytes.len()]
    };
    let mut tags = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if in_code[i] || !bytes[i..].starts_with(b"r[") {
            i += 1;
            continue;
        }
        let glued = i > 0 && !bytes[i - 1].is_ascii_whitespace();
        let id_start = i + 2;
        let tag = match line[id_start..].find(']') {
            Some(offset) => Tag {
                start: i,
                end: id_start + offset + 1,
                id: line[id_start..id_start + offset].to_string(),
                terminated: true,
                glued,
            },
            None => Tag {
                start: i,
                end: line.len(),
                id: line[id_start..].to_string(),
                terminated: false,
                glued,
            },
        };
        if glued && !(tag.terminated && is_valid_id(&tag.id)) {
            i += 1;
            continue;
        }
        i = tag.end;
        tags.push(tag);
    }
    tags
}

/// Marks the bytes inside inline code spans. A span runs from a backtick run to the next run
/// of the same length; a run with no closing run on the line is literal text.
fn inline_code_mask(line: &str) -> Vec<bool> {
    let bytes = line.as_bytes();
    let run_at = |at: usize| bytes[at..].iter().take_while(|&&b| b == b'`').count();
    let mut mask = vec![false; bytes.len()];
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'`' {
            i += 1;
            continue;
        }
        let run = run_at(i);
        let mut j = i + run;
        let mut close = None;
        while j < bytes.len() {
            if bytes[j] != b'`' {
                j += 1;
                continue;
            }
            let other = run_at(j);
            if other == run {
                close = Some(j);
                break;
            }
            j += other;
        }
        match close {
            Some(j) => {
                mask[i..j + run].fill(true);
                i = j + run;
            }
            None => i += run,
        }
    }
    mask
}

#[cfg(test)]
mod tests {
    use super::*;

    fn topics(names: &[&str]) -> Topics {
        names
            .iter()
            .enumerate()
            .map(|(i, name)| (name.to_string(), i + 1))
            .collect()
    }

    fn test_topics() -> Topics {
        topics(&["bind", "lit", "type", "a", "ok"])
    }

    fn rules(spec: &str) -> Vec<Rule> {
        check(spec, &HashSet::new(), &test_topics()).unwrap()
    }

    fn messages(spec: &str) -> Vec<String> {
        match check(spec, &HashSet::new(), &test_topics()) {
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
        assert!(is_grammar_id("grammar.fn_decl"));
        assert!(!is_grammar_id("grammar._x") && !is_grammar_id("grammar."));
    }

    #[test]
    fn finds_tags_with_their_sections() {
        let spec = "## 2. Syntax\n\n- r[bind.let] `let` binds. r[bind.var] `var` too.\n\n## 3. Types\n\nr[type.int] Int.\n";
        let found: Vec<_> = rules(spec)
            .into_iter()
            .map(|r| (r.id, r.line, r.section))
            .collect();
        let expected = [
            ("bind.let", 3, "2. Syntax"),
            ("bind.var", 3, "2. Syntax"),
            ("type.int", 7, "3. Types"),
        ]
        .map(|(id, line, section)| (id.to_string(), line, section.to_string()));
        assert_eq!(found, expected);
    }

    #[test]
    fn table_cell_tags() {
        let spec = "| Integer | `42` | r[lit.int.default] Default type `Int`. |\n";
        assert_eq!(rules(spec)[0].id, "lit.int.default");
    }

    #[test]
    fn code_is_not_a_tag() {
        let spec = "Index with `r[i]` or `xs[i]`, or arr[i].\n```\nlet v = r[0]\nxs[i] = v\nlet n = counter[user.id]\nheader[key.name] = v\n```\n";
        assert_eq!(rules(spec), Vec::new());
    }

    #[test]
    fn unclosed_backtick_is_literal() {
        let spec = "Use ` here r[bind.let] x.\nA ``double` run r[bind.var] y.\n";
        let ids: Vec<_> = rules(spec).into_iter().map(|r| r.id).collect();
        assert_eq!(ids, ["bind.let", "bind.var"]);
    }

    #[test]
    fn glued_tags_are_errors() {
        let errors = messages("(r[bind.var] y)\n**r[bind.let]** z\n");
        assert_eq!(errors.len(), 2, "{errors:?}");
        assert!(
            errors[0]
                .starts_with("line 1: rule tag `r[bind.var]` must start a line or follow a space"),
            "{errors:?}"
        );
        assert!(errors[1].starts_with("line 2: rule tag `r[bind.let]`"));
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
        let (retired, errors) = parse_retired("# retired\n\nbind.old\ngrammar.old_arg\n");
        assert!(errors.is_empty(), "{errors:?}");
        assert!(retired.contains("grammar.old_arg"));
        let errors = check("r[bind.old] a\n", &retired, &test_topics()).unwrap_err();
        assert_eq!(errors[0].line, 1);
        assert!(errors[0].message.contains("is retired"));
        let (ok, bad) = parse_retired("ok.id\nNot An Id\n");
        assert!(ok.contains("ok.id"));
        assert_eq!(bad[0].line, 2);
    }

    #[test]
    fn topics_must_be_listed() {
        let errors = messages("r[types.nominal] a\nr[grammar.block] b\n");
        assert_eq!(errors.len(), 2, "{errors:?}");
        assert!(
            errors[0].contains("topic `types`, which spec/rule-topics.txt doesn't list"),
            "{errors:?}"
        );
        assert!(errors[1].contains("reserved topic `grammar`"), "{errors:?}");
    }

    #[test]
    fn topic_file() {
        let (listed, errors) = parse_topics(
            "# topics\n\nbind  Bindings\nlit Literals\nBad_Topic x\nnone\ngrammar Grammar\nbind again\n",
        );
        assert_eq!(
            listed,
            Topics::from([("bind".to_string(), 3), ("lit".to_string(), 4)])
        );
        let lines: Vec<_> = errors.iter().map(|e| e.line).collect();
        assert_eq!(lines, [5, 6, 7, 8], "{errors:?}");
        assert!(errors[1].message.contains("needs the subject"));
        assert!(
            errors[3]
                .message
                .contains("listed twice; it is first listed on line 3")
        );
    }

    #[test]
    fn unused_topic() {
        let found = rules("r[bind.let] a\n");
        let errors = unused_topics(&found, &topics(&["bind", "lit"]));
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].line, 2);
        assert!(
            errors[0]
                .message
                .contains("`lit` is listed, but no rule tag uses it")
        );
    }

    #[test]
    fn strip_removes_only_tags() {
        let untagged = "## 2. Syntax\n\n- `let` binds. `var` too.\n| A | B |\n```\nxs[i]\n```\n(r[bind.x] kept) End";
        let tagged = "## 2. Syntax\n\n- r[bind.let] `let` binds. r[bind.var] `var` too.\n| A | r[lit.a] B |\n```\nxs[i]\n```\n(r[bind.x] kept) End";
        assert_eq!(strip(tagged), untagged);
        assert_eq!(strip(untagged), untagged);
    }
}
