//! Declarations: the file, `fn`, `test`, `const` and `impl`, and their parts.

use super::{Break, Holds, Parser, Sync, exprs, types};
use crate::SyntaxKind::{self, *};

/// The file: declarations, one per line (`grammar.file`).
pub(super) fn file(p: &mut Parser) {
    let marker = p.start();
    p.lines(Holds::File, None, |p| decl(p, Holds::File));
    p.complete(marker, File);
}

/// A declaration in the file, or a method in an `impl` body (`grammar.decl`,
/// `grammar.impl_item`), with its `pub`. Anything else is skipped as an error. Layer 7 of the
/// parser adds the other members.
fn decl(p: &mut Parser, holds: Holds) {
    if p.kind() == PubKw && p.nth(1).is_some_and(|t| t.brk != Break::None) {
        // `pub` doesn't continue the line, so a break after it ends the declaration.
        let marker = p.start();
        p.bump();
        p.expected(if holds == Holds::File {
            "a declaration"
        } else {
            "`fn`"
        });
        p.complete(marker, Skipped);
        return;
    }
    let item = if p.kind() == PubKw {
        p.nth_kind(1)
    } else {
        p.kind()
    };
    let (kind, parse): (SyntaxKind, fn(&mut Parser)) = match (holds, item) {
        (_, FnKw) => (FnDecl, fn_decl),
        (Holds::File, ConstKw) => (ConstDecl, const_decl),
        (Holds::File, TestKw) => (TestDecl, test_decl),
        (Holds::File, ImplKw) => (ImplDecl, impl_decl),
        (Holds::File, _) => return skip("a declaration", p),
        _ => return skip("`fn` or `}`", p),
    };
    let marker = p.start();
    if p.kind() == PubKw {
        p.bump();
    }
    parse(p);
    p.complete(marker, kind);
}

fn skip(what: &str, p: &mut Parser) {
    p.expected_member(what);
    p.recover(Sync::Line, true);
}

/// `fn` name `(` params `)` [ `->` type ] block (`grammar.fn_decl`, `grammar.fn_sig`). A line
/// that starts with `->` or `{` continues the header (`r[lex.continue.signature]`).
fn fn_decl(p: &mut Parser) {
    p.bump();
    let outer_header = std::mem::replace(&mut p.in_fn_header, true);
    let mut ok = name(p, "a function name") && param_list(p);
    let mut what = "`->` or `{`";
    if ok && p.at(Arrow) {
        let marker = p.start();
        p.bump();
        ok = types::type_(p);
        p.complete(marker, RetType);
        what = "`{`";
    }
    if ok && p.at(LBrace) {
        p.in_fn_header = false;
        exprs::block(p);
    } else if ok {
        p.expected(what);
    }
    p.in_fn_header = outer_header;
}

/// `(` [ param { `,` param } [ `,` ] ] `)` (`grammar.params`). False when there is no `(`.
fn param_list(p: &mut Parser) -> bool {
    if !p.at(LParen) {
        p.expected("`(`");
        return false;
    }
    let marker = p.start();
    p.bump();
    p.list(RParen, "a parameter", param, starts_param);
    p.complete(marker, ParamList);
    true
}

/// [ `var` ] `self`, or [ `var` ] a name or `_` with `:`, a type and an optional default
/// (`grammar.param`).
fn param(p: &mut Parser) {
    if !starts_param(p.kind()) || p.at_break() {
        return p.expected("a parameter");
    }
    let marker = p.start();
    if p.kind() == VarKw {
        p.bump();
    }
    match p.kind() {
        SelfKw if !p.at_break() => p.bump(),
        Underscore if !p.at_break() => {
            p.bump();
            param_type(p);
        }
        _ => {
            if name(p, "a parameter name") {
                param_type(p);
            }
        }
    }
    p.complete(marker, Param);
}

/// A parameter's `:` type [ `=` expr ].
fn param_type(p: &mut Parser) {
    if !p.at(Colon) {
        return p.expected("`:`");
    }
    p.bump();
    if types::type_(p) && p.at(Eq) {
        p.bump();
        exprs::expr(p);
    }
}

fn starts_param(kind: SyntaxKind) -> bool {
    matches!(kind, Ident | SelfKw | VarKw | Underscore)
}

/// `test` string block (`grammar.test_decl`).
fn test_decl(p: &mut Parser) {
    p.bump();
    if !p.at(StringStart) {
        return p.expected("the test's name, a string");
    }
    exprs::literal(p);
    if !p.at(LBrace) {
        return p.expected("`{`");
    }
    exprs::block(p);
}

/// `const` NAME `:` type `=` expr (`grammar.const_decl`).
fn const_decl(p: &mut Parser) {
    p.bump();
    if !name(p, "a constant name") {
        return;
    }
    if !p.at(Colon) {
        return p.expected("`:`");
    }
    p.bump();
    if !types::type_(p) {
        return;
    }
    if !p.at(Eq) {
        return p.expected("`=`");
    }
    p.bump();
    exprs::expr(p);
}

/// `impl` type `{` methods `}`: an inherent `impl`, with one `[ pub ] fn` per line
/// (`grammar.impl_decl`).
fn impl_decl(p: &mut Parser) {
    p.bump();
    if !types::type_(p) {
        return;
    }
    if !p.at(LBrace) {
        return p.expected("`{`");
    }
    p.bump();
    p.lines(Holds::Body, Some(RBrace), |p| decl(p, Holds::Body));
}

/// A name being declared, as a `Name` node. A keyword can't be one (`r[lex.keyword-not-name]`).
/// False when there is none, after reporting the token as not `what`.
pub(super) fn name(p: &mut Parser, what: &str) -> bool {
    if p.at(Ident) {
        p.bump_node(Name);
        true
    } else {
        p.expected(what);
        false
    }
}
