//! `cargo xtask spec-grammar`: checks that `spec/grammar.ebnf` matches the spec's grammar
//! appendices and that every name it uses is defined. `--write` regenerates the file.

use crate::spec_rules::{SPEC_PATH, read};
use quince_spec::grammar;
use std::path::Path;

pub const GRAMMAR_PATH: &str = "spec/grammar.ebnf";

/// Used by `--write` when the file doesn't exist yet.
const DEFAULT_HEADER: &str = "\
// The Quince grammar, copied verbatim from the two grammar appendices of spec/quince-spec.md.
// Don't edit below this header by hand: change the spec, then run `cargo xtask spec-grammar --write`.
// `cargo xtask spec-grammar` fails when this file and the spec disagree.
//
// Notation: `{ x }` is zero or more, `[ x ]` is optional. The names below are defined in the
// spec's prose, not as productions. `path` is `name { \".\" name }`, a package path.
// terminals: name Name NAME c_name path literal string int doc_comment NEWLINE
";

pub fn check(root: &Path) -> Result<(), String> {
    let expected = extract(root)?;
    let file = read(root, GRAMMAR_PATH)?;
    let (header, actual) = grammar::split_header(&file);
    if actual != expected {
        eprintln!(
            "{GRAMMAR_PATH} differs from the grammar appendices in {SPEC_PATH} (- file, + spec):"
        );
        eprint!("{}", grammar::diff(actual, &expected));
        return Err(format!(
            "{GRAMMAR_PATH} is out of date; run `cargo xtask spec-grammar --write`"
        ));
    }
    let errors = grammar::check_closure(actual, &grammar::terminals(header));
    let header_lines = header.lines().count() + 1;
    for error in &errors {
        eprintln!(
            "{GRAMMAR_PATH}:{}: {}",
            error.line + header_lines,
            error.message
        );
    }
    if !errors.is_empty() {
        return Err(format!("{} problem(s) in {GRAMMAR_PATH}", errors.len()));
    }
    let count = grammar::productions(actual).len();
    eprintln!("{GRAMMAR_PATH}: {count} productions, matches the spec, every name defined");
    Ok(())
}

/// Rewrites the grammar part of the file from the spec, keeping its header.
pub fn write(root: &Path) -> Result<(), String> {
    let expected = extract(root)?;
    let path = root.join(GRAMMAR_PATH);
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let (header, _) = grammar::split_header(&existing);
    let header = if header.is_empty() {
        DEFAULT_HEADER
    } else {
        header
    };
    std::fs::write(&path, format!("{header}\n{expected}"))
        .map_err(|e| format!("can't write {GRAMMAR_PATH}: {e}"))?;
    eprintln!("wrote {GRAMMAR_PATH}");
    Ok(())
}

fn extract(root: &Path) -> Result<String, String> {
    grammar::extract(&read(root, SPEC_PATH)?).map_err(|e| format!("{SPEC_PATH}: {}", e.message))
}
