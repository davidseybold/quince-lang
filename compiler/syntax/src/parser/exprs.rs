//! Blocks, statements and expressions: `let`, `var`, `while`, assignment, `if`, literals,
//! names, collections, operators and postfix forms. Later layers of the parser add the rest.

use super::input::Break;
use super::{Completed, Holds, Parser, Sync, items, types};
use crate::SyntaxKind::{self, *};
use crate::{SyntaxError, codes};
use quince_diagnostics::Applicability;
use std::ops::Range;

/// `{` { stmt NEWLINE } [ stmt ] `}` (`grammar.block`). The last statement needs no line break
/// before the `}` (`r[expr.block.last-newline]`).
pub(super) fn block(p: &mut Parser) {
    let marker = p.start();
    p.bump();
    p.lines(Holds::Block, Some(RBrace), stmt);
    p.complete(marker, Block);
}

/// A statement (`grammar.stmt`): `let`, `var`, `while`, an assignment, or an expression.
/// Anything else is skipped as an error.
fn stmt(p: &mut Parser) {
    match p.kind() {
        LetKw => let_stmt(p),
        VarKw => var_stmt(p),
        WhileKw => while_stmt(p),
        kind if starts_expr(kind) => expr_or_assign(p),
        _ => {
            p.expected_member("a statement or `}`");
            p.recover(Sync::Line, true);
        }
    }
}

/// An expression, or a place, `=` or a compound assignment, and an expression
/// (`r[stmt.assign.stmt]`). The left side is parsed as an expression and then checked: when it
/// isn't a place, and holds no syntax error, it is an E0026. Assignment is a statement, so a
/// second `=` after the value is an E0016 at the end of the statement
/// (`r[stmt.assign.no-chain]`).
fn expr_or_assign(p: &mut Parser) {
    let (first, slips) = (p.pos, p.slips);
    let Some(left) = expr_node(p) else {
        return;
    };
    if p.recovering || !is_assign_op(p.kind()) {
        return;
    }
    if p.at_break() {
        if !p.can_join() {
            return;
        }
        p.join_line();
    }
    let marker = p.precede(left);
    if p.slips == slips && !is_place(p, first) {
        let range = text_range(p, (first, p.pos - 1));
        let message = "the left side of an assignment must be a place: a name or `self`, \
                       followed by fields, tuple positions and indexes";
        p.emit(SyntaxError::new(range, codes::NOT_A_PLACE, message.into()));
    }
    p.bump();
    expr(p);
    p.complete(marker, AssignStmt);
}

fn is_assign_op(kind: SyntaxKind) -> bool {
    matches!(kind, Eq | PlusEq | MinusEq | StarEq | SlashEq | PercentEq)
}

/// The significant tokens from `first` up to the current one are a place (`grammar.place`): a
/// name or `self`, followed by `.` and a name or a tuple position, or an index in square
/// brackets, any number of times. The tokens hold no syntax error, so their brackets match.
fn is_place(p: &Parser, first: usize) -> bool {
    let tokens = &p.input.tokens[first..p.pos];
    let Some((head, mut rest)) = tokens.split_first() else {
        return false;
    };
    if !matches!(head.kind, Ident | SelfKw) {
        return false;
    }
    loop {
        match rest {
            [] => return true,
            [dot, step, tail @ ..] if dot.kind == Dot && matches!(step.kind, Ident | Int) => {
                rest = tail;
            }
            [open, ..] if open.kind == LBracket => {
                let mut depth = 0usize;
                let close = rest.iter().position(|token| {
                    if super::closer_of(token.kind).is_some() {
                        depth += 1;
                    } else if super::is_closer(token.kind) {
                        depth -= 1;
                    }
                    depth == 0
                });
                let Some(close) = close else {
                    return false;
                };
                rest = &rest[close + 1..];
            }
            _ => return false,
        }
    }
}

/// `let` name [ `:` type ] `=` expr. Layer 5 of the parser takes a pattern after `let`.
fn let_stmt(p: &mut Parser) {
    binding(p, LetStmt);
}

/// `var` name [ `:` type ] `=` expr. A `var` takes a name, never a pattern.
fn var_stmt(p: &mut Parser) {
    binding(p, VarStmt);
}

