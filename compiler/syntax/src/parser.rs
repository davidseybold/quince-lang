//! The parser: the lexer's tokens to events, which [`crate::cst::build`] turns into the tree.
//! The README's CST section has the contract, and `docs/design/m1-parser.md` the design. Each
//! grammar area has its own module.

mod arms;
mod exprs;
mod input;
mod items;
mod patterns;
mod types;

use crate::cst::{self, Event};
use crate::{Decoded, Lexed, SyntaxError, SyntaxKind, SyntaxNode, codes, lex, lex_str};
use SyntaxKind::*;
use input::{Break, Input, Sig};
use quince_diagnostics::{Applicability, Diagnostic, LineIndex};
use rowan::GreenNode;
use std::cell::Cell;
use std::collections::HashMap;
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
    let mut parser = Parser::new(text, &lexed, input);
    items::file(&mut parser);
    debug_assert_eq!(
        parser.pos,
        parser.input.tokens.len(),
        "the parser left a significant token"
    );
    let events = into_events(parser.steps);
    let (green, parser_errors) = cst::build(text, &lexed.tokens, &parser.taken, events);
    let mut errors = lexed.errors;
    errors.extend(parser_errors);
    // Stable, so the lexer's errors come first where both start at one offset.
    errors.sort_by_key(|error| error.range.start);
    Parse { green, errors }
}

/// How many tokens parsing `text` takes while parsing ahead from `<`s.
#[cfg(test)]
pub(crate) fn speculated_tokens(text: &str) -> usize {
    let lexed = lex_str(text);
    let mut parser = Parser::new(text, &lexed, Input::new(text, &lexed));
    items::file(&mut parser);
    parser.speculated
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
    /// Takes two adjacent tokens as one of this kind (decision 5).
    Join(SyntaxKind),
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
            Step::Join(kind) => events.push(Event::Join(kind)),
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
    /// Struct braces, enum braces and a variant's fields: a list, whose member start is `pub`
    /// and a name.
    Fields,
    Block,
    /// A map literal's entries: a list, which a block's statements may close.
    Map,
    /// A `match`'s arms: a list. An arm's `=>` closes every bracket opened inside the arm
    /// outside a block (decision 9).
    Arms,
}

impl Holds {
    /// A list context, where a line break is spacing.
    fn is_list(self) -> bool {
        matches!(
            self,
            Holds::Items | Holds::Fields | Holds::Map | Holds::Arms
        )
    }
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
    /// At the `{` on the line, or the next break: in a condition, before its block.
    Block,
}

struct Parser<'t> {
    text: &'t str,
    lexed: &'t Lexed,
    input: Input,
    pos: usize,
    steps: Vec<Step>,
    /// The index of each lexer token the parser took, in order: the significant tokens, and
    /// the doc comments it took as part of what they document. The rest is trivia.
    taken: Vec<usize>,
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
    /// The parser is in the condition of an `if` or a `while`, the subject of a `match` or the
    /// iterable of a `for`, and this many frames are open at its top, where `{` always starts
    /// the block (`r[expr.block.cond-brace]`).
    condition_frames: Option<usize>,
    /// The parser has just taken attributes, so the line after them continues what they
    /// apply to.
    after_attributes: bool,
    /// The parser has just taken a binary operator, so a line break after it is spacing,
    /// after `*` and `>` too (`r[lex.line-end]`).
    after_operator: bool,
    /// The index among the significant tokens of the token after a line break the parser has
    /// reported as an E0018, and parses on from as though the lines were joined (decision 4).
    joined: Option<usize>,
    /// How many slips the parser has met: each error it reports or would report, and each
    /// bracket closed without its closing bracket. The operator checks skip an operand that
    /// holds one.
    slips: u32,
    /// The file has had a declaration, so an import is out of place.
    imports_closed: bool,
    lookahead: Cell<u32>,
    /// A block's statement closed the map literal whose `{` is at this index among the
    /// significant tokens: no entry came before it, so the braces were a block, which can't go
    /// where an expression goes (`r[expr.block.no-bare]`). The map skips to its `}`.
    block_at: Option<usize>,
    /// The parser is parsing ahead to see whether a `<` in an expression starts type
    /// arguments (decision 6).
    speculating: bool,
    /// While speculating, a slip happened. From then on the parser sees no more tokens, so
    /// every construct it has open closes at once.
    speculation_failed: bool,
    /// Each `<` the parser has parsed ahead from, nested ones included, by its index among
    /// the significant tokens: the index after the `>` that closes its type arguments, or
    /// `None` when what follows isn't type arguments. So each `<` is examined once.
    type_args_end: HashMap<usize, Option<usize>>,
    /// How many tokens the parser has taken while parsing ahead, for the test that it stays
    /// linear.
    #[cfg(test)]
    speculated: usize,
}

