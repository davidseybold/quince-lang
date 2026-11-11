//! The normalizations the own-line tag format needs (docs/design/spec-restructure.md, decision
//! 13): leading bold labels split off tagged units, on both sides, and the baseline's tagged
//! tables rebuilt as the lists they became, from the patterns in `tables.tsv`.

use super::TABLES;
use super::segment::{Kind, Unit, cells};
use std::collections::{HashMap, HashSet};

/// A row of `tables.tsv`: a baseline table that became a list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertedTable {
    /// The table row, as `path:line`.
    pub at: String,
    /// The chapter file that holds the list.
    pub path: String,
    /// The baseline line of the table's header row.
    pub header_line: usize,
    pub pattern: Vec<Piece>,
}

/// A piece of a `tables.tsv` pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piece {
    Text(String),
    /// `{n}`: the text of column n, counting from 1.
    Column(usize),
    /// `[...]`: dropped when every column in it is empty.
    Optional(Vec<Piece>),
}

/// Parses a pattern such as `{1} ({2}): {3}` or `{1}[ ({2})]`. A pattern can't hold a literal
/// `[`, `]`, `{` or `}`.
pub fn parse_pattern(text: &str) -> Result<Vec<Piece>, String> {
    let mut top: Vec<Piece> = Vec::new();
    let mut optional: Option<Vec<Piece>> = None;
    let mut literal = String::new();
    let mut chars = text.chars();
    let flush = |literal: &mut String, pieces: &mut Vec<Piece>| {
        if !literal.is_empty() {
            pieces.push(Piece::Text(std::mem::take(literal)));
        }
    };
    while let Some(c) = chars.next() {
        match c {
            '[' if optional.is_some() => {
                return Err("`[` inside `[...]`; optional pieces don't nest".to_string());
            }
            '[' => {
                flush(&mut literal, &mut top);
                optional = Some(Vec::new());
            }
            ']' => {
                let Some(mut pieces) = optional.take() else {
                    return Err("`]` without its `[`".to_string());
                };
                flush(&mut literal, &mut pieces);
                if !pieces.iter().any(|p| matches!(p, Piece::Column(_))) {
                    return Err("an optional piece `[...]` holds no `{n}`".to_string());
                }
                top.push(Piece::Optional(pieces));
            }
            '{' => {
                let current = optional.as_mut().unwrap_or(&mut top);
                flush(&mut literal, current);
                let mut digits = String::new();
                let mut closed = false;
                for c in chars.by_ref() {
                    if c == '}' {
                        closed = true;
                        break;
                    }
                    digits.push(c);
                }
                match digits.parse::<usize>() {
                    _ if !closed => return Err(format!("`{{{digits}` has no closing `}}`")),
                    Ok(n) if n > 0 => current.push(Piece::Column(n)),
                    _ => return Err(format!("`{{{digits}}}` isn't a column such as `{{1}}`")),
                }
            }
            '}' => return Err("`}` without its `{`".to_string()),
            _ => literal.push(c),
        }
    }
    if optional.is_some() {
        return Err("`[` without its `]`".to_string());
    }
    flush(&mut literal, &mut top);
    if !top.iter().any(|p| matches!(p, Piece::Column(_))) {
        return Err("a pattern needs a column outside brackets, such as `{1}`".to_string());
    }
    Ok(top)
}

/// The highest column a pattern names.
fn max_column(pattern: &[Piece]) -> usize {
    pattern
        .iter()
        .map(|piece| match piece {
            Piece::Text(_) => 0,
            Piece::Column(n) => *n,
            Piece::Optional(pieces) => max_column(pieces),
        })
        .max()
        .unwrap_or(0)
}

