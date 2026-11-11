//! Splits a Markdown file into the units that `spec-compare` matches: one per tagged rule,
//! and one per untagged paragraph, list item, table row, code declaration and heading.
//!
//! The baseline writes tags inline, and the chapters on their own line (decision 13 of
//! docs/design/spec-restructure.md), so the segmenter reads both. The inline parser lives here
//! rather than in `quince_spec`, because only the baseline still uses that form; it goes with
//! `spec-compare`.

use quince_spec::rules::{candidates, is_valid_id};

/// How a file writes its rule tags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form {
    /// Directly before the text a tag covers, which runs to the next tag or the end of its
    /// paragraph, list item or table cell, as the baseline does.
    Inline,
    /// On a line of its own, covering the paragraph or `-` or `*` item after it, as the
    /// chapters do.
    OwnLine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    /// An ATX heading, with its `#` markers.
    Heading,
    Paragraph,
    /// An item of a `-` or `*` list. The marker isn't part of the text, so the two count as
    /// the same.
    Item,
    /// An item of a numbered list, with its number.
    Numbered,
    /// A table's header row and delimiter row. A table split in two repeats them.
    TableHeader,
    TableRow,
    /// A declaration or production unit of a code block. Fences and info strings aren't part
    /// of any unit.
    Code,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Heading => "heading",
            Kind::Paragraph => "paragraph",
            Kind::Item => "list item",
            Kind::Numbered => "numbered item",
            Kind::TableHeader => "table header",
            Kind::TableRow => "table row",
            Kind::Code => "code",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unit {
    pub kind: Kind,
    /// The rule ID of a tagged unit. Its text is what the tag covers, without the tag.
    pub id: Option<String>,
    /// Trimmed at both ends.
    pub text: String,
    /// 1-based line where the unit starts: for a tag on its own line, the tag's line.
    pub line: usize,
    /// For a tagged unit in a table row, the 1-based column of its cell.
    pub cell: Option<usize>,
}

/// Every unit of `text`, in file order. In the own-line form, a tag line followed by anything
/// but a paragraph or a `-` or `*` item gives a tagged unit with no text.
pub fn segment(text: &str, form: Form) -> Vec<Unit> {
    let lines: Vec<&str> = text.lines().collect();
    let mut units = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if form == Form::OwnLine
            && let Some(id) = tag_line(line)
        {
            let next = lines.get(i + 1).copied().unwrap_or_default();
            let covers = !next.trim().is_empty()
                && !next.starts_with(char::is_whitespace)
                && tag_line(next).is_none()
                && !is_fence(next)
                && !is_heading(next)
                && !is_table_row(next)
                && list_marker(next).is_none_or(|(kind, _)| kind == Kind::Item);
            if !covers {
                units.push(Unit {
                    kind: Kind::Paragraph,
                    id: Some(id.to_string()),
                    text: String::new(),
                    line: i + 1,
                    cell: None,
                });
                i += 1;
                continue;
            }
            let end = continuation_end(&lines, i + 2, form);
            let (kind, first) = match list_marker(next) {
                Some((kind, marker_len)) => (kind, &next.trim_start()[marker_len..]),
                None => (Kind::Paragraph, next),
            };
            let mut covered = first.to_string();
            for continued in &lines[i + 2..end] {
                covered.push('\n');
                covered.push_str(continued);
            }
            units.push(Unit {
                kind,
                id: Some(id.to_string()),
                text: covered.trim().to_string(),
                line: i + 1,
                cell: None,
            });
            i = end;
        } else if line.trim().is_empty() {
            i += 1;
        } else if is_fence(line) {
            let close = (i + 1..lines.len())
                .find(|&j| is_fence(lines[j]))
                .unwrap_or(lines.len());
            code_units(&lines[i + 1..close], i + 2, &mut units);
            i = close + 1;
        } else if is_heading(line) {
            units.push(Unit {
                kind: Kind::Heading,
                id: None,
                text: line.trim().to_string(),
                line: i + 1,
                cell: None,
            });
            i += 1;
        } else if is_table_row(line) {
            let end = (i..lines.len())
                .find(|&j| !is_table_row(lines[j]))
                .unwrap_or(lines.len());
            table_units(&lines[i..end], i + 1, &mut units);
            i = end;
        } else if let Some((kind, marker_len)) = list_marker(line) {
            let end = continuation_end(&lines, i + 1, form);
            let numbered = kind == Kind::Numbered;
            let mut item = if numbered {
                line.trim_start().to_string()
            } else {
                line.trim_start()[marker_len..].to_string()
            };
            for continued in &lines[i + 1..end] {
                item.push('\n');
                item.push_str(continued);
            }
            block_units(&item, kind, i + 1, form, &mut units);
            i = end;
        } else {
            let end = continuation_end(&lines, i + 1, form);
            block_units(
                &lines[i..end].join("\n"),
                Kind::Paragraph,
                i + 1,
                form,
                &mut units,
            );
            i = end;
        }
    }
    units
}

