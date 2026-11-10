//! `cargo xtask ci`: runs every step, even after one fails, then prints a summary table.

use std::fmt::Write as _;
use std::path::Path;
use std::process::{Command, ExitCode};
use std::time::{Duration, Instant};

/// One check. It returns `Err` with a one-line reason when it fails; its own output goes to
/// the terminal as it runs.
pub struct Step {
    pub name: &'static str,
    pub run: fn(&Path) -> Result<(), String>,
}

pub struct StepResult {
    pub name: &'static str,
    pub outcome: Result<(), String>,
    pub elapsed: Duration,
}

pub fn steps() -> Vec<Step> {
    vec![
        Step {
            name: "fmt",
            run: |root| cargo(root, &["fmt", "--all", "--check"]),
        },
        Step {
            name: "clippy",
            run: |root| {
                cargo(
                    root,
                    &[
                        "clippy",
                        "--workspace",
                        "--all-targets",
                        "--",
                        "-D",
                        "warnings",
                    ],
                )
            },
        },
        Step {
            name: "test",
            run: |root| cargo(root, &["test", "--workspace"]),
        },
        Step {
            name: "spec-rules",
            run: crate::spec_rules::check,
        },
        Step {
            name: "spec-grammar",
            run: crate::spec_grammar::check,
        },
    ]
}

pub fn run(root: &Path) -> ExitCode {
    let results: Vec<StepResult> = steps()
        .into_iter()
        .map(|step| {
            eprintln!("\n==> {}", step.name);
            let start = Instant::now();
            let outcome = (step.run)(root);
            StepResult {
                name: step.name,
                outcome,
                elapsed: start.elapsed(),
            }
        })
        .collect();
    eprint!("{}", summary(&results));
    if results.iter().all(|r| r.outcome.is_ok()) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// The table printed at the end of a run: one line per step, then a total.
pub fn summary(results: &[StepResult]) -> String {
    let width = results.iter().map(|r| r.name.len()).max().unwrap_or(0);
    let mut out = String::from("\nci summary\n");
    for r in results {
        let (status, reason) = match &r.outcome {
            Ok(()) => ("ok  ", String::new()),
            Err(why) => ("FAIL", format!("  {why}")),
        };
        let seconds = r.elapsed.as_secs_f64();
        let _ = writeln!(
            out,
            "  {status}  {:<width$}  {seconds:>6.1}s{reason}",
            r.name
        );
    }
    let failed = results.iter().filter(|r| r.outcome.is_err()).count();
    if failed == 0 {
        let _ = writeln!(out, "all {} steps passed", results.len());
    } else {
        let _ = writeln!(out, "{failed} of {} steps failed", results.len());
    }
    out
}

/// Runs the cargo that launched xtask, so the pinned toolchain is used throughout.
pub fn cargo(root: &Path, args: &[&str]) -> Result<(), String> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = Command::new(cargo)
        .args(args)
        .current_dir(root)
        .status()
        .map_err(|e| format!("could not start cargo: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("`cargo {}` failed ({status})", args.join(" ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(name: &'static str, outcome: Result<(), String>) -> StepResult {
        StepResult {
            name,
            outcome,
            elapsed: Duration::from_millis(1500),
        }
    }

    #[test]
    fn summary_reports_every_failure() {
        let text = summary(&[
            result("fmt", Err("`cargo fmt` failed".into())),
            result("clippy", Ok(())),
            result("test", Err("`cargo test` failed".into())),
        ]);
        assert!(text.contains("FAIL  fmt"), "{text}");
        assert!(text.contains("ok    clippy"), "{text}");
        assert!(text.contains("FAIL  test"), "{text}");
        assert!(text.contains("`cargo test` failed"), "{text}");
        assert!(text.ends_with("2 of 3 steps failed\n"), "{text}");
    }

    #[test]
    fn summary_all_passed() {
        let text = summary(&[result("fmt", Ok(())), result("test", Ok(()))]);
        assert!(text.ends_with("all 2 steps passed\n"), "{text}");
        assert!(!text.contains("FAIL"), "{text}");
    }

    #[test]
    fn every_step_has_a_unique_name() {
        let names: Vec<_> = steps().iter().map(|s| s.name).collect();
        let mut unique = names.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(names.len(), unique.len(), "{names:?}");
    }
}
