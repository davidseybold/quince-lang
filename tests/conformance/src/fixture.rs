//! Parsing one `.qnc` fixture: its header directives and its inline expectations.
//!
//! The header is the leading `//` lines. A header line of the form `// <word>: <value>` is a
//! directive: `rule`, `mode` or `exit`. Any other directive is an error, so a typo such as
//! `// rules:` never silently does nothing. Other header comments are free text.

use quince_diagnostics::Code;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Check,
    Run,
}

/// `//~ CODE` on line N expects a diagnostic starting on line N; each `^` moves one line up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expectation {
    pub line: u32,
    pub code: Code,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fixture {
    /// Path below `testdata/` without the extension, with `/` separators.
    pub name: String,
    pub rules: Vec<String>,
    pub mode: Mode,
    pub exit: i32,
    pub expectations: Vec<Expectation>,
    /// The contents of the sibling `<name>.stdout`, when there is one.
    pub stdout: Option<String>,
    pub source: String,
}

/// Parses a fixture. Every problem is reported, not just the first.
pub fn parse(name: &str, source: &str, stdout: Option<String>) -> Result<Fixture, Vec<String>> {
    let mut errors = Vec::new();
    let mut rules = Vec::new();
    let mut mode = None;
    let mut exit = None;
    for (index, line) in source.lines().enumerate() {
        let Some(comment) = line.strip_prefix("//") else {
            break;
        };
        let line_no = index + 1;
        let Some((key, value)) = directive(comment) else {
            continue;
        };
        match key {
            "rule" => rules.push(value.to_string()),
            "mode" => match value {
                "check" => mode = Some(Mode::Check),
                "run" => mode = Some(Mode::Run),
                _ => errors.push(format!(
                    "line {line_no}: unknown mode `{value}`; use `check` or `run`"
                )),
            },
            "exit" => match value.parse() {
                Ok(code) => exit = Some(code),
                Err(_) => errors.push(format!(
                    "line {line_no}: `exit` takes an integer, found `{value}`"
                )),
            },
            _ => errors.push(format!(
                "line {line_no}: unknown directive `{key}`; the directives are `rule`, `mode` and `exit`"
            )),
        }
    }
    if rules.is_empty() {
        errors.push(
            "a fixture must cite at least one rule with `// rule: <id>` before any code"
                .to_string(),
        );
    }
    let mode = mode.unwrap_or(Mode::Check);
    if exit.is_some() && mode != Mode::Run {
        errors.push("`exit` applies only to `// mode: run` fixtures".to_string());
    }
    if stdout.is_some() && mode != Mode::Run {
        errors.push(format!(
            "{name}.stdout exists, but only `// mode: run` fixtures compare output"
        ));
    }
    let (expectations, expectation_errors) = expectations(source);
    errors.extend(expectation_errors);
    if mode == Mode::Run && !expectations.is_empty() {
        errors.push("`//~` expectations apply only to `// mode: check` fixtures".to_string());
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(Fixture {
        name: name.to_string(),
        rules,
        mode,
        exit: exit.unwrap_or(0),
        expectations,
        stdout,
        source: source.to_string(),
    })
}

/// Every `//~` expectation in a source file, and the malformed ones as errors.
pub fn expectations(source: &str) -> (Vec<Expectation>, Vec<String>) {
    let mut found = Vec::new();
    let mut errors = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let line_no = index + 1;
        let Some(at) = line.find("//~") else {
            continue;
        };
        let rest = &line[at + 3..];
        let carets = rest.chars().take_while(|&c| c == '^').count();
        let code_text = rest[carets..].split_whitespace().next().unwrap_or("");
        let Some(target) = line_no.checked_sub(carets).filter(|&t| t >= 1) else {
            errors.push(format!(
                "line {line_no}: `//~{}` points above the first line",
                "^".repeat(carets)
            ));
            continue;
        };
        match Code::parse(code_text) {
            Ok(code) => found.push(Expectation {
                line: u32::try_from(target).expect("fixture lines fit in u32"),
                code,
            }),
            Err(why) => errors.push(format!("line {line_no}: {why}")),
        }
    }
    (found, errors)
}

/// `rule: x` → `("rule", "x")`, when the key is one lowercase word directly before the colon.
fn directive(comment: &str) -> Option<(&str, &str)> {
    let (key, value) = comment.trim_start().split_once(':')?;
    let is_word = !key.is_empty() && key.bytes().all(|b| b.is_ascii_lowercase() || b == b'-');
    is_word.then(|| (key, value.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_and_expectations() {
        let source = "// rule: bind.shadow.same-scope\n// rule: bind.let-var\n// Shadowing in one scope is an error.\nfn f() {\n  let x = 1\n  let x = 2 //~ E0100\n}\n//~^^ W0003\n";
        let f = parse("bindings/shadow", source, None).unwrap();
        assert_eq!(f.rules, ["bind.shadow.same-scope", "bind.let-var"]);
        assert_eq!(f.mode, Mode::Check);
        let lines: Vec<_> = f
            .expectations
            .iter()
            .map(|e| (e.line, e.code.as_str()))
            .collect();
        assert_eq!(lines, [(6, "E0100"), (6, "W0003")]);
    }

    #[test]
    fn missing_rule_citation() {
        let errors = parse("x", "fn main() {}\n", None).unwrap_err();
        assert_eq!(
            errors,
            ["a fixture must cite at least one rule with `// rule: <id>` before any code"]
        );
    }

    #[test]
    fn rule_after_code_does_not_count() {
        assert!(parse("x", "fn f() {}\n// rule: bind.let-var\n", None).is_err());
    }

    #[test]
    fn unknown_directive_is_an_error() {
        let errors = parse("x", "// rules: bind.let-var\n", None).unwrap_err();
        assert!(
            errors[0].contains("unknown directive `rules`"),
            "{errors:?}"
        );
    }

    #[test]
    fn run_mode() {
        let f = parse(
            "hello",
            "// rule: main.signature\n// mode: run\n// exit: 1\nfn main() {}\n",
            Some("hi\n".into()),
        )
        .unwrap();
        assert_eq!((f.mode, f.exit), (Mode::Run, 1));
        assert_eq!(f.stdout.as_deref(), Some("hi\n"));
    }

    #[test]
    fn mode_mismatches() {
        let errors = parse("x", "// rule: a.b\n// exit: 2\n", Some(String::new())).unwrap_err();
        assert_eq!(errors.len(), 2, "{errors:?}");
        let errors = parse("x", "// rule: a.b\n// mode: run\nf() //~ E0001\n", None).unwrap_err();
        assert!(errors[0].contains("only to `// mode: check`"), "{errors:?}");
    }

    #[test]
    fn malformed_expectations() {
        let (found, errors) = expectations("//~^ E0001\nx //~ e12\n");
        assert!(found.is_empty());
        assert_eq!(errors.len(), 2, "{errors:?}");
        assert!(errors[0].contains("points above the first line"));
        assert!(errors[1].contains("`e12` is not a diagnostic code"));
    }
}
