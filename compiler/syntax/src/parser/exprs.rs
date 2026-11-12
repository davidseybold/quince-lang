//! Blocks, statements and expressions: `let`, literals, names, list literals, operators,
//! calls, method calls and fields. Later layers of the parser add the rest.

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

/// A statement: `let`, or an expression (`grammar.stmt`). Anything else is skipped as an
/// error.
fn stmt(p: &mut Parser) {
    if p.kind() == LetKw {
        return let_stmt(p);
    }
    if starts_expr(p.kind()) {
        expr(p);
    } else {
        p.expected_member("a statement or `}`");
        p.recover(Sync::Line, true);
    }
}

/// `let` name [ `:` type ] `=` expr. Layer 5 of the parser takes a pattern after `let`.
fn let_stmt(p: &mut Parser) {
    let marker = p.start();
    p.bump();
    if items::name(p, "a name") {
        let_rest(p);
    }
    p.complete(marker, LetStmt);
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
    if !p.at(Eq) {
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
    if let Some(operand) = binary(p, 0) {
        report_chain(p, operand.chain, operand.tokens, None);
    }
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
/// and how tightly it binds: higher binds tighter. The order is decision 6's usual reading.
fn binary_op(p: &Parser) -> Option<(SyntaxKind, u8)> {
    if p.at_break() {
        return None;
    }
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
/// number. A `.` after it on the next line is one, as it continues the line.
fn negative_literal(p: &Parser) -> bool {
    let tokens = &p.input.tokens[p.pos..];
    let [minus, number, ..] = tokens else {
        return false;
    };
    if !matches!(number.kind, Int | Float) || number.range.start != minus.range.end {
        return false;
    }
    match tokens.get(2) {
        Some(next) if next.kind == Dot => false,
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
        Some(next) if next.brk == Break::None => matches!(
            next.kind,
            LParen | Dot | Question | Comma | RParen | RBracket | RBrace
        ),
        _ => true,
    }
}

/// A primary expression and its calls, method calls and fields (`grammar.postfix`). `None`
/// when there is no expression, after reporting it.
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
        } else {
            break;
        }
    }
    Some(done)
}

/// A literal, a name, `self`, `Self`, a list literal or an expression in parentheses
/// (`grammar.primary`). A statement starts
/// after a break, so this doesn't check for one: each caller has taken a token that continues
/// the line, or is at the start of a statement.
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
            p.bump();
            p.push_frame(Holds::Items, Some(RParen));
            match p.stop() {
                Some(stop) => p.close_frame(stop),
                None => {
                    expr(p);
                    p.end_list(RParen);
                }
            }
            Paren
        }
        _ => {
            p.abandon(marker);
            p.expected("an expression");
            return None;
        }
    };
    Some(p.complete(marker, kind))
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
/// one. Layer 3 of the parser adds named arguments here too.
pub(super) fn attribute_args(p: &mut Parser) {
    let marker = p.start();
    p.bump();
    if p.kind() == RParen {
        p.expected("an expression");
    }
    p.list(RParen, "an expression", expr, starts_expr);
    p.complete(marker, ArgList);
}

/// `(` [ arg { `,` arg } [ `,` ] ] `)` (`grammar.args`). Layer 3 of the parser adds named
/// arguments.
fn arg_list(p: &mut Parser) {
    let marker = p.start();
    p.bump();
    p.list(RParen, "an expression", expr, starts_expr);
    p.complete(marker, ArgList);
}