fn binding(p: &mut Parser, kind: SyntaxKind) {
    let marker = p.start();
    p.bump();
    if items::name(p, "a name") {
        let_rest(p);
    }
    p.complete(marker, kind);
}

/// `while` expr block (`grammar.stmt`).
fn while_stmt(p: &mut Parser) {
    let marker = p.start();
    p.bump();
    condition_and_block(p);
    p.complete(marker, WhileStmt);
}

/// `if` expr block [ `else` ( if_expr | block ) ] (`grammar.if_expr`).
fn if_expr(p: &mut Parser) -> Completed {
    let marker = p.start();
    p.bump();
    if condition_and_block(p) && else_follows(p) {
        p.bump();
        match p.kind() {
            IfKw if !p.at_break() => {
                if_expr(p);
            }
            LBrace if p.at_or_join(LBrace) => block(p),
            _ => p.expected("`if` or `{`"),
        }
    }
    p.complete(marker, IfExpr)
}

/// The current token is an `else` after an `if`'s block. On the next line it is an E0018,
/// in a list too, where a line break is otherwise spacing (`r[lex.else-same-line]`). After a
/// `;` it isn't: a hard break never joins.
fn else_follows(p: &mut Parser) -> bool {
    let Some(token) = p.current() else {
        return false;
    };
    if token.kind != ElseKw {
        return false;
    }
    match token.brk {
        Break::None => true,
        Break::Line if !p.closed_early => {
            p.join_line();
            true
        }
        _ => false,
    }
}

/// The condition of an `if` or a `while`, and the block after it on its line. False when there
/// is no block. After a slip, the rest of the condition is skipped up to the `{` on its line,
/// so the block is still the block, and nothing after the condition is a second error. A
/// condition missing from its line is taken from the next line, after the error, for the same
/// reason.
fn condition_and_block(p: &mut Parser) -> bool {
    let missing = p.at_break();
    if missing {
        p.expected("an expression");
        // A frame that closes here closes the condition too, with its own E0017.
        if p.frame_closes_here() {
            return false;
        }
        if p.kind() == LBrace {
            block(p);
            return true;
        }
    }
    let outer = p.condition_frames.replace(p.frames.len());
    expr(p);
    p.condition_frames = outer;
    if !p.at_or_join(LBrace) {
        // A block missing after a condition taken from the next line is part of that slip.
        p.recovering |= missing;
        p.expected("`{`");
        p.recover(Sync::Block, false);
        if !p.at(LBrace) {
            return false;
        }
    }
    block(p);
    true
}

fn let_rest(p: &mut Parser) {
    let mut what = "`:` or `=`";
    if p.at(Colon) {
        p.bump();
        if !types::type_(p) {
            return;
        }
        what = "`=`";
    }
    if !p.at_or_join(Eq) {
        return p.expected(what);
    }
    p.bump();
    expr(p);
}

/// An expression (`grammar.expr`): operators over prefix expressions. Any mix of operators
/// parses, by precedence climbing in the usual reading (decision 6), and then the parser
/// reports what the spec forbids: E0019 where parentheses are needed and E0020 where
/// operators chain.
pub(super) fn expr(p: &mut Parser) {
    expr_node(p);
}

/// [`expr`], and the node it completed, if there is an expression.
fn expr_node(p: &mut Parser) -> Option<Completed> {
    let operand = binary(p, 0)?;
    report_chain(p, operand.chain, operand.tokens, None);
    Some(operand.done)
}

/// An expression whose operators all bind tighter than `min_power`.
fn binary(p: &mut Parser, min_power: u8) -> Option<Operand> {
    let (first, slips) = (p.pos, p.slips);
    let done = prefix(p)?;
    let mut lhs = Operand {
        done,
        op: None,
        tokens: (first, p.pos - 1),
        right: (first, p.pos - 1),
        right_op: None,
        chain: None,
        broken: p.slips != slips,
    };
    while let Some((op, power)) = binary_op(p)
        && power > min_power
    {
        let marker = p.precede(lhs.done);
        let at = p.pos;
        match op {
            GtGt | GtEq => p.bump_joined(op),
            _ => p.bump(),
        }
        p.after_operator = true;
        let op_range = p.input.tokens[at].range.start..p.input.tokens[p.pos - 1].range.end;
        let rhs = binary(p, power);
        let kind = match op {
            DotDot | DotDotEq => SyntaxKind::Range,
            _ => Binary,
        };
        let done = p.complete(marker, kind);
        let Some(rhs) = rhs else {
            return Some(Operand {
                done,
                op: Some(op),
                tokens: (first, p.pos - 1),
                right: (p.pos - 1, p.pos - 1),
                right_op: None,
                chain: None,
                broken: true,
            });
        };
        lhs = combine(p, lhs, op, op_range, rhs, done);
        if p.recovering {
            break;
        }
    }
    Some(lhs)
}

