//! Rule tags: `r[<id>]` on a line of its own, directly above the paragraph or top-level `-` or
//! `*` list item it covers, as in the Rust Reference.
//!
//! A tag line holds only the tag, at column 0, and follows a blank line, a heading or the start
//! of the file. It covers exactly the block after it: the paragraph, or the item's own text and
//! its lazy continuation lines. `r[` anywhere else outside code blocks and inline code spans is
//! an error, so a misplaced tag can't silently drop out of coverage. A backtick run with no
//! closing run on the same line is literal text, as in CommonMark.

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

/// A chapter file with every tag line removed.
pub fn strip(spec: &str) -> String {
    let mut out = String::with_capacity(spec.len());
    let mut in_code_block = false;
    for chunk in spec.split_inclusive('\n') {
        if is_fence(chunk) {
            in_code_block = !in_code_block;
        }
        if in_code_block || tag_line(chunk).is_none() {
            out.push_str(chunk);
        }
    }
    out
}

/// The ID of a tag line: a line holding only `r[<id>]` from column 0, perhaps with trailing
/// whitespace. The ID isn't checked.
fn tag_line(line: &str) -> Option<&str> {
    let inner = line.trim_end().strip_prefix("r[")?.strip_suffix(']')?;
    (!inner.contains(']')).then_some(inner)
}

/// Tag-shaped text on one prose line: `r[` at the start of the line or after whitespace, or
/// glued to other text around a valid ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub id: String,
    /// Byte offset of the `r`.
    pub start: usize,
    /// Byte offset just past the `]`, or the end of the line when unterminated.
    pub end: usize,
    pub terminated: bool,
    /// Preceded by something other than whitespace, as in `(r[x.y]`. Only found when the text
    /// inside is a valid ID, so code-like prose such as `arr[i]` isn't flagged.
    pub glued: bool,
}

/// The tag candidates on one prose line, outside inline code spans, in order.
pub fn candidates(line: &str) -> Vec<Candidate> {
    find_tags(line, true)
}

/// Finds every tag in one file and reports misplaced or malformed ones, without the uniqueness
/// and chapter checks.
pub fn parse(spec: &str) -> (Vec<Rule>, Vec<SpecError>) {
    let lines: Vec<&str> = spec.lines().collect();
    let mut rules = Vec::new();
    let mut errors = Vec::new();
    let mut in_code_block = false;
    for (index, &line) in lines.iter().enumerate() {
        let line_no = index + 1;
        if is_fence(line) {
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
                        "rule tag `r[{}]` is inside a code block; tags go on their own line above \
                         prose, never in code",
                        tag.id
                    ));
                }
            }
            continue;
        }
        if let Some(id) = tag_line(line) {
            if !is_valid_id(id) {
                error(invalid_id(id));
            } else if let Some(problem) = placement_problem(&lines, index) {
                error(format!("rule tag `r[{id}]` {problem}"));
            } else {
                rules.push(Rule {
                    id: id.to_string(),
                    line: line_no,
                });
            }
            continue;
        }
        let in_heading = is_heading(line);
        let indented_tag = tag_line(line.trim_start()).is_some();
        for tag in find_tags(line, true) {
            error(if !tag.terminated {
                "unterminated rule tag: `r[` has no closing `]`".to_string()
            } else if !is_valid_id(&tag.id) {
                invalid_id(&tag.id)
            } else if in_heading {
                format!(
                    "rule tag `r[{}]` is in a heading; put it on its own line below the heading, \
                     directly above the paragraph or item it covers",
                    tag.id
                )
            } else if indented_tag {
                format!(
                    "rule tag `r[{}]` is indented; a tag line starts at column 0, and a nested \
                     block can't be tagged: make the rule a top-level paragraph or item",
                    tag.id
                )
            } else {
                format!(
                    "rule tag `r[{}]` must be on a line of its own, directly above the paragraph \
                     or top-level `-` or `*` item it covers; tags don't go inside paragraphs, \
                     list items or table cells (if this is code, put it in backticks)",
                    tag.id
                )
            });
        }
    }
    (rules, errors)
}

fn invalid_id(id: &str) -> String {
    format!(
        "invalid rule ID `{id}`: use 2 to 4 dot-separated segments of lowercase letters, digits \
         and hyphens, such as `stmt.bind.shadow.same-scope`"
    )
}

