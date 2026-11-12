//! Types: type paths with type arguments. Layer 2 of the parser adds the rest.

use super::Parser;
use crate::SyntaxKind::*;

/// A type path with optional type arguments (`grammar.type`, `grammar.type_path`). Case-blind:
/// names separated by dots, so `Self` and C names are paths too. False when there is no type,
/// after reporting it.
pub(super) fn type_(p: &mut Parser) -> bool {
    if !matches!(p.kind(), Ident | SelfTypeKw) || p.at_break() {
        p.expected("a type");
        return false;
    }
    let marker = p.start();
    p.bump();
    while p.at(Dot) {
        p.bump();
        if !p.at(Ident) {
            p.expected("a type name");
            p.complete(marker, TypePath);
            return false;
        }
        p.bump();
    }
    let ok = !p.at(Lt) || type_args(p);
    p.complete(marker, TypePath);
    ok
}

/// `<` type { `,` type } `>` (`grammar.type_args`). Line breaks in it are spacing.
fn type_args(p: &mut Parser) -> bool {
    let marker = p.start();
    p.bump();
    let ok = loop {
        if !type_(p) {
            break false;
        }
        match p.kind() {
            Comma => p.bump(),
            Gt => {
                p.bump();
                break true;
            }
            _ => {
                p.expected("`,` or `>`");
                break false;
            }
        }
    };
    p.complete(marker, TypeArgs);
    ok
}
