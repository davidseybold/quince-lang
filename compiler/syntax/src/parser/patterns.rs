//! Patterns (`grammar.pattern`): wildcards, literals and ranges, bindings, paths, tuples,
//! variants with their positional or named fields, and alternatives.

use super::input::Break;
use super::{Completed, Holds, Parser, exprs, types};
use crate::SyntaxKind::{self, *};
use crate::{SyntaxError, codes};

/// A pattern: one or more alternatives joined by `|`. Inside a pattern `|` is always
/// alternation (`r[lex.grammar-rules]`). `None` when there is no pattern, after reporting it.
pub(super) fn pattern(p: &mut Parser) -> Option<Completed> {
    if p.nests_too_deep(None) {
        return None;
    }
    let first = alt(p)?;
    if p.recovering || !p.at_or_join(Pipe) {
        return Some(first);
    }
    let marker = p.precede(first);
    loop {
        p.bump();
        if alt(p).is_none() || p.recovering || !p.at_or_join(Pipe) {
            break;
        }
    }
    Some(p.complete(marker, OrPat))
}

/// The tokens that can start a pattern.
pub(super) fn starts_pattern(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        Underscore
            | Minus
            | Int
            | Float
            | Char
            | StringStart
            | TrueKw
            | FalseKw
            | Ident
            | SelfTypeKw
            | LParen
    )
}

/// One alternative (`grammar.alt`).
fn alt(p: &mut Parser) -> Option<Completed> {
    match p.kind() {
        Underscore => {
            let marker = p.start();
            p.bump();
            Some(p.complete(marker, WildcardPat))
        }
        Minus | Int | Float | Char | StringStart | TrueKw | FalseKw => Some(literal_pat(p)),
        Ident | SelfTypeKw => Some(path_pat(p)),
        LParen => Some(tuple_pat(p)),
        _ => {
            p.expected("a pattern");
            None
        }
    }
}

/// A literal, or a range of two literals (`r[expr.range.pattern]`). Either may be a number
/// with a `-` (`r[pat.negative]`).
fn literal_pat(p: &mut Parser) -> Completed {
    let marker = p.start();
    if !bound(p) {
        return p.complete(marker, LiteralPat);
    }
    let range = matches!(p.kind(), DotDot | DotDotEq) && p.at_or_join(p.kind());
    if !range {
        return p.complete(marker, LiteralPat);
    }
    p.bump();
    if starts_bound(p.kind()) {
        bound(p);
    } else {
        p.expected("a literal");
    }
    p.complete(marker, RangePat)
}

fn starts_bound(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        Minus | Int | Float | Char | StringStart | TrueKw | FalseKw
    )
}

/// A literal, as a `Literal` node. A pattern has no prefix minus, so a `-` and the number after
/// it are one literal whatever lies between them, as in Rust (decided by David, 2026-11-12):
/// `- 5` is `-5`, which the formatter writes. False when there is no literal, after reporting
/// it.
fn bound(p: &mut Parser) -> bool {
    let marker = p.start();
    match p.kind() {
        StringStart => {
            p.abandon(marker);
            let first = p.pos;
            exprs::literal(p);
            no_interpolation(p, first);
            return true;
        }
        Minus => {
            p.bump();
            if !matches!(p.kind(), Int | Float) {
                p.expected("a number after `-`");
                p.complete(marker, Literal);
                return false;
            }
            p.bump();
        }
        _ => p.bump(),
    }
    p.complete(marker, Literal);
    true
}

/// Reports the first interpolation in the string pattern whose tokens start at `first`: a
/// pattern is a constant (`r[pat.no-interp]`). The string is whole, so the pattern goes on and
/// no recovery starts. It still counts as a slip, so the operator checks around the pattern,
/// such as a chained `is`, report nothing more.
fn no_interpolation(p: &mut Parser, first: usize) {
    let tokens = &p.input.tokens[first..p.pos];
    let Some(open) = tokens.iter().position(|t| t.kind == InterpStart) else {
        return;
    };
    // The lexer closes every interpolation it opens, so the fallback never runs.
    let close = tokens[open..]
        .iter()
        .position(|t| t.kind == InterpEnd)
        .map_or(open, |at| open + at);
    let range = tokens[open].range.start..tokens[close].range.end;
    let message = "a string pattern can't interpolate, since a pattern is a constant: \
                   compare with a built string in a guard instead"
        .to_string();
    p.slip();
    p.emit(SyntaxError::new(range, codes::UNEXPECTED_TOKEN, message));
}

