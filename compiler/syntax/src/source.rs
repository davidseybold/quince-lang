//! `decode_source`: the one place source bytes become text (`r[lex.source.utf8]`).

use crate::{SyntaxError, codes};
use std::ops::Range;

/// Source text decoded from bytes, with what decoding replaced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    /// The text, with each maximal run of invalid bytes replaced by one U+FFFD.
    pub text: String,
    /// The byte range in `text` of each U+FFFD that replaced invalid bytes, in order.
    pub replaced: Vec<Range<u32>>,
    /// One E0014 for each line that holds invalid bytes, at its first run.
    pub errors: Vec<SyntaxError>,
}

impl Decoded {
    /// True when `text` isn't the file's bytes, so no tool may write it back to the file.
    pub fn is_lossy(&self) -> bool {
        !self.replaced.is_empty()
    }
}

/// Decodes a source file. Invalid UTF-8 is reported, not fatal, so the rest of the file is
/// still checked. A file over 4 GiB, more than a `u32` offset can address, is an I/O error.
pub fn decode_source(bytes: &[u8]) -> std::io::Result<Decoded> {
    if u32::try_from(bytes.len()).is_err() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::FileTooLarge,
            format!(
                "a source file can be at most 4 GiB, and this one is {} bytes",
                bytes.len()
            ),
        ));
    }
    let mut text = String::with_capacity(bytes.len());
    let mut replaced: Vec<Range<u32>> = Vec::new();
    // The invalid bytes of each run, with where its line starts in `text`.
    let mut runs: Vec<(Vec<u8>, usize)> = Vec::new();
    let mut line_start = 0;
    let mut in_run = false;
    for chunk in bytes.utf8_chunks() {
        if !chunk.valid().is_empty() {
            if let Some(end) = chunk.valid().rfind(['\n', '\r']) {
                line_start = text.len() + end + 1;
            }
            text.push_str(chunk.valid());
            in_run = false;
        }
        if chunk.invalid().is_empty() {
            continue;
        }
        if in_run {
            runs.last_mut()
                .expect("a run is open")
                .0
                .extend(chunk.invalid());
        } else {
            let start = text.len();
            text.push(char::REPLACEMENT_CHARACTER);
            replaced.push(crate::offset(start)..crate::offset(text.len()));
            runs.push((chunk.invalid().to_vec(), line_start));
            in_run = true;
        }
    }
    let mut errors: Vec<SyntaxError> = Vec::new();
    let mut last_line = None;
    for ((bytes, line), range) in runs.iter().zip(&replaced) {
        if last_line == Some(*line) {
            continue;
        }
        last_line = Some(*line);
        let shown: Vec<String> = bytes.iter().take(8).map(|b| format!("0x{b:02X}")).collect();
        let more = if bytes.len() > 8 { " ..." } else { "" };
        let message = format!(
            "this line isn't valid UTF-8: the bytes {}{more} don't decode, and a Quince source \
             file must be UTF-8 text",
            shown.join(" ")
        );
        let range = range.start as usize..range.end as usize;
        errors.push(SyntaxError::new(range, codes::INVALID_UTF8, message));
    }
    Ok(Decoded {
        text,
        replaced,
        errors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_text_is_unchanged() {
        let decoded = decode_source("\u{FEFF}let café = 1\r\n".as_bytes()).unwrap();
        assert_eq!(decoded.text, "\u{FEFF}let café = 1\r\n");
        assert!(!decoded.is_lossy());
        assert!(decoded.errors.is_empty());
    }

    #[test]
    fn each_run_is_one_replacement_and_each_line_one_error() {
        let decoded = decode_source(b"a\xFF\xFE\xC0b\xFFc\n\xE2\x82\rok\r\n\xFF").unwrap();
        assert_eq!(
            decoded.text,
            "a\u{FFFD}b\u{FFFD}c\n\u{FFFD}\rok\r\n\u{FFFD}"
        );
        assert!(decoded.is_lossy());
        assert_eq!(decoded.replaced, [1..4, 5..8, 10..13, 18..21]);
        let errors: Vec<_> = decoded.errors.iter().map(|e| e.range.clone()).collect();
        assert_eq!(errors, [1..4, 10..13, 18..21]);
        assert!(decoded.errors[0].message.contains("0xFF 0xFE 0xC0 don't"));
        assert!(decoded.errors.iter().all(|e| e.code == "E0014"));
    }
}