/// The binary operator at the current token, joined with the token after it when it is `>`,
/// and how tightly it binds: higher binds tighter. The order is decision 6's usual reading. A
/// line that starts with one continues the line above, an E0018, unless it is `-`, which can
/// start an expression (`r[lex.line-end]`).
fn binary_op(p: &mut Parser) -> Option<(SyntaxKind, u8)> {
    let op = p.joined_gt().unwrap_or(p.kind());
    let power = match op {
        PipePipe => 1,
        AmpAmp => 2,
        EqEq | BangEq | Lt | LtEq | Gt | GtEq => 3,
        DotDot | DotDotEq => 4,
        Pipe => 5,
        Caret => 6,
        Amp => 7,
        LtLt | GtGt => 8,
        Plus | Minus => 9,
        Star | Slash | Percent => 10,
        _ => return None,
    };
    if p.at_break() {
        if op == Minus || !p.can_join() {
            return None;
        }
        p.join_line();
    }
    Some((op, power))
}

/// What the checks of decision 6 tell operators apart by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Group {
    Arith,
    /// `&`, `|` and `^`, which chain only with themselves.
    Bit,
    Shift,
    Range,
    Compare,
    And,
    Or,
}

fn group(op: SyntaxKind) -> Group {
    match op {
        Plus | Minus | Star | Slash | Percent => Group::Arith,
        Amp | Pipe | Caret => Group::Bit,
        LtLt | GtGt => Group::Shift,
        DotDot | DotDotEq => Group::Range,
        AmpAmp => Group::And,
        PipePipe => Group::Or,
        _ => Group::Compare,
    }
}

/// An operand of a binary operator, parsed, with what the checks need to know about it.
struct Operand {
    done: Completed,
    /// The operator at its top. `None` for a prefix or postfix expression, or one in
    /// parentheses.
    op: Option<SyntaxKind>,
    /// Its first and last significant tokens.
    tokens: (usize, usize),
    /// Its right operand's first and last significant tokens, or its own when it has no
    /// operator, and that operand's operator: the middle operand of a chain it starts.
    right: (usize, usize),
    right_op: Option<SyntaxKind>,
    /// The E0020 of the chain of comparisons or ranges this operand is, reported once the
    /// chain is complete.
    chain: Option<Chain>,
    /// It holds a syntax error, which is the slip, so the operator checks skip it and every
    /// operand that holds it.
    broken: bool,
}

/// A chain of comparisons or of ranges: where its second operator is, and for comparisons
/// the edits of the fix that writes `a < b && b < c`, or `None` when there is no fix.
struct Chain {
    at: Range<usize>,
    message: &'static str,
    edits: Option<Vec<(Range<usize>, String)>>,
}

/// The node `done`, `lhs op rhs`, as an operand. Unless an operand is broken, reports an
/// operand that needs parentheses, and starts or extends a chain.
fn combine(
    p: &mut Parser,
    mut lhs: Operand,
    op: SyntaxKind,
    op_range: Range<usize>,
    rhs: Operand,
    done: Completed,
) -> Operand {
    let broken = lhs.broken || rhs.broken;
    let mut chain = None;
    if !broken {
        let chains = lhs.op.is_some_and(|inner| {
            group(inner) == group(op) && matches!(group(op), Group::Compare | Group::Range)
        });
        if chains {
            chain = Some(extend_chain(p, &mut lhs, op, op_range));
        } else {
            needs_parens(p, &lhs, op, true);
            report_chain(p, lhs.chain, lhs.tokens, Some(op));
        }
        needs_parens(p, &rhs, op, false);
        report_chain(p, rhs.chain, rhs.tokens, Some(op));
    } else {
        // A chain in an operand that holds no error is complete, so it is still reported.
        if !lhs.broken {
            report_chain(p, lhs.chain, lhs.tokens, Some(op));
        }
        if !rhs.broken {
            report_chain(p, rhs.chain, rhs.tokens, Some(op));
        }
    }
    Operand {
        done,
        op: Some(op),
        tokens: (lhs.tokens.0, rhs.tokens.1),
        right: rhs.tokens,
        right_op: rhs.op,
        chain,
        broken,
    }
}

