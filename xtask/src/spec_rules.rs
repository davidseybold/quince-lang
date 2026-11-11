//! `cargo xtask spec-rules`: checks the chapter manifest and the rule tags in every chapter
//! file. `--strip` prints the spec with every tag removed.

use quince_spec::{
    CHAPTERS_PATH, FileError, MOVED_PATH, README_PATH, RETIRED_PATH, Spec, SpecError,
};
use quince_spec::{manifest, rules};
use std::path::Path;

/// The CI step: the manifest, the README's chapter list, and every tag's format, placement,
/// chapter, construct and uniqueness, and the moved-rules list. Problems in every file are
/// reported together.
pub fn check(root: &Path) -> Result<(), String> {
    let spec = load(root)?;
    let (retired, retired_errors) = rules::parse_retired(&read(root, RETIRED_PATH)?);
    let (moved, moved_errors) = rules::parse_moved(&read(root, MOVED_PATH)?);
    let mut problems = report(RETIRED_PATH, &retired_errors) + report(MOVED_PATH, &moved_errors);
    problems += report(
        README_PATH,
        &manifest::check_readme(&spec.readme, &spec.files),
    );
    let (found, errors) = rules::check(&spec.files, &retired, &spec.chapters);
    problems += report_located(&errors);
    problems += report_located(&rules::check_moved(&moved, &found, &retired));
    problems += report(
        CHAPTERS_PATH,
        &rules::unused_constructs(&found, &spec.chapters),
    );
    if problems > 0 {
        return Err(format!("{problems} problem(s) in the spec's rule IDs"));
    }
    eprintln!(
        "spec: {} rule IDs in {} chapter file(s) of {} chapters, all valid",
        found.len(),
        spec.files.len(),
        spec.chapters.len()
    );
    Ok(())
}

/// Prints one chapter file with every tag removed, or with no file, every chapter file in
/// manifest order, each after a `<!-- spec/<path> -->` line. For checking that a change to tags
/// changed no wording.
pub fn strip(root: &Path, file: Option<&str>) -> Result<(), String> {
    let spec = load(root)?;
    match file {
        Some(path) => {
            let chapter = spec.file(path).ok_or_else(|| {
                format!("{path} is not a chapter file listed in spec/chapters.txt")
            })?;
            print!("{}", rules::strip(&chapter.text));
        }
        None => {
            for chapter in &spec.files {
                println!("<!-- {} -->", chapter.path);
                print!("{}", rules::strip(&chapter.text));
            }
        }
    }
    Ok(())
}

/// Loads the spec, printing every problem with the manifest or its files.
pub fn load(root: &Path) -> Result<Spec, String> {
    Spec::load(root).map_err(|errors| {
        report_located(&errors);
        format!("{} problem(s) loading the spec", errors.len())
    })
}

pub fn read(root: &Path, relative: &str) -> Result<String, String> {
    std::fs::read_to_string(root.join(relative)).map_err(|e| format!("can't read {relative}: {e}"))
}

/// Prints every error as `path:line: message` and returns how many there were.
fn report(path: &str, errors: &[SpecError]) -> usize {
    let located: Vec<_> = errors
        .iter()
        .map(|e| FileError::new(path, e.clone()))
        .collect();
    report_located(&located)
}

/// Prints every error and returns how many there were.
pub fn report_located(errors: &[FileError]) -> usize {
    for error in errors {
        eprintln!("{error}");
    }
    errors.len()
}
