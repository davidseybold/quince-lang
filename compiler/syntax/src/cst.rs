//! The lossless CST: a `rowan` tree whose text is the source. The parser emits [`Event`]s and
//! [`build`] turns them and the lexer's tokens into the tree, placing trivia. The README's CST
//! section has the contract.

use crate::{SyntaxError, SyntaxKind, Token};
use rowan::{GreenNode, GreenNodeBuilder, Language};

/// [`SyntaxKind`] as `rowan` sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum QuinceLanguage {}

impl Language for QuinceLanguage {
    type Kind = SyntaxKind;

    fn kind_from_raw(raw: rowan::SyntaxKind) -> SyntaxKind {
        SyntaxKind::ALL[usize::from(raw.0)]
    }

    fn kind_to_raw(kind: SyntaxKind) -> rowan::SyntaxKind {
        rowan::SyntaxKind(kind as u16)
    }
}

pub type SyntaxNode = rowan::SyntaxNode<QuinceLanguage>;
pub type SyntaxToken = rowan::SyntaxToken<QuinceLanguage>;
pub type SyntaxElement = rowan::SyntaxElement<QuinceLanguage>;

/// One step of the parser's output. The parser sees only significant tokens, so these never
/// mention trivia: the builder places it.
#[derive(Debug)]
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "layer 3 of the parser joins tokens")
)]
pub(crate) enum Event {
    /// Opens a node of this kind inside the innermost open one.
    Start(SyntaxKind),
    /// Closes the innermost open node.
    Finish,
    /// Takes the next significant token, keeping its kind.
    Token,
    /// Takes the next two significant tokens, which are adjacent, as one token of this kind,
    /// one of [`SyntaxKind::JOINED`].
    Join(SyntaxKind),
    Error(SyntaxError),
}

/// Builds the tree for `text`, whose lexer tokens are `tokens`, from the parser's `events`.
/// `significant` holds the indices of the tokens the parser took, in order: its significant
/// tokens, and the doc comments it took as part of what they document. Every other token is
/// trivia. Returns the tree and the errors in the order they were reported.
///
/// The first event starts the root and the last finishes it. Every significant token is taken,
/// and every node but the root holds a token. Fuzz and debug builds check both.
pub(crate) fn build(
    text: &str,
    tokens: &[Token],
    significant: &[usize],
    events: Vec<Event>,
) -> (GreenNode, Vec<SyntaxError>) {
    let mut builder = Builder {
        text,
        tokens,
        significant,
        next: 0,
        cursor: 0,
        offset: 0,
        green: GreenNodeBuilder::new(),
        open: Vec::new(),
        errors: Vec::new(),
    };
    for event in events {
        match event {
            Event::Start(kind) => builder.start(kind),
            Event::Finish => builder.finish(),
            Event::Token => builder.token(1, None),
            Event::Join(kind) => builder.token(2, Some(kind)),
            Event::Error(error) => builder.errors.push(error),
        }
    }
    assert!(builder.open.is_empty(), "a node was never finished");
    (builder.green.finish(), builder.errors)
}

struct Builder<'a> {
    text: &'a str,
    tokens: &'a [Token],
    significant: &'a [usize],
    /// The next entry of `significant` to take.
    next: usize,
    /// The next lexer token to write, and the offset of its text.
    cursor: usize,
    offset: usize,
    green: GreenNodeBuilder<'static>,
    /// One entry per open node: whether it holds a token yet.
    open: Vec<bool>,
    errors: Vec<SyntaxError>,
}

