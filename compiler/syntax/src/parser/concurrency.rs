//! `concurrent` (`grammar.concurrent`) and its spawns (`grammar.spawn`), `spawn` as a jump
//! (`grammar.jump`), `background` (`grammar.background`), `select` (`grammar.select`) and
//! `with` (`grammar.with_expr`).

use super::{Completed, Holds, Parser, Sync, arms, exprs, items, patterns, types};
use crate::SyntaxKind::*;

/// `concurrent` [ `(` arg { `,` arg } [ `,` ] `)` ], then `spawn`s in braces or `for` pattern
/// `in` expr block. The braces go on the line of what comes before them, as a body's do, so on
/// the next line they are an E0018. A fan-out's `for` is a `ForStmt` inside the node.
pub(super) fn concurrent(p: &mut Parser) -> Completed {
    let marker = p.start();
    p.bump();
    let mut what = "`(`, `{` or `for`";
    if p.at(LParen) {
        exprs::nonempty_args(p);
        what = "`{` or `for`";
    }
    if p.at(ForKw) {
        exprs::for_stmt(p);
    } else if p.at_or_join(LBrace) {
        spawn_list(p);
    } else {
        p.expected(what);
    }
    p.complete(marker, ConcurrentExpr)
}

/// `{` spawn { `,` spawn } [ `,` ] `}`: a comma-separated list (`r[lex.continue.lists]`) of one
/// or more spawns.
fn spawn_list(p: &mut Parser) {
    let marker = p.start();
    p.bump();
    if p.kind() == RBrace {
        p.expected("`spawn`");
    }
    p.list(RBrace, "`spawn`", spawn_item, |kind| kind == SpawnKw);
    p.complete(marker, SpawnList);
}

/// `spawn` expr, an item of a `concurrent`'s braces (`grammar.spawn`). It has no scope: `in`
/// after it is an E0016.
fn spawn_item(p: &mut Parser) {
    if p.kind() != SpawnKw || p.at_break() {
        return p.expected("`spawn`");
    }
    let marker = p.start();
    p.bump();
    operand(p);
    p.complete(marker, SpawnExpr);
}

/// `spawn` [ `in` postfix ] expr where an expression goes (`grammar.jump`). Like `return`, it
/// is a keyword form, so it takes everything to its right (`r[expr.op.keyword-forms]`). The
/// scope after `in` is a postfix expression, so `spawn in bg (x).run()` calls `bg`.
pub(super) fn spawn(p: &mut Parser) -> Completed {
    let marker = p.start();
    p.bump();
    if p.at(InKw) {
        p.bump();
        if p.at_break() {
            p.expected("a task scope");
            return p.complete(marker, SpawnExpr);
        }
        if exprs::postfix(p).is_none() || p.recovering {
            return p.complete(marker, SpawnExpr);
        }
    }
    operand(p);
    p.complete(marker, SpawnExpr)
}

/// A spawn's expression, which starts on its line.
fn operand(p: &mut Parser) {
    if p.at_break() || !exprs::starts_expr(p.kind()) {
        p.expected("an expression");
    } else {
        exprs::expr(p);
    }
}

/// `background` `(` name `:` `TaskScope` [ `,` arg ] `)` block (`grammar.background`). The
/// parentheses are a list of one or two items, with no trailing `,`.
pub(super) fn background(p: &mut Parser) -> Completed {
    let marker = p.start();
    p.bump();
    if !p.at(LParen) {
        p.expected("`(`");
        return p.complete(marker, BackgroundExpr);
    }
    p.bump();
    p.push_frame(Holds::Items, Some(RParen));
    if p.stop().is_none() && !p.unmatched_closer() {
        scope_param(p);
        if !p.recovering {
            match p.kind() {
                Comma => {
                    p.bump();
                    exprs::arg(p);
                }
                RParen => {}
                _ if p.stop().is_some() => {}
                _ => p.expected("`,` or `)`"),
            }
        }
    }
    p.end_list(RParen);
    if p.at_or_join(LBrace) {
        exprs::block(p);
    } else {
        p.expected("`{`");
    }
    p.complete(marker, BackgroundExpr)
}

/// name `:` `TaskScope`, as a `Param`: the scope's name and type, which is always `TaskScope`.
fn scope_param(p: &mut Parser) {
    let marker = p.start();
    if items::name(p, "a name") {
        if !p.at(Colon) {
            p.expected("`:`");
        } else {
            p.bump();
            let task_scope = p
                .current()
                .is_some_and(|t| &p.text[t.range.clone()] == "TaskScope");
            if task_scope && p.at(Ident) {
                p.bump_node(TypePath);
            } else {
                p.expected("`TaskScope`");
            }
        }
    }
    p.complete(marker, Param);
}

/// `select` `{` { pattern `=` expr `=>` ( block | expr ) `,` } `}` (`grammar.select`). Its arms
/// are a `match`'s list, so an arm's `=>` closes only the brackets opened in its own arm, even
/// in a `select` inside a `match` arm. The braces go on the line of `select`.
pub(super) fn select(p: &mut Parser) -> Completed {
    let marker = p.start();
    p.bump();
    if p.at_or_join(LBrace) {
        arms::arms(p, "a select arm", select_arm, SelectArmList);
    } else {
        p.expected("`{`");
    }
    p.complete(marker, SelectExpr)
}

fn select_arm(p: &mut Parser) {
    let marker = p.start();
    select_arm_parts(p);
    p.complete(marker, SelectArm);
}

/// pattern `=` expr `=>` ( block | expr ) (`r[conc.select.arm]`).
fn select_arm_parts(p: &mut Parser) {
    if patterns::pattern(p).is_none() || p.recovering {
        return;
    }
    if !p.at_or_join(Eq) {
        return p.expected("`=`");
    }
    p.bump();
    exprs::expr(p);
    arms::arrow_and_body(p, "`=>`");
}

/// `with` handler { `,` handler } block (`grammar.with_expr`). The handlers aren't in brackets,
/// so a trailing `,` is an error, and a line break after a `,` is spacing. After a slip in a
/// handler, the rest of the line up to its `{` is skipped, so the block is still the block, as
/// after a slip in a condition.
pub(super) fn with_expr(p: &mut Parser) -> Completed {
    let marker = p.start();
    p.bump();
    loop {
        handler(p);
        if p.recovering || !p.at(Comma) {
            break;
        }
        p.bump();
    }
    if p.recovering {
        p.recover(Sync::Block, false);
    }
    if p.at_or_join(LBrace) {
        exprs::block(p);
    } else {
        p.expected("`,` or `{`");
    }
    p.complete(marker, WithExpr)
}

/// type_path `=` expr (`grammar.handler`): an effect's path, with no type arguments, and the
/// value that handles it.
fn handler(p: &mut Parser) {
    if p.at_break() || !matches!(p.kind(), Ident | SelfTypeKw) {
        return p.expected("an effect");
    }
    let marker = p.start();
    if types::bare_path(p) {
        if p.at_or_join(Eq) {
            p.bump();
            exprs::expr(p);
        } else {
            p.expected("`=`");
        }
    }
    p.complete(marker, Handler);
}
