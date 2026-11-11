//! Blocks, statements and expressions: `let`, literals, names, calls, method calls and fields.
//! Later layers of the parser add the operators and the rest.

use super::{Completed, Holds, Parser, Sync, items, types};
use crate::SyntaxKind::{self, *};

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

/// An expression: so far, a postfix expression (`grammar.expr`).
pub(super) fn expr(p: &mut Parser) {
    postfix(p);
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
                Ident if !p.at_break() => p.bump_node(NameRef),
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

/// A literal, a name, `self` or `Self` (`grammar.primary`). A statement starts
/// after a break, so this doesn't check for one: each caller has taken a token that continues
/// the line, or is at the start of a statement.
fn primary(p: &mut Parser) -> Option<Completed> {
    let marker = p.start();
    let kind = match p.kind() {
        Int | Float | Char => {
            p.bump();
            Literal
        }
        StringStart => {
            p.abandon(marker);
            return Some(literal(p));
        }
        Ident | SelfKw | SelfTypeKw => {
            p.bump();
            NameRef
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
        Int | Float | Char | StringStart | Ident | SelfKw | SelfTypeKw
    )
}

/// A string literal, at its `StringStart`: its text, escapes and interpolations, and its
/// `StringEnd` when it has one. The lexer reported a missing one.
fn literal(p: &mut Parser) -> Completed {
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

/// `(` [ arg { `,` arg } [ `,` ] ] `)` (`grammar.args`). Layer 3 of the parser adds named
/// arguments.
fn arg_list(p: &mut Parser) {
    let marker = p.start();
    p.bump();
    p.list(RParen, "an expression", expr, starts_expr);
    p.complete(marker, ArgList);
}