/// The text range of the significant tokens `first` to `last`.
fn text_range(p: &Parser, (first, last): (usize, usize)) -> Range<usize> {
    p.input.tokens[first].range.start..p.input.tokens[last].range.end
}

/// Reports an E0019 when `operand`, the left one when `left`, needs parentheses as an operand
/// of `op`, with the fix that adds them.
fn needs_parens(p: &mut Parser, operand: &Operand, op: SyntaxKind, left: bool) {
    let Some(inner) = operand.op else {
        return;
    };
    let outer = group(op);
    let needed = match group(inner) {
        // `r[expr.op.bits.parens]` and `r[expr.op.bits.chain]`. A shift's operands are prefix
        // expressions (`grammar.bits`), so a shift never chains.
        Group::Bit => !(left && inner == op),
        Group::Shift => true,
        Group::Arith => matches!(outer, Group::Bit | Group::Shift),
        // `r[expr.op.range.parens]`.
        Group::Range => true,
        // `r[expr.op.logic.no-mix]`.
        Group::And => outer == Group::Or,
        Group::Or => outer == Group::And,
        Group::Compare => false,
    };
    if !needed {
        return;
    }
    let (inner, outer) = (super::punctuation(inner), super::punctuation(op));
    let message = format!("`{inner}` needs parentheses as an operand of `{outer}`");
    let range = text_range(p, operand.tokens);
    let edits = vec![
        (range.start..range.start, "(".to_string()),
        (range.end..range.end, ")".to_string()),
    ];
    let error = SyntaxError::new(range, codes::PARENTHESES_NEEDED, message).with_fix(
        "add the parentheses",
        Applicability::NeedsReview,
        edits,
    );
    p.emit(error);
}

/// The chain that `lhs`, a comparison or a range, and `op` after it make
/// (`r[expr.op.compare.no-chain]`, `r[expr.op.range.no-chain]`). A comparison chain's fix
/// repeats each middle operand, so `a < b < c` becomes `a < b && b < c`. There is none when a
/// middle operand holds a comment or a line break, which repeating it would copy, or needs
/// parentheses itself, whose E0019 has its own fix.
fn extend_chain(p: &Parser, lhs: &mut Operand, op: SyntaxKind, op_range: Range<usize>) -> Chain {
    let mut chain = lhs.chain.take().unwrap_or_else(|| match group(op) {
        Group::Compare => Chain {
            at: op_range,
            message: "comparisons don't chain",
            edits: Some(Vec::new()),
        },
        _ => Chain {
            at: op_range,
            message: "ranges don't chain",
            edits: None,
        },
    });
    if let Some(edits) = &mut chain.edits {
        let (first, last) = (&p.input.tokens[lhs.right.0], &p.input.tokens[lhs.right.1]);
        let plain = p.lexed.tokens[first.index..=last.index]
            .iter()
            .all(|t| t.kind == Whitespace || !(t.kind.is_trivia() || t.kind == DocComment));
        let bare = lhs
            .right_op
            .is_none_or(|op| !matches!(group(op), Group::Bit | Group::Shift | Group::Range));
        if plain && bare {
            let middle = &p.text[first.range.start..last.range.end];
            let end = last.range.end;
            edits.push((end..end, format!(" && {middle}")));
        } else {
            chain.edits = None;
        }
    }
    chain
}

/// Reports the E0020 of `chain`, the operand whose tokens are `tokens`, if it is one. As an
/// operand of `||` its fix puts the chain in parentheses, since `&&` can't be mixed with `||`.
fn report_chain(
    p: &mut Parser,
    chain: Option<Chain>,
    tokens: (usize, usize),
    outer: Option<SyntaxKind>,
) {
    let Some(chain) = chain else {
        return;
    };
    let mut error = SyntaxError::new(chain.at, codes::CHAINED_OPERATOR, chain.message.into());
    if let Some(mut edits) = chain.edits {
        if outer == Some(PipePipe) {
            let range = text_range(p, tokens);
            edits.insert(0, (range.start..range.start, "(".to_string()));
            edits.push((range.end..range.end, ")".to_string()));
        }
        error = error.with_fix(
            "join the comparisons with `&&`",
            Applicability::NeedsReview,
            edits,
        );
    }
    p.emit(error);
}

