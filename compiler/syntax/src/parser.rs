//! The parser: the lexer's tokens to events, which [`crate::cst::build`] turns into the tree.
//! The README's CST section has the contract, and `docs/design/m1-parser.md` the design. Each
//! grammar area has its own module.

mod exprs;
mod input;
mod items;
mod types;

use crate::cst::{self, Event};
use crate::{Decoded, Lexed, SyntaxError, SyntaxKind, SyntaxNode, codes, lex, lex_str};
use SyntaxKind::*;
use input::{Break, Input, Sig};
use quince_diagnostics::{Applicability, Diagnostic, LineIndex};
use rowan::GreenNode;
use std::cell::Cell;
use std::ops::Range;

/// A parsed file: its tree, and every error the lexer and the parser found, in source order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parse {
    green: GreenNode,
    errors: Vec<SyntaxError>,
}

impl Parse {
    /// The root of the tree, a `File` node.
    pub fn syntax(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.clone())
    }

    pub fn errors(&self) -> &[SyntaxError] {
        &self.errors
    }

    /// Every error as a diagnostic in `file`. `text` is the text that was parsed.
    pub fn diagnostics(&self, text: &str, file: &str) -> Vec<Diagnostic> {
        let index = LineIndex::new(text);
        self.errors
            .iter()
            .map(|error| error.to_diagnostic(file, &index))
            .collect()
    }
}

/// Parses decoded source. When the text is lossy, no error has a fix, whichever stage found
/// it, for the reason [`lex`] gives.
pub fn parse(decoded: &Decoded) -> Parse {
    let mut parse = parse_lexed(&decoded.text, lex(decoded));
    if decoded.is_lossy() {
        for error in &mut parse.errors {
            error.fixes.clear();
        }
    }
    parse
}

/// Parses text that never came from bytes, as [`lex_str`] lexes it.
pub fn parse_str(text: &str) -> Parse {
    parse_lexed(text, lex_str(text))
}

fn parse_lexed(text: &str, lexed: Lexed) -> Parse {
    let input = Input::new(text, &lexed);
    let significant: Vec<usize> = input.tokens.iter().map(|t| t.index).collect();
    let mut parser = Parser::new(text, &lexed, input);
    items::file(&mut parser);
    let events = into_events(parser.steps);
    let (green, parser_errors) = cst::build(text, &lexed.tokens, &significant, events);
    let mut errors = lexed.errors;
    errors.extend(parser_errors);
    // Stable, so the lexer's errors come first where both start at one offset.
    errors.sort_by_key(|error| error.range.start);
    Parse { green, errors }
}

/// The kind [`Parser::kind`] gives past the last token. It is a node kind, so no token has it.
const EOF: SyntaxKind = File;

/// How many times the parser may look at the tokens without taking one, closing a construct
/// or completing a node. More means it is stuck in a loop. Debug and fuzz builds check it.
const LOOKAHEAD_LIMIT: u32 = 256;

/// One step of the parser's output. Like [`Event`], except that a node can be opened around
/// nodes already completed, through `forward_parent`, as rust-analyzer's parser does. One pass
/// over the steps, [`into_events`], gives the builder's events.
#[derive(Debug)]
enum Step {
    /// Opens a node. `kind` is `None` until the node is completed, and stays `None` when it is
    /// abandoned. `forward_parent` is the distance to the `Start` of a node that wraps this
    /// one, opened after it.
    Start {
        kind: Option<SyntaxKind>,
        forward_parent: Option<usize>,
    },
    Finish,
    Token,
    Error(SyntaxError),
}

const TOMBSTONE: Step = Step::Start {
    kind: None,
    forward_parent: None,
};

