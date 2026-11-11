//! Links between Markdown files: finding them, resolving them, and the heading anchors a
//! `#fragment` may name.
//!
//! Anchors follow GitHub's rules as github-slugger implements them: the heading's rendered
//! text, lowercased, with every character removed that isn't a letter, a digit, a space, `-` or
//! `_`, and each space turned into `-`. A heading whose slug was already used gets `-1`, `-2`
//! and so on.

use std::collections::HashMap;

/// One link in a Markdown file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    /// The destination as written, such as `effects.md#handlers` or `https://example.com`.
    pub dest: String,
    /// 1-based line.
    pub line: usize,
}

impl Link {
    /// Whether the destination has a URL scheme (`https:`, `mailto:`) or is
    /// protocol-relative, so it isn't a file in the repo.
    pub fn is_external(&self) -> bool {
        is_external(&self.dest)
    }

    /// The path part of the destination, empty for a same-file `#fragment` link.
    pub fn path(&self) -> &str {
        self.dest.split('#').next().unwrap_or_default()
    }

    /// The fragment after `#`, if there is one.
    pub fn fragment(&self) -> Option<&str> {
        self.dest.split_once('#').map(|(_, fragment)| fragment)
    }
}

fn is_external(dest: &str) -> bool {
    if dest.starts_with("//") {
        return true;
    }
    let scheme = dest.split(':').next().unwrap_or_default();
    dest.contains(':')
        && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))
}

/// Every inline link, image and link reference definition outside code blocks and inline code
/// spans, in file order.
pub fn links(text: &str) -> Vec<Link> {
    let mut out = Vec::new();
    for (line_no, line) in prose_lines(text) {
        if let Some(dest) = reference_definition(line) {
            out.push(Link {
                dest: dest.to_string(),
                line: line_no,
            });
            continue;
        }
        let code = inline_code_mask(line);
        let mut i = 0;
        while let Some(offset) = line[i..].find("](") {
            let at = i + offset;
            i = at + 2;
            if code[at] {
                continue;
            }
            if let Some((dest, end)) = destination(&line[at + 2..]) {
                out.push(Link {
                    dest: dest.to_string(),
                    line: line_no,
                });
                i = at + 2 + end;
            }
        }
    }
    out
}

/// The destination of an inline link whose text has just closed, given the text after `](`,
/// and the length consumed up to and including the closing `)`. Handles `<...>` destinations,
/// titles, and balanced parentheses.
fn destination(rest: &str) -> Option<(&str, usize)> {
    let trimmed = rest.trim_start();
    let lead = rest.len() - trimmed.len();
    if let Some(inner) = trimmed.strip_prefix('<') {
        let close = inner.find('>')?;
        let after = inner[close + 1..].find(')')?;
        return Some((&inner[..close], lead + 1 + close + 1 + after + 1));
    }
    let mut depth = 0usize;
    for (index, c) in trimmed.char_indices() {
        match c {
            '(' => depth += 1,
            ')' if depth == 0 => return Some((&trimmed[..index], lead + index + 1)),
            ')' => depth -= 1,
            c if c.is_whitespace() => {
                // A title follows; the destination ends here.
                let after = &trimmed[index..];
                let title = after.trim_start();
                let title_at = index + after.len() - title.len();
                let closer = match title.chars().next()? {
                    '"' => '"',
                    '\'' => '\'',
                    '(' => ')',
                    ')' => return Some((&trimmed[..index], title_at + 1 + lead)),
                    _ => return None,
                };
                let end = title[1..].find(closer)? + 2;
                let close = title[end..].find(')')?;
                return Some((&trimmed[..index], lead + title_at + end + close + 1));
            }
            _ => {}
        }
    }
    None
}

/// The destination of a link reference definition, `[label]: dest`, indented at most three
/// spaces.
fn reference_definition(line: &str) -> Option<&str> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }
    let rest = line[indent..].strip_prefix('[')?;
    let close = rest.find("]:")?;
    if close == 0 || rest[..close].contains('[') {
        return None;
    }
    let dest = rest[close + 2..].split_whitespace().next()?;
    Some(
        dest.strip_prefix('<')
            .and_then(|d| d.strip_suffix('>'))
            .unwrap_or(dest),
    )
}

