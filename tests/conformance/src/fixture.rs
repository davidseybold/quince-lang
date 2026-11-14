//! Parsing one `.qnc` fixture: its header directives and its inline expectations.
//!
//! The header is the leading run of `//` lines and blank lines. A header line of the form
//! `// <word>:`, where the word is lowercase ASCII letters and hyphens, is a directive: `rule`,
//! `mode` or `exit`. Any other lowercase word is an error, and so is a case variant of a
//! directive, so a typo such as `// rules:` or `// Mode:` never silently does nothing. Other
//! header comments, such as `// Note: ...`, are free text. A directive after the header, in any
//! case, is an error too.
//!
//! Lines are numbered as positions count them (`compiler/diagnostics/README.md`), so a lone CR
//! ends a line, and a leading byte order mark isn't part of line 1.

use quince_diagnostics::{Code, LineIndex};

const DIRECTIVES: [&str; 3] = ["rule", "mode", "exit"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Runs the whole checker as far as it exists.
    Check,
    /// Runs only the parser, even once later stages exist.
    Parse,
    /// Builds and runs the file as a program.
    Run,
}

/// `//~ CODE` on line N expects a diagnostic starting on line N; each `^` moves one line up.
/// A comment listing a code twice yields two expectations.
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
    /// The contents of the sibling `<name>.stdout`, when there is one. A run fixture without
    /// one expects no output.
    pub stdout: Option<String>,
    /// The contents of the sibling `<name>.fixed`, when there is one: the fixture's text once
    /// every fix its diagnostics offer is applied. [`parse`] leaves it `None`; the runner sets
    /// it from the file it found.
    pub fixed: Option<String>,
    /// The file's bytes, as backends get them.
    pub source: Vec<u8>,
    /// The file as text, from [`decode`]. The header and expectations are read from it, and the
    /// positions in diagnostics and fixes refer to it.
    pub text: String,
}

/// Fixture bytes as text, decoded as the compiler decodes source: each maximal run of invalid
/// UTF-8 becomes one U+FFFD.
pub fn decode(source: &[u8]) -> Result<String, String> {
    quince_syntax::decode_source(source)
        .map(|decoded| decoded.text)
        .map_err(|e| e.to_string())
}