/// Each `Start` becomes the starts of its forward parents, outermost first, then its own.
fn into_events(mut steps: Vec<Step>) -> Vec<Event> {
    let mut events = Vec::with_capacity(steps.len());
    let mut kinds = Vec::new();
    for i in 0..steps.len() {
        match std::mem::replace(&mut steps[i], TOMBSTONE) {
            Step::Start {
                kind,
                mut forward_parent,
            } => {
                kinds.extend(kind);
                let mut at = i;
                while let Some(distance) = forward_parent {
                    at += distance;
                    let Step::Start {
                        kind,
                        forward_parent: next,
                    } = std::mem::replace(&mut steps[at], TOMBSTONE)
                    else {
                        unreachable!("a forward parent is a Start");
                    };
                    kinds.extend(kind);
                    forward_parent = next;
                }
                events.extend(kinds.drain(..).rev().map(Event::Start));
            }
            Step::Finish => events.push(Event::Finish),
            Step::Token => events.push(Event::Token),
            Step::Error(error) => events.push(Event::Error(error)),
        }
    }
    events
}

/// An open node, which must be completed or abandoned.
#[must_use]
struct Marker(usize);

/// A completed node, which [`Parser::precede`] can wrap in another.
#[derive(Clone, Copy)]
struct Completed(usize);

/// What the members of a line context can be (decision 9's member starts).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Holds {
    /// A list: no line holds a member.
    Items,
    File,
    /// The body of an `impl`.
    Body,
    Block,
}

/// An open construct that error recovery knows about: the file, and each bracket.
#[derive(Debug, Clone, Copy)]
struct Frame {
    /// The closing bracket, and the index of the opening one among the significant tokens.
    bracket: Option<(SyntaxKind, usize)>,
    holds: Holds,
}

/// Why the innermost frame must close at the current token without its closing bracket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stop {
    Eof,
    /// A closing bracket of a frame further out, or a member start that only a frame further
    /// out can hold.
    Outer,
    /// The line that opened this list held an unclosed one-line literal, and has ended.
    LineEnd,
}

/// Where error recovery stops skipping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sync {
    /// At the next break, in a line context.
    Line,
    /// At the next `,`, in a list.
    List,
}

struct Parser<'t> {
    text: &'t str,
    lexed: &'t Lexed,
    input: Input,
    pos: usize,
    steps: Vec<Step>,
    frames: Vec<Frame>,
    /// An error was reported and the parser hasn't taken a token since, so nothing more is
    /// reported (decision 9).
    recovering: bool,
    /// A frame closed without its closing bracket and the parser hasn't taken a token since,
    /// so what the constructs around it lack is part of that slip. Unlike `recovering`, it
    /// doesn't silence the E0017s of the frames around it.
    closed_early: bool,
    /// The parser is in a `fn` header, where a line that starts with `->` or `{` continues.
    in_fn_header: bool,
    lookahead: Cell<u32>,
}