/// What is wrong with where the tag line `lines[index]` sits, if anything, worded to follow
/// "rule tag `r[id]`".
fn placement_problem(lines: &[&str], index: usize) -> Option<String> {
    let follows_break = index == 0 || {
        let before = lines[index - 1];
        before.trim().is_empty() || is_heading(before)
    };
    if !follows_break {
        return Some(
            "must follow a blank line, a heading or the start of the file; put a blank line \
             before it"
                .to_string(),
        );
    }
    let cover = "put the paragraph or top-level `-` or `*` item it covers on the next line";
    let Some(&next) = lines.get(index + 1) else {
        return Some(format!("ends the file and covers nothing; {cover}"));
    };
    let uncoverable = [
        (next.trim().is_empty(), "a blank line"),
        (tag_line(next).is_some(), "another tag line"),
        (next.starts_with(char::is_whitespace), "an indented line"),
        (is_heading(next), "a heading"),
        (is_fence(next), "a code block"),
        (next.starts_with('|'), "a table"),
        (is_thematic_break(next), "a thematic break"),
        (next.starts_with('>'), "a blockquote"),
        (next.starts_with('<'), "an HTML block"),
        (is_numbered_item(next), "a numbered item"),
        (next.starts_with("+ "), "a `+` item (use `-` or `*`)"),
    ];
    if let Some((_, what)) = uncoverable.iter().find(|(found, _)| *found) {
        return Some(format!(
            "is followed by {what}, which it can't cover; {cover}"
        ));
    }
    let item = next.starts_with("- ") || next.starts_with("* ");
    // A paragraph's lines, tag line included, become a heading over a setext underline.
    let mut end = index + 1;
    while end < lines.len() && !lines[end].trim().is_empty() {
        let line = lines[end];
        if !item && is_setext_underline(line) {
            return Some(format!(
                "covers a paragraph with the setext underline `{}` on line {}, which makes the \
                 paragraph and the tag a heading; put a blank line before the underline, or make \
                 it a `#` heading",
                line.trim(),
                end + 1
            ));
        }
        if end > index + 1 && is_block_start(line) {
            break;
        }
        end += 1;
    }
    // An item's own text ends at a blank line or a block start. The next non-blank line is
    // nested in the item if it's indented, whether or not a blank line comes before it.
    let nested = lines[end.min(lines.len())..]
        .iter()
        .position(|line| !line.trim().is_empty())
        .map(|offset| end + offset)
        .filter(|&at| item && lines[at].starts_with(char::is_whitespace));
    if let Some(at) = nested {
        let line_no = at + 1;
        return Some(format!(
            "covers a list item with a nested block (line {line_no}); a rule never extends into \
             nested blocks, so make that content a top-level block or join it to the item"
        ));
    }
    None
}

fn is_fence(line: &str) -> bool {
    line.trim_start().starts_with("```")
}

/// An ATX heading: one to six `#` and then a space or the end of the line.
fn is_heading(line: &str) -> bool {
    let hashes = line.len() - line.trim_start_matches('#').len();
    (1..=6).contains(&hashes)
        && line[hashes..]
            .chars()
            .next()
            .is_none_or(char::is_whitespace)
}

/// Three or more `*`, `-` or `_`, all the same, with only spaces between: `***`, `- - -`.
fn is_thematic_break(line: &str) -> bool {
    let marks: Vec<char> = line.chars().filter(|c| !c.is_whitespace()).collect();
    marks.len() >= 3 && matches!(marks[0], '*' | '-' | '_') && marks.iter().all(|&c| c == marks[0])
}

/// A line of only `=` or only `-`, with at most three spaces before it, which turns the
/// paragraph above it into a heading.
fn is_setext_underline(line: &str) -> bool {
    let indent = line.len() - line.trim_start_matches(' ').len();
    let marks = line.trim();
    indent <= 3
        && !marks.is_empty()
        && (marks.bytes().all(|b| b == b'=') || marks.bytes().all(|b| b == b'-'))
}

fn is_numbered_item(line: &str) -> bool {
    let digits = line.len() - line.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    digits > 0 && (line[digits..].starts_with(". ") || line[digits..].starts_with(") "))
}

/// A line that ends a paragraph or an item's own lines by starting another block.
fn is_block_start(line: &str) -> bool {
    let trimmed = line.trim_start();
    is_fence(line)
        || is_heading(trimmed)
        || trimmed.starts_with('|')
        || trimmed.starts_with('>')
        || trimmed.starts_with("- ")
        || trimmed.starts_with("* ")
        || trimmed.starts_with("+ ")
        || is_numbered_item(trimmed)
        || tag_line(line).is_some()
}