/// `-`, `!` or `~` and the prefix expression it applies to, or a postfix expression
/// (`grammar.prefix`). `None` when there is no expression, after reporting it. A line break
/// after `-` is spacing, since `-` can be binary (`r[lex.line-end]`), but one after `!` or `~`
/// ends the statement.
fn prefix(p: &mut Parser) -> Option<Completed> {
    if !matches!(p.kind(), Minus | Bang | Tilde) {
        return postfix(p);
    }
    let marker = p.start();
    if negative_literal(p) {
        p.bump();
        p.bump();
        return Some(p.complete(marker, Literal));
    }
    p.bump();
    if p.at_break() {
        p.expected("an expression");
    } else {
        prefix(p);
    }
    Some(p.complete(marker, Prefix))
}

/// The current `-` and the number after it are one literal (`r[lex.lit.int.neg]`,
/// `r[lex.lit.neg-adjacent]`): nothing comes between them, and no postfix operator follows the
/// number. A `.` after it on the next line is one, as it continues the line, unless a `;`
/// ends the line.
fn negative_literal(p: &Parser) -> bool {
    let tokens = &p.input.tokens[p.pos..];
    let [minus, number, ..] = tokens else {
        return false;
    };
    if !matches!(number.kind, Int | Float) || number.range.start != minus.range.end {
        return false;
    }
    match tokens.get(2) {
        Some(next) if next.kind == Dot && next.brk != Break::Hard => false,
        Some(next) if next.brk == Break::None => !matches!(next.kind, LParen | LBracket | Question),
        _ => true,
    }
}

/// A name being used, and its type arguments when a `<` after it starts them.
fn name_ref(p: &mut Parser) -> Completed {
    let marker = p.start();
    p.bump();
    if p.at(Lt) {
        type_args(p);
    }
    p.complete(marker, NameRef)
}

/// Type arguments after a name in an expression, when the `<` starts them
/// (`r[expr.op.type-args]`): what follows parses as type arguments closed by `>`, and the
/// token after the `>` can follow them. Otherwise the `<` is less-than, and nothing is taken.
/// The parser parses ahead from each `<` once, and keeps what it parsed when they are type
/// arguments.
fn type_args(p: &mut Parser) {
    let at = p.pos;
    if let Some(&end) = p.type_args_end.get(&at) {
        if end.is_some_and(|end| follows_type_args(p, end)) {
            types::expr_type_args(p);
        }
        return;
    }
    let checkpoint = p.checkpoint();
    p.speculating = true;
    types::expr_type_args(p);
    p.speculating = false;
    p.speculation_failed = false;
    let end = p.type_args_end[&at];
    if !end.is_some_and(|end| follows_type_args(p, end)) {
        p.rewind(checkpoint);
    }
}

/// The significant token at `end`, after type arguments' `>`, is `(`, `.`, `?`, `,`, `)`, `]`,
/// `}`, or comes after a line break or at the end of the file. So `=` and `>` aren't.
fn follows_type_args(p: &Parser, end: usize) -> bool {
    match p.input.tokens.get(end) {
        Some(next) if next.brk == Break::None => super::can_follow_type_args(next.kind),
        _ => true,
    }
}

/// A primary expression and its postfix forms (`grammar.postfix`): calls, fields and method
/// calls, indexes and `?`. `None` when there is no expression, after reporting it.
fn postfix(p: &mut Parser) -> Option<Completed> {
    let mut done = primary(p)?;
    loop {
        if p.at(LParen) {
            let marker = p.precede(done);
            arg_list(p);
            done = p.complete(marker, Call);
        } else if p.at(Dot) {
            let marker = p.precede(done);
            p.bump();
            match p.kind() {
                Ident if !p.at_break() => {
                    name_ref(p);
                }
                Int if !p.at_break() => p.bump(),
                _ => p.expected("a field or method name"),
            }
            done = p.complete(marker, Field);
            if p.recovering {
                break;
            }
        } else if p.at(LBracket) {
            let marker = p.precede(done);
            p.bump();
            in_brackets(p, RBracket);
            done = p.complete(marker, Index);
        } else if p.at_or_join(Question) {
            let marker = p.precede(done);
            p.bump();
            done = p.complete(marker, Try);
        } else {
            break;
        }
    }
    Some(done)
}