/// The index of the first line from `from` on that doesn't continue the current paragraph or
/// list item: a blank line or the start of another block, including, in the own-line form, a
/// tag line.
fn continuation_end(lines: &[&str], from: usize, form: Form) -> usize {
    (from..lines.len())
        .find(|&j| {
            let line = lines[j];
            line.trim().is_empty()
                || is_fence(line)
                || is_heading(line)
                || is_table_row(line)
                || list_marker(line).is_some()
                || (form == Form::OwnLine && tag_line(line).is_some())
        })
        .unwrap_or(lines.len())
}

/// The ID of a tag on its own line: `r[<id>]` from column 0, perhaps with trailing whitespace.
fn tag_line(line: &str) -> Option<&str> {
    let inner = line.trim_end().strip_prefix("r[")?.strip_suffix(']')?;
    (!inner.contains(']')).then_some(inner)
}

/// An untagged block in the own-line form, or a block split at its inline tags.
fn block_units(text: &str, kind: Kind, first_line: usize, form: Form, units: &mut Vec<Unit>) {
    match form {
        Form::Inline => split_tags(text, kind, first_line, units),
        Form::OwnLine => push_untagged(kind, text, first_line, units),
    }
}

/// A well-formed inline tag on a prose line: `r[<id>]` from byte `start` up to `end`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct TagSpan {
    id: String,
    start: usize,
    /// Just past the `]`.
    end: usize,
}

/// The well-formed inline tags on one prose line, outside inline code spans, in order.
/// Malformed and glued tag-shaped text is skipped; the baseline has none.
fn tag_spans(line: &str) -> Vec<TagSpan> {
    candidates(line)
        .into_iter()
        .filter(|tag| !tag.glued && tag.terminated && is_valid_id(&tag.id))
        .map(|tag| TagSpan {
            id: tag.id,
            start: tag.start,
            end: tag.end,
        })
        .collect()
}

fn is_fence(line: &str) -> bool {
    line.trim_start().starts_with("```")
}

fn is_heading(line: &str) -> bool {
    let hashes = line.len() - line.trim_start_matches('#').len();
    (1..=6).contains(&hashes) && line[hashes..].chars().next().is_none_or(|c| c == ' ')
}

fn is_table_row(line: &str) -> bool {
    line.trim_start().starts_with('|')
}

/// The kind of list item `line` starts, and the length of its marker with the space after it.
fn list_marker(line: &str) -> Option<(Kind, usize)> {
    let item = line.trim_start();
    if item.starts_with("- ") || item.starts_with("* ") {
        return Some((Kind::Item, 2));
    }
    let digits = item.len() - item.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    (digits > 0 && item[digits..].starts_with(". ")).then_some((Kind::Numbered, digits + 2))
}