/// Parses a fixture. Every problem is reported, not just the first.
pub fn parse(name: &str, source: &[u8], stdout: Option<String>) -> Result<Fixture, Vec<String>> {
    let text = decode(source).map_err(|e| vec![e])?;
    let mut errors = Vec::new();
    let mut rules: Vec<String> = Vec::new();
    let mut mode = None;
    let mut exit = None;
    let mut in_header = true;
    for (line_no, line) in LineIndex::new(&text).lines() {
        let comment = line.trim_start().strip_prefix("//");
        if in_header && comment.is_none() && !line.trim().is_empty() {
            in_header = false;
        }
        let Some((key, value)) = comment.and_then(directive) else {
            continue;
        };
        if !in_header {
            if DIRECTIVES.contains(&key.to_ascii_lowercase().as_str()) {
                errors.push(format!(
                    "line {line_no}: `// {key}:` comes after code; move it into the header at the top of the file"
                ));
            }
            continue;
        }
        let is_lowercase = key.bytes().all(|b| b.is_ascii_lowercase() || b == b'-');
        if !is_lowercase && !DIRECTIVES.contains(&key.to_ascii_lowercase().as_str()) {
            // Free text that happens to start with a capitalized word and a colon.
            continue;
        }
        match key {
            "rule" => {
                if rules.iter().any(|r| r == value) {
                    errors.push(format!("line {line_no}: rule `{value}` is cited twice"));
                } else {
                    rules.push(value.to_string());
                }
            }
            "mode" => {
                let parsed = match value {
                    "check" => Some(Mode::Check),
                    "parse" => Some(Mode::Parse),
                    "run" => Some(Mode::Run),
                    _ => {
                        errors.push(format!(
                            "line {line_no}: unknown mode `{value}`; use `check`, `parse` or `run`"
                        ));
                        None
                    }
                };
                if mode.is_some() {
                    errors.push(format!("line {line_no}: `mode` is set twice"));
                } else {
                    mode = parsed;
                }
            }
            "exit" => {
                if exit.is_some() {
                    errors.push(format!("line {line_no}: `exit` is set twice"));
                }
                match value.parse() {
                    Ok(code) => exit = Some(code),
                    Err(_) => errors.push(format!(
                        "line {line_no}: `exit` takes an integer, found `{value}`"
                    )),
                }
            }
            _ => errors.push(format!(
                "line {line_no}: unknown directive `{key}`; the directives are `rule`, `mode` and `exit`, in lowercase"
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
    let (expectations, expectation_errors) = expectations(&text);
    errors.extend(expectation_errors);
    if mode == Mode::Run && !expectations.is_empty() {
        errors.push(
            "`//~` expectations apply only to `// mode: check` and `// mode: parse` fixtures"
                .to_string(),
        );
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
        fixed: None,
        source: source.to_vec(),
        text,
    })
}

/// Every `//~` expectation in a source file, and the malformed ones as errors. Everything after
/// the carets must be codes, separated by spaces.
pub fn expectations(source: &str) -> (Vec<Expectation>, Vec<String>) {
    let mut found = Vec::new();
    let mut errors = Vec::new();
    for (line_no, line) in LineIndex::new(source).lines() {
        let Some(at) = line.find("//~") else {
            continue;
        };
        let rest = &line[at + 3..];
        let carets = rest.chars().take_while(|&c| c == '^').count();
        let target = u32::try_from(carets)
            .ok()
            .and_then(|carets| line_no.checked_sub(carets))
            .filter(|&t| t >= 1);
        let Some(target) = target else {
            errors.push(format!(
                "line {line_no}: `//~{}` points above the first line",
                "^".repeat(carets)
            ));
            continue;
        };
        let codes: Vec<&str> = rest[carets..].split_whitespace().collect();
        if codes.is_empty() {
            errors.push(format!(
                "line {line_no}: `//~` needs at least one code, such as `//~ E0100`"
            ));
        }
        for text in codes {
            match Code::parse(text) {
                Ok(code) => found.push(Expectation { line: target, code }),
                Err(why) => errors.push(format!(
                    "line {line_no}: {why}; a `//~` comment holds only codes"
                )),
            }
        }
    }
    (found, errors)
}

/// `rule: x` → `("rule", "x")`, when the key is one word of ASCII letters and hyphens directly
/// before the colon. The key's case is kept, so `Mode` can be reported as unknown.
fn directive(comment: &str) -> Option<(&str, &str)> {
    let (key, value) = comment.trim_start().split_once(':')?;
    let is_word = !key.is_empty() && key.bytes().all(|b| b.is_ascii_alphabetic() || b == b'-');
    is_word.then(|| (key, value.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// [`super::parse`] on text, as most tests write fixtures.
    fn parse(name: &str, source: &str, stdout: Option<String>) -> Result<Fixture, Vec<String>> {
        super::parse(name, source.as_bytes(), stdout)
    }

    fn lines_and_codes(f: &Fixture) -> Vec<(u32, &str)> {
        f.expectations
            .iter()
            .map(|e| (e.line, e.code.as_str()))
            .collect()
    }

    #[test]
    fn header_and_expectations() {
        let source = "// rule: stmt.bind.shadow.same-scope\n// rule: bind.let-var\n// Shadowing in one scope is an error.\nfn f() {\n  let x = 1\n  let x = 2 //~ E0100\n}\n//~^^ W0003\n";
        let f = parse("bindings/shadow", source, None).unwrap();
        assert_eq!(f.rules, ["stmt.bind.shadow.same-scope", "bind.let-var"]);
        assert_eq!(f.mode, Mode::Check);
        assert_eq!(lines_and_codes(&f), [(6, "E0100"), (6, "W0003")]);
    }

    #[test]
    fn blank_lines_stay_in_the_header() {
        let f = parse(
            "x",
            "// rule: bind.let\n\n// mode: run\nfn main() {}\n",
            None,
        )
        .unwrap();
        assert_eq!(f.mode, Mode::Run);
        assert_eq!(f.rules, ["bind.let"]);
    }

    #[test]
    fn directive_after_code_is_an_error() {
        let errors = parse("x", "// rule: a.b\nfn f() {}\n// rule: c.d\n", None).unwrap_err();
        assert_eq!(
            errors,
            ["line 3: `// rule:` comes after code; move it into the header at the top of the file"]
        );
        let errors = parse("x", "fn f() {}\n  // mode: run\n", None).unwrap_err();
        assert!(
            errors[0].starts_with("line 2: `// mode:` comes after code"),
            "{errors:?}"
        );
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
    fn free_text_notes_are_comments() {
        let f = parse(
            "x",
            "// rule: a.b\n// Note: shadowing is rejected\n// TODO: more\nx\n",
            None,
        )
        .unwrap();
        assert_eq!(f.rules, ["a.b"]);
        let errors = parse("x", "// rule: a.b\nfn f() {}\n// Mode: run\n", None).unwrap_err();
        assert!(
            errors[0].contains("`// Mode:` comes after code"),
            "{errors:?}"
        );
    }

    #[test]
    fn unknown_directives_in_any_case() {
        for (source, key) in [
            ("// rules: bind.let\n", "rules"),
            ("// rule: a.b\n// Mode: run\n", "Mode"),
        ] {
            let errors = parse("x", source, None).unwrap_err();
            assert!(
                errors
                    .iter()
                    .any(|e| e.contains(&format!("unknown directive `{key}`"))),
                "{errors:?}"
            );
        }
    }

    #[test]
    fn repeated_directives() {
        let errors = parse(
            "x",
            "// rule: a.b\n// rule: a.b\n// mode: check\n// mode: run\n",
            None,
        )
        .unwrap_err();
        assert_eq!(
            errors,
            [
                "line 2: rule `a.b` is cited twice",
                "line 4: `mode` is set twice"
            ]
        );
    }

    #[test]
    fn modes() {
        let f = parse("x", "// rule: lex.x\n// mode: parse\nx //~ E0001\n", None).unwrap();
        assert_eq!(f.mode, Mode::Parse);
        let errors = parse("x", "// rule: a.b\n// mode: compile\n", None).unwrap_err();
        assert!(errors[0].contains("unknown mode `compile`; use `check`, `parse` or `run`"));
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
        assert!(
            errors[0].contains("apply only to `// mode: check`"),
            "{errors:?}"
        );
    }

    #[test]
    fn several_codes_in_one_comment() {
        let f = parse("x", "// rule: a.b\nf() //~ E0001 E0001 W0002\n", None).unwrap();
        assert_eq!(
            lines_and_codes(&f),
            [(2, "E0001"), (2, "E0001"), (2, "W0002")]
        );
    }

    #[test]
    fn malformed_expectations() {
        let (found, errors) = expectations("//~^ E0001\nx //~ e12\ny //~\nz //~ E0001 because\n");
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(errors.len(), 4, "{errors:?}");
        assert!(errors[0].contains("points above the first line"));
        assert!(errors[1].contains("`e12` is not a diagnostic code"));
        assert!(errors[2].contains("needs at least one code"));
        assert!(errors[3].contains("`because` is not a diagnostic code"));
    }

    #[test]
    fn a_byte_order_mark_is_not_part_of_the_header() {
        let f = parse("x", "\u{FEFF}// rule: a.b\nx //~ E0001\n", None).unwrap();
        assert_eq!(f.rules, ["a.b"]);
        assert_eq!(lines_and_codes(&f), [(2, "E0001")]);
        assert!(f.source.starts_with("\u{FEFF}".as_bytes()));
    }

    #[test]
    fn every_line_ending_ends_a_line() {
        for ending in ["\n", "\r\n", "\r"] {
            let source = [
                "// rule: a.b",
                "// mode: parse",
                "x",
                "y //~ E0001",
                "//~^^ E0002",
                "",
            ]
            .join(ending);
            let f = parse("x", &source, None).unwrap();
            assert_eq!(f.rules, ["a.b"], "{ending:?}");
            assert_eq!(f.mode, Mode::Parse, "{ending:?}");
            assert_eq!(
                lines_and_codes(&f),
                [(4, "E0001"), (3, "E0002")],
                "{ending:?}"
            );
        }
    }

    #[test]
    fn each_run_of_invalid_utf8_is_one_replacement_character() {
        let f = super::parse("x", b"// rule: a.b\n// \xFF\xFE\nx //~ E0001\n", None).unwrap();
        assert_eq!(f.text, "// rule: a.b\n// \u{FFFD}\nx //~ E0001\n");
        assert_eq!(lines_and_codes(&f), [(3, "E0001")]);
    }
}