impl<'t> Parser<'t> {
    fn new(text: &'t str, lexed: &'t Lexed, input: Input) -> Parser<'t> {
        Parser {
            text,
            lexed,
            input,
            pos: 0,
            steps: Vec::new(),
            frames: Vec::new(),
            recovering: false,
            closed_early: false,
            in_fn_header: false,
            lookahead: Cell::new(0),
        }
    }

    // Looking at tokens.

    /// The `n`th significant token from the current one, if there is one.
    fn nth(&self, n: usize) -> Option<&Sig> {
        let count = self.lookahead.get() + 1;
        debug_assert!(count <= LOOKAHEAD_LIMIT, "the parser is stuck");
        self.lookahead.set(count);
        self.input.tokens.get(self.pos + n)
    }

    fn current(&self) -> Option<&Sig> {
        self.nth(0)
    }

    /// The current token's kind, whatever the break before it, or [`EOF`].
    fn kind(&self) -> SyntaxKind {
        self.current().map_or(EOF, |t| t.kind)
    }

    fn nth_kind(&self, n: usize) -> SyntaxKind {
        self.nth(n).map_or(EOF, |t| t.kind)
    }

    /// The current token is `kind`, on the line of the construct the parser is in.
    fn at(&self, kind: SyntaxKind) -> bool {
        self.kind() == kind && !self.at_break()
    }

    /// A break before the current token ends the construct the parser is in (decision 4):
    /// there is one, and the token before it doesn't continue the line. Right after frames
    /// closed early, at a member start, every break ends it: the token before belongs to a
    /// construct that is already closed.
    fn at_break(&self) -> bool {
        let Some(token) = self.current() else {
            return true;
        };
        match token.brk {
            Break::None => false,
            Break::Hard => true,
            Break::Line if self.closed_early => true,
            Break::Line => {
                let previous = self.input.tokens[self.pos - 1].kind;
                let header_continues = self.in_fn_header
                    && matches!(token.kind, Arrow | UsesKw | RequiresKw | EnsuresKw | LBrace);
                !(previous.continues_line() || token.kind == Dot || header_continues)
            }
        }
    }

    /// The current token is the first on its line.
    fn starts_line(&self) -> bool {
        self.current().is_some_and(|t| t.brk != Break::None)
    }

    // Taking tokens and building nodes.

    fn bump(&mut self) {
        self.recovering = false;
        self.closed_early = false;
        self.take();
    }

    /// Takes the current token without ending error recovery.
    fn take(&mut self) {
        debug_assert!(
            self.pos < self.input.tokens.len(),
            "took a token past the end"
        );
        self.steps.push(Step::Token);
        self.pos += 1;
        self.lookahead.set(0);
    }

    fn start(&mut self) -> Marker {
        self.steps.push(TOMBSTONE);
        Marker(self.steps.len() - 1)
    }

    fn complete(&mut self, marker: Marker, kind: SyntaxKind) -> Completed {
        let Step::Start { kind: slot, .. } = &mut self.steps[marker.0] else {
            unreachable!("a marker is a Start");
        };
        *slot = Some(kind);
        self.steps.push(Step::Finish);
        self.lookahead.set(0);
        Completed(marker.0)
    }

    /// Drops a node that turned out to hold nothing. It must still be the last step.
    fn abandon(&mut self, marker: Marker) {
        debug_assert_eq!(
            marker.0,
            self.steps.len() - 1,
            "abandoned a node that holds steps"
        );
        self.steps.pop();
    }

    /// Opens a node around `completed`.
    fn precede(&mut self, completed: Completed) -> Marker {
        let marker = self.start();
        let Step::Start { forward_parent, .. } = &mut self.steps[completed.0] else {
            unreachable!("a completed marker is a Start");
        };
        *forward_parent = Some(marker.0 - completed.0);
        marker
    }

    /// Takes `kind` as a node of one token.
    fn bump_node(&mut self, node: SyntaxKind) {
        let marker = self.start();
        self.bump();
        self.complete(marker, node);
    }

    // Frames.

    fn push_frame(&mut self, holds: Holds, closer: Option<SyntaxKind>) {
        let opener = self.pos.checked_sub(1);
        let bracket = closer.map(|closer| (closer, opener.expect("a bracket was taken")));
        self.frames.push(Frame { bracket, holds });
    }

    fn top(&self) -> Frame {
        *self.frames.last().expect("the file is a frame")
    }

    fn own_closer(&self) -> Option<SyntaxKind> {
        self.top().bracket.map(|(closer, _)| closer)
    }

    /// Closes the innermost frame at its closing bracket, which the caller takes next.
    fn pop_frame(&mut self) {
        self.frames.pop();
        self.lookahead.set(0);
    }

    /// Closes the innermost frame without its closing bracket, with an E0017 at its opening
    /// one unless the slip is already reported.
    fn close_frame(&mut self, stop: Stop) {
        let frame = self.frames.pop().expect("a frame to close");
        self.lookahead.set(0);
        self.closed_early = true;
        let Some((closer, opener)) = frame.bracket else {
            return;
        };
        let silent = self.recovering
            || stop == Stop::LineEnd
            || (stop == Stop::Eof && self.input.eof_swallowed);
        if !silent {
            let range = self.input.tokens[opener].range.clone();
            let open = &self.text[range.clone()];
            let close = punctuation(closer);
            let message = format!("this `{open}` has no matching `{close}`");
            self.emit(SyntaxError::new(range, codes::UNCLOSED_BRACKET, message));
        }
    }

    /// Why the innermost frame must close here without its closing bracket, if it must
    /// (decision 9).
    fn stop(&self) -> Option<Stop> {
        let Some(token) = self.current() else {
            return Some(Stop::Eof);
        };
        let top = self.frames.len() - 1;
        let frame = self.top();
        if let (Some(line_start), Holds::Items, Some((_, opener))) =
            (token.closes_line_from, frame.holds, frame.bracket)
            && self.input.tokens[opener].range.start >= line_start
        {
            return Some(Stop::LineEnd);
        }
        if let Some(at) = self.frame_closed_by(token.kind)
            && at < top
        {
            return Some(Stop::Outer);
        }
        match self.member_holder() {
            Some(at) if at < top => Some(Stop::Outer),
            _ => None,
        }
    }

    /// The innermost frame whose closing bracket is `kind`.
    fn frame_closed_by(&self, kind: SyntaxKind) -> Option<usize> {
        self.frames
            .iter()
            .rposition(|frame| frame.bracket.is_some_and(|(closer, _)| closer == kind))
    }

    /// When the current token starts a line and can only start a member of some containers,
    /// the innermost open one.
    fn member_holder(&self) -> Option<usize> {
        if !self.starts_line() {
            return None;
        }
        let item_start = |n: usize| match self.nth_kind(n) {
            FnKw => self.nth_kind(n + 1) == Ident,
            StructKw | EnumKw | TypeKw | InterfaceKw | ImplKw | EffectKw | ForeignKw | ImportKw
            | ConstKw | TestKw => true,
            _ => false,
        };
        let member_start = |n: usize| match self.nth_kind(n) {
            FnKw => self.nth_kind(n + 1) == Ident,
            TypeKw | ConstKw => true,
            _ => false,
        };
        let public = usize::from(self.kind() == PubKw);
        let holds = |holds: Holds| match holds {
            Holds::Items => false,
            Holds::File => item_start(public),
            Holds::Body => member_start(public),
            Holds::Block => matches!(self.kind(), LetKw | VarKw | WhileKw),
        };
        self.frames.iter().rposition(|frame| holds(frame.holds))
    }

    // Errors.

    /// Reports an E0016 at the current token: it isn't `what`, which may go here. A frame
    /// that must close here reports its own E0017 instead, or closes silently. At a break that
    /// holds a `;`, the end of the file after a `;` included, the lexer's E0002 is the error,
    /// so the parser only recovers.
    fn expected(&mut self, what: &str) {
        if self.frame_closes_here() {
            return;
        }
        let brk = self.current().map_or(self.input.eof_brk, |t| t.brk);
        if self.at_break() && brk == Break::Hard {
            self.recovering = true;
            return;
        }
        let error = self.unexpected(what, true);
        self.report(error);
    }

    /// Reports an E0016 at a token that can't start a member of a line context. A member
    /// starts after a break, so the error is the token's, not the break's.
    fn expected_member(&mut self, what: &str) {
        let error = self.unexpected(what, false);
        self.report(error);
    }

    /// [`Parser::expected`], after a list item, with a fix that inserts the `,` when the
    /// current token starts another item on the next line.
    fn expected_in_list(&mut self, what: &str, starts_item: fn(SyntaxKind) -> bool) {
        if self.frame_closes_here() {
            return;
        }
        let mut error = self.unexpected(what, false);
        if self.starts_line() && starts_item(self.kind()) {
            let end = self.input.tokens[self.pos - 1].range.end;
            error = error.with_fix(
                "insert the missing `,`",
                Applicability::NeedsReview,
                vec![(end..end, ",".to_string())],
            );
        }
        self.report(error);
    }

    fn frame_closes_here(&self) -> bool {
        match self.stop() {
            Some(Stop::Outer | Stop::LineEnd) => true,
            Some(Stop::Eof) => self.top().bracket.is_some(),
            None => false,
        }
    }

    /// "expected `what`, found" the current token, or the end of the file. With `breaks`, a
    /// break that ends the construct is what was found, at the break. In a list a break is
    /// only spacing.
    fn unexpected(&self, what: &str, breaks: bool) -> SyntaxError {
        let (range, found) = match self.current() {
            None => (
                self.text.len()..self.text.len(),
                "the end of the file".to_string(),
            ),
            Some(_) if breaks && self.at_break() => {
                (self.break_range(), "a line break".to_string())
            }
            Some(token) => (token.range.clone(), self.describe(token)),
        };
        let message = format!("expected {what}, found {found}");
        SyntaxError::new(range, codes::UNEXPECTED_TOKEN, message)
    }

    /// The first line ending or `;` before the current token.
    fn break_range(&self) -> Range<usize> {
        let previous = &self.input.tokens[self.pos - 1];
        let mut start = previous.range.end;
        for token in &self.lexed.tokens[previous.index + 1..] {
            let end = start + token.len as usize;
            if matches!(token.kind, Newline | Semicolon) {
                return start..end;
            }
            start = end;
        }
        unreachable!("a break is a line ending or a `;`");
    }

    /// The token as an error message names it.
    fn describe(&self, token: &Sig) -> String {
        let text = &self.text[token.range.clone()];
        match token.kind {
            Ident => format!("the name `{text}`"),
            Int | Float => format!("the number `{text}`"),
            Char => "a char literal".to_string(),
            StringStart => "a string".to_string(),
            kind if SyntaxKind::keyword(text) == Some(kind) => format!("the keyword `{text}`"),
            _ => format!("`{text}`"),
        }
    }

    /// Reports `error` unless the slip it is part of is already reported, and starts error
    /// recovery either way.
    fn report(&mut self, error: SyntaxError) {
        let after_error = self
            .current()
            .map_or(self.input.eof_after_error, |t| t.after_error);
        if !self.recovering && !self.closed_early && !after_error {
            self.emit(error);
        }
        self.recovering = true;
    }

    /// Adds `error`, unless it lies on a line an unclosed one-line literal ends, or at the end
    /// of a file that an unclosed string or comment swallowed.
    fn emit(&mut self, error: SyntaxError) {
        let start = error.range.start as usize;
        let swallowed = self.input.eof_swallowed && start == self.text.len();
        if !swallowed && !self.input.on_unclosed_line(start) {
            self.steps.push(Step::Error(error));
        }
    }

    // Recovery.

    /// Skips tokens into a `Skipped` node up to the next sync point of the innermost frame:
    /// `sync`, its closing bracket, or a [`Stop`]. With `force`, the current token is skipped
    /// even where it is a sync point. A bracket the skip opens is skipped up to its closing
    /// bracket, unless a frame's closing bracket or a member start of the file or an `impl`
    /// body comes first.
    fn recover(&mut self, sync: Sync, force: bool) {
        let marker = self.start();
        let mut skipped = false;
        // The closing brackets the skip has opened, with where each opener's line starts.
        let mut groups: Vec<(SyntaxKind, usize)> = Vec::new();
        while let Some(token) = self.current() {
            let (kind, start) = (token.kind, token.range.start);
            if let Some(line_start) = token.closes_line_from {
                groups.retain(|&(_, opened)| opened < line_start);
            }
            if let Some(at) = groups.iter().rposition(|&(closer, _)| closer == kind) {
                groups.truncate(at);
            } else if groups.is_empty() {
                let at_sync = match sync {
                    Sync::Line => self.at_break(),
                    Sync::List => kind == Comma,
                };
                let closes = Some(kind) == self.own_closer() || self.stop().is_some();
                if closes || (at_sync && !(force && !skipped)) {
                    break;
                }
            } else if self.frame_closed_by(kind).is_some()
                || self
                    .member_holder()
                    .is_some_and(|at| matches!(self.frames[at].holds, Holds::File | Holds::Body))
            {
                break;
            }
            if let Some(closer) = closer_of(kind) {
                groups.push((closer, start));
            }
            self.take();
            skipped = true;
        }
        if skipped {
            self.complete(marker, Skipped);
        } else {
            self.abandon(marker);
        }
    }

    // Shared shapes.

    /// A comma-separated list, after its opening bracket, which the caller took: items until
    /// `closer`, each parsed by `item`. Line breaks in it are spacing.
    fn list(
        &mut self,
        closer: SyntaxKind,
        what_item: &str,
        item: fn(&mut Parser),
        starts_item: fn(SyntaxKind) -> bool,
    ) {
        self.push_frame(Holds::Items, Some(closer));
        let close = format!("`{}`", punctuation(closer));
        loop {
            if self.kind() == closer {
                self.pop_frame();
                self.bump();
                return;
            }
            if let Some(stop) = self.stop() {
                self.close_frame(stop);
                return;
            }
            if self.unmatched_closer() {
                self.close_in_place(&format!("{what_item} or {close}"));
                return;
            }
            item(self);
            match self.kind() {
                Comma => self.bump(),
                kind if kind == closer => {}
                _ if self.stop().is_some() => {}
                _ if self.unmatched_closer() => {
                    self.close_in_place(&format!("`,` or {close}"));
                    return;
                }
                _ => {
                    self.expected_in_list(&format!("`,` or {close}"), starts_item);
                    self.recover(Sync::List, false);
                }
            }
        }
    }

    /// The current token is a closing bracket that matches no open frame.
    fn unmatched_closer(&self) -> bool {
        is_closer(self.kind()) && self.frame_closed_by(self.kind()).is_none()
    }

    /// A closing bracket that matches nothing open closes the innermost list in its place,
    /// with an E0016 that names the bracket expected.
    fn close_in_place(&mut self, what: &str) {
        let error = self.unexpected(what, false);
        self.report(error);
        self.pop_frame();
        self.take();
    }

    /// The members of a line context, after its opening bracket, which the caller took: the
    /// file, an `impl` body or a block. Each member is parsed by `member`, which skips a
    /// token that can't start one, and ends at a break.
    fn lines(&mut self, holds: Holds, closer: Option<SyntaxKind>, member: fn(&mut Parser)) {
        self.push_frame(holds, closer);
        loop {
            if closer.is_some() && Some(self.kind()) == closer {
                self.pop_frame();
                self.bump();
                return;
            }
            if let Some(stop) = self.stop() {
                self.close_frame(stop);
                return;
            }
            member(self);
            let ends = Some(self.kind()) == closer || self.stop().is_some() || self.at_break();
            if !ends {
                let what = match closer {
                    Some(_) => "a line break or `}`",
                    None => "a line break",
                };
                self.expected(what);
                self.recover(Sync::Line, false);
            }
        }
    }
}

/// The closing bracket for an opening one.
fn closer_of(kind: SyntaxKind) -> Option<SyntaxKind> {
    match kind {
        LParen => Some(RParen),
        LBracket => Some(RBracket),
        LBrace => Some(RBrace),
        _ => None,
    }
}

fn is_closer(kind: SyntaxKind) -> bool {
    matches!(kind, RParen | RBracket | RBrace)
}

/// The text of a punctuation kind, for messages.
fn punctuation(kind: SyntaxKind) -> &'static str {
    SyntaxKind::PUNCTUATION
        .iter()
        .find(|&&(_, k)| k == kind)
        .map_or("", |&(text, _)| text)
}
