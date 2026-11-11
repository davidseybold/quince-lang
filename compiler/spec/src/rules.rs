//! Rule tags: `r[<id>]`, written directly before the sentence or sentences a rule covers.
//!
//! A tag is `r[` at the start of a line or after whitespace, outside code blocks and inline
//! code spans, closed by `]` and followed by one space. Tags never appear in headings. Text
//! shaped like a tag anywhere else is an error, so a misplaced tag can't silently drop out of
//! coverage. A backtick run with no closing run on the same line is literal text, as in
//! CommonMark.

use crate::manifest::{Chapter, LEGACY_PREFIX};
use crate::{CHAPTERS_PATH, ChapterFile, FileError, MOVED_PATH, SpecError};
use std::collections::{HashMap, HashSet};

/// The prefix reserved for IDs derived from grammar productions, such as `grammar.fn_decl`.
pub const GRAMMAR_PREFIX: &str = "grammar";

/// One tag found in a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub id: String,
    /// 1-based line of the tag.
    pub line: usize,
}

/// One tagged rule in the spec, with the chapter file that holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecRule {
    pub id: String,
    /// The chapter file, relative to the repo root.
    pub path: String,
    /// 1-based line of the tag.
    pub line: usize,
    /// The title of the chapter, from the manifest.
    pub chapter: String,
}

/// An ID is 2 to 4 dot-separated segments, each lowercase letters and digits joined by
/// single hyphens: `stmt.bind.shadow.same-scope`.
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

/// The first segment of an ID: the prefix of the chapter that holds the rule.
pub fn prefix(id: &str) -> &str {
    id.split('.').next().unwrap_or(id)
}

/// One ID segment: lowercase letters and digits joined by single hyphens.
pub fn is_valid_segment(segment: &str) -> bool {
    segment.split('-').all(|word| {
        !word.is_empty()
            && word
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    })
}

/// Parses every tag in every chapter file and checks format, placement, chapter, construct and
/// uniqueness across them. `chapters` is the manifest. Returns every rule found, even when there
/// are problems, and every problem.
pub fn check(
    files: &[ChapterFile],
    retired: &HashSet<String>,
    chapters: &[Chapter],
) -> (Vec<SpecRule>, Vec<FileError>) {
    let mut rules = Vec::new();
    let mut errors = Vec::new();
    let mut first_use: HashMap<String, (String, usize)> = HashMap::new();
    for file in files {
        let (found, mut file_errors) = parse(&file.text);
        for rule in &found {
            let mut error = |message: String| {
                file_errors.push(SpecError {
                    line: rule.line,
                    message,
                })
            };
            if let Some((path, line)) = first_use.get(&rule.id) {
                error(format!(
                    "duplicate rule ID `{}`; it is first used at {path}:{line}",
                    rule.id
                ));
            } else {
                first_use.insert(rule.id.clone(), (file.path.clone(), rule.line));
            }
            if retired.contains(&rule.id) {
                error(format!(
                    "rule ID `{}` is retired (spec/retired-rules.txt) and can't be reused; pick a new ID",
                    rule.id
                ));
            }
            if let Some(message) = chapter_problem(&rule.id, &file.prefix, chapters) {
                error(message);
            }
        }
        file_errors.sort_by_key(|e| e.line);
        errors.extend(
            file_errors
                .into_iter()
                .map(|e| FileError::new(&file.path, e)),
        );
        rules.extend(found.into_iter().map(|rule| SpecRule {
            id: rule.id,
            path: file.path.clone(),
            line: rule.line,
            chapter: file.title.clone(),
        }));
    }
    (rules, errors)
}

