//! `spec/chapters.txt`: the chapters of the spec, in reading order.
//!
//! A chapter line starts at column 0 and gives the chapter's ID prefix, its file (relative to
//! `spec/`) and its title. An indented line under it gives one construct of that chapter and
//! the subject it covers. Blank lines and `#` comments are ignored.

use crate::{ChapterFile, SpecError};
use std::collections::HashMap;

/// One chapter line and the construct lines under it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chapter {
    pub prefix: String,
    /// Relative to `spec/`, such as `reference/lexical.md`.
    pub file: String,
    pub title: String,
    /// 1-based line in the manifest.
    pub line: usize,
    pub constructs: Vec<Construct>,
}

/// A construct a chapter covers, which a rule ID may name as its second segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Construct {
    pub name: String,
    pub subject: String,
    /// 1-based line in the manifest.
    pub line: usize,
}

/// Parses the manifest. Returns the chapters, and an error for each bad line.
pub fn parse(text: &str) -> (Vec<Chapter>, Vec<SpecError>) {
    let mut chapters: Vec<Chapter> = Vec::new();
    let mut errors = Vec::new();
    let mut prefixes: HashMap<String, usize> = HashMap::new();
    let mut files: HashMap<String, usize> = HashMap::new();
    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        let entry = raw.trim();
        if entry.is_empty() || entry.starts_with('#') {
            continue;
        }
        let mut error = |message: String| errors.push(SpecError { line, message });
        let (first, rest) = split_word(entry);
        if raw.starts_with(char::is_whitespace) {
            let Some(chapter) = chapters.last_mut() else {
                error(format!(
                    "construct `{first}` comes before any chapter line; put it under its chapter"
                ));
                continue;
            };
            if !crate::rules::is_valid_segment(first) {
                error(format!(
                    "`{first}` is not a valid construct: use lowercase letters and digits joined by single hyphens"
                ));
            } else if rest.is_empty() {
                error(format!(
                    "construct `{first}` needs the subject it covers after it, such as `if  If expressions`"
                ));
            } else if let Some(other) = chapter.constructs.iter().find(|c| c.name == first) {
                error(format!(
                    "construct `{first}` is listed twice under chapter `{}`; it is first listed on line {}",
                    chapter.prefix, other.line
                ));
            } else {
                chapter.constructs.push(Construct {
                    name: first.to_string(),
                    subject: rest.to_string(),
                    line,
                });
            }
            continue;
        }

        let (file, title) = split_word(rest);
        let problem = if !crate::rules::is_valid_segment(first) {
            Some(format!(
                "`{first}` is not a valid chapter prefix: use lowercase letters and digits joined by single hyphens"
            ))
        } else if first == crate::rules::GRAMMAR_PREFIX {
            Some("`grammar` is reserved for IDs derived from grammar productions".to_string())
        } else if let Some(first_line) = prefixes.get(first) {
            Some(format!(
                "chapter prefix `{first}` is listed twice; it is first listed on line {first_line}"
            ))
        } else if file.is_empty() || title.is_empty() {
            Some(format!(
                "chapter `{first}` needs a file and a title after it, such as `lex  reference/lexical.md  Lexical structure`"
            ))
        } else {
            file_problem(file).or_else(|| {
                files.get(file).map(|first_line| {
                    format!("{file} is listed twice; it is first listed on line {first_line}")
                })
            })
        };
        if let Some(message) = problem {
            error(message);
            continue;
        }
        prefixes.insert(first.to_string(), line);
        files.insert(file.to_string(), line);
        chapters.push(Chapter {
            prefix: first.to_string(),
            file: file.to_string(),
            title: title.to_string(),
            line,
            constructs: Vec::new(),
        });
    }
    if chapters.is_empty() && errors.is_empty() {
        errors.push(SpecError {
            line: 0,
            message: "the manifest lists no chapters".to_string(),
        });
    }
    (chapters, errors)
}