/// The text `pattern` builds from `column`, or the first column outside brackets that is empty.
fn render(pattern: &[Piece], column: &dyn Fn(usize) -> String) -> Result<String, usize> {
    let mut out = String::new();
    for piece in pattern {
        match piece {
            Piece::Text(text) => out.push_str(text),
            Piece::Column(n) => {
                let text = column(*n);
                if text.is_empty() {
                    return Err(*n);
                }
                out.push_str(&text);
            }
            Piece::Optional(pieces) => {
                let filled = pieces.iter().any(|p| match p {
                    Piece::Column(n) => !column(*n).is_empty(),
                    _ => false,
                });
                if filled {
                    for p in pieces {
                        match p {
                            Piece::Text(text) => out.push_str(text),
                            Piece::Column(n) => out.push_str(&column(*n)),
                            Piece::Optional(_) => {}
                        }
                    }
                }
            }
        }
    }
    Ok(out)
}

/// An item a converted table row became, which its `tables.tsv` row says is in `path`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rebuilt {
    /// The `tables.tsv` row.
    pub at: String,
    pub path: String,
    /// The baseline line of the table row.
    pub line: usize,
    pub id: Option<String>,
    pub text: String,
}

/// The baseline units with each converted table replaced by its items. A row becomes an item
/// built by the table's pattern from its untagged cells and its first rule, with that rule's ID
/// or none; each later rule in the row becomes an item of its own; and the header row drops
/// out. `\|` becomes `|`. Problems go to `problems`, and a row that can't be rebuilt keeps its
/// table units, so the comparison reports it too.
pub fn convert_tables(
    units: Vec<Unit>,
    converted: &[ConvertedTable],
    problems: &mut Vec<String>,
) -> (Vec<Unit>, Vec<Rebuilt>) {
    let mut headers: HashSet<usize> = HashSet::new();
    let mut rows: HashMap<usize, Vec<Unit>> = HashMap::new();
    let mut rebuilt = Vec::new();
    for table in converted {
        let Some(header) = units
            .iter()
            .find(|u| u.kind == Kind::TableHeader && u.line == table.header_line)
        else {
            problems.push(format!(
                "{}: no table header row is on baseline line {}",
                table.at, table.header_line
            ));
            continue;
        };
        let width = cells(header.text.lines().next().unwrap_or_default()).len();
        if max_column(&table.pattern) > width {
            problems.push(format!(
                "{}: the pattern names column {}, but the table has {width}",
                table.at,
                max_column(&table.pattern)
            ));
            continue;
        }
        headers.insert(table.header_line);
        let mut line = table.header_line + 2;
        loop {
            let row: Vec<&Unit> = units
                .iter()
                .filter(|u| u.kind == Kind::TableRow && u.line == line)
                .collect();
            if row.is_empty() {
                break;
            }
            match rebuild_row(table, line, &row) {
                Ok(items) => {
                    rebuilt.extend(items.iter().map(|item| Rebuilt {
                        at: table.at.clone(),
                        path: table.path.clone(),
                        line,
                        id: item.id.clone(),
                        text: item.text.clone(),
                    }));
                    rows.insert(line, items);
                }
                Err(problem) => problems.push(problem),
            }
            line += 1;
        }
    }
    let mut out = Vec::with_capacity(units.len());
    for unit in units {
        match unit.kind {
            Kind::TableHeader if headers.contains(&unit.line) => {}
            Kind::TableRow if rows.contains_key(&unit.line) => {
                if let Some(items) = rows.get_mut(&unit.line) {
                    out.append(items);
                }
            }
            _ => out.push(unit),
        }
    }
    (out, rebuilt)
}