/// Why `id`, tagged in a file of the chapter `file_prefix`, doesn't fit the manifest, if it
/// doesn't. Its first segment must be that chapter's prefix (in the legacy file, any chapter's),
/// and in an ID of 3 or 4 segments the second must be a construct listed under that chapter.
fn chapter_problem(id: &str, file_prefix: &str, chapters: &[Chapter]) -> Option<String> {
    let first = prefix(id);
    if first == GRAMMAR_PREFIX {
        return Some(format!(
            "rule ID `{id}` uses the reserved prefix `grammar`, which belongs to IDs derived from \
             grammar productions; start it with its chapter's prefix"
        ));
    }
    if file_prefix != LEGACY_PREFIX && first != file_prefix {
        return Some(format!(
            "rule ID `{id}` starts with `{first}`, but this file holds chapter `{file_prefix}` \
             ({CHAPTERS_PATH}); start the ID with `{file_prefix}.`"
        ));
    }
    let Some(chapter) = chapters
        .iter()
        .find(|chapter| chapter.prefix == first && chapter.prefix != LEGACY_PREFIX)
    else {
        return Some(format!(
            "rule ID `{id}` starts with `{first}`, which isn't a chapter prefix in \
             {CHAPTERS_PATH}; start it with the prefix of the chapter the rule belongs to"
        ));
    };
    let segments: Vec<&str> = id.split('.').collect();
    let construct = segments[1];
    (segments.len() >= 3 && !chapter.constructs.iter().any(|c| c.name == construct)).then(|| {
        format!(
            "rule ID `{id}` names the construct `{construct}`, which chapter `{first}` doesn't list \
             in {CHAPTERS_PATH}; use a listed construct, or list this one under `{first}` with the \
             subject it covers"
        )
    })
}

