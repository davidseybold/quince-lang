//! Types (`grammar.type`): type paths with type arguments, tuples, function types bare or in
//! parentheses, generic parameters with their bounds, and `uses` lists.

use super::{Holds, Marker, Parser, Sync, items};
use crate::SyntaxKind::{self, *};
use crate::{SyntaxError, codes};
use quince_diagnostics::Applicability;

/// Whether a bare function type takes a `uses` after it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Uses {
    Take,
    /// A return type's: the function it is returned from takes it (`r[effect.uses-return]`).
    Leave,
}

/// A type. False when there is none, or it ends in an error, after reporting it.
pub(super) fn type_(p: &mut Parser) -> bool {
    type_with(p, Uses::Take)
}

/// A return type, after `->`. A bare function type here leaves the `uses` after it to the
/// function it is returned from (`r[effect.uses-return]`).
pub(super) fn return_type(p: &mut Parser) -> bool {
    type_with(p, Uses::Leave)
}

fn type_with(p: &mut Parser, uses: Uses) -> bool {
    if p.nests_too_deep(None) {
        return false;
    }
    if p.at_break() {
        p.expected("a type");
        return false;
    }
    match p.kind() {
        Ident | SelfTypeKw => type_path(p),
        FnKw => fn_type(p, uses),
        LParen => paren_type(p),
        _ => {
            p.expected("a type");
            false
        }
    }
}

fn starts_type(kind: SyntaxKind) -> bool {
    matches!(kind, Ident | SelfTypeKw | FnKw | LParen)
}

fn starts_path(p: &Parser) -> bool {
    matches!(p.kind(), Ident | SelfTypeKw) && !p.at_break()
}

/// A type path with optional type arguments (`grammar.type_path`, `grammar.type_args`).
fn type_path(p: &mut Parser) -> bool {
    let (marker, ok) = path_names(p, "a type name");
    let ok = ok && (!p.at(Lt) || type_args(p, false));
    p.complete(marker, TypePath);
    ok
}

/// A path with no type arguments, in a pattern or a `with` handler: its names and the dots
/// between them, as a `TypePath`. The caller checked the first name.
pub(super) fn bare_path(p: &mut Parser) -> bool {
    let (marker, ok) = path_names(p, "a name");
    p.complete(marker, TypePath);
    ok
}

/// A path's names and the dots between them, in a `TypePath` node the caller completes.
/// Case-blind, so `Self` and C names are paths too. The caller checked the first name.
fn path_names(p: &mut Parser, what_name: &str) -> (Marker, bool) {
    let marker = p.start();
    p.bump();
    while p.at(Dot) {
        p.bump();
        if !p.at(Ident) {
            p.expected(what_name);
            return (marker, false);
        }
        p.bump();
    }
    (marker, true)
}

/// `<` type_arg { `,` type_arg } `>` (`grammar.type_args`, `grammar.type_arg`), or a bound's
/// arguments, which may also name an associated type (`grammar.bound_arg`). A list closed by
/// `>`, so line breaks in it are spacing. While the parser is speculating, it records where
/// they end, or that they don't parse.
fn type_args(p: &mut Parser, in_bound: bool) -> bool {
    let (marker, at) = (p.start(), p.pos);
    angle_list(p, "a type", if in_bound { bound_arg } else { type_arg });
    p.complete(marker, TypeArgs);
    if p.speculating && !p.speculation_too_deep {
        let end = (!p.speculation_failed).then_some(p.pos);
        p.type_args_end.insert(at, end);
    }
    true
}

/// Type arguments after a name in an expression, at their `<`.
pub(super) fn expr_type_args(p: &mut Parser) {
    type_args(p, false);
}

/// After a `<`: one item or more until `>`, each parsed by `item`, with no trailing `,`.
fn angle_list(p: &mut Parser, what_item: &str, item: fn(&mut Parser)) {
    p.bump();
    if p.kind() == Gt {
        p.expected(what_item);
    }
    p.list(Gt, what_item, item, starts_angle_item);
}

fn starts_angle_item(kind: SyntaxKind) -> bool {
    starts_type(kind) || kind == UsesKw
}