impl Builder<'_> {
    fn start(&mut self, kind: SyntaxKind) {
        // The trivia before a node goes outside it, apart from the comments it takes, which go
        // in at once. So a node that starts at the same token inside this one finds no trivia
        // left, and the outermost node takes the comments. The root has no outside, so its
        // leading trivia waits for the root's first child.
        let first = self.next_significant();
        if !self.open.is_empty() {
            let inside = if kind.attaches_comments() {
                self.attached_comments(first)
            } else {
                first
            };
            self.write_to(inside);
        }
        self.green.start_node(QuinceLanguage::kind_to_raw(kind));
        self.open.push(false);
        if self.open.len() > 1 {
            self.write_to(first);
        }
    }

    fn finish(&mut self) {
        // Trivia after a node's last token goes outside it, so it waits for the next token.
        // Only the root takes what is left.
        if self.open.len() == 1 {
            debug_assert_eq!(
                self.next,
                self.significant.len(),
                "the parser left a significant token"
            );
            self.write_to(self.tokens.len());
        }
        let holds_token = self.open.pop().expect("a Finish without a Start");
        debug_assert!(
            holds_token || self.open.is_empty(),
            "only the root may be empty"
        );
        if holds_token {
            self.mark_holds_token();
        }
        self.green.finish_node();
    }

    /// Writes the trivia before the next `count` significant tokens, then those tokens as one
    /// token, of `joined` if given and of the lexer's kind otherwise.
    fn token(&mut self, count: usize, joined: Option<SyntaxKind>) {
        let taken = self
            .significant
            .get(self.next..self.next + count)
            .expect("the parser took a token past the last one");
        let (first, last) = (taken[0], taken[count - 1]);
        debug_assert_eq!(
            last - first,
            count - 1,
            "a join of tokens that aren't adjacent"
        );
        self.write_to(first);
        let start = self.offset;
        for token in &self.tokens[first..=last] {
            self.offset += token.len as usize;
        }
        let text = &self.text[start..self.offset];
        debug_assert!(
            joined.is_none_or(|kind| SyntaxKind::JOINED.contains(&(text, kind))),
            "a join whose tokens don't spell its kind"
        );
        let kind = joined.unwrap_or(self.tokens[first].kind);
        self.green.token(QuinceLanguage::kind_to_raw(kind), text);
        self.cursor = last + 1;
        self.next += count;
        self.mark_holds_token();
    }

    /// The index of the next significant token, or the end of the tokens.
    fn next_significant(&self) -> usize {
        self.significant
            .get(self.next)
            .copied()
            .unwrap_or(self.tokens.len())
    }

    /// Writes every lexer token from the cursor up to `end`, each as its own token. These are
    /// trivia, so a node that holds only trivia doesn't count as holding a token.
    fn write_to(&mut self, end: usize) {
        while self.cursor < end {
            let len = self.tokens[self.cursor].len as usize;
            let kind = self.tokens[self.cursor].kind;
            let text = &self.text[self.offset..self.offset + len];
            self.green.token(QuinceLanguage::kind_to_raw(kind), text);
            self.offset += len;
            self.cursor += 1;
        }
    }

    /// Records that the innermost open node holds a significant token, its own or a child's.
    fn mark_holds_token(&mut self) {
        if let Some(holds_token) = self.open.last_mut() {
            *holds_token = true;
        }
    }

    /// Where the comment lines directly above the token at `first` begin, or `first` when
    /// there are none. A comment line holds comments and whitespace, starts a line, and ends
    /// with a line ending. A blank line, or a line with anything else on it, ends the run. The
    /// indentation before the first comment stays outside.
    fn attached_comments(&self, first: usize) -> usize {
        use SyntaxKind::*;
        let kind = |i: usize| self.tokens[i].kind;
        let is_comment = |k: SyntaxKind| matches!(k, Comment | BlockComment | DocComment | Error);
        // A line starts at the start of the text, after the byte order mark, or after a line
        // ending.
        let starts_line = |i: usize| i == 0 || matches!(kind(i - 1), Newline | Bom);
        let mut start = first;
        // The node's own indentation.
        let mut end = first;
        while end > self.cursor && kind(end - 1) == Whitespace {
            end -= 1;
        }
        // Each pass reads the line before the line ending at `end - 1`.
        while end > self.cursor && kind(end - 1) == Newline {
            let mut line = end - 1;
            while line > self.cursor && (kind(line - 1) == Whitespace || is_comment(kind(line - 1)))
            {
                line -= 1;
            }
            if !starts_line(line) {
                break;
            }
            match (line..end - 1).find(|&i| is_comment(kind(i))) {
                Some(comment) => start = comment,
                None => break,
            }
            end = line;
        }
        start
    }
}
