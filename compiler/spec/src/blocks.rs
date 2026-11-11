//! Fenced code blocks, and the labels a code block in the spec or the std API doc may carry.
//!
//! A fence is a line that starts with three backticks after any indentation. The text after
//! the backticks of an opening fence, trimmed, is the block's label. The next fence closes it.

use crate::SpecError;
use crate::grammar::GRAMMAR_INFO;

/// The label of a block of Quince source, which the lexer's corpus test lexes.
pub const QUINCE_LABEL: &str = "quince";

/// Every label a code block may carry.
pub const LABELS: [&str; 5] = [QUINCE_LABEL, "toml", "json", "text", GRAMMAR_INFO];

/// One fenced code block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeBlock<'a> {
    /// The 1-based line of the opening fence. Body line `i` is on line `line + 1 + i`.
    pub line: usize,
    /// The info string after the opening fence, trimmed.
    pub label: &'a str,
    /// The lines between the fences, without their line endings.
    pub lines: Vec<&'a str>,
    /// False when the file ends before a closing fence.
    pub closed: bool,
}

impl CodeBlock<'_> {
    /// The body, with each line ending in a newline.
    pub fn text(&self) -> String {
        self.lines.iter().map(|line| format!("{line}\n")).collect()
    }
}

/// Every fenced code block in `text`, in order.
pub fn code_blocks(text: &str) -> Vec<CodeBlock<'_>> {
    let mut blocks = Vec::new();
    let mut open: Option<CodeBlock> = None;
    for (index, line) in text.lines().enumerate() {
        let fence = line.trim_start().strip_prefix("```");
        match (open.take(), fence) {
            (None, Some(info)) => {
                open = Some(CodeBlock {
                    line: index + 1,
                    label: info.trim(),
                    lines: Vec::new(),
                    closed: false,
                });
            }
            (Some(mut block), Some(_)) => {
                block.closed = true;
                blocks.push(block);
            }
            (Some(mut block), None) => {
                block.lines.push(line);
                open = Some(block);
            }
            (None, None) => {}
        }
    }
    blocks.extend(open);
    blocks
}

/// One error for every code block in `text` whose label isn't one of [`LABELS`], and one for
/// a block the file never closes.
pub fn check_labels(text: &str) -> Vec<SpecError> {
    let allowed = LABELS.map(|label| format!("`{label}`")).join(", ");
    let mut errors = Vec::new();
    for block in code_blocks(text) {
        let error = |message| SpecError {
            line: block.line,
            message,
        };
        if block.label.is_empty() {
            errors.push(error(format!(
                "this code block has no label; label it with one of {allowed}"
            )));
        } else if !LABELS.contains(&block.label) {
            errors.push(error(format!(
                "the code block label `{}` isn't one of {allowed}",
                block.label
            )));
        }
        if !block.closed {
            errors.push(error(
                "this code block is never closed; end it with a line of three backticks"
                    .to_string(),
            ));
        }
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_blocks_with_their_lines_and_labels() {
        let text =
            "# Title\n\n```quince\nlet x = 1\n\nx\n```\n\n  ```\n  indented\n  ```\n```toml\nopen";
        let blocks = code_blocks(text);
        let found: Vec<_> = (blocks.iter())
            .map(|b| (b.line, b.label, b.lines.clone(), b.closed))
            .collect();
        assert_eq!(
            found,
            [
                (3, "quince", vec!["let x = 1", "", "x"], true),
                (9, "", vec!["  indented"], true),
                (12, "toml", vec!["open"], false),
            ]
        );
        assert_eq!(blocks[0].text(), "let x = 1\n\nx\n");
        assert!(code_blocks("no code\n").is_empty());
    }

    #[test]
    fn every_allowed_label_passes() {
        for label in LABELS {
            let text = format!("Text.\n\n```{label}\nbody\n```\n");
            assert_eq!(check_labels(&text), [], "{label}");
        }
        assert_eq!(check_labels("```  quince  \nx\n```\n"), []);
    }

    #[test]
    fn a_missing_or_unknown_label_fails() {
        let text = "Text.\n\n```\nx\n```\n\n```rust\nfn main() {}\n```\n```quince title\n```\n";
        let messages: Vec<_> = check_labels(text).iter().map(|e| e.to_string()).collect();
        assert_eq!(
            messages,
            [
                "line 3: this code block has no label; label it with one of `quince`, `toml`, `json`, `text`, `ebnf`",
                "line 7: the code block label `rust` isn't one of `quince`, `toml`, `json`, `text`, `ebnf`",
                "line 10: the code block label `quince title` isn't one of `quince`, `toml`, `json`, `text`, `ebnf`",
            ]
        );
    }

    #[test]
    fn an_unclosed_block_fails() {
        let messages: Vec<_> = (check_labels(
            "Text.

```quince
let x = 1
",
        )
        .iter())
        .map(|e| e.to_string())
        .collect();
        assert_eq!(
            messages,
            ["line 3: this code block is never closed; end it with a line of three backticks"]
        );
    }
}