/// A literal, a name, `self`, `Self`, a list or map literal, a tuple, an expression in
/// parentheses, or an `if` (`grammar.primary`). A statement starts after a break, so this doesn't check
/// for one: each caller has taken a token that continues the line, or is at the start of a
/// statement.
fn primary(p: &mut Parser) -> Option<Completed> {
    let marker = p.start();
    let kind = match p.kind() {
        Int | Float | Char | TrueKw | FalseKw => {
            p.bump();
            Literal
        }
        StringStart => {
            p.abandon(marker);
            return Some(literal(p));
        }
        Ident => {
            p.abandon(marker);
            return Some(name_ref(p));
        }
        SelfKw | SelfTypeKw => {
            p.bump();
            NameRef
        }
        LBracket => {
            p.bump();
            p.list(RBracket, "an expression", expr, starts_expr);
            ListLit
        }
        LParen => {
            p.abandon(marker);
            return Some(paren(p));
        }
        IfKw => {
            p.abandon(marker);
            return Some(if_expr(p));
        }
        // At the top of a condition, `{` is the block, so there is no expression
        // (`r[expr.block.cond-brace]`). Inside brackets in it, `{` starts a map.
        LBrace if p.condition_frames == Some(p.frames.len()) => {
            p.abandon(marker);
            return condition_brace(p);
        }
        LBrace => {
            p.abandon(marker);
            return Some(map_lit(p));
        }
        _ => {
            p.abandon(marker);
            p.expected("an expression");
            return None;
        }
    };
    Some(p.complete(marker, kind))
}

/// A `{` where an expression goes at the top of a condition, which starts the block
/// (`r[expr.block.cond-brace]`). Braces that hold an entry's `:` at their top, `{:}` among them,
/// were meant as a map: the same test a map's own entries use (`holds_entry`). That is an E0016
/// whose fix puts them in parentheses (`needs-review`), and the parser parses the map, so the rest
/// of the condition and its block parse as written. Other braces are the block, and the
/// expression is missing.
fn condition_brace(p: &mut Parser) -> Option<Completed> {
    let Some(end) = map_close(p) else {
        p.expected("an expression");
        return None;
    };
    let (open, close) = (&p.input.tokens[p.pos], &p.input.tokens[end]);
    let range = open.range.start..close.range.end;
    let commented = p.lexed.tokens[open.index..close.index]
        .iter()
        .any(|t| matches!(t.kind, Comment | BlockComment | DocComment));
    let mut error = p.unexpected("an expression", false);
    if !commented {
        error = error.with_fix(
            "put the map in parentheses",
            Applicability::NeedsReview,
            vec![
                (range.start..range.start, "(".to_string()),
                (range.end..range.end, ")".to_string()),
            ],
        );
    }
    // The map is whole, so it isn't a slip that hides the operators around it.
    let after_error = p.current().is_some_and(|t| t.after_error);
    if !(p.recovering || p.closed_early || after_error) {
        p.emit(error);
    }
    Some(map_lit(p))
}

/// The index among the significant tokens of the `}` that closes the current `{`, when the braces
/// hold an entry's `:` at their top, as a map does. A line that starts with a member start, which
/// no map holds, ends the scan, so a block's `let x: T` is never taken for an entry, and the scan
/// reads no further than the braces' own lines. A direct scan, done once, at an error.
fn map_close(p: &Parser) -> Option<usize> {
    let mut open = 0usize;
    let mut entry = false;
    for (i, token) in p.input.tokens.iter().enumerate().skip(p.pos) {
        if i > p.pos && token.brk != Break::None && super::starts_member(token.kind) {
            return None;
        }
        if super::closer_of(token.kind).is_some() {
            open += 1;
        } else if super::is_closer(token.kind) {
            open = open.saturating_sub(1);
            if open == 0 {
                return (token.kind == RBrace && entry).then_some(i);
            }
        } else if open == 1 && token.kind == Colon {
            entry = true;
        }
    }
    None
}