/// One heading and the anchor GitHub gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    pub text: String,
    pub anchor: String,
    /// 1-based line.
    pub line: usize,
}

/// Every ATX heading outside code blocks, with its anchor.
pub fn headings(text: &str) -> Vec<Heading> {
    let mut slugger = Slugger::default();
    prose_lines(text)
        .filter_map(|(line_no, line)| {
            let text = heading_text(line)?;
            Some(Heading {
                anchor: slugger.slug(&rendered(text)),
                text: text.to_string(),
                line: line_no,
            })
        })
        .collect()
}

/// The text of an ATX heading line, without its `#` markers or a closing `#` sequence.
fn heading_text(line: &str) -> Option<&str> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }
    let rest = &line[indent..];
    let level = rest.len() - rest.trim_start_matches('#').len();
    if !(1..=6).contains(&level) {
        return None;
    }
    let content = &rest[level..];
    if !content.is_empty() && !content.starts_with([' ', '\t']) {
        return None;
    }
    let content = content.trim();
    // A closing sequence is `#`s preceded by a space, or the whole content.
    let without_closing = content.trim_end_matches('#');
    let content = if without_closing.is_empty() {
        without_closing
    } else if without_closing.ends_with([' ', '\t']) {
        without_closing.trim_end()
    } else {
        content
    };
    Some(content)
}

/// The visible text of inline Markdown, as far as anchors care: a link shows only its text.
/// Other markup (backticks, `*`) is punctuation that the slug removes anyway.
fn rendered(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        let Some(close) = rest[open..].find("](") else {
            break;
        };
        let close = open + close;
        let Some((_, end)) = destination(&rest[close + 2..]) else {
            break;
        };
        out.push_str(&rest[..open]);
        out.push_str(&rest[open + 1..close]);
        rest = &rest[close + 2 + end..];
    }
    out.push_str(rest);
    out
}

/// github-slugger: slugs for headings in one file, made unique in order.
#[derive(Debug, Default)]
pub struct Slugger {
    occurrences: HashMap<String, usize>,
}

impl Slugger {
    /// The anchor for the next heading with rendered text `text`.
    pub fn slug(&mut self, text: &str) -> String {
        let base = slug(text);
        let mut result = base.clone();
        while self.occurrences.contains_key(&result) {
            let count = self.occurrences.entry(base.clone()).or_insert(0);
            *count += 1;
            result = format!("{base}-{count}");
        }
        self.occurrences.insert(result.clone(), 0);
        result
    }
}

/// The slug of one heading's text, before duplicates are numbered.
pub fn slug(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter(|&c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_'))
        .map(|c| if c == ' ' { '-' } else { c })
        .collect()
}

/// Resolves `dest`'s path part against the file `from` (both relative to the repo root, with
/// `/` separators). Returns `None` when it climbs above the root. An empty path is `from`
/// itself.
pub fn resolve(from: &str, dest_path: &str) -> Option<String> {
    if dest_path.is_empty() {
        return Some(from.to_string());
    }
    let mut parts: Vec<&str> = if dest_path.starts_with('/') {
        Vec::new()
    } else {
        let mut dir: Vec<&str> = from.split('/').collect();
        dir.pop();
        dir
    };
    for part in dest_path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            part => parts.push(part),
        }
    }
    Some(parts.join("/"))
}

/// Text left in prose that locates other text by position, such as "section 5" or "(below)",
/// which a link should replace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PositionalReference {
    /// 1-based line.
    pub line: usize,
    /// The text that matched, such as `section 5` or `(see Mutation below)`.
    pub text: String,
}

/// Every positional reference in the prose of `text`: `section` or `sections` and a number, `§`
/// and a number,
/// `appendix`, `(see ` and a capital letter, and a parenthesized `below` or `above`. Case is
/// ignored. Headings are checked too. Code blocks, inline code, link reference definitions
/// and link destinations aren't prose, so they are skipped.
pub fn positional_references(text: &str) -> Vec<PositionalReference> {
    let mut out = Vec::new();
    for (line_no, line) in prose_lines(text) {
        if reference_definition(line).is_some() {
            continue;
        }
        out.extend(reference_matches(&prose_of(line)).into_iter().map(|text| {
            PositionalReference {
                line: line_no,
                text,
            }
        }));
    }
    out
}