/// Splits block text into a unit for the text before its first tag, if any, and one unit per
/// tag, covering the text up to the next tag.
fn split_tags(text: &str, kind: Kind, first_line: usize, units: &mut Vec<Unit>) {
    let mut tags = Vec::new();
    let mut offset = 0;
    for line in text.split('\n') {
        for tag in tag_spans(line) {
            tags.push((tag.id, offset + tag.start, offset + tag.end));
        }
        offset += line.len() + 1;
    }
    let line_at = |at: usize| first_line + text[..at].matches('\n').count();
    let prefix_end = tags.first().map_or(text.len(), |(_, start, _)| *start);
    push_untagged(kind, &text[..prefix_end], first_line, units);
    for (index, (id, start, end)) in tags.iter().enumerate() {
        let covers_to = tags.get(index + 1).map_or(text.len(), |next| next.1);
        units.push(Unit {
            kind,
            id: Some(id.clone()),
            text: text[*end..covers_to].trim().to_string(),
            line: line_at(*start),
            cell: None,
        });
    }
}

fn push_untagged(kind: Kind, text: &str, line: usize, units: &mut Vec<Unit>) {
    let text = text.trim();
    if !text.is_empty() {
        units.push(Unit {
            kind,
            id: None,
            text: text.to_string(),
            line,
            cell: None,
        });
    }
}

/// A table: its header and delimiter rows as one unit, then one unit per row. A tag in a cell
/// covers the text up to the next tag or the end of the cell, and the rest of the row is the
/// row's untagged unit.
fn table_units(rows: &[&str], first_line: usize, units: &mut Vec<Unit>) {
    let header_rows = if rows.len() > 1 && is_delimiter_row(rows[1]) {
        2
    } else {
        0
    };
    if header_rows > 0 {
        units.push(Unit {
            kind: Kind::TableHeader,
            id: None,
            text: format!("{}\n{}", rows[0].trim(), rows[1].trim()),
            line: first_line,
            cell: None,
        });
    }
    for (index, row) in rows.iter().enumerate().skip(header_rows) {
        let line = first_line + index;
        let tags = tag_spans(row);
        let mut rest = String::new();
        let mut copied_to = 0;
        for (n, tag) in tags.iter().enumerate() {
            let cell_end = cell_end(row, tag.end);
            let covers_to = tags
                .get(n + 1)
                .map_or(cell_end, |next| next.start.min(cell_end));
            units.push(Unit {
                kind: Kind::TableRow,
                id: Some(tag.id.clone()),
                text: row[tag.end..covers_to].trim().to_string(),
                line,
                cell: Some(cell_number(row, tag.start)),
            });
            rest.push_str(&row[copied_to..tag.start]);
            copied_to = covers_to;
        }
        rest.push_str(&row[copied_to..]);
        push_untagged(Kind::TableRow, &rest, line, units);
    }
}

/// The byte offset of the `|` that ends the cell containing `from`, or the end of the row. A
/// `|` escaped with a backslash doesn't end a cell.
fn cell_end(row: &str, from: usize) -> usize {
    let bytes = row.as_bytes();
    (from..bytes.len())
        .find(|&i| is_cell_pipe(bytes, i))
        .unwrap_or(row.len())
}

/// The 1-based column of the cell holding byte `at` of a row that starts with `|`.
fn cell_number(row: &str, at: usize) -> usize {
    let bytes = row.as_bytes();
    (0..at).filter(|&i| is_cell_pipe(bytes, i)).count()
}

fn is_cell_pipe(bytes: &[u8], at: usize) -> bool {
    bytes[at] == b'|' && (at == 0 || bytes[at - 1] != b'\\')
}

/// The cells of a table row, trimmed, without the empty text outside its first and last `|`.
/// Escaped `\|` stays escaped.
pub fn cells(row: &str) -> Vec<String> {
    let bytes = row.as_bytes();
    let mut cells = Vec::new();
    let mut start = 0;
    for at in 0..bytes.len() {
        if is_cell_pipe(bytes, at) {
            cells.push(row[start..at].trim().to_string());
            start = at + 1;
        }
    }
    cells.push(row[start..].trim().to_string());
    if row.trim_start().starts_with('|') {
        cells.remove(0);
    }
    if row.trim_end().ends_with('|') && !row.trim_end().ends_with("\\|") {
        cells.pop();
    }
    cells
}