/// A type, or a `uses` list, which comes last and takes the rest of the list.
fn type_arg(p: &mut Parser) {
    if p.at(UsesKw) {
        effects_arg(p);
    } else {
        type_(p);
    }
}

/// A [`type_arg`], or a name, `=` and a type.
fn bound_arg(p: &mut Parser) {
    if p.at(Ident) && p.nth_kind(1) == Eq {
        named_type_arg(p);
    } else {
        type_arg(p);
    }
}

/// A `uses` list in type arguments, which only `>` may follow, after a trailing `,` or not.
fn effects_arg(p: &mut Parser) {
    if effects(p) && p.kind() == Comma && p.nth_kind(1) != Gt {
        p.expected("`>`");
    }
}

/// A name, `=` and a type, as in `Iterator<Item = Int>` (`grammar.bound_arg`).
fn named_type_arg(p: &mut Parser) -> bool {
    let marker = p.start();
    p.bump_node(NameRef);
    p.bump();
    let ok = type_(p);
    p.complete(marker, NamedTypeArg);
    ok
}

/// `fn` `(` [ type { `,` type } ] `)` [ `->` type ] [ `uses` effects ] (`grammar.fn_type`).
/// With [`Uses::Leave`], a `uses` after it is left to the function around it.
fn fn_type(p: &mut Parser, uses: Uses) -> bool {
    let marker = p.start();
    p.bump();
    // A function type's header continues on a line that starts with `->` or `uses`
    // (`r[lex.continue.fn-header]`).
    let outer_header = std::mem::replace(&mut p.in_fn_header, true);
    let mut ok = p.at(LParen);
    if ok {
        p.bump();
        p.list(RParen, "a type", type_item, starts_type);
    } else {
        p.expected("`(`");
    }
    if ok && p.at(Arrow) {
        let ret = p.start();
        p.bump();
        ok = return_type(p);
        p.complete(ret, RetType);
    }
    if ok && uses == Uses::Take && p.at(UsesKw) {
        ok = effects(p);
    }
    p.in_fn_header = outer_header;
    p.complete(marker, FnType);
    ok
}

fn type_item(p: &mut Parser) {
    type_(p);
}

/// A tuple of two or more types, or one function type in parentheses, which ends it
/// (`r[type.fn-parens]`). Any other type in parentheses is an E0016 whose fix removes them.
fn paren_type(p: &mut Parser) -> bool {
    // With an `Error` token between them, its E0001 is the one error, and the usual path
    // reports nothing more.
    if p.nth(1)
        .is_some_and(|close| close.kind == RParen && !close.after_error)
    {
        unit_type(p);
        return true;
    }
    let marker = p.start();
    let open = p.pos;
    p.bump();
    p.push_frame(Holds::Items, Some(RParen));
    if let Some(stop) = p.stop() {
        p.close_frame(stop);
        p.complete(marker, TupleType);
        return false;
    }
    let fn_type = p.kind() == FnKw;
    let ok = type_(p);
    if !ok && !matches!(p.kind(), Comma | RParen) {
        // The slip ends at a `,` or the `)`, which say whether it was a tuple.
        p.recover(Sync::List, false);
    }
    if p.kind() == RParen {
        p.pop_frame();
        if ok && !fn_type {
            let mut error = p.unexpected("`,` and a second type", false);
            // Before `->` the parentheses look like a function type missing its `fn`, which
            // removing them wouldn't fix.
            if p.nth_kind(1) != Arrow {
                let close = p.pos;
                let edits =
                    [open, close].map(|at| (p.input.tokens[at].range.clone(), String::new()));
                error = error.with_fix("remove the parentheses", Applicability::Safe, edits.into());
            }
            p.report(error);
            // Taken without ending the error's recovery, so what follows is part of the slip.
            p.take();
        } else {
            p.bump();
        }
        p.complete(marker, ParenType);
        return true;
    }
    if ok && p.kind() == Comma && p.nth_kind(1) == RParen {
        // `(T,)`: a tuple has two types or more.
        p.bump();
        p.expected("a second type");
        p.pop_frame();
        p.bump();
        p.complete(marker, TupleType);
        return true;
    }
    p.list_items(RParen, "a type", type_item, starts_type, true);
    p.complete(marker, TupleType);
    true
}