/// Checks that `readme` links every chapter file in manifest order. `files` are the chapter
/// files that exist, in manifest order. Errors are on lines of the README.
pub fn check_readme(readme: &str, files: &[ChapterFile]) -> Vec<SpecError> {
    let links = relative_links(readme);
    let mut errors = Vec::new();
    let mut previous: Option<(&ChapterFile, usize)> = None;
    for file in files {
        let Some(&(_, line)) = links.iter().find(|(target, _)| *target == file.file) else {
            errors.push(SpecError {
                line: 0,
                message: format!(
                    "the chapter list doesn't link {}; add `[{}]({})` in its place in the order of spec/chapters.txt",
                    file.file, file.title, file.file
                ),
            });
            continue;
        };
        if let Some((before, before_line)) = previous
            && line < before_line
        {
            errors.push(SpecError {
                line,
                message: format!(
                    "{} is linked before {}, but spec/chapters.txt lists it after; list the chapters in manifest order",
                    file.file, before.file
                ),
            });
        }
        previous = Some((file, line));
    }
    errors
}

/// Every relative Markdown link target outside code, without its `#fragment` or a leading
/// `./`, with its 1-based line.
fn relative_links(text: &str) -> Vec<(&str, usize)> {
    let mut links = Vec::new();
    let mut in_code_block = false;
    for (index, line) in text.lines().enumerate() {
        if line.trim_start().starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }
        if in_code_block {
            continue;
        }
        let mut rest = line;
        while let Some(at) = rest.find("](") {
            let after = &rest[at + 2..];
            let Some(end) = after.find(')') else { break };
            let target = after[..end].split('#').next().unwrap_or_default();
            let target = target.strip_prefix("./").unwrap_or(target);
            if !target.is_empty() && !target.contains("://") {
                links.push((target, index + 1));
            }
            rest = &after[end..];
        }
    }
    links
}

/// The first whitespace-separated word of `text`, and the rest with surrounding whitespace
/// trimmed.
fn split_word(text: &str) -> (&str, &str) {
    text.split_once(char::is_whitespace)
        .map_or((text, ""), |(word, rest)| (word, rest.trim()))
}