fn is_delimiter_row(row: &str) -> bool {
    let row = row.trim();
    row.contains('-') && row.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

/// The declaration units of a code block's lines. A unit is a line at column 0 and the lines
/// after it up to the next line at column 0. A line that only closes (`}`, `)`, `]`) belongs to
/// the unit before it, and a column-0 `//` comment line is a unit on its own. Blank lines
/// between units belong to none. Productions follow the same rule: a production's first line
/// is at column 0 and its continuation lines are indented.
fn code_units(lines: &[&str], first_line: usize, units: &mut Vec<Unit>) {
    let mut current: Vec<&str> = Vec::new();
    let mut start = 0;
    let mut blanks = 0;
    let flush = |current: &mut Vec<&str>, start: usize, units: &mut Vec<Unit>| {
        if !current.is_empty() {
            units.push(Unit {
                kind: Kind::Code,
                id: None,
                text: current.join("\n").trim().to_string(),
                line: start,
                cell: None,
            });
            current.clear();
        }
    };
    for (index, &line) in lines.iter().enumerate() {
        let line_no = first_line + index;
        if line.trim().is_empty() {
            blanks += 1;
            continue;
        }
        let at_column_0 = !line.starts_with(char::is_whitespace);
        let closes = line.trim().chars().all(|c| matches!(c, '}' | ')' | ']'));
        let starts_unit = current.is_empty() || (at_column_0 && !closes);
        if starts_unit {
            flush(&mut current, start, units);
            start = line_no;
        } else {
            current.extend(std::iter::repeat_n("", blanks));
        }
        blanks = 0;
        current.push(line);
        if at_column_0 && line.starts_with("//") {
            flush(&mut current, start, units);
        }
    }
    flush(&mut current, start, units);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(text: &str) -> Vec<(Kind, Option<String>, String, usize)> {
        summary_in(text, Form::Inline)
    }

    fn summary_in(text: &str, form: Form) -> Vec<(Kind, Option<String>, String, usize)> {
        segment(text, form)
            .into_iter()
            .map(|u| (u.kind, u.id, u.text, u.line))
            .collect()
    }

    fn unit(
        kind: Kind,
        id: Option<&str>,
        text: &str,
        line: usize,
    ) -> (Kind, Option<String>, String, usize) {
        (kind, id.map(str::to_string), text.to_string(), line)
    }

    #[test]
    fn paragraphs_and_tags() {
        let text = "## Bindings\n\nIntro text. r[bind.let] `let` binds. r[bind.var] `var` too.\n\nr[bind.x] Only a rule, `r[not.a-tag]`.\n";
        assert_eq!(
            summary(text),
            [
                unit(Kind::Heading, None, "## Bindings", 1),
                unit(Kind::Paragraph, None, "Intro text.", 3),
                unit(Kind::Paragraph, Some("bind.let"), "`let` binds.", 3),
                unit(Kind::Paragraph, Some("bind.var"), "`var` too.", 3),
                unit(
                    Kind::Paragraph,
                    Some("bind.x"),
                    "Only a rule, `r[not.a-tag]`.",
                    5
                ),
            ]
        );
    }

    #[test]
    fn list_items() {
        let text = "- r[a.b] First.\n* Second\n  continued.\n1. **One.** r[a.c] Numbered.\n";
        assert_eq!(
            summary(text),
            [
                unit(Kind::Item, Some("a.b"), "First.", 1),
                unit(Kind::Item, None, "Second\n  continued.", 2),
                unit(Kind::Numbered, None, "1. **One.**", 4),
                unit(Kind::Numbered, Some("a.c"), "Numbered.", 4),
            ]
        );
    }

    #[test]
    fn tables() {
        let text = "| Name | Rule |\n| --- | --- |\n| Int | r[lit.a] Default `Int`. r[lit.b] More. |\n| `a\\|b` | r[lit.c] Piped. | x |\n| Plain | row |\n";
        assert_eq!(
            summary(text),
            [
                unit(Kind::TableHeader, None, "| Name | Rule |\n| --- | --- |", 1),
                unit(Kind::TableRow, Some("lit.a"), "Default `Int`.", 3),
                unit(Kind::TableRow, Some("lit.b"), "More.", 3),
                unit(Kind::TableRow, None, "| Int | |", 3),
                unit(Kind::TableRow, Some("lit.c"), "Piped.", 4),
                unit(Kind::TableRow, None, "| `a\\|b` | | x |", 4),
                unit(Kind::TableRow, None, "| Plain | row |", 5),
            ]
        );
    }

    #[test]
    fn code_declarations() {
        let text = "```ebnf\nexpr = a\n     | b   // alt\n// comment\nterm = c\n```\n\n```\nfn f() {\n  let x = 1\n\n  x\n}\n\nstruct S { a: Int }\n  r[in.code] stays code\n)\n```\n";
        assert_eq!(
            summary(text),
            [
                unit(Kind::Code, None, "expr = a\n     | b   // alt", 2),
                unit(Kind::Code, None, "// comment", 4),
                unit(Kind::Code, None, "term = c", 5),
                unit(Kind::Code, None, "fn f() {\n  let x = 1\n\n  x\n}", 9),
                unit(
                    Kind::Code,
                    None,
                    "struct S { a: Int }\n  r[in.code] stays code\n)",
                    15
                ),
            ]
        );
    }

    #[test]
    fn table_cells_and_columns() {
        let row = "| Int | r[lit.a] A. | `a\\|b` | r[lit.b] B. |";
        let columns: Vec<_> = segment(row, Form::Inline)
            .into_iter()
            .map(|u| (u.id, u.cell))
            .collect();
        assert_eq!(
            columns,
            [
                (Some("lit.a".to_string()), Some(2)),
                (Some("lit.b".to_string()), Some(4)),
                (None, None),
            ]
        );
        assert_eq!(cells("| Int | | `a\\|b` | |"), ["Int", "", "`a\\|b`", ""]);
        assert_eq!(cells("| a \\|"), ["a \\|"]);
    }

    #[test]
    fn own_line_tags_cover_the_next_block() {
        let text = "## Bindings\n\nIntro text.\n\nr[bind.let]\n`let` binds\nover lines. r[not.inline] here.\n\nr[bind.var]\n- `var` too,\n  continued.\n- Untagged sibling.\n\nr[bind.x]\n* Starred.\n";
        assert_eq!(
            summary_in(text, Form::OwnLine),
            [
                unit(Kind::Heading, None, "## Bindings", 1),
                unit(Kind::Paragraph, None, "Intro text.", 3),
                unit(
                    Kind::Paragraph,
                    Some("bind.let"),
                    "`let` binds\nover lines. r[not.inline] here.",
                    5
                ),
                unit(Kind::Item, Some("bind.var"), "`var` too,\n  continued.", 9),
                unit(Kind::Item, None, "Untagged sibling.", 12),
                unit(Kind::Item, Some("bind.x"), "Starred.", 14),
            ]
        );
    }

    #[test]
    fn own_line_tags_over_nothing_cover_no_text() {
        for after in [
            "",
            "\n\nText.",
            "\nr[b.c]\nText.",
            "\n## H",
            "\n```\nx\n```",
            "\n| a |",
            "\n1. One.",
            "\n  Indented.",
        ] {
            let text = format!("r[a.b]{after}\n");
            let units = segment(&text, Form::OwnLine);
            assert_eq!(
                (
                    units[0].kind,
                    units[0].id.as_deref(),
                    units[0].text.as_str(),
                    units[0].line
                ),
                (Kind::Paragraph, Some("a.b"), "", 1),
                "{after:?}: {units:?}"
            );
        }
        // A tag line ends the paragraph before it.
        assert_eq!(
            summary_in("Text.\nr[a.b]\nRule.\n", Form::OwnLine),
            [
                unit(Kind::Paragraph, None, "Text.", 1),
                unit(Kind::Paragraph, Some("a.b"), "Rule.", 2),
            ]
        );
    }

    #[test]
    fn hash_without_space_is_a_paragraph() {
        assert_eq!(
            summary("#hashtag\n####### seven\n"),
            [unit(Kind::Paragraph, None, "#hashtag\n####### seven", 1)]
        );
    }
}
