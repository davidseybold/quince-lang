//! Closures (`grammar.closure`), and `return`, `break` and `continue` (`grammar.jump`).

use super::{Completed, Parser, exprs, items, types};
use crate::SyntaxKind::{self, *};

/// `fn` `(` [ cparam { `,` cparam } [ `,` ] ] `)` [ `->` type ] [ `uses` effects ] block
/// (`r[expr.closure.syntax]`). A line that starts with `->`, `uses` or `{` continues its header
/// (`r[lex.continue.fn-header]`). `fn` and a name start a declaration instead, which a block
/// can't hold (decision 9), so a closure is `fn` and `(`.
pub(super) fn closure(p: &mut Parser) -> Completed {
    let marker = p.start();
    p.bump();
    let outer_header = std::mem::replace(&mut p.in_fn_header, true);
    if items::param_list(p, cparam, starts_cparam) {
        items::fn_rest(p, false);
    }
    p.in_fn_header = outer_header;
    p.complete(marker, Closure)
}

/// A name or `_`, and `:` and a type if present (`grammar.cparam`). A closure's parameter has
/// no `var`, `self` or default.
fn cparam(p: &mut Parser) {
    if !starts_cparam(p.kind()) || p.at_break() {
        return p.expected("a parameter name or `_`");
    }
    let marker = p.start();
    if p.kind() == Underscore {
        p.bump();
    } else {
        p.bump_node(Name);
    }
    if p.at(Colon) {
        p.bump();
        types::type_(p);
    }
    p.complete(marker, Param);
}

fn starts_cparam(kind: SyntaxKind) -> bool {
    matches!(kind, Ident | Underscore)
}

/// `return` [ expr ], `break` or `continue` (`grammar.jump`), at the current keyword. They are
/// keyword forms, not operators, so `return` takes everything to its right
/// (`r[expr.op.keyword-forms]`), and it takes an expression only on its own line: `return` on a
/// line of its own is a bare `return`. `break` takes no value (`r[stmt.loop.stmt]`), so an
/// expression after it is an error where the construct around it ends.
pub(super) fn jump(p: &mut Parser) -> Completed {
    let marker = p.start();
    let keyword = p.kind();
    p.bump();
    let kind = match keyword {
        ReturnKw => {
            if !p.at_break() && exprs::starts_expr(p.kind()) {
                exprs::expr(p);
            }
            ReturnExpr
        }
        BreakKw => BreakExpr,
        _ => ContinueExpr,
    };
    p.complete(marker, kind)
}