/// One expression and the closing bracket after it, after the opening one, which the caller
/// took.
fn in_brackets(p: &mut Parser, closer: SyntaxKind) {
    p.push_frame(Holds::Items, Some(closer));
    match p.stop() {
        Some(stop) => p.close_frame(stop),
        None => {
            expr(p);
            p.end_list(closer);
        }
    }
}

/// `()`, a tuple of two or more expressions (`grammar.tuple`), or one expression in
/// parentheses. One expression and a `,` is an error, since a tuple has two or more.
fn paren(p: &mut Parser) -> Completed {
    let marker = p.start();
    p.bump();
    if p.kind() == RParen {
        p.bump();
        return p.complete(marker, Tuple);
    }
    p.push_frame(Holds::Items, Some(RParen));
    if let Some(stop) = p.stop() {
        p.close_frame(stop);
        return p.complete(marker, Paren);
    }
    expr(p);
    if p.kind() != Comma {
        p.end_list(RParen);
        return p.complete(marker, Paren);
    }
    p.bump();
    if p.kind() == RParen {
        p.expected("a second expression");
    }
    p.list_items(RParen, "an expression", expr, starts_expr, false);
    p.complete(marker, Tuple)
}

/// A map literal, where `{` starts an expression (`grammar.map_lit`,
/// `r[expr.block.brace-map]`): `{:}`, or entries in braces. `{}` is an E0025, whose fix
/// writes `{:}` (`r[lex.lit.empty-map]`). When a block's statement closes the list before
/// any entry, the braces were a block, which can't go here (`r[expr.block.no-bare]`): one
/// E0016 at the `{`, and the rest of the block up to its `}` is skipped.
fn map_lit(p: &mut Parser) -> Completed {
    let marker = p.start();
    let open = p.pos;
    p.bump();
    if p.kind() == RBrace {
        let brace = p.input.tokens[open].range.clone();
        let range = brace.start..p.input.tokens[p.pos].range.end;
        let message = "`{}` isn't a value: the empty map is `{:}`".to_string();
        let error = SyntaxError::new(range, codes::EMPTY_BRACES, message).with_fix(
            "write the empty map",
            Applicability::NeedsReview,
            vec![(brace.end..brace.end, ":".to_string())],
        );
        // `{}` is whole, so it isn't a slip that hides the operators around it.
        let after_error = p.current().is_some_and(|t| t.after_error);
        if !(p.recovering || p.closed_early || after_error) {
            p.emit(error);
        }
        p.bump();
    } else if p.kind() == Colon && p.nth_kind(1) == RBrace {
        p.bump();
        p.bump();
    } else {
        p.push_frame(Holds::Map, Some(RBrace));
        p.list_items(RBrace, "a map entry", entry, starts_expr, false);
        if p.block_at == Some(open) {
            p.block_at = None;
            skip_block(p);
        }
    }
    p.complete(marker, MapLit)
}

/// The rest of a block written where an expression goes, into a `Skipped` node, up to and
/// including the `}` that closes it. A line that starts a member of the file or an `impl`
/// body ends it, as it ends any skip.
fn skip_block(p: &mut Parser) {
    let marker = p.start();
    let mut open = 1;
    let mut skipped = false;
    while let Some(token) = p.current() {
        let kind = token.kind;
        let member = p
            .member_holder()
            .is_some_and(|at| matches!(p.frames[at].holds, Holds::File | Holds::Body));
        if member {
            break;
        }
        p.take();
        skipped = true;
        match kind {
            LBrace => open += 1,
            RBrace if open == 1 => break,
            RBrace => open -= 1,
            _ => {}
        }
    }
    if skipped {
        p.complete(marker, Skipped);
    } else {
        p.abandon(marker);
    }
}

/// expr `:` expr (`grammar.entry`). With no expression, there is no entry, only the error.
fn entry(p: &mut Parser) {
    if !starts_expr(p.kind()) {
        return expr(p);
    }
    let marker = p.start();
    expr(p);
    if !p.recovering {
        if p.kind() == Colon {
            p.bump();
            expr(p);
        } else if !p.frame_closes_here() {
            let error = match p.top().bracket {
                // The braces hold no entry's `:`, and end or have a line after this item: they
                // hold statements, a block, which can't go here (`r[expr.block.no-bare]`).
                Some((_, opener))
                    if (p.kind() == RBrace || p.starts_line()) && !p.braces_hold_entry(opener) =>
                {
                    let range = p.input.tokens[opener].range.clone();
                    let message = "expected an expression, found a block".to_string();
                    SyntaxError::new(range, codes::UNEXPECTED_TOKEN, message)
                }
                // A line break in the braces is spacing, so the error is the token after it.
                _ => p.unexpected("`:`", false),
            };
            p.report(error);
        }
    }
    p.complete(marker, MapEntry);
}