/// Where the parser was, so it can go back there after parsing ahead.
struct Checkpoint {
    pos: usize,
    steps: usize,
    taken: usize,
    frames: usize,
    slips: u32,
    recovering: bool,
    closed_early: bool,
    after_operator: bool,
    joined: Option<usize>,
}

impl<'t> Parser<'t> {
    fn new(text: &'t str, lexed: &'t Lexed, input: Input) -> Parser<'t> {
        Parser {
            text,
            lexed,
            input,
            pos: 0,
            steps: Vec::new(),
            taken: Vec::new(),
            frames: Vec::new(),
            recovering: false,
            closed_early: false,
            in_fn_header: false,
            condition_frames: None,
            after_attributes: false,
            after_operator: false,
            joined: None,
            slips: 0,
            imports_closed: false,
            lookahead: Cell::new(0),
            block_at: None,
            speculating: false,
            speculation_failed: false,
            type_args_end: HashMap::new(),
            #[cfg(test)]
            speculated: 0,
        }
    }

    // Looking at tokens.

    /// The `n`th significant token from the current one, if there is one.
    fn nth(&self, n: usize) -> Option<&Sig> {
        let count = self.lookahead.get() + 1;
        debug_assert!(count <= LOOKAHEAD_LIMIT, "the parser is stuck");
        self.lookahead.set(count);
        if self.speculation_failed {
            return None;
        }
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
    /// there is one, and the token before it doesn't continue the line. A list reads its `,`
    /// and its closing bracket by kind, whatever the break before them, so a break there is
    /// only spacing (`r[lex.continue.lists]`); elsewhere in a list the same rules apply. Right
    /// after frames closed early, at a member start, every line break ends it: the token before
    /// belongs to a construct that is already closed. Right after attributes, a line break
    /// doesn't: an attribute goes on its own line. Nor does one right after a binary operator,
    /// or one reported as an E0018.
    fn at_break(&self) -> bool {
        let Some(token) = self.current() else {
            return true;
        };
        if self.joined == Some(self.pos) {
            return false;
        }
        match token.brk {
            Break::None => false,
            Break::Hard => true,
            Break::Line if self.after_attributes || self.after_operator => false,
            Break::Line if self.closed_early => true,
            Break::Line => {
                let previous = self.input.tokens[self.pos - 1].kind;
                let header_continues = self.in_fn_header
                    && matches!(token.kind, Arrow | UsesKw | RequiresKw | EnsuresKw | LBrace);
                !(previous.continues_line() || token.kind == Dot || header_continues)
            }
        }
    }

    /// The current token comes after a line break that ends the construct the parser is in,
    /// so a token that can't start a statement but continues that construct may join it.
    fn can_join(&self) -> bool {
        self.at_break()
            && !self.closed_early
            && self.current().is_some_and(|t| t.brk == Break::Line)
    }

    /// The current token is `kind`, on the line of the construct the parser is in, or after a
    /// line break that ends it too early, which [`Parser::join_line`] reports.
    fn at_or_join(&mut self, kind: SyntaxKind) -> bool {
        if self.kind() != kind {
            return false;
        }
        if !self.at_break() {
            return true;
        }
        if !self.can_join() {
            return false;
        }
        self.join_line();
        true
    }

    /// Reports an E0018 at the line break before the current token, which continues the line
    /// above, and parses on as though the lines were joined (decision 4). The fix replaces
    /// everything between the two tokens with a space, unless a comment lies between them,
    /// which joining would move code into or delete.
    fn join_line(&mut self) {
        let (previous, token) = (
            &self.input.tokens[self.pos - 1],
            &self.input.tokens[self.pos],
        );
        let gap = previous.range.end..token.range.start;
        let commented = self.lexed.tokens[previous.index + 1..token.index]
            .iter()
            .any(|t| matches!(t.kind, Comment | BlockComment | DocComment));
        // Type arguments in an expression can be accepted because a line break follows their
        // `>` (`r[expr.op.type-args]`). Joining the lines would make the `<` less-than, unless
        // the token after the break may follow type arguments too.
        let after_type_args = previous.kind == Gt
            && self
                .type_args_end
                .values()
                .any(|&end| end == Some(self.pos))
            && !can_follow_type_args(token.kind);
        let after_error = token.after_error;
        let text = match self.joined_gt() {
            Some(joined) => punctuation(joined),
            None => &self.text[token.range.clone()],
        };
        let message =
            format!("a line break can't come before `{text}`, which continues the line above");
        let mut error = SyntaxError::new(self.break_range(), codes::EARLY_LINE_BREAK, message);
        if !commented && !after_type_args {
            error = error.with_fix(
                "join the lines",
                Applicability::Safe,
                vec![(gap, " ".into())],
            );
        }
        if !self.recovering && !after_error {
            self.emit(error);
        }
        self.joined = Some(self.pos);
    }

    /// What the current `>` and the token after it join into, `>>` or `>=`, when no trivia
    /// sits between them (decision 5).
    fn joined_gt(&self) -> Option<SyntaxKind> {
        let (gt, next) = (self.current()?, self.nth(1)?);
        if gt.kind != Gt || next.range.start != gt.range.end {
            return None;
        }
        match next.kind {
            Gt => Some(GtGt),
            Eq => Some(GtEq),
            _ => None,
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
        self.taken.push(self.input.tokens[self.pos].index);
        self.pos += 1;
        #[cfg(test)]
        if self.speculating {
            self.speculated += 1;
        }
        self.after_attributes = false;
        self.after_operator = false;
        self.lookahead.set(0);
    }

    /// Takes the current `>` and the token right after it as one token of `kind`, `>>` or `>=`
    /// (decision 5). [`Parser::joined_gt`] says whether they join.
    fn bump_joined(&mut self, kind: SyntaxKind) {
        self.bump();
        self.bump();
        self.steps.truncate(self.steps.len() - 2);
        self.steps.push(Step::Join(kind));
    }

    /// Takes the doc comments right before the current token into a `DocComments` node, if
    /// there are any. Those it doesn't take stay trivia.
    fn doc_comments(&mut self) {
        let Some(from) = self.current().and_then(|t| t.docs_from) else {
            return;
        };
        let marker = self.start();
        for index in from..self.input.tokens[self.pos].index {
            if self.lexed.tokens[index].kind == DocComment {
                self.steps.push(Step::Token);
                self.taken.push(index);
            }
        }
        self.complete(marker, DocComments);
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
        self.slip();
        let frame = self.frames.pop().expect("a frame to close");
        self.lookahead.set(0);
        self.closed_early = true;
        let Some((closer, opener)) = frame.bracket else {
            return;
        };
        if frame.holds == Holds::Map && stop == Stop::Outer {
            let block_member = self
                .member_holder()
                .is_some_and(|at| self.frames[at].holds == Holds::Block);
            if block_member && !self.holds_entry(opener) {
                self.block_at = Some(opener);
                if !self.recovering {
                    let range = self.input.tokens[opener].range.clone();
                    let message = "expected an expression, found a block".to_string();
                    self.emit(SyntaxError::new(range, codes::UNEXPECTED_TOKEN, message));
                }
                self.recovering = true;
                return;
            }
        }
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

    /// A map's tokens from its `{` at `opener` up to the current token hold an entry's `:`,
    /// outside any bracket inside it. A direct scan, done once when a map closes early.
    fn holds_entry(&self, opener: usize) -> bool {
        let mut open = 0usize;
        for token in &self.input.tokens[opener + 1..self.pos] {
            if closer_of(token.kind).is_some() {
                open += 1;
            } else if is_closer(token.kind) {
                open = open.saturating_sub(1);
            } else if open == 0 && token.kind == Colon {
                return true;
            }
        }
        false
    }

    /// The braces whose `{` is at `opener` hold an entry's `:`, outside any bracket inside
    /// them, before their `}`. The scan stops at a line that starts with a member start, which
    /// no map holds. A direct scan, done once, when an entry has no `:`.
    fn braces_hold_entry(&self, opener: usize) -> bool {
        let mut open = 0usize;
        for token in &self.input.tokens[opener + 1..] {
            if token.brk != Break::None && starts_member(token.kind) {
                return false;
            }
            if closer_of(token.kind).is_some() {
                open += 1;
            } else if is_closer(token.kind) {
                if open == 0 {
                    return false;
                }
                open -= 1;
            } else if open == 0 && token.kind == Colon {
                return true;
            }
        }
        false
    }

    /// Why the innermost frame must close here without its closing bracket, if it must
    /// (decision 9).
    fn stop(&self) -> Option<Stop> {
        let Some(token) = self.current() else {
            return Some(Stop::Eof);
        };
        let top = self.frames.len() - 1;
        let frame = self.top();
        if let (Some(line_start), true, Some((_, opener))) =
            (token.closes_line_from, frame.holds.is_list(), frame.bracket)
            && self.input.tokens[opener].range.start >= line_start
        {
            return Some(Stop::LineEnd);
        }
        if let Some(at) = self.frame_closed_by(token.kind)
            && at < top
        {
            return Some(Stop::Outer);
        }
        // `=>` goes only between an arm's pattern or guard and its body, never inside a
        // bracket, so it closes what the arm left open.
        if token.kind == FatArrow && self.arms_frame().is_some_and(|at| at < top) {
            return Some(Stop::Outer);
        }
        match self.member_holder() {
            Some(at) if at < top => Some(Stop::Outer),
            _ => None,
        }
    }

    /// The innermost frame that holds a `match`'s arms, when no block lies inside it: a block
    /// is a line context again, whose lines the arms' rules don't reach.
    fn arms_frame(&self) -> Option<usize> {
        let at = self
            .frames
            .iter()
            .rposition(|frame| matches!(frame.holds, Holds::Arms | Holds::Block))?;
        (self.frames[at].holds == Holds::Arms).then_some(at)
    }

    /// The innermost frame whose closing bracket is `kind`.
    fn frame_closed_by(&self, kind: SyntaxKind) -> Option<usize> {
        self.frames
            .iter()
            .rposition(|frame| frame.bracket.is_some_and(|(closer, _)| closer == kind))
    }

    /// When the current token starts a line and can only start a member of some containers,
    /// the innermost open one. Attributes belong to the member after them, so the check looks
    /// past them (decision 9).
    fn member_holder(&self) -> Option<usize> {
        if !self.starts_line() {
            return None;
        }
        // The line's first tokens, past its attributes and a `pub`, are read once, so the check
        // takes as many looks however many frames are open.
        let past = self.attributes_len();
        let first = self.nth_kind(past);
        let public = first == PubKw;
        let start = past + usize::from(public);
        let (kind, next) = (self.nth_kind(start), self.nth_kind(start + 1));
        let fn_start = kind == FnKw && next == Ident;
        let item = fn_start
            || matches!(
                kind,
                StructKw
                    | EnumKw
                    | TypeKw
                    | InterfaceKw
                    | ImplKw
                    | EffectKw
                    | ForeignKw
                    | ImportKw
                    | ConstKw
                    | TestKw
            );
        let member = fn_start || matches!(kind, TypeKw | ConstKw);
        let field = public && kind == Ident;
        let statement = matches!(first, LetKw | VarKw | WhileKw);
        self.frames.iter().rposition(|frame| match frame.holds {
            Holds::Items | Holds::Map | Holds::Arms => false,
            Holds::File => item,
            Holds::Body => member,
            Holds::Fields => field,
            Holds::Block => statement,
        })
    }

    /// How many tokens the attributes at the current token hold: each is `@`, a name on its
    /// line, and its arguments in parentheses if it has any. Zero when one is malformed, so a
    /// slip in an attribute closes nothing. The scan stops at a line that starts a member,
    /// which no attribute's arguments hold, so it reads no further than the attribute's own
    /// lines. A direct scan: it takes nothing, and isn't a loop that could stick.
    fn attributes_len(&self) -> usize {
        let tokens = &self.input.tokens[self.pos..];
        let on_line = |n: usize, kind: SyntaxKind| {
            tokens
                .get(n)
                .is_some_and(|t| t.kind == kind && t.brk == Break::None)
        };
        let mut n = 0;
        while tokens.get(n).is_some_and(|t| t.kind == At) {
            if !on_line(n + 1, Ident) {
                return 0;
            }
            n += 2;
            if !on_line(n, LParen) {
                continue;
            }
            let mut open = Vec::new();
            loop {
                let Some(token) = tokens.get(n) else {
                    return 0;
                };
                if !open.is_empty() && token.brk != Break::None && starts_member(token.kind) {
                    return 0;
                }
                n += 1;
                if let Some(closer) = closer_of(token.kind) {
                    open.push(closer);
                } else if is_closer(token.kind) {
                    if open.pop() != Some(token.kind) {
                        return 0;
                    }
                    if open.is_empty() {
                        break;
                    }
                }
            }
        }
        n
    }

    // Errors.

    /// Reports an E0016 at the current token: it isn't `what`, which may go here. A frame
    /// that must close here reports its own E0017 instead, or closes silently. At a break that
    /// holds a `;`, the end of the file after a `;` included, the lexer's E0002 is the error,
    /// so the parser only recovers.
    fn expected(&mut self, what: &str) {
        if self.frame_closes_here() {
            self.slip();
            return;
        }
        let brk = self.current().map_or(self.input.eof_brk, |t| t.brk);
        if self.at_break() && brk == Break::Hard {
            self.slip();
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
            self.slip();
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

    /// True when an unclosed one-line literal ends the line of the token before the current
    /// one.
    fn after_unclosed_line(&self) -> bool {
        self.pos > 0
            && self
                .input
                .on_unclosed_line(self.input.tokens[self.pos - 1].range.end)
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
        self.slip();
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

    /// Counts a slip. While speculating, it means what follows the `<` isn't type arguments.
    fn slip(&mut self) {
        self.slips += 1;
        if self.speculating {
            self.speculation_failed = true;
        }
    }

    // Parsing ahead.

    fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            pos: self.pos,
            steps: self.steps.len(),
            taken: self.taken.len(),
            frames: self.frames.len(),
            slips: self.slips,
            recovering: self.recovering,
            closed_early: self.closed_early,
            after_operator: self.after_operator,
            joined: self.joined,
        }
    }

    /// Goes back to `checkpoint`, dropping everything parsed since, errors included.
    fn rewind(&mut self, checkpoint: Checkpoint) {
        debug_assert_eq!(
            self.frames.len(),
            checkpoint.frames,
            "parsing ahead left a frame"
        );
        self.pos = checkpoint.pos;
        self.steps.truncate(checkpoint.steps);
        self.taken.truncate(checkpoint.taken);
        self.slips = checkpoint.slips;
        self.recovering = checkpoint.recovering;
        self.closed_early = checkpoint.closed_early;
        self.after_operator = checkpoint.after_operator;
        self.joined = checkpoint.joined;
        self.after_attributes = false;
        self.lookahead.set(0);
    }

    // Recovery.

    /// Skips tokens into a `Skipped` node up to the next sync point of the innermost frame:
    /// `sync`, its closing bracket, or a [`Stop`]. With `force`, the current token is skipped
    /// even where it is a sync point. A bracket the skip opens is skipped up to its closing
    /// bracket, and so is a `<` in a list, unless a frame's closing bracket, a member start of
    /// the file or an `impl` body, or an arm's `=>` comes first.
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
                    Sync::Block => kind == LBrace || self.at_break(),
                };
                let closes = Some(kind) == self.own_closer() || self.stop().is_some();
                if closes || (at_sync && !(force && !skipped)) {
                    break;
                }
            } else if self.frame_closed_by(kind).is_some()
                || (kind == FatArrow && self.arms_frame().is_some())
                || self
                    .member_holder()
                    .is_some_and(|at| matches!(self.frames[at].holds, Holds::File | Holds::Body))
            {
                break;
            }
            if let Some(closer) = closer_of(kind) {
                groups.push((closer, start));
            } else if kind == Lt && self.top().holds.is_list() {
                // A `<` in a list is taken to open type arguments or generics, so the skip
                // stays out of their commas. One that is less-than in an argument skips to the
                // list's closing bracket. In a line context it may be less-than.
                groups.push((Gt, start));
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
    /// `closer`, each parsed by `item`, with an optional trailing `,`. Line breaks in it are
    /// spacing.
    fn list(
        &mut self,
        closer: SyntaxKind,
        what_item: &str,
        item: fn(&mut Parser),
        starts_item: fn(SyntaxKind) -> bool,
    ) {
        self.push_frame(Holds::Items, Some(closer));
        self.list_items(closer, what_item, item, starts_item, false);
    }

    /// The rest of a [`Parser::list`] whose frame is open. With `after_item`, the caller has
    /// parsed an item, so the list goes on at the token after it.
    fn list_items(
        &mut self,
        closer: SyntaxKind,
        what_item: &str,
        item: fn(&mut Parser),
        starts_item: fn(SyntaxKind) -> bool,
        mut after_item: bool,
    ) {
        let close = format!("`{}`", punctuation(closer));
        loop {
            if !after_item {
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
            }
            after_item = false;
            match self.kind() {
                Comma => self.bump(),
                kind if kind == closer => {}
                _ if self.stop().is_some() => {}
                _ if self.unmatched_closer() => {
                    self.close_in_place(&format!("`,` or {close}"));
                    return;
                }
                // After an item whose line an unclosed string or char ends, the missing `,`
                // is part of that slip, and may be inside the literal, so it isn't reported
                // and the next item is parsed as one. An item can stop at the line break
                // before it, as a parameter does, so one that takes nothing is skipped.
                kind if starts_item(kind) && self.after_unclosed_line() => {
                    self.slip();
                    let at = self.pos;
                    item(self);
                    if self.pos == at {
                        self.recover(Sync::List, true);
                    }
                    after_item = true;
                }
                _ => {
                    self.expected_in_list(&format!("`,` or {close}"), starts_item);
                    self.recover(Sync::List, false);
                }
            }
        }
    }

    /// The closing bracket of a list that holds one item, after it: what is left before the
    /// bracket is skipped, after one E0016.
    fn end_list(&mut self, closer: SyntaxKind) {
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
                self.close_in_place(&close);
                return;
            }
            self.expected(&close);
            self.recover(Sync::List, true);
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

/// Kinds that, at the start of a line, can only start a member or an attribute: never part of
/// an attribute's arguments.
fn starts_member(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        At | FnKw
            | StructKw
            | EnumKw
            | TypeKw
            | InterfaceKw
            | ImplKw
            | EffectKw
            | ForeignKw
            | ImportKw
            | ConstKw
            | TestKw
            | PubKw
            | LetKw
            | VarKw
            | WhileKw
    )
}

/// The tokens that may come right after type arguments' `>` in an expression
/// (`r[expr.op.type-args]`). So `=` and `>` can't.
fn can_follow_type_args(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        LParen | Dot | Question | Comma | RParen | RBracket | RBrace
    )
}

fn is_closer(kind: SyntaxKind) -> bool {
    matches!(kind, RParen | RBracket | RBrace)
}

/// The text of a punctuation kind, joined ones included, for messages.
fn punctuation(kind: SyntaxKind) -> &'static str {
    SyntaxKind::PUNCTUATION
        .iter()
        .chain(SyntaxKind::JOINED)
        .find(|&&(_, k)| k == kind)
        .map_or("", |&(text, _)| text)
}
