//! Declarations: the file, imports, `fn`, `test`, `const`, `impl`, structs, newtypes, enums
//! and type aliases, with their doc comments and attributes, and their parts.

use super::{Holds, Parser, Sync, exprs, types};
use crate::SyntaxKind::{self, *};
use crate::{SyntaxError, codes};
use quince_diagnostics::Applicability;

/// The file: declarations, one per line (`grammar.file`).
pub(super) fn file(p: &mut Parser) {
    let marker = p.start();
    p.lines(Holds::File, None, |p| decl(p, Holds::File));
    p.complete(marker, File);
}

/// A declaration in the file with its doc comments and attributes, or a method in an `impl`
/// body (`grammar.decl`, `grammar.impl_item`), with its `pub`. Anything else is skipped as an
/// error. Layer 7 of the parser adds the other members, and their doc comments and attributes.
/// The item is chosen at its keyword, once what comes before it is parsed.
fn decl(p: &mut Parser, holds: Holds) {
    let file = holds == Holds::File;
    if file && p.kind() == ImportKw {
        return import(p);
    }
    let marker = p.start();
    if file {
        p.imports_closed = true;
        p.doc_comments();
        attributes(p);
    }
    if p.kind() == PubKw {
        p.bump();
        if p.at_break() {
            // `pub` doesn't continue the line, so a break after it ends the declaration.
            p.expected(if file { "a declaration" } else { "`fn`" });
            p.complete(marker, Skipped);
            return;
        }
    }
    let parse: fn(&mut Parser) -> SyntaxKind = match (file, p.kind()) {
        (_, FnKw) => |p| {
            fn_decl(p);
            FnDecl
        },
        (true, ConstKw) => |p| {
            const_decl(p);
            ConstDecl
        },
        (true, TestKw) => |p| {
            test_decl(p);
            TestDecl
        },
        (true, ImplKw) => |p| {
            impl_decl(p);
            ImplDecl
        },
        (true, StructKw) => struct_decl,
        (true, EnumKw) => |p| {
            enum_decl(p);
            EnumDecl
        },
        (true, TypeKw) => |p| {
            alias_decl(p);
            AliasDecl
        },
        _ => {
            let what = if file { "a declaration" } else { "`fn` or `}`" };
            if p.steps.len() == marker.0 + 1 {
                // Nothing before the item, so the skip's own node is enough.
                p.abandon(marker);
                return skip(what, p);
            }
            skip(what, p);
            p.complete(marker, Skipped);
            return;
        }
    };
    let kind = parse(p);
    p.complete(marker, kind);
}

/// `@` name [ args ] (`grammar.attribute`), each on a line of its own (`r[item.attr.placement]`).
/// One with something after it on its line is an E0016 whose fix inserts a line break. Whether
/// its name is known is for a later stage (`r[item.attr.unknown]`).
fn attributes(p: &mut Parser) {
    let mut any = false;
    while p.kind() == At {
        let at = p.pos;
        let marker = p.start();
        p.bump();
        if p.at(Ident) {
            p.bump_node(NameRef);
            if p.at(LParen) {
                exprs::attribute_args(p);
            }
        } else if !p.at_break() && p.kind().is_keyword() {
            // A keyword can't name an attribute (`r[lex.keyword-not-name]`). Taken without
            // ending the error's recovery, so the declaration after it is still parsed.
            p.expected("an attribute name");
            p.take();
        } else {
            p.expected("an attribute name");
            p.recover(Sync::Line, false);
        }
        p.complete(marker, Attribute);
        any = true;
        if p.current().is_some() && !p.starts_line() {
            not_on_own_line(p, at);
        }
    }
    if any {
        p.after_attributes = true;
    }
}