/// Constructs listed in the manifest that no rule names as its second segment, as errors on
/// their lines in the manifest.
pub fn unused_constructs(rules: &[SpecRule], chapters: &[Chapter]) -> Vec<SpecError> {
    let used: HashSet<(&str, &str)> = rules
        .iter()
        .filter_map(|rule| {
            let segments: Vec<&str> = rule.id.split('.').collect();
            (segments.len() >= 3).then(|| (segments[0], segments[1]))
        })
        .collect();
    chapters
        .iter()
        .flat_map(|chapter| {
            chapter
                .constructs
                .iter()
                .filter(|c| !used.contains(&(chapter.prefix.as_str(), c.name.as_str())))
                .map(|c| SpecError {
                    line: c.line,
                    message: format!(
                        "construct `{}` of chapter `{}` is listed, but no rule ID names it; remove it",
                        c.name, chapter.prefix
                    ),
                })
        })
        .collect()
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

/// Rules that moved chapter, from `spec/moved-rules.txt`: each old ID, with the ID it moved to
/// and the 1-based line that lists it.
pub type Moved = HashMap<String, (String, usize)>;

/// Parses `spec/moved-rules.txt`: an old ID and the ID it moved to on each line, separated by
/// whitespace; blank lines and `#` comments are ignored. Returns the valid lines and an error
/// for each bad one.
pub fn parse_moved(text: &str) -> (Moved, Vec<SpecError>) {
    let mut moved = Moved::new();
    let mut errors = Vec::new();
    for (line, entry) in entries(text) {
        let words: Vec<&str> = entry.split_whitespace().collect();
        let problem = match words[..] {
            [old, new] => {
                if let Some(bad) = [old, new].into_iter().find(|id| !is_valid_id(id)) {
                    Some(format!("`{bad}` is not a valid rule ID"))
                } else if [old, new]
                    .into_iter()
                    .any(|id| prefix(id) == GRAMMAR_PREFIX)
                {
                    Some(
                        "grammar IDs never move, because they follow their production; list only tagged rules"
                            .to_string(),
                    )
                } else if old == new {
                    Some(format!(
                        "`{old}` moves to itself; list a rule only when its ID changed"
                    ))
                } else if let Some((_, first)) = moved.get(old) {
                    Some(format!(
                        "`{old}` is listed twice; it is first listed on line {first}"
                    ))
                } else {
                    moved.insert(old.to_string(), (new.to_string(), line));
                    None
                }
            }
            _ => Some(format!(
                "`{entry}` should be the old rule ID, then the ID it moved to, such as \
                 `stmt.loop.fanout-sources  conc.fanout.sources`"
            )),
        };
        if let Some(message) = problem {
            errors.push(SpecError { line, message });
        }
    }
    (moved, errors)
}

/// Checks `moved` against the spec's tags. A moved ID is never a tag again, and the ID it moved
/// to is a tag or a retired ID, never one that moved too. A moved ID that is tagged is reported
/// at the tag; every other problem is on its line of `spec/moved-rules.txt`.
pub fn check_moved(moved: &Moved, rules: &[SpecRule], retired: &HashSet<String>) -> Vec<FileError> {
    let tags: HashMap<&str, &SpecRule> =
        rules.iter().map(|rule| (rule.id.as_str(), rule)).collect();
    let mut listed: Vec<_> = moved.iter().collect();
    listed.sort_by_key(|(_, (_, line))| *line);
    let mut errors = Vec::new();
    for (old, (new, line)) in listed {
        if let Some(rule) = tags.get(old.as_str()) {
            errors.push(FileError {
                path: rule.path.clone(),
                line: rule.line,
                message: format!(
                    "rule ID `{old}` moved to `{new}` ({MOVED_PATH}) and can't be a tag again; pick a new ID"
                ),
            });
        }
        let problem = if let Some((newer, _)) = moved.get(new) {
            Some(format!(
                "`{old}` moved to `{new}`, which moved again to `{newer}`; point `{old}` at the final ID"
            ))
        } else if !tags.contains_key(new.as_str()) && !retired.contains(new) {
            Some(format!(
                "`{old}` moved to `{new}`, which is neither a rule tag nor retired; give the rule's current ID"
            ))
        } else {
            None
        };
        if let Some(message) = problem {
            errors.push(FileError {
                path: MOVED_PATH.to_string(),
                line: *line,
                message,
            });
        }
    }
    errors
}

/// The non-blank, non-comment lines of a list file, trimmed, with their 1-based line numbers.
fn entries(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.lines()
        .enumerate()
        .map(|(index, line)| (index + 1, line.trim()))
        .filter(|(_, entry)| !entry.is_empty() && !entry.starts_with('#'))
}

/// A chapter file with every tag and the space after it removed.
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
        for tag in tag_spans(chunk) {
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

/// A well-formed tag on a prose line: `r[<id>]` from byte `start` up to `end`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagSpan {
    pub id: String,
    pub start: usize,
    /// Just past the `]`.
    pub end: usize,
}

/// The well-formed tags on one prose line, outside inline code spans, in order. Misplaced or
/// malformed tag-shaped text is skipped; `parse` reports it.
pub fn tag_spans(line: &str) -> Vec<TagSpan> {
    find_tags(line, true)
        .into_iter()
        .filter(|tag| !tag.glued && tag.terminated && is_valid_id(&tag.id))
        .map(|tag| TagSpan {
            id: tag.id,
            start: tag.start,
            end: tag.end,
        })
        .collect()
}

/// Finds every tag in one file and reports misplaced or malformed ones, without the uniqueness
/// and chapter checks.
pub fn parse(spec: &str) -> (Vec<Rule>, Vec<SpecError>) {
    let mut rules = Vec::new();
    let mut errors = Vec::new();
    let mut in_code_block = false;
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
                     digits and hyphens, such as `stmt.bind.shadow.same-scope`",
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

    /// A manifest for the tests. The legacy file may hold any chapter's tags.
    fn chapters() -> Vec<Chapter> {
        let (chapters, errors) = crate::manifest::parse(
            "bind  reference/bindings.md  Bindings\n\
             lit  reference/literals.md  Literals\n\
             \x20 int  Integer literals\n\
             \x20 float  Float literals\n\
             type  reference/types.md  Types\n\
             legacy  quince-spec.md  Being split\n",
        );
        assert_eq!(errors, Vec::new());
        chapters
    }

    fn file(prefix: &str, path: &str, title: &str, text: &str) -> ChapterFile {
        ChapterFile {
            prefix: prefix.into(),
            title: title.into(),
            path: path.into(),
            file: path.into(),
            text: text.into(),
        }
    }

    fn check_one(spec: &str, retired: &HashSet<String>) -> (Vec<SpecRule>, Vec<FileError>) {
        check(
            &[file(LEGACY_PREFIX, "spec.md", "Spec", spec)],
            retired,
            &chapters(),
        )
    }

    fn rules(spec: &str) -> Vec<SpecRule> {
        let (rules, errors) = check_one(spec, &HashSet::new());
        assert_eq!(errors, Vec::new());
        rules
    }

    /// Each problem as `line N: message`.
    fn messages(spec: &str) -> Vec<String> {
        check_one(spec, &HashSet::new())
            .1
            .into_iter()
            .map(|e| format!("line {}: {}", e.line, e.message))
            .collect()
    }

    #[test]
    fn id_format() {
        for ok in [
            "bind.let",
            "stmt.bind.shadow.same-scope",
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
    fn finds_tags_with_their_chapters() {
        let files = [
            file(
                "bind",
                "spec/reference/a.md",
                "Syntax",
                "# Syntax\n\n- r[bind.let] `let` binds. r[bind.var] `var` too.\n",
            ),
            file(
                "type",
                "spec/reference/b.md",
                "Types",
                "# Types\n\nr[type.int] Int.\n",
            ),
        ];
        let (found, errors) = check(&files, &HashSet::new(), &chapters());
        assert_eq!(errors, Vec::new());
        let found: Vec<_> = found
            .into_iter()
            .map(|r| (r.id, r.path, r.line, r.chapter))
            .collect();
        let expected = [
            ("bind.let", "spec/reference/a.md", 3, "Syntax"),
            ("bind.var", "spec/reference/a.md", 3, "Syntax"),
            ("type.int", "spec/reference/b.md", 3, "Types"),
        ]
        .map(|(id, path, line, chapter)| {
            (id.to_string(), path.to_string(), line, chapter.to_string())
        });
        assert_eq!(found, expected);
    }

    #[test]
    fn errors_name_their_file() {
        let files = [
            file(LEGACY_PREFIX, "spec/a.md", "A", "r[bind.let] a\n"),
            file(
                LEGACY_PREFIX,
                "spec/b.md",
                "B",
                "\nr[bind.let] b\nr[nope.x] c\n",
            ),
        ];
        let (found, errors) = check(&files, &HashSet::new(), &chapters());
        assert_eq!(found.len(), 3);
        let errors: Vec<_> = errors.into_iter().map(|e| e.to_string()).collect();
        assert_eq!(errors.len(), 2, "{errors:?}");
        assert_eq!(
            errors[0],
            "spec/b.md:2: duplicate rule ID `bind.let`; it is first used at spec/a.md:1"
        );
        assert!(errors[1].starts_with("spec/b.md:3: rule ID `nope.x` starts with `nope`"));
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
            ["line 3: duplicate rule ID `bind.let`; it is first used at spec.md:1"]
        );
    }

    #[test]
    fn retired_ids() {
        let (retired, errors) = parse_retired("# retired\n\nbind.old\ngrammar.old_arg\n");
        assert!(errors.is_empty(), "{errors:?}");
        assert!(retired.contains("grammar.old_arg"));
        let (_, errors) = check_one("r[bind.old] a\n", &retired);
        assert_eq!(errors[0].line, 1);
        assert!(errors[0].message.contains("is retired"));
        let (ok, bad) = parse_retired("ok.id\nNot An Id\n");
        assert!(ok.contains("ok.id"));
        assert_eq!(bad[0].line, 2);
    }

    #[test]
    fn moved_file() {
        let (moved, errors) = parse_moved(
            "# moved\n\nbind.a  type.a\nbind.b type.b extra\nbind.c\nBad.Id type.c\n\
             grammar.block type.d\nbind.e bind.e\nbind.a type.f\n",
        );
        assert_eq!(
            moved,
            Moved::from([("bind.a".to_string(), ("type.a".to_string(), 3))])
        );
        let errors: Vec<_> = errors.into_iter().map(|e| e.to_string()).collect();
        assert_eq!(
            errors,
            [
                "line 4: `bind.b type.b extra` should be the old rule ID, then the ID it moved to, such as `stmt.loop.fanout-sources  conc.fanout.sources`",
                "line 5: `bind.c` should be the old rule ID, then the ID it moved to, such as `stmt.loop.fanout-sources  conc.fanout.sources`",
                "line 6: `Bad.Id` is not a valid rule ID",
                "line 7: grammar IDs never move, because they follow their production; list only tagged rules",
                "line 8: `bind.e` moves to itself; list a rule only when its ID changed",
                "line 9: `bind.a` is listed twice; it is first listed on line 3",
            ]
        );
    }

    #[test]
    fn moved_ids_point_at_live_or_retired_rules() {
        let (moved, errors) = parse_moved(
            "bind.let  type.let\nbind.old  type.old\nbind.gone  type.gone\nbind.twice  bind.let\n\
             bind.var  type.var\n",
        );
        assert_eq!(errors, Vec::new());
        let found = rules("r[type.let] a\nr[bind.var] b\nr[type.var] c\n");
        let retired = HashSet::from(["type.old".to_string()]);
        let errors: Vec<_> = check_moved(&moved, &found, &retired)
            .into_iter()
            .map(|e| e.to_string())
            .collect();
        assert_eq!(
            errors,
            [
                "spec/moved-rules.txt:3: `bind.gone` moved to `type.gone`, which is neither a rule tag nor retired; give the rule's current ID",
                "spec/moved-rules.txt:4: `bind.twice` moved to `bind.let`, which moved again to `type.let`; point `bind.twice` at the final ID",
                "spec.md:2: rule ID `bind.var` moved to `type.var` (spec/moved-rules.txt) and can't be a tag again; pick a new ID",
            ]
        );
    }

    #[test]
    fn legacy_tags_need_a_chapter_prefix() {
        assert_eq!(
            messages("r[types.nominal] a\nr[grammar.block] b\nr[legacy.x] c\nr[type.int] d\n"),
            [
                "line 1: rule ID `types.nominal` starts with `types`, which isn't a chapter prefix \
                 in spec/chapters.txt; start it with the prefix of the chapter the rule belongs to",
                "line 2: rule ID `grammar.block` uses the reserved prefix `grammar`, which belongs \
                 to IDs derived from grammar productions; start it with its chapter's prefix",
                "line 3: rule ID `legacy.x` starts with `legacy`, which isn't a chapter prefix in \
                 spec/chapters.txt; start it with the prefix of the chapter the rule belongs to",
            ]
        );
    }

    #[test]
    fn chapter_tags_need_their_chapters_prefix() {
        let files = [file(
            "type",
            "spec/reference/types.md",
            "Types",
            "r[type.int] a\nr[bind.let] b\nr[nope.x] c\n",
        )];
        let (_, errors) = check(&files, &HashSet::new(), &chapters());
        let errors: Vec<_> = errors.into_iter().map(|e| e.to_string()).collect();
        let wrong = |id: &str, first: &str| {
            format!(
                "rule ID `{id}` starts with `{first}`, but this file holds chapter `type` \
                 (spec/chapters.txt); start the ID with `type.`"
            )
        };
        assert_eq!(
            errors,
            [
                format!("spec/reference/types.md:2: {}", wrong("bind.let", "bind")),
                format!("spec/reference/types.md:3: {}", wrong("nope.x", "nope")),
            ]
        );
    }

    #[test]
    fn constructs_must_be_listed() {
        assert_eq!(
            messages(
                "r[lit.int.hex] a\nr[lit.float] b\nr[lit.str.raw] c\nr[lit.int.hex.upper] d\n"
            ),
            [
                "line 3: rule ID `lit.str.raw` names the construct `str`, which chapter `lit` \
                 doesn't list in spec/chapters.txt; use a listed construct, or list this one \
                 under `lit` with the subject it covers"
            ]
        );
    }

    #[test]
    fn unused_construct() {
        let found = rules("r[lit.int.hex] a\nr[lit.float] b\n");
        let errors = unused_constructs(&found, &chapters());
        assert_eq!(
            errors,
            [SpecError {
                line: 4,
                message: "construct `float` of chapter `lit` is listed, but no rule ID names it; remove it".into()
            }]
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
