//! `cargo xtask spec-grammar`: checks that `spec/grammar.ebnf` matches the spec's grammar
//! appendices, that the grammar is closed, and that no production reuses a retired name.
//! `--write` regenerates the file.

use crate::spec_rules::read;
use quince_spec::{GRAMMAR_PATH, RETIRED_PATH, SPEC_PATH, grammar, rules};
use std::io::ErrorKind;
use std::path::Path;

pub fn check(root: &Path) -> Result<(), String> {
    let extracted = extract(root)?;
    let file = read(root, GRAMMAR_PATH)?;
    let (header, actual) = grammar::split_header(&file);
    if actual != extracted.text {
        eprintln!(
            "{GRAMMAR_PATH} differs from the grammar appendices in {SPEC_PATH} (- file, + spec):"
        );
        eprint!("{}", grammar::diff(actual, &extracted.text));
        return Err(format!(
            "{GRAMMAR_PATH} is out of date; run `cargo xtask spec-grammar --write`"
        ));
    }
    let (retired, _) = rules::parse_retired(&read(root, RETIRED_PATH)?);
    let spec_line = |line| extracted.spec_line(line);
    let mut errors = grammar::check_closure(actual, &grammar::terminals(header), &spec_line);
    errors.extend(grammar::check_retired(actual, &retired, &spec_line));
    for error in &errors {
        eprintln!("{SPEC_PATH}:{}: {}", error.line, error.message);
    }
    if !errors.is_empty() {
        eprintln!(
            "Fix the grammar in {SPEC_PATH} (or a terminal in the header of {GRAMMAR_PATH}), \
             then run `cargo xtask spec-grammar --write`."
        );
        return Err(format!("{} problem(s) in the grammar", errors.len()));
    }
    let count = grammar::productions(actual).len();
    eprintln!("{GRAMMAR_PATH}: {count} productions, matches the spec, closed");
    Ok(())
}

/// Rewrites the grammar part of the file from the spec, keeping its header.
pub fn write(root: &Path) -> Result<(), String> {
    let extracted = extract(root)?;
    let path = root.join(GRAMMAR_PATH);
    let existing = match std::fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == ErrorKind::NotFound => None,
        Err(e) => return Err(format!("can't read {GRAMMAR_PATH}: {e}")),
    };
    std::fs::write(
        &path,
        grammar::regenerate(existing.as_deref(), &extracted.text),
    )
    .map_err(|e| format!("can't write {GRAMMAR_PATH}: {e}"))?;
    eprintln!("wrote {GRAMMAR_PATH}");
    Ok(())
}

fn extract(root: &Path) -> Result<grammar::Extracted, String> {
    grammar::extract(&read(root, SPEC_PATH)?).map_err(|e| match e.line {
        // Line 0 means the problem has no line, such as a missing heading.
        0 => format!("{SPEC_PATH}: {}", e.message),
        line => format!("{SPEC_PATH}:{line}: {}", e.message),
    })
}
