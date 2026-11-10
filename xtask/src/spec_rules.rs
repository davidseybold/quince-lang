//! `cargo xtask spec-rules`: checks the rule tags in the spec. `--strip` prints the spec
//! with every tag removed.

use std::path::Path;

pub const SPEC_PATH: &str = "spec/quince-spec.md";
pub const RETIRED_PATH: &str = "spec/retired-rules.txt";

/// The CI step: format, placement, uniqueness and retired IDs.
pub fn check(root: &Path) -> Result<(), String> {
    let spec = read(root, SPEC_PATH)?;
    let retired_text = read(root, RETIRED_PATH)?;
    let retired = quince_spec::rules::parse_retired(&retired_text)
        .map_err(|errors| report(RETIRED_PATH, &errors))?;
    let rules =
        quince_spec::rules::check(&spec, &retired).map_err(|errors| report(SPEC_PATH, &errors))?;
    eprintln!("{SPEC_PATH}: {} rule IDs, all valid", rules.len());
    Ok(())
}

/// Prints the spec with every tag removed, for checking that tagging changed no wording.
pub fn strip(root: &Path) -> Result<(), String> {
    print!("{}", quince_spec::rules::strip(&read(root, SPEC_PATH)?));
    Ok(())
}

pub fn read(root: &Path, relative: &str) -> Result<String, String> {
    std::fs::read_to_string(root.join(relative)).map_err(|e| format!("can't read {relative}: {e}"))
}

/// Prints every error as `path:line: message` and returns the one-line step summary.
fn report(path: &str, errors: &[quince_spec::SpecError]) -> String {
    for error in errors {
        eprintln!("{path}:{}: {}", error.line, error.message);
    }
    format!("{} problem(s) in {path}", errors.len())
}