/// A name or a path. A name that stands alone, with no `.` or `(` after it, binds a new
/// variable unless it starts with an uppercase letter (`r[pat.const-exhaustive]`). Anything
/// else is a path, and name resolution decides whether it is a constant, a variant or a struct
/// (`r[pat.const]`): a `PathPat`, or a `VariantPat` when its fields follow in parentheses.
fn path_pat(p: &mut Parser) -> Completed {
    let marker = p.start();
    // A `.` on the next line continues the path, as it continues a line (`r[lex.continue.more]`).
    let continues = p.nth(1).is_some_and(|next| match next.kind {
        Dot => next.brk != Break::Hard,
        LParen => next.brk == Break::None,
        _ => false,
    });
    if !continues && p.kind() == Ident && !starts_uppercase(p) {
        p.bump_node(Name);
        return p.complete(marker, BindPat);
    }
    let ok = types::bare_path(p);
    if !(ok && p.at(LParen)) {
        return p.complete(marker, PathPat);
    }
    variant_fields(p);
    p.complete(marker, VariantPat)
}

fn starts_uppercase(p: &Parser) -> bool {
    let token = p.current().expect("a name");
    p.text[token.range.clone()]
        .chars()
        .next()
        .is_some_and(char::is_uppercase)
}

/// A variant's fields in parentheses (`grammar.variant_pat`): patterns by position, or fields
/// by name with an optional `..` last (`grammar.pfields`), and an optional trailing `,`. The
/// first item says which. At least one item.
fn variant_fields(p: &mut Parser) {
    p.bump();
    let named = (p.kind() == Ident && p.nth_kind(1) == Colon) || p.kind() == DotDot;
    if p.kind() == RParen {
        p.expected("a pattern or `..`");
    }
    if named {
        p.list(RParen, "`name:` or `..`", field_pat, starts_field);
    } else {
        p.list(RParen, "a pattern", item, starts_pattern);
    }
}

fn item(p: &mut Parser) {
    pattern(p);
}

fn starts_field(kind: SyntaxKind) -> bool {
    matches!(kind, Ident | DotDot)
}

/// `name:` and a pattern if there is one (`grammar.pfield`), or `..` (`r[pat.rest]`), which
/// comes last.
fn field_pat(p: &mut Parser) {
    let after_rest = p.pos >= 2
        && p.input.tokens[p.pos - 2].kind == DotDot
        && p.input.tokens[p.pos - 1].kind == Comma;
    if after_rest {
        return p.expected("`)` after `..`");
    }
    if p.kind() == DotDot {
        return p.bump();
    }
    if !(p.kind() == Ident && p.nth_kind(1) == Colon) {
        return p.expected("`name:` or `..`");
    }
    let marker = p.start();
    p.bump_node(NameRef);
    p.bump();
    if !matches!(p.kind(), Comma | RParen) {
        pattern(p);
    }
    p.complete(marker, FieldPat);
}

/// `()`, the unit value, or a tuple of two or more patterns (`grammar.tuple_pat`). One pattern
/// and no `,` is an error, since there are no parentheses around a single pattern.
fn tuple_pat(p: &mut Parser) -> Completed {
    let marker = p.start();
    p.bump();
    if p.kind() == RParen {
        p.bump();
        return p.complete(marker, TuplePat);
    }
    p.push_frame(Holds::Items, Some(RParen));
    if let Some(stop) = p.stop() {
        p.close_frame(stop);
        return p.complete(marker, TuplePat);
    }
    pattern(p);
    match p.kind() {
        Comma => {
            p.bump();
            if p.kind() == RParen {
                p.expected("a second pattern");
            }
            p.list_items(RParen, "a pattern", item, starts_pattern, false);
        }
        RParen => {
            p.expected("`,` and a second pattern");
            p.list_items(RParen, "a pattern", item, starts_pattern, true);
        }
        _ => p.list_items(RParen, "a pattern", item, starts_pattern, true),
    }
    p.complete(marker, TuplePat)
}
