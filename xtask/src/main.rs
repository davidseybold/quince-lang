//! Repo automation. `cargo xtask ci` runs every fast check; see AGENTS.md.

mod ci;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "\
usage: cargo xtask <command>

commands:
  ci    run every check (fmt, clippy, test) and print a summary
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = workspace_root();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["ci"] => ci::run(&root),
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

/// The workspace root: xtask lives one folder below it.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives one folder below the workspace root")
        .to_path_buf()
}