/// Tag candidates on one line. With `skip_inline_code`, text inside backtick spans is ignored.
fn find_tags(line: &str, skip_inline_code: bool) -> Vec<Candidate> {
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
            Some(offset) => Candidate {
                start: i,
                end: id_start + offset + 1,
                id: line[id_start..id_start + offset].to_string(),
                terminated: true,
                glued,
            },
            None => Candidate {
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
                "# Syntax\n\nr[bind.let]\n- `let` binds.\n\nr[bind.var]\n* `var` too.\n",
            ),
            file(
                "type",
                "spec/reference/b.md",
                "Types",
                "# Types\nr[type.int]\nInt.\n",
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
            ("bind.var", "spec/reference/a.md", 6, "Syntax"),
            ("type.int", "spec/reference/b.md", 2, "Types"),
        ]
        .map(|(id, path, line, chapter)| {
            (id.to_string(), path.to_string(), line, chapter.to_string())
        });
        assert_eq!(found, expected);
    }

    #[test]
    fn errors_name_their_file() {
        let files = [
            file(LEGACY_PREFIX, "spec/a.md", "A", "r[bind.let]\na\n"),
            file(
                LEGACY_PREFIX,
                "spec/b.md",
                "B",
                "\nr[bind.let]\nb\n\nr[nope.x]\nc\n",
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
        assert!(errors[1].starts_with("spec/b.md:5: rule ID `nope.x` starts with `nope`"));
    }

    #[test]
    fn covers_paragraphs_and_top_level_items() {
        let spec = "r[bind.let]\nA paragraph\nwith a lazy line.\n\nr[bind.var]\n- An item\nlazily continued\n  and indented.\n- An untagged sibling.\n\n  Its nested paragraph.\n\nr[lit.int.hex]\n* A star item.\n\n- Loose sibling.\n\nr[lit.int.oct]\n- A dash item\n- - -\n";
        let ids: Vec<_> = rules(spec).into_iter().map(|r| (r.id, r.line)).collect();
        assert_eq!(
            ids,
            [
                ("bind.let".to_string(), 1),
                ("bind.var".to_string(), 5),
                ("lit.int.hex".to_string(), 13),
                ("lit.int.oct".to_string(), 18),
            ]
        );
    }

    /// The problem with the tag on line `line` of `spec`, after "rule tag `r[bind.let]` ".
    fn placement(spec: &str, line: usize) -> String {
        let errors = messages(spec);
        assert_eq!(errors.len(), 1, "{errors:?}");
        let prefix = format!("line {line}: rule tag `r[bind.let]` ");
        errors[0]
            .strip_prefix(&prefix)
            .unwrap_or_else(|| panic!("{errors:?}"))
            .to_string()
    }

    #[test]
    fn tag_lines_follow_a_break() {
        assert_eq!(
            placement("Some text.\nr[bind.let]\nCovered.\n", 2),
            "must follow a blank line, a heading or the start of the file; put a blank line before it"
        );
        assert_eq!(
            placement("- An item.\nr[bind.let]\n- Covered.\n", 2),
            "must follow a blank line, a heading or the start of the file; put a blank line before it"
        );
    }

    #[test]
    fn tag_lines_cover_only_paragraphs_and_bullet_items() {
        let cover = "which it can't cover; put the paragraph or top-level `-` or `*` item it covers on the next line";
        for (next, what) in [
            ("", "a blank line"),
            ("r[bind.var]", "another tag line"),
            ("  Indented text.", "an indented line"),
            ("    code", "an indented line"),
            ("## Heading", "a heading"),
            ("```", "a code block"),
            ("| A | B |", "a table"),
            ("***", "a thematic break"),
            ("- - -", "a thematic break"),
            ("> Quoted.", "a blockquote"),
            ("<div>", "an HTML block"),
            ("1. Numbered.", "a numbered item"),
            ("2) Numbered.", "a numbered item"),
            ("+ Plus.", "a `+` item (use `-` or `*`)"),
        ] {
            let spec = format!("r[bind.let]\n{next}\n\nText.\n```\n");
            let errors = messages(&spec);
            let wanted = format!("line 1: rule tag `r[bind.let]` is followed by {what}, {cover}");
            assert!(errors.contains(&wanted), "{next:?}: {errors:?}");
        }
        assert_eq!(
            placement("Text.\n\nr[bind.let]\n", 3),
            "ends the file and covers nothing; put the paragraph or top-level `-` or `*` item it covers on the next line"
        );
    }

    #[test]
    fn tagged_paragraphs_have_no_setext_underline() {
        assert_eq!(
            placement("r[bind.let]\nCovered\ntext.\n---\n", 1),
            "covers a paragraph with the setext underline `---` on line 4, which makes the paragraph and the tag a heading; put a blank line before the underline, or make it a `#` heading"
        );
        assert!(
            placement("r[bind.let]\n===\n", 1)
                .starts_with("covers a paragraph with the setext underline `===` on line 2")
        );
        // After a blank line, `---` is a thematic break, and an item can't take an underline.
        let ids: Vec<_> = rules("r[bind.let]\nText.\n\n---\n\nr[bind.var]\n- Item\n---\n")
            .into_iter()
            .map(|r| r.id)
            .collect();
        assert_eq!(ids, ["bind.let", "bind.var"]);
    }

    #[test]
    fn tagged_items_have_no_nested_blocks() {
        assert_eq!(
            placement("r[bind.let]\n- Covered.\n\n  A nested paragraph.\n", 1),
            "covers a list item with a nested block (line 4); a rule never extends into nested blocks, so make that content a top-level block or join it to the item"
        );
        let nested_at = |spec: &str, line: usize| {
            let problem = placement(spec, 1);
            let expected = format!("covers a list item with a nested block (line {line})");
            assert!(problem.starts_with(&expected), "{spec:?}: {problem}");
        };
        nested_at("r[bind.let]\n- Covered.\n\n\n  - Nested.\n", 5);
        nested_at(
            "r[bind.let]\n- Covered.\n  - Nested.\n\n  More nested.\n",
            3,
        );
        nested_at("r[bind.let]\n- Covered.\n  ```\n  code\n  ```\n", 3);
        nested_at("r[bind.let]\n- Covered.\n  | A | B |\n  | - | - |\n", 3);
        nested_at("r[bind.let]\n- Covered\n  and continued.\n  - Nested.\n", 4);
    }

    #[test]
    fn tagged_items_keep_their_lazy_continuation_lines() {
        assert!(messages("r[bind.let]\n- Covered\n  and continued.\n- Next.\n").is_empty());
    }

    #[test]
    fn tags_off_their_own_line_are_errors() {
        let errors = messages(
            "Text r[bind.let] inline.\nr[bind.var] Old form.\n- r[lit.int.hex] In an item.\n| A | r[lit.int.oct] B |\n  r[lit.float] \n\n- Item\n\n  r[bind.x]\n  Nested.\n",
        );
        let own_line = "must be on a line of its own, directly above the paragraph or top-level `-` or `*` item it covers; tags don't go inside paragraphs, list items or table cells (if this is code, put it in backticks)";
        let indented = "is indented; a tag line starts at column 0, and a nested block can't be tagged: make the rule a top-level paragraph or item";
        assert_eq!(
            errors,
            [
                format!("line 1: rule tag `r[bind.let]` {own_line}"),
                format!("line 2: rule tag `r[bind.var]` {own_line}"),
                format!("line 3: rule tag `r[lit.int.hex]` {own_line}"),
                format!("line 4: rule tag `r[lit.int.oct]` {own_line}"),
                format!("line 5: rule tag `r[lit.float]` {indented}"),
                format!("line 9: rule tag `r[bind.x]` {indented}"),
            ]
        );
    }

    #[test]
    fn code_is_not_a_tag() {
        let spec = "Index with `r[i]` or `xs[i]`, or arr[i], and `r[bind.let]`.\n```\nlet v = r[0]\nxs[i] = v\nlet n = counter[user.id]\nheader[key.name] = v\n```\n";
        assert_eq!(rules(spec), Vec::new());
    }

    #[test]
    fn unclosed_backtick_is_literal() {
        let errors = messages("Use ` here r[bind.let] x.\nA ``double` run r[bind.var] y.\n");
        assert_eq!(errors.len(), 2, "{errors:?}");
        assert!(
            errors[0].starts_with("line 1: rule tag `r[bind.let]` must be on a line of its own")
        );
        assert!(
            errors[1].starts_with("line 2: rule tag `r[bind.var]` must be on a line of its own")
        );
    }

    #[test]
    fn glued_tags_are_errors() {
        let errors = messages("(r[bind.var] y)\n**r[bind.let]** z\n");
        assert_eq!(errors.len(), 2, "{errors:?}");
        assert!(
            errors[0].starts_with("line 1: rule tag `r[bind.var]` must be on a line of its own"),
            "{errors:?}"
        );
        assert!(
            errors[1].starts_with("line 2: rule tag `r[bind.let]` must be on a line of its own")
        );
    }

    #[test]
    fn tag_in_code_block() {
        assert_eq!(
            messages("```\n// r[bind.let] no\nr[bind.var]\n```\n"),
            [
                "line 2: rule tag `r[bind.let]` is inside a code block; tags go on their own line above prose, never in code",
                "line 3: rule tag `r[bind.var]` is inside a code block; tags go on their own line above prose, never in code",
            ]
        );
    }

    #[test]
    fn tag_in_heading() {
        assert_eq!(
            messages("### r[bind.let] Bindings\n"),
            [
                "line 1: rule tag `r[bind.let]` is in a heading; put it on its own line below the heading, directly above the paragraph or item it covers"
            ]
        );
    }

    #[test]
    fn invalid_and_unterminated() {
        let errors =
            messages("r[Bind_Shadow]\nx\n\nr[bind]\ny\n\nr[bind.let\nz\n\nText r[Bad] here.\n");
        assert_eq!(errors.len(), 4, "{errors:?}");
        assert!(errors[0].starts_with("line 1: invalid rule ID `Bind_Shadow`"));
        assert!(errors[1].starts_with("line 4: invalid rule ID `bind`"));
        assert!(errors[2].starts_with("line 7: unterminated rule tag"));
        assert!(errors[3].starts_with("line 10: invalid rule ID `Bad`"));
    }

    #[test]
    fn duplicate_ids() {
        assert_eq!(
            messages("r[bind.let]\na\n\nr[bind.let]\nb\n"),
            ["line 4: duplicate rule ID `bind.let`; it is first used at spec.md:1"]
        );
    }

    #[test]
    fn retired_ids() {
        let (retired, errors) = parse_retired("# retired\n\nbind.old\ngrammar.old_arg\n");
        assert!(errors.is_empty(), "{errors:?}");
        assert!(retired.contains("grammar.old_arg"));
        let (_, errors) = check_one("r[bind.old]\na\n", &retired);
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
        let found = rules("r[type.let]\na\n\nr[bind.var]\nb\n\nr[type.var]\nc\n");
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
                "spec.md:4: rule ID `bind.var` moved to `type.var` (spec/moved-rules.txt) and can't be a tag again; pick a new ID",
            ]
        );
    }

    /// `ids` as consecutive tagged paragraphs, each covering one letter.
    fn tagged(ids: &[&str]) -> String {
        ids.iter()
            .map(|id| format!("r[{id}]\nx\n"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn legacy_tags_need_a_chapter_prefix() {
        assert_eq!(
            messages(&tagged(&[
                "types.nominal",
                "grammar.block",
                "legacy.x",
                "type.int"
            ])),
            [
                "line 1: rule ID `types.nominal` starts with `types`, which isn't a chapter prefix \
                 in spec/chapters.txt; start it with the prefix of the chapter the rule belongs to",
                "line 4: rule ID `grammar.block` uses the reserved prefix `grammar`, which belongs \
                 to IDs derived from grammar productions; start it with its chapter's prefix",
                "line 7: rule ID `legacy.x` starts with `legacy`, which isn't a chapter prefix in \
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
            &tagged(&["type.int", "bind.let", "nope.x"]),
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
                format!("spec/reference/types.md:4: {}", wrong("bind.let", "bind")),
                format!("spec/reference/types.md:7: {}", wrong("nope.x", "nope")),
            ]
        );
    }

    #[test]
    fn constructs_must_be_listed() {
        assert_eq!(
            messages(&tagged(&[
                "lit.int.hex",
                "lit.float",
                "lit.str.raw",
                "lit.int.hex.upper"
            ])),
            [
                "line 7: rule ID `lit.str.raw` names the construct `str`, which chapter `lit` \
                 doesn't list in spec/chapters.txt; use a listed construct, or list this one \
                 under `lit` with the subject it covers"
            ]
        );
    }

    #[test]
    fn unused_construct() {
        let found = rules(&tagged(&["lit.int.hex", "lit.float"]));
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
    fn strip_removes_only_tag_lines() {
        let untagged = "## 2. Syntax\n\n- `let` binds.\n\n`var` too.\n```\nr[bind.x]\nxs[i]\n```\n(r[bind.y] kept) End";
        let tagged = "## 2. Syntax\n\nr[bind.let]\n- `let` binds.\n\nr[bind.var]\n`var` too.\n```\nr[bind.x]\nxs[i]\n```\n(r[bind.y] kept) End";
        assert_eq!(strip(tagged), untagged);
        assert_eq!(strip(untagged), untagged);
    }

    #[test]
    fn candidates_skip_inline_code() {
        let found: Vec<_> = candidates("r[a.b] `r[c.d]` x(r[e.f] arr[i] r[g")
            .into_iter()
            .map(|c| (c.id, c.start, c.terminated, c.glued))
            .collect();
        assert_eq!(
            found,
            [
                ("a.b".to_string(), 0, true, false),
                ("e.f".to_string(), 18, true, true),
                ("g".to_string(), 32, false, false),
            ]
        );
    }
}