/// The items one baseline row becomes.
fn rebuild_row(table: &ConvertedTable, line: usize, row: &[&Unit]) -> Result<Vec<Unit>, String> {
    let at = format!("{} (baseline line {line})", table.at);
    let leftover = row
        .iter()
        .find(|u| u.id.is_none())
        .map(|u| cells(&u.text))
        .unwrap_or_default();
    let tagged: Vec<&&Unit> = row.iter().filter(|u| u.id.is_some()).collect();
    for unit in &tagged {
        let cell = unit.cell.unwrap_or_default();
        if leftover
            .get(cell.wrapping_sub(1))
            .is_some_and(|text| !text.is_empty())
        {
            return Err(format!(
                "{at}: column {cell} holds untagged text before its first tag; a converted cell \
                 is wholly untagged, starts with the row's first rule, or holds only later rules"
            ));
        }
    }
    let first = tagged.first();
    let column = |n: usize| -> String {
        let text = match first {
            Some(unit) if unit.cell == Some(n) => unit.text.as_str(),
            _ => leftover.get(n - 1).map(String::as_str).unwrap_or_default(),
        };
        unescape(text)
    };
    let text = render(&table.pattern, &column).map_err(|n| {
        format!(
            "{at}: column {n} is empty, but the pattern needs it; put the piece that holds `{{{n}}}` \
             in brackets"
        )
    })?;
    let mut items = vec![Unit {
        kind: Kind::Item,
        id: first.and_then(|unit| unit.id.clone()),
        text,
        line,
        cell: None,
    }];
    items.extend(tagged.iter().skip(1).map(|unit| Unit {
        kind: Kind::Item,
        id: unit.id.clone(),
        text: unescape(&unit.text),
        line,
        cell: None,
    }));
    Ok(items)
}

fn unescape(text: &str) -> String {
    text.replace("\\|", "|")
}

/// Splits the leading bold run or runs of each tagged unit (`**Label.**`, or `**Quince**
/// **memory never escapes to C.**`) into an untagged unit of the same kind before it, when text
/// follows them.
pub fn split_labels(units: Vec<Unit>) -> Vec<Unit> {
    let mut out = Vec::with_capacity(units.len());
    for mut unit in units {
        if unit.id.is_some()
            && let Some(end) = label_end(&unit.text)
        {
            out.push(Unit {
                kind: unit.kind,
                id: None,
                text: unit.text[..end].trim().to_string(),
                line: unit.line,
                cell: None,
            });
            unit.text = unit.text[end..].trim().to_string();
        }
        out.push(unit);
    }
    out
}

/// The end of the bold runs that start `text`, separated only by whitespace, when text follows
/// them.
fn label_end(text: &str) -> Option<usize> {
    let mut end = 0;
    loop {
        let rest = &text[end..];
        let start = end + (rest.len() - rest.trim_start().len());
        if !text[start..].starts_with("**") {
            break;
        }
        match text[start + 2..].find("**") {
            Some(close) => end = start + 2 + close + 2,
            None => break,
        }
    }
    (end > 0 && !text[end..].trim().is_empty()).then_some(end)
}