/// `()` where a type goes: the unit type is written `Unit` (`r[type.unit]`), so it is an
/// E0027 over the parentheses, in a `ParenType` that holds no type. Its fix writes `Unit`,
/// and a space after it when a word follows the `)` directly. There is none when anything
/// but spaces lies between them, such as a comment, which replacing them would delete.
/// Doc comments before the `(` get their E0023 first. What follows is parsed as after any
/// type.
fn unit_type(p: &mut Parser) {
    let (open, close) = (&p.input.tokens[p.pos], &p.input.tokens[p.pos + 1]);
    let range = open.range.start..close.range.end;
    let message = "`()` is the unit value; the unit type is written `Unit`".to_string();
    let mut error = SyntaxError::new(range.clone(), codes::UNIT_TYPE, message);
    if p.text[open.range.end..close.range.start].trim().is_empty() {
        let glued = p.input.tokens.get(p.pos + 2).is_some_and(|next| {
            next.range.start == close.range.end
                && p.text[next.range.clone()]
                    .starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_')
        });
        let unit = if glued { "Unit " } else { "Unit" };
        let edit = (range, unit.to_string());
        error = error.with_fix("write `Unit`", Applicability::NeedsReview, vec![edit]);
    }
    p.stray_docs();
    p.report(error);
    let marker = p.start();
    p.bump();
    p.bump();
    p.complete(marker, ParenType);
}

/// `<` generic { `,` generic } `>` after a declaration's name (`grammar.generics`).
pub(super) fn generics(p: &mut Parser) -> bool {
    let marker = p.start();
    angle_list(p, "a type parameter", generic);
    p.complete(marker, Generics);
    true
}

/// A type parameter with optional bounds, or `uses` and an effect variable
/// (`grammar.generic`).
fn generic(p: &mut Parser) {
    if !(p.at(Ident) || p.at(UsesKw)) {
        return p.expected("a type parameter");
    }
    let marker = p.start();
    if p.at(UsesKw) {
        p.bump();
        items::name(p, "an effect variable");
    } else if items::name(p, "a type parameter") && p.at(Colon) {
        bounds(p);
    }
    p.complete(marker, GenericParam);
}

/// `:` and bounds joined by `+`, each a type path whose arguments may name an associated type
/// (`grammar.bounds`, `grammar.bound`).
fn bounds(p: &mut Parser) -> bool {
    p.bump();
    loop {
        if !starts_path(p) {
            p.expected("a bound");
            return false;
        }
        let (marker, ok) = path_names(p, "a type name");
        let ok = ok && (!p.at(Lt) || type_args(p, true));
        p.complete(marker, TypePath);
        if !ok {
            return false;
        }
        if !p.at(Plus) {
            return true;
        }
        p.bump();
    }
}

/// `uses` and `*`, or effect paths separated by commas (`grammar.effects`,
/// `grammar.effect_ref`). A comma continues the list only before a type path that no `:` or
/// `=` follows (`r[effect.uses-list]`). Otherwise it is left to the list around it.
pub(super) fn effects(p: &mut Parser) -> bool {
    let marker = p.start();
    p.bump();
    let ok = if p.at(Star) {
        p.bump();
        true
    } else {
        loop {
            if !starts_path(p) {
                p.expected("an effect");
                break false;
            }
            let (path, ok) = path_names(p, "a type name");
            p.complete(path, TypePath);
            if !ok {
                break false;
            }
            if !(p.at(Comma) && effect_follows(p)) {
                break true;
            }
            p.bump();
        }
    };
    p.complete(marker, Effects);
    ok
}

/// At a `,`: a type path follows it, and no `:` or `=` follows that. The path can be any
/// length, so this reads the tokens directly: it takes nothing, and isn't a loop that could
/// stick.
fn effect_follows(p: &Parser) -> bool {
    let tokens = &p.input.tokens[p.pos + 1..];
    let kind = |n: usize| tokens.get(n).map(|t| t.kind);
    if !matches!(kind(0), Some(Ident | SelfTypeKw)) {
        return false;
    }
    let mut n = 1;
    while kind(n) == Some(Dot) && kind(n + 1) == Some(Ident) {
        n += 2;
    }
    !matches!(kind(n), Some(Colon | Eq))
}