/// The E0016 for an attribute that starts at token `at` and has the current token after it
/// on its line. When that token can start what an attribute applies to, the fix moves it to
/// the next line, at the attribute's indentation, unless a comment lies between them.
fn not_on_own_line(p: &mut Parser, at: usize) {
    let error = p.unexpected("a line break", false);
    let start = p.input.tokens[at].range.start;
    let line_start = p.text[..start].rfind(['\n', '\r']).map_or(0, |i| i + 1);
    let newline = if p.text[..line_start].ends_with("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let indent = &p.text[line_start..start];
    let indent = if indent.trim().is_empty() { indent } else { "" };
    let end = p.input.tokens[p.pos - 1].range.end;
    let next = p.input.tokens[p.pos].range.start;
    let applies_to = p.kind() == Ident || p.kind().is_keyword();
    let error = if applies_to && p.text[end..next].trim().is_empty() {
        let edit = (end..next, format!("{newline}{indent}"));
        error.with_fix(
            "put the attribute on its own line",
            Applicability::Safe,
            vec![edit],
        )
    } else {
        error
    };
    p.report(error);
}

fn skip(what: &str, p: &mut Parser) {
    p.expected_member(what);
    p.recover(Sync::Line, true);
}

/// `fn` name [ generics ] `(` params `)` [ `->` type ] [ `uses` effects ] block
/// (`grammar.fn_decl`, `grammar.fn_sig`). A line that starts with `->`, `uses` or `{`
/// continues the header (`r[lex.continue.signature]`).
fn fn_decl(p: &mut Parser) {
    p.bump();
    let outer_header = std::mem::replace(&mut p.in_fn_header, true);
    let mut ok = name(p, "a function name") && (!p.at(Lt) || types::generics(p)) && param_list(p);
    let mut what = "`->`, `uses` or `{`";
    if ok && p.at(Arrow) {
        let marker = p.start();
        p.bump();
        ok = types::return_type(p);
        p.complete(marker, RetType);
        what = "`uses` or `{`";
    }
    if ok && p.at(UsesKw) {
        ok = types::effects(p);
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
    if types::type_(p) && !p.at(LBrace) {
        p.expected("`{`");
    }
    skip_to_brace(p);
    if !p.at(LBrace) {
        return;
    }
    p.bump();
    p.lines(Holds::Body, Some(RBrace), |p| decl(p, Holds::Body));
}

/// `import` path, then `.` and names in braces, or [ `as` name ] (`grammar.import`). There is
/// no wildcard import (`r[intro.excl.wildcard-imports]`). Imports come before the declarations
/// (`grammar.file`), so a later one is an E0016, and is still parsed.
fn import(p: &mut Parser) {
    if p.imports_closed {
        let error = p.unexpected("a declaration", false);
        p.report(error);
    }
    let marker = p.start();
    p.bump();
    if p.at(Ident) {
        p.bump();
        let mut names = false;
        while p.at(Dot) {
            p.bump();
            match p.kind() {
                Ident if !p.at_break() => p.bump(),
                LBrace if !p.at_break() => {
                    import_names(p);
                    names = true;
                    break;
                }
                _ => {
                    p.expected("a name or `{`");
                    break;
                }
            }
        }
        // Only a package import takes `as`.
        if !names && p.at(AsKw) {
            p.bump();
            name(p, "a package name");
        }
    } else {
        p.expected("a package path");
    }
    p.complete(marker, Import);
}

/// `{` name_ref { `,` name_ref } [ `,` ] `}` (`grammar.names`, `grammar.name_ref`).
fn import_names(p: &mut Parser) {
    let marker = p.start();
    p.bump();
    if p.kind() == RBrace {
        p.expected("a name");
    }
    p.list(RBrace, "a name", name_ref, starts_name_ref);
    p.complete(marker, ImportNames);
}

fn name_ref(p: &mut Parser) {
    if starts_name_ref(p.kind()) && !p.at_break() {
        p.bump_node(NameRef);
    } else {
        p.expected("a name");
    }
}

fn starts_name_ref(kind: SyntaxKind) -> bool {
    matches!(kind, Ident | SelfKw)
}

/// `struct` Name [ generics ], then fields in braces and an optional `requires`, or nothing
/// (`grammar.struct_decl`), or one type in parentheses (`grammar.newtype_decl`).
fn struct_decl(p: &mut Parser) -> SyntaxKind {
    p.bump();
    if !(name(p, "a struct name") && (!p.at(Lt) || types::generics(p))) {
        return StructDecl;
    }
    if p.at(LParen) {
        newtype_body(p);
        return NewtypeDecl;
    }
    if p.at(LBrace) {
        if p.nth_kind(1) == RBrace {
            empty_braces(p);
        } else {
            let marker = p.start();
            p.bump();
            fields(p, RBrace, "a field", field, starts_field);
            p.complete(marker, FieldList);
        }
        if p.at(RequiresKw) {
            p.bump();
            exprs::expr(p);
        }
    }
    StructDecl
}

/// `{}`: a struct with no fields is declared without braces (`r[item.struct.no-fields]`), so
/// it is an E0024. Its fix removes them, unless a `requires` follows, which only a struct with
/// fields may have, or a comment lies before or between them, which removing would delete.
fn empty_braces(p: &mut Parser) {
    let before = p.input.tokens[p.pos - 1].range.end;
    let (open, close) = (&p.input.tokens[p.pos], &p.input.tokens[p.pos + 1]);
    let range = open.range.start..close.range.end;
    let message = "a struct with no fields is declared without braces".to_string();
    let mut error = SyntaxError::new(range.clone(), codes::EMPTY_STRUCT_BRACES, message);
    let comments = [before..open.range.start, open.range.end..close.range.start]
        .into_iter()
        .any(|gap| !p.text[gap].trim().is_empty());
    if p.nth_kind(2) != RequiresKw && !comments {
        let edit = (before..range.end, String::new());
        error = error.with_fix("remove the braces", Applicability::Safe, vec![edit]);
    }
    p.report(error);
    let marker = p.start();
    p.bump();
    p.bump();
    p.complete(marker, FieldList);
}

/// `(` [ `pub` ] type `)` (`grammar.newtype_decl`).
fn newtype_body(p: &mut Parser) {
    p.bump();
    p.push_frame(Holds::Items, Some(RParen));
    if p.stop().is_none() && !p.unmatched_closer() {
        if p.at(PubKw) {
            p.bump();
        }
        types::type_(p);
    }
    p.end_list(RParen);
}

/// The items of struct braces, enum braces or a variant's fields, after the opening bracket.
fn fields(
    p: &mut Parser,
    closer: SyntaxKind,
    what_item: &str,
    item: fn(&mut Parser),
    starts_item: fn(SyntaxKind) -> bool,
) {
    p.push_frame(Holds::Fields, Some(closer));
    p.list_items(closer, what_item, item, starts_item, false);
}

fn starts_field(kind: SyntaxKind) -> bool {
    matches!(kind, Ident | PubKw | At)
}

fn field(p: &mut Parser) {
    field_with(p, true);
}

/// A field: its doc comments and attributes, [ `pub` ], a name, `:` and a type
/// (`grammar.field`). A variant's field has no `pub` (`grammar.vfield`,
/// `r[item.enum.field-visibility]`), so one is an E0016, and the field is parsed on.
fn field_with(p: &mut Parser, pub_allowed: bool) {
    if !starts_field(p.kind()) || p.at_break() {
        return p.expected("a field");
    }
    let marker = p.start();
    p.doc_comments();
    attributes(p);
    if p.kind() == PubKw {
        if !pub_allowed {
            p.expected("a field name");
        }
        p.bump();
    }
    if name(p, "a field name") {
        if p.at(Colon) {
            p.bump();
            types::type_(p);
        } else {
            p.expected("`:`");
        }
    }
    p.complete(marker, FieldDecl);
}

/// `enum` Name [ generics ] `{` variants `}` (`grammar.enum_decl`).
fn enum_decl(p: &mut Parser) {
    p.bump();
    if !(name(p, "an enum name") && (!p.at(Lt) || types::generics(p))) {
        return;
    }
    if !p.at(LBrace) {
        return p.expected("`{`");
    }
    let marker = p.start();
    p.bump();
    fields(p, RBrace, "a variant", variant, starts_variant);
    p.complete(marker, VariantList);
}

fn starts_variant(kind: SyntaxKind) -> bool {
    matches!(kind, Ident | At)
}

/// A variant: its doc comments and attributes, a name, and its fields in parentheses if it has
/// any (`grammar.evariant`, `grammar.variant`).
fn variant(p: &mut Parser) {
    if !starts_variant(p.kind()) || p.at_break() {
        return p.expected("a variant");
    }
    let marker = p.start();
    p.doc_comments();
    attributes(p);
    if name(p, "a variant name") && p.at(LParen) {
        let list = p.start();
        p.bump();
        if p.kind() == RParen {
            // A variant with no fields has no parentheses.
            p.expected("a field");
        }
        fields(p, RParen, "a field", |p| field_with(p, false), starts_field);
        p.complete(list, VariantFields);
    }
    p.complete(marker, Variant);
}

/// `type` Name [ generics ] `=` type (`grammar.alias_decl`).
fn alias_decl(p: &mut Parser) {
    p.bump();
    if !(name(p, "a type name") && (!p.at(Lt) || types::generics(p))) {
        return;
    }
    if !p.at(Eq) {
        return p.expected("`=`");
    }
    p.bump();
    types::type_(p);
}

/// After a slip in a header, skips the rest of its line up to its `{`, so the body is still
/// parsed as the body. A bracket that some frame closes stops the skip.
fn skip_to_brace(p: &mut Parser) {
    let marker = p.start();
    let mut skipped = false;
    while !p.at_break() && p.kind() != LBrace && p.frame_closed_by(p.kind()).is_none() {
        p.take();
        skipped = true;
    }
    if skipped {
        p.complete(marker, Skipped);
    } else {
        p.abandon(marker);
    }
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