/// `line` with its inline code spans and link destinations blanked out.
fn prose_of(line: &str) -> String {
    let code = inline_code_mask(line);
    let mut blank = code.clone();
    let mut i = 0;
    while let Some(offset) = line[i..].find("](") {
        let at = i + offset;
        i = at + 2;
        if code[at] {
            continue;
        }
        if let Some((_, end)) = destination(&line[at + 2..]) {
            blank[at + 1..at + 2 + end].fill(true);
            i = at + 2 + end;
        }
    }
    line.char_indices()
        .map(|(at, c)| if blank[at] { ' ' } else { c })
        .collect()
}

/// The positional references in one line of prose, in order.
fn reference_matches(prose: &str) -> Vec<String> {
    // Every pattern is ASCII, and ASCII lowercasing never moves a byte offset, so a match in
    // `lower` is the same range of `prose`, whatever else the line holds.
    let lower = prose.to_ascii_lowercase();
    let digits = |at: usize| lower[at..].bytes().take_while(u8::is_ascii_digit).count();
    let mut found: Vec<(usize, usize)> = Vec::new();
    for (at, word) in lower.match_indices("section") {
        let mut end = at + word.len();
        end += usize::from(lower[end..].starts_with('s'));
        if lower[end..].starts_with(' ') && digits(end + 1) > 0 {
            found.push((at, end + 1 + digits(end + 1)));
        }
    }
    for (at, sign) in lower.match_indices('§') {
        let after = at + sign.len();
        let space = usize::from(lower[after..].starts_with(' '));
        let count = digits(after + space);
        if count > 0 {
            found.push((at, after + space + count));
        }
    }
    for (at, word) in lower.match_indices("appendix") {
        found.push((at, at + word.len()));
    }
    for (at, word) in lower.match_indices("(see ") {
        if prose[at + word.len()..].starts_with(|c: char| c.is_ascii_uppercase()) {
            let end = prose[at..]
                .find(')')
                .map_or(prose.len(), |close| at + close + 1);
            found.push((at, end));
        }
    }
    for (at, _) in prose.match_indices('(') {
        let Some(close) = prose[at..].find(')') else {
            continue;
        };
        let inner = &lower[at + 1..at + close];
        let is_word = |word: &str| {
            inner.match_indices(word).any(|(i, _)| {
                let before = inner[..i].chars().next_back();
                let after = inner[i + word.len()..].chars().next();
                !before.is_some_and(char::is_alphanumeric)
                    && !after.is_some_and(char::is_alphanumeric)
            })
        };
        if is_word("below") || is_word("above") {
            found.push((at, at + close + 1));
        }
    }
    found.sort();
    found.dedup_by(|later, earlier| later.0 < earlier.1);
    found
        .into_iter()
        .map(|(start, end)| prose[start..end].to_string())
        .collect()
}

/// The lines outside fenced code blocks, with their 1-based numbers.
fn prose_lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut in_code_block = false;
    text.lines().enumerate().filter_map(move |(index, line)| {
        if line.trim_start().starts_with("```") {
            in_code_block = !in_code_block;
            return None;
        }
        (!in_code_block).then_some((index + 1, line))
    })
}