/// Parses `tables.tsv`: the chapter file holding the list, the baseline line of the table's
/// header row (`L120`) and the pattern. Columns are tab-separated; blank lines and `#` comments
/// are ignored.
pub fn parse_tables(text: &str) -> (Vec<ConvertedTable>, Vec<String>) {
    let mut tables = Vec::new();
    let mut errors = Vec::new();
    for (line, columns) in super::rows(text) {
        let at = format!("{TABLES}:{line}");
        let header_line = |unit: &str| unit.strip_prefix('L').and_then(|n| n.parse().ok());
        match columns.as_slice() {
            [path, unit, pattern] if !path.is_empty() => match header_line(unit) {
                Some(header_line) => match parse_pattern(pattern) {
                    Ok(pattern) => tables.push(ConvertedTable {
                        at,
                        path: path.to_string(),
                        header_line,
                        pattern,
                    }),
                    Err(e) => errors.push(format!("{at}: bad pattern: {e}")),
                },
                None => errors.push(format!("{at}: `{unit}` is not a line such as `L120`")),
            },
            _ => errors.push(format!(
                "{at}: a row is the chapter file, the baseline line of the header row and the pattern"
            )),
        }
    }
    (tables, errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns() {
        assert_eq!(
            parse_pattern("{1} ({2}): {3}"),
            Ok(vec![
                Piece::Column(1),
                Piece::Text(" (".into()),
                Piece::Column(2),
                Piece::Text("): ".into()),
                Piece::Column(3),
            ])
        );
        assert_eq!(
            parse_pattern("`O0`: {1}[; {2}]"),
            Ok(vec![
                Piece::Text("`O0`: ".into()),
                Piece::Column(1),
                Piece::Optional(vec![Piece::Text("; ".into()), Piece::Column(2)]),
            ])
        );
        for (bad, why) in [
            (
                "{1} [a [{2}]]",
                "`[` inside `[...]`; optional pieces don't nest",
            ),
            ("{1}]", "`]` without its `[`"),
            ("{1}[ (x)]", "an optional piece `[...]` holds no `{n}`"),
            ("{1}[ ({2})", "`[` without its `]`"),
            ("{0}", "`{0}` isn't a column such as `{1}`"),
            ("{a}", "`{a}` isn't a column such as `{1}`"),
            ("{1", "`{1` has no closing `}`"),
            ("{1}}", "`}` without its `{`"),
            (
                "[{1}]",
                "a pattern needs a column outside brackets, such as `{1}`",
            ),
        ] {
            assert_eq!(parse_pattern(bad), Err(why.to_string()), "{bad}");
        }
    }

    #[test]
    fn rendering() {
        let pattern = parse_pattern("{1}[ ({2})]: {3}").unwrap();
        let columns = |values: [&'static str; 3]| move |n: usize| values[n - 1].to_string();
        assert_eq!(
            render(&pattern, &columns(["a", "b", "c"])),
            Ok("a (b): c".into())
        );
        assert_eq!(
            render(&pattern, &columns(["a", "", "c"])),
            Ok("a: c".into())
        );
        assert_eq!(render(&pattern, &columns(["a", "b", ""])), Err(3));
    }

    #[test]
    fn labels() {
        let unit = |id: Option<&str>, text: &str| Unit {
            kind: Kind::Item,
            id: id.map(str::to_string),
            text: text.into(),
            line: 1,
            cell: None,
        };
        let split: Vec<_> = split_labels(vec![
            unit(Some("a.b"), "**Local mutation.** `var` binds."),
            unit(
                Some("a.c"),
                "**Quince** **memory never escapes to C.** Even here.",
            ),
            unit(Some("a.d"), "**Only bold.**"),
            unit(Some("a.e"), "**Unclosed bold."),
            unit(None, "**Untagged.** Text."),
        ])
        .into_iter()
        .map(|u| (u.id, u.text))
        .collect();
        let pair = |id: Option<&str>, text: &str| (id.map(str::to_string), text.to_string());
        assert_eq!(
            split,
            [
                pair(None, "**Local mutation.**"),
                pair(Some("a.b"), "`var` binds."),
                pair(None, "**Quince** **memory never escapes to C.**"),
                pair(Some("a.c"), "Even here."),
                pair(Some("a.d"), "**Only bold.**"),
                pair(Some("a.e"), "**Unclosed bold."),
                pair(None, "**Untagged.** Text."),
            ]
        );
    }

    #[test]
    fn table_rows() {
        let (tables, errors) = parse_tables(
            "# file\theader\tpattern\nspec/a.md\tL12\t{1}: {2}\nspec/a.md\t12\t{1}\nspec/a.md\tL3\t{1\nspec/a.md\tL3\n",
        );
        assert_eq!(
            tables,
            [ConvertedTable {
                at: "docs/design/spec-restructure/tables.tsv:2".into(),
                path: "spec/a.md".into(),
                header_line: 12,
                pattern: vec![Piece::Column(1), Piece::Text(": ".into()), Piece::Column(2)],
            }]
        );
        assert_eq!(
            errors,
            [
                "docs/design/spec-restructure/tables.tsv:3: `12` is not a line such as `L120`",
                "docs/design/spec-restructure/tables.tsv:4: bad pattern: `{1` has no closing `}`",
                "docs/design/spec-restructure/tables.tsv:5: a row is the chapter file, the baseline line of the header row and the pattern",
            ]
        );
    }
}
