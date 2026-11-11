//! Repo automation. `cargo xtask ci` runs every fast check; see AGENTS.md.

mod ci;
mod conformance;
mod diagnostics;
mod spec_grammar;
mod spec_links;
mod spec_rules;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "\
usage: cargo xtask <command>

commands:
  ci                    run every check and print a summary
  spec-rules            check spec/chapters.txt and the rule-ID tags in every chapter
  spec-rules --strip [<file>]
                        print one chapter file, or every one, with its rule-ID tags removed
  spec-grammar          check spec/grammar.ebnf against the spec's `ebnf` blocks
  spec-grammar --write  regenerate spec/grammar.ebnf from the spec
  spec-links            check that the spec's relative links and their #fragments resolve
  diagnostics           check every diagnostic code has explain text and a conformance test
  conformance           run the conformance fixtures and write the spec coverage report
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = workspace_root();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["ci"] => ci::run(&root),
        ["spec-rules"] => exit(spec_rules::check(&root)),
        ["spec-rules", "--strip"] => exit(spec_rules::strip(&root, None)),
        ["spec-rules", "--strip", file] => exit(spec_rules::strip(&root, Some(file))),
        ["spec-grammar"] => exit(spec_grammar::check(&root)),
        ["spec-grammar", "--write"] => exit(spec_grammar::write(&root)),
        ["spec-links"] => exit(spec_links::check(&root)),
        ["diagnostics"] => exit(diagnostics::check(&root)),
        ["conformance"] => exit(conformance::check(&root)),
        ["help" | "--help" | "-h"] => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        _ => {
            eprint!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

/// Exit status for a single step run on its own.
fn exit(result: Result<(), String>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("error: {why}");
            ExitCode::FAILURE
        }
    }
}

/// The workspace root: xtask lives one folder below it.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives one folder below the workspace root")
        .to_path_buf()
}