/// The tokens that can start an expression.
fn starts_expr(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        Int | Float
            | Char
            | StringStart
            | TrueKw
            | FalseKw
            | Ident
            | SelfKw
            | SelfTypeKw
            | LBracket
            | LParen
            | LBrace
            | IfKw
            | Minus
            | Bang
            | Tilde
    )
}

/// A string literal, at its `StringStart`: its text, escapes and interpolations, and its
/// `StringEnd` when it has one. The lexer reported a missing one.
pub(super) fn literal(p: &mut Parser) -> Completed {
    let marker = p.start();
    p.bump();
    loop {
        match p.kind() {
            StringText | StringNewline | StringIndent => p.bump(),
            InterpStart => {
                let interp = p.start();
                p.bump();
                while matches!(p.kind(), Ident | SelfKw | Dot | Int) {
                    p.bump();
                }
                if p.kind() == InterpEnd {
                    p.bump();
                }
                p.complete(interp, Interp);
            }
            StringEnd => {
                p.bump();
                break;
            }
            _ => break,
        }
    }
    p.complete(marker, Literal)
}

/// An attribute's arguments: `(` arg { `,` arg } [ `,` ] `)` (`grammar.attribute`), at least
/// one.
pub(super) fn attribute_args(p: &mut Parser) {
    let marker = p.start();
    p.bump();
    if p.kind() == RParen {
        p.expected("an expression");
    }
    p.list(RParen, "an expression", arg, starts_expr);
    p.complete(marker, ArgList);
}

/// `(` [ arg { `,` arg } [ `,` ] ] `)` (`grammar.args`), where the first argument may be
/// `..` and an expression, the base of a struct update (`r[item.struct.update]`). Only name
/// resolution can tell a struct from a function, so any call takes one. After the base come
/// one or more named fields (`grammar.struct_lit`), so `User(..user)`, which would mean
/// `user`, is an error.
fn arg_list(p: &mut Parser) {
    let marker = p.start();
    p.bump();
    if p.kind() == DotDot {
        p.push_frame(Holds::Items, Some(RParen));
        let base = p.start();
        p.bump();
        if p.at_break() {
            p.expected("an expression");
        } else {
            expr(p);
        }
        p.complete(base, UpdateArg);
        if p.kind() == RParen {
            p.expected("`,` and `name:`");
        } else if p.kind() == Comma && p.nth_kind(1) == RParen {
            p.bump();
            p.expected("`name:`");
        }
        p.list_items(RParen, "`name:`", field_arg, |kind| kind == Ident, true);
    } else {
        p.list(RParen, "an expression", arg, starts_expr);
    }
    p.complete(marker, ArgList);
}

/// An expression, or a name, `:` and an expression if there is one (`grammar.arg`). `name:`
/// alone, before a `,` or the `)`, passes the binding of that name. A line break in the list is
/// spacing, after the `:` too.
fn arg(p: &mut Parser) {
    if at_named(p) {
        return named_arg(p);
    }
    if p.kind() == DotDot {
        return p.expected("an expression or `name:`");
    }
    expr(p);
}

/// A named argument after a struct update's base (`grammar.field_init`).
fn field_arg(p: &mut Parser) {
    if at_named(p) {
        named_arg(p);
    } else {
        p.expected("`name:`");
    }
}

fn at_named(p: &Parser) -> bool {
    p.kind() == Ident && p.nth_kind(1) == Colon
}

/// A name, `:` and an expression if there is one: `name:` alone, before a `,` or the `)`.
fn named_arg(p: &mut Parser) {
    let marker = p.start();
    p.bump_node(NameRef);
    p.bump();
    if !matches!(p.kind(), Comma | RParen) {
        expr(p);
    }
    p.complete(marker, NamedArg);
}
