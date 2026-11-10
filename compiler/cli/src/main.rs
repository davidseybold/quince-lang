//! The `quince` command-line tool. Only `--version` works so far.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [flag] if flag == "--version" => {
            println!("quince {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!(
                "quince: no commands are implemented yet; `quince --version` is the only one"
            );
            ExitCode::from(2)
        }
    }
}
