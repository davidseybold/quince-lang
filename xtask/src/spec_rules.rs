//! `cargo xtask spec-rules`: checks the rule tags in the spec. `--strip` prints the spec
//! with every tag removed.

use quince_spec::rules;
use quince_spec::{RETIRED_PATH, SPEC_PATH, SpecError, TOPICS_PATH};
use std::path::Path;

/// The CI step: format, placement, topics, uniqueness and retired IDs. Problems in all three
/// files are reported together.
pub fn check(root: &Path) -> Result<(), String> {
    let spec = read(root, SPEC_PATH)?;
    let (retired, retired_errors) = rules::parse_retired(&read(root, RETIRED_PATH)?);
    let (topics, topic_errors) = rules::parse_topics(&read(root, TOPICS_PATH)?);
    let mut problems = report(RETIRED_PATH, &retired_errors) + report(TOPICS_PATH, &topic_errors);
    let count = match rules::check(&spec, &retired, &topics) {
        Ok(found) => found.len(),
        Err(errors) => {
            problems += report(SPEC_PATH, &errors);
            0
        }
    };
    let (parsed, _) = rules::parse(&spec);
    problems += report(TOPICS_PATH, &rules::unused_topics(&parsed, &topics));
    if problems > 0 {
        return Err(format!("{problems} problem(s) in the spec's rule IDs"));
    }
    eprintln!(
        "{SPEC_PATH}: {count} rule IDs under {} topics, all valid",
        topics.len()
    );
    Ok(())
}

/// Prints the spec with every tag removed, for checking that tagging changed no wording.
pub fn strip(root: &Path) -> Result<(), String> {
    print!("{}", rules::strip(&read(root, SPEC_PATH)?));
    Ok(())
}

pub fn read(root: &Path, relative: &str) -> Result<String, String> {
    std::fs::read_to_string(root.join(relative)).map_err(|e| format!("can't read {relative}: {e}"))
}

/// Prints every error as `path:line: message` and returns how many there were.
fn report(path: &str, errors: &[SpecError]) -> usize {
    for error in errors {
        eprintln!("{path}:{}: {}", error.line, error.message);
    }
    errors.len()
}