/// Why `file` can't be a chapter's file, if it can't. A chapter file is a Markdown file
/// directly in one of the chapter folders.
fn file_problem(file: &str) -> Option<String> {
    let simple = |name: &str| {
        name.ends_with(".md")
            && name.len() > 3
            && !name.contains('/')
            && !name.contains('\\')
            && !name.starts_with('.')
    };
    let ok = crate::CHAPTER_DIRS.iter().any(|dir| {
        file.strip_prefix(dir)
            .and_then(|rest| rest.strip_prefix('/'))
            .is_some_and(simple)
    });
    (!ok).then(|| {
        format!(
            "`{file}` must be a Markdown file directly in spec/reference/ or spec/tooling/, such as `reference/lexical.md`"
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(errors: &[SpecError]) -> Vec<usize> {
        errors.iter().map(|e| e.line).collect()
    }

    #[test]
    fn parses_chapters_and_constructs() {
        let (chapters, errors) = parse(
            "# chapter  file  title\n\
             intro  reference/introduction.md  Introduction\n\
             \n\
             lex  reference/lexical.md  Lexical structure\n\
             \x20 continue   Line continuation\n\
             \x20 name       Naming conventions\n\
             dep  tooling/dependencies.md  Dependencies and publishing\n",
        );
        assert!(errors.is_empty(), "{errors:?}");
        let found: Vec<_> = chapters
            .iter()
            .map(|c| (c.prefix.as_str(), c.file.as_str(), c.title.as_str(), c.line))
            .collect();
        assert_eq!(
            found,
            [
                ("intro", "reference/introduction.md", "Introduction", 2),
                ("lex", "reference/lexical.md", "Lexical structure", 4),
                (
                    "dep",
                    "tooling/dependencies.md",
                    "Dependencies and publishing",
                    7
                ),
            ]
        );
        assert_eq!(
            chapters[1].constructs,
            [
                Construct {
                    name: "continue".into(),
                    subject: "Line continuation".into(),
                    line: 5
                },
                Construct {
                    name: "name".into(),
                    subject: "Naming conventions".into(),
                    line: 6
                },
            ]
        );
    }

    #[test]
    fn bad_chapter_lines() {
        let (chapters, errors) = parse(
            "lex  reference/lexical.md  Lexical structure\n\
             Bad  reference/bad.md  Bad\n\
             grammar  reference/grammar.md  Grammar\n\
             lex  reference/other.md  Again\n\
             type  reference/types.md\n\
             type  reference/lexical.md  Types\n\
             expr  expressions.md  Expressions\n\
             stmt  reference/nested/statements.md  Statements\n\
             pat  reference/patterns.txt  Patterns\n\
             legacy  quince-spec.md  Old\n",
        );
        assert_eq!(chapters.len(), 1);
        assert_eq!(lines(&errors), [2, 3, 4, 5, 6, 7, 8, 9, 10], "{errors:?}");
        assert!(errors[0].message.contains("not a valid chapter prefix"));
        assert!(errors[1].message.contains("reserved"));
        assert!(
            errors[2]
                .message
                .contains("`lex` is listed twice; it is first listed on line 1")
        );
        assert!(errors[3].message.contains("needs a file and a title"));
        assert!(
            errors[4]
                .message
                .contains("reference/lexical.md is listed twice")
        );
        assert!(errors[5].message.contains("directly in spec/reference/"));
        assert!(errors[8].message.contains("directly in spec/reference/"));
    }

    #[test]
    fn bad_construct_lines() {
        let (chapters, errors) = parse(
            "  stray  Before any chapter\n\
             intro  reference/introduction.md  Introduction\n\
             \x20 excl  Features deliberately excluded\n\
             expr  reference/expressions.md  Expressions\n\
             \x20 If  If expressions\n\
             \x20 if\n\
             \x20 if  If expressions\n\
             \x20 if  Again\n",
        );
        assert_eq!(chapters[1].constructs.len(), 1);
        assert_eq!(lines(&errors), [1, 5, 6, 8], "{errors:?}");
        assert!(errors[0].message.contains("before any chapter line"));
        assert!(errors[1].message.contains("not a valid construct"));
        assert!(errors[2].message.contains("needs the subject"));
        assert!(
            errors[3]
                .message
                .contains("listed twice under chapter `expr`; it is first listed on line 7")
        );
    }

    #[test]
    fn empty_manifest() {
        let (_, errors) = parse("# nothing\n");
        assert_eq!(errors[0].message, "the manifest lists no chapters");
    }

    fn file(file: &str, title: &str) -> ChapterFile {
        ChapterFile {
            prefix: "x".into(),
            title: title.into(),
            path: format!("spec/{file}"),
            file: file.into(),
            text: String::new(),
        }
    }

    #[test]
    fn readme_links_chapters_in_order() {
        let files = [
            file("reference/lexical.md", "Lexical structure"),
            file("reference/items.md", "Items"),
            file("tooling/toolchain.md", "Toolchain"),
        ];
        let readme = "# Spec\n\n\
             - [Lexical structure](reference/lexical.md): tokens.\n\
             - [Items](./reference/items.md#items) and [x](https://example.com/a.md)\n\
             ```\n[Toolchain](tooling/toolchain.md)\n```\n\
             - [Toolchain](tooling/toolchain.md)\n";
        assert_eq!(check_readme(readme, &files), Vec::new());

        let swapped = "[Items](reference/items.md)\n[Lexical](reference/lexical.md)\n";
        let errors = check_readme(swapped, &files);
        assert_eq!(errors.len(), 2, "{errors:?}");
        assert_eq!(errors[0].line, 1);
        assert_eq!(
            errors[0].message,
            "reference/items.md is linked before reference/lexical.md, but spec/chapters.txt lists it after; list the chapters in manifest order"
        );
        assert_eq!(
            errors[1].message,
            "the chapter list doesn't link tooling/toolchain.md; add `[Toolchain](tooling/toolchain.md)` in its place in the order of spec/chapters.txt"
        );
    }
}
