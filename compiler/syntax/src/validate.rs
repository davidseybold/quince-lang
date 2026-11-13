//! The checks that run on the finished tree. Only the naming case so far
//! (`r[lex.name.case]`, `r[lex.name.enforced]`): the README's "Naming case" section has the
//! contract.

use crate::{SyntaxError, SyntaxKind, SyntaxNode, codes};
use SyntaxKind::*;

/// The errors the checks find in `root`, in source order.
pub(crate) fn validate(root: &SyntaxNode) -> Vec<SyntaxError> {
    root.descendants()
        .filter(|node| node.kind() == Name)
        .filter_map(|name| case_error(&name))
        .collect()
}

/// The three naming classes (`r[lex.name.snake]`, `r[lex.name.pascal]`,
/// `r[lex.name.screaming]`). No identifier is in two of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Case {
    Snake,
    Pascal,
    Screaming,
}

impl Case {
    fn label(self) -> &'static str {
        match self {
            Case::Snake => "snake_case",
            Case::Pascal => "PascalCase",
            Case::Screaming => "SCREAMING_SNAKE_CASE",
        }
    }

    /// Whether `name` is of this class.
    pub(crate) fn fits(self, name: &str) -> bool {
        let letters = name.chars().filter(char::is_ascii_alphabetic).count();
        let starts = |first: fn(&char) -> bool| name.chars().next().as_ref().is_some_and(first);
        // Letters of one case, digits, and single underscores between them.
        let words = |case: fn(&char) -> bool| {
            name.chars()
                .all(|c| case(&c) || c.is_ascii_digit() || c == '_')
                && !name.contains("__")
                && !name.ends_with('_')
        };
        match self {
            Case::Snake => starts(char::is_ascii_lowercase) && words(char::is_ascii_lowercase),
            Case::Screaming => {
                starts(char::is_ascii_uppercase) && words(char::is_ascii_uppercase) && letters >= 2
            }
            Case::Pascal => {
                starts(char::is_ascii_uppercase)
                    && name.chars().all(|c| c.is_ascii_alphanumeric())
                    && (letters < 2 || name.chars().any(|c| c.is_ascii_lowercase()))
            }
        }
    }

    /// `name` written in this class, word by word, when that gives a name of the class that
    /// isn't a keyword. `const N` has none, since `SCREAMING_SNAKE_CASE` takes two letters.
    pub(crate) fn convert(self, name: &str) -> Option<String> {
        let words = words(name);
        let converted = match self {
            Case::Snake => words.join("_").to_ascii_lowercase(),
            Case::Screaming => words.join("_").to_ascii_uppercase(),
            Case::Pascal => words
                .iter()
                .map(|word| {
                    let lower = word.to_ascii_lowercase();
                    lower[..1].to_ascii_uppercase() + &lower[1..]
                })
                .collect(),
        };
        (self.fits(&converted) && SyntaxKind::keyword(&converted).is_none()).then_some(converted)
    }
}

/// The words of `name`: its runs between underscores, each split where a lowercase letter or
/// a digit meets an uppercase letter (`fooBar`, `utf8Len`) and before the last capital of a
/// run followed by a lowercase letter (`HTTPServer`). Digits stay with the word before them.
fn words(name: &str) -> Vec<&str> {
    let mut words = Vec::new();
    for run in name.split('_').filter(|run| !run.is_empty()) {
        let bytes = run.as_bytes();
        let mut start = 0;
        for i in 1..bytes.len() {
            let (before, at) = (bytes[i - 1], bytes[i]);
            let next_lower = bytes.get(i + 1).is_some_and(u8::is_ascii_lowercase);
            let boundary = at.is_ascii_uppercase()
                && (before.is_ascii_lowercase()
                    || before.is_ascii_digit()
                    || (before.is_ascii_uppercase() && next_lower));
            if boundary {
                words.push(&run[start..i]);
                start = i;
            }
        }
        words.push(&run[start..]);
    }
    words
}

/// The class a `Name` takes where it is declared, and what it names, for the message. `None`
/// in a foreign block, whose names keep their C spelling (`r[lex.name.foreign-exempt]`).
pub(crate) fn class_of(name: &SyntaxNode) -> Option<(Case, &'static str)> {
    if name.ancestors().any(|node| node.kind() == ForeignDecl) {
        return None;
    }
    let parent = name.parent()?;
    let class = match parent.kind() {
        FnDecl => (Case::Snake, "a function"),
        Param => (Case::Snake, "a parameter"),
        BindPat | VarStmt => (Case::Snake, "a binding"),
        Contract => (Case::Snake, "a return value"),
        FieldDecl => (Case::Snake, "a field"),
        Import => (Case::Snake, "a package"),
        ConstDecl => (Case::Screaming, "a constant"),
        StructDecl | NewtypeDecl | EnumDecl | AliasDecl | AssocType => (Case::Pascal, "a type"),
        InterfaceDecl => (Case::Pascal, "an interface"),
        EffectDecl => (Case::Pascal, "an effect"),
        Variant => (Case::Pascal, "a variant"),
        GenericParam if parent.children_with_tokens().any(|t| t.kind() == UsesKw) => {
            (Case::Pascal, "an effect variable")
        }
        GenericParam => (Case::Pascal, "a type parameter"),
        _ => return None,
    };
    Some(class)
}

/// E0021 for a `Name` that isn't of the class its position takes.
fn case_error(name: &SyntaxNode) -> Option<SyntaxError> {
    let (case, what) = class_of(name)?;
    let text = name.text().to_string();
    if case.fits(&text) {
        return None;
    }
    let label = case.label();
    let message = match case.convert(&text) {
        Some(converted) => {
            format!("`{text}` isn't {label}, the case of {what}'s name: write `{converted}`")
        }
        None => format!("`{text}` isn't {label}, the case of {what}'s name"),
    };
    let range = name.text_range();
    Some(SyntaxError::new(
        usize::from(range.start())..usize::from(range.end()),
        codes::NAME_CASE,
        message,
    ))
}