/// Marks the bytes inside inline code spans, as `rules` does for tags.
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

    fn anchors(text: &str) -> Vec<String> {
        headings(text).into_iter().map(|h| h.anchor).collect()
    }

    #[test]
    fn anchors_of_awkward_headings() {
        // Headings from the spec, with the anchors GitHub gives them.
        let text = "\
## 1. Overview and design principles
### Formatting rules (enforced by `quince` `fmt`)
### The `?` operator
### Expected failures: `Result` and `Option`
### Timeouts and resources (std.time, std.ffi)
## Appendix: Expression grammar
### Declarations, types and patterns
### snake_case names & C++ (FFI) / \"quotes\"
### Links: [Effects](effects.md#handlers) and *emphasis*
## Closing hashes ##
#### Not#closing
";
        assert_eq!(
            anchors(text),
            [
                "1-overview-and-design-principles",
                "formatting-rules-enforced-by-quince-fmt",
                "the--operator",
                "expected-failures-result-and-option",
                "timeouts-and-resources-stdtime-stdffi",
                "appendix-expression-grammar",
                "declarations-types-and-patterns",
                "snake_case-names--c-ffi--quotes",
                "links-effects-and-emphasis",
                "closing-hashes",
                "notclosing",
            ]
        );
    }

    #[test]
    fn duplicate_headings_are_numbered() {
        let text = "## Examples\n## Examples\n## Examples-1\n## Examples\n";
        assert_eq!(
            anchors(text),
            ["examples", "examples-1", "examples-1-1", "examples-2"]
        );
    }

    #[test]
    fn not_headings() {
        let text = "#hashtag\n    # indented code\n```\n# in code\n```\n####### seven\n# \n";
        let found = headings(text);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!((found[0].anchor.as_str(), found[0].line), ("", 7));
    }

    #[test]
    fn finds_positional_references() {
        let text = "\
# Appendix: Grammar
Its type is pure (section 5), as in sections 2 and §3 or § 11.
See [Appendix: Prelude](a.md) and [Effects](appendix.md#section-5), `section 9`.
A count below the bit width (see Ranges) and (see the list) and (listed below) (above).
Nothing (belowground), (aboveboard) or Subsection 1 here; this appendix.
İstanbul's Section 7 (See Effects).
```
(section 3)
```
[ref]: section-1.md
";
        let found: Vec<_> = positional_references(text)
            .into_iter()
            .map(|r| (r.line, r.text))
            .collect();
        let expected = [
            (1, "Appendix"),
            (2, "section 5"),
            (2, "sections 2"),
            (2, "§3"),
            (2, "§ 11"),
            (3, "Appendix"),
            (4, "(see Ranges)"),
            (4, "(listed below)"),
            (4, "(above)"),
            (5, "section 1"),
            (5, "appendix"),
            (6, "Section 7"),
            (6, "(See Effects)"),
        ]
        .map(|(line, text)| (line, text.to_string()));
        assert_eq!(found, expected);
    }

    #[test]
    fn finds_links() {
        let text = "\
See [Effects](effects.md#handlers), ![img](a/b.png \"title\") and [web](https://x.y/(z)).
[t](a.md 'x') [u](b.md (y)) [v](c.md   ) end.
`[not](code.md)` and [ok](<with space.md>) and [nested](a_(b).md).
```
[in](block.md)
```
[ref]: ../docs/plan.md \"Plan\"
";
        let found: Vec<_> = links(text).into_iter().map(|l| (l.dest, l.line)).collect();
        let expected = [
            ("effects.md#handlers", 1),
            ("a/b.png", 1),
            ("https://x.y/(z)", 1),
            ("a.md", 2),
            ("b.md", 2),
            ("c.md", 2),
            ("with space.md", 3),
            ("a_(b).md", 3),
            ("../docs/plan.md", 7),
        ]
        .map(|(dest, line)| (dest.to_string(), line));
        assert_eq!(found, expected);
    }

    #[test]
    fn link_parts() {
        let link = |dest: &str| Link {
            dest: dest.into(),
            line: 1,
        };
        assert!(link("https://example.com").is_external());
        assert!(link("mailto:a@b.c").is_external());
        assert!(link("//cdn.example.com/x").is_external());
        assert!(!link("effects.md").is_external());
        assert!(!link("#handlers").is_external());
        assert_eq!(link("#handlers").path(), "");
        assert_eq!(link("#handlers").fragment(), Some("handlers"));
        assert_eq!(link("a.md").fragment(), None);
    }

    #[test]
    fn resolves_relative_paths() {
        let from = "spec/reference/effects.md";
        assert_eq!(
            resolve(from, "types.md").as_deref(),
            Some("spec/reference/types.md")
        );
        assert_eq!(
            resolve(from, "../tooling/./toolchain.md").as_deref(),
            Some("spec/tooling/toolchain.md")
        );
        assert_eq!(resolve(from, "").as_deref(), Some(from));
        assert_eq!(
            resolve("std/API.md", "../spec/README.md").as_deref(),
            Some("spec/README.md")
        );
        assert_eq!(resolve("spec/README.md", "../../x.md"), None);
        assert_eq!(
            resolve("spec/README.md", "/AGENTS.md").as_deref(),
            Some("AGENTS.md")
        );
    }
}
