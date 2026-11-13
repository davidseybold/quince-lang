//! Declarations: the file, imports, `fn`, `test`, `const`, `impl`, `interface`, structs,
//! newtypes, enums and type aliases, with their doc comments and attributes, and their parts.

use super::input::Break;
use super::{Holds, Parser, Sync, exprs, types};
use crate::SyntaxKind::{self, *};
use crate::{SyntaxError, codes};
use quince_diagnostics::Applicability;

/// The file: declarations, one per line (`grammar.file`).
pub(super) fn file(p: &mut Parser) {
    let marker = p.start();
    p.lines(Holds::File, None, |p| decl(p, Owner::File));
    // Doc comments at the end of the file document nothing.
    p.stray_docs();
    p.complete(marker, File);
}

/// What holds a declaration: the file, or the body of an `impl` or an `interface`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Owner {
    File,
    Impl,
    Interface,
}

/// A declaration in the file, or a member of an `impl` or an `interface`, with its doc
/// comments, its attributes and its `pub` (`grammar.decl`, `grammar.impl_item`,
/// `grammar.interface_decl`). Anything else is skipped as an error, so a `const` in an
/// interface is one (`r[item.const.assoc.no-interface]`). The item is chosen at its keyword,
/// once what comes before it is parsed.
fn decl(p: &mut Parser, owner: Owner) {
    let file = owner == Owner::File;
    if file && p.kind() == ImportKw {
        return import(p);
    }
    // Taking attributes or `pub` ends recovery, but a line that turns out to be no item is
    // still part of the slip before it.
    let quiet = p.recovering;
    let marker = p.start();
    p.doc_comments();
    attributes(p);
    if p.kind() == PubKw {
        // An interface's members take no `pub`, and neither does an impl's associated type:
        // an E0016, and the member is parsed on. Taking the `pub` ends recovery, as a member
        // start does, so the error is the member's own, unless the line is no member.
        let interface = owner == Owner::Interface;
        let wrong = interface || (owner == Owner::Impl && p.nth_kind(1) == TypeKw);
        let error = wrong.then(|| {
            p.unexpected(
                if interface {
                    "`fn` or `type`"
                } else {
                    "`type`"
                },
                false,
            )
        });
        p.bump();
        if let Some(error) = error
            && (!quiet || matches!(p.kind(), FnKw | TypeKw))
        {
            p.report(error);
        }
        if p.at_break() {
            // `pub` doesn't continue the line, so a break after it ends the declaration.
            if !wrong {
                p.expected(if file {
                    "a declaration"
                } else {
                    "`fn` or `const`"
                });
            }
            p.complete(marker, Skipped);
            return;
        }
    }
    let parse: fn(&mut Parser) -> SyntaxKind = match (owner, p.kind()) {
        (Owner::Interface, FnKw) => |p| {
            fn_decl(p, false);
            FnDecl
        },
        (_, FnKw) => |p| {
            fn_decl(p, true);
            FnDecl
        },
        (Owner::File | Owner::Impl, ConstKw) => |p| {
            const_decl(p);
            ConstDecl
        },
        (Owner::Impl, TypeKw) => |p| {
            assoc_type(p, true);
            AssocType
        },
        (Owner::Interface, TypeKw) => |p| {
            assoc_type(p, false);
            AssocType
        },
        (Owner::File, TestKw) => |p| {
            test_decl(p);
            TestDecl
        },
        (Owner::File, ImplKw) => |p| {
            impl_decl(p);
            ImplDecl
        },
        (Owner::File, InterfaceKw) => |p| {
            interface_decl(p);
            InterfaceDecl
        },
        (Owner::File, StructKw) => struct_decl,
        (Owner::File, EnumKw) => |p| {
            enum_decl(p);
            EnumDecl
        },
        (Owner::File, TypeKw) => |p| {
            alias_decl(p);
            AliasDecl
        },
        _ => {
            p.recovering |= quiet;
            let what = match owner {
                Owner::File => "a declaration",
                Owner::Impl => "`fn`, `const`, `type` or `}`",
                Owner::Interface => "`fn`, `type` or `}`",
            };
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
    // Only an item ends the imports: a slip among them doesn't.
    p.imports_closed |= file;
    let kind = parse(p);
    p.complete(marker, kind);
}

/// `@` name [ args ] (`grammar.attribute`), each on a line of its own (`r[item.attr.own-line]`).
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
                exprs::nonempty_args(p);
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

/// `fn` name [ generics ] `(` params `)` [ `->` type ] [ `uses` effects ] { contract }, then
/// a block when `body` (`grammar.fn_decl`, `grammar.fn_sig`). An interface's methods have none
/// (`r[item.iface.no-default]`). A line that starts with `->`, `uses`, `requires`, `ensures` or
/// `{` continues the header (`r[lex.continue.fn-header]`).
fn fn_decl(p: &mut Parser, body: bool) {
    p.bump();
    let outer_header = std::mem::replace(&mut p.in_fn_header, true);
    if name(p, "a function name")
        && (!p.at(Lt) || types::generics(p))
        && param_list(p, param, starts_param)
    {
        fn_rest(p, true, body);
    }
    p.in_fn_header = outer_header;
}

/// What follows a function's parameters, in a declaration or a closure: [ `->` type ]
/// [ `uses` effects ], its contracts when `contracts`, and its block when `body`. Without one
/// the signature ends its line. The caller has set `in_fn_header`.
pub(super) fn fn_rest(p: &mut Parser, contracts: bool, body: bool) {
    const HEADER_PARTS: [&str; 4] = ["`->`", "`uses`", "`requires`", "`ensures`"];
    let mut from = 0;
    if p.at(Arrow) {
        let marker = p.start();
        p.bump();
        let ok = types::return_type(p);
        p.complete(marker, RetType);
        if !ok {
            return;
        }
        from = 1;
    }
    if p.at(UsesKw) {
        if !types::effects(p) {
            return;
        }
        from = 2;
    }
    let to = if contracts { HEADER_PARTS.len() } else { 2 };
    if contracts && (p.at(RequiresKw) || p.at(EnsuresKw)) {
        from = 2;
    }
    // A body missing after a contract that held a slip is part of that slip.
    let slipped = contracts && self::contracts(p);
    if !body {
        // A `}` on the line closes the body the signature is in.
        if !p.at_break() && p.kind() != RBrace {
            p.recovering |= slipped;
            p.expected(&one_of(&HEADER_PARTS[from.min(to)..to], "a line break"));
        }
        return;
    }
    if p.at(LBrace) {
        exprs::block(p);
    } else {
        p.recovering |= slipped;
        p.expected(&one_of(&HEADER_PARTS[from.min(to)..to], "`{`"));
    }
}

/// `items`, then `last`, as a message lists them: "a, b or c".
fn one_of(items: &[&str], last: &str) -> String {
    match items {
        [] => last.to_string(),
        _ => format!("{} or {last}", items.join(", ")),
    }
}

/// The contracts after a function's signature, each `requires` expr, or `ensures`, an optional
/// name in parentheses and expr (`grammar.contract`). `ensures` followed by a name in
/// parentheses always names the return value (`r[contract.ensures-binder]`), so in
/// `ensures (x) - 1 > 0` the condition is `-1 > 0`. The expression starts on its contract's
/// line: a line that starts with `{` or another contract continues the header, not the
/// contract. The condition is a condition (`r[expr.block.cond-brace]`), so `{` where an
/// expression starts is the body. After a slip in a contract, the parser skips to the next
/// contract or the body's `{`, so the next contract and the body are still parsed as such.
/// Something after a whole condition, on its line, is left to the caller, as after a return
/// type: which `{` after it is the body isn't known. True when a contract held a slip.
fn contracts(p: &mut Parser) -> bool {
    let slips = p.slips;
    while p.at(RequiresKw) || p.at(EnsuresKw) {
        let marker = p.start();
        let ensures = p.kind() == EnsuresKw;
        p.bump();
        if ensures && p.at(LParen) && p.nth_kind(1) == Ident && p.nth_kind(2) == RParen {
            p.bump();
            p.bump_node(Name);
            p.bump();
        }
        if p.current().is_some_and(|t| t.brk == Break::Line) && p.header_continues(p.kind()) {
            let message = "expected an expression, found a line break".to_string();
            let error = SyntaxError::new(p.break_range(), codes::UNEXPECTED_TOKEN, message);
            p.report(error);
        } else if p.at_break() || !exprs::starts_expr(p.kind()) {
            p.expected("an expression");
        } else {
            let outer = p.condition_frames.replace(p.frames.len());
            exprs::expr(p);
            p.condition_frames = outer;
        }
        p.complete(marker, Contract);
        if p.recovering {
            p.recover(Sync::Contract, false);
        }
    }
    p.slips != slips
}

/// `(` [ item { `,` item } [ `,` ] ] `)`: a declaration's parameters (`grammar.params`) or a
/// closure's. False when there is no `(`.
pub(super) fn param_list(
    p: &mut Parser,
    param: fn(&mut Parser),
    starts_param: fn(SyntaxKind) -> bool,
) -> bool {
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
    if !p.at_or_join(LBrace) {
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
    if !p.at_or_join(Colon) {
        return p.expected("`:`");
    }
    p.bump();
    if !types::type_(p) {
        return;
    }
    if !p.at_or_join(Eq) {
        return p.expected("`=`");
    }
    p.bump();
    exprs::expr(p);
}

/// `impl` [ generics ] [ type_path [ type_args ] `for` ] type `{` members `}`
/// (`grammar.impl_decl`): an inherent `impl`, or one of an interface or an effect for a type.
fn impl_decl(p: &mut Parser) {
    p.bump();
    let mut ok = !p.at(Lt) || types::generics(p);
    // Only a type path can name an interface or an effect.
    let path = matches!(p.kind(), Ident | SelfTypeKw);
    ok = ok && types::type_(p);
    let mut target = !path;
    if ok && path && p.at(ForKw) {
        p.bump();
        ok = types::type_(p);
        target = true;
    }
    if ok && !p.at_or_join(LBrace) {
        p.expected(if target { "`{`" } else { "`for` or `{`" });
    }
    body(p, Owner::Impl);
}

/// `interface` Name [ generics ] `{` members `}` (`grammar.interface_decl`).
fn interface_decl(p: &mut Parser) {
    p.bump();
    let ok = name(p, "an interface name") && (!p.at(Lt) || types::generics(p));
    if ok && !p.at_or_join(LBrace) {
        p.expected("`{`");
    }
    body(p, Owner::Interface);
}

/// The members of an `impl` or an `interface` in braces, one per line. After a slip in the
/// header, the rest of its line is skipped up to its `{`, so the body is still parsed.
fn body(p: &mut Parser, owner: Owner) {
    skip_to_brace(p);
    if !p.at(LBrace) {
        return;
    }
    p.bump();
    let member: fn(&mut Parser) = match owner {
        Owner::Interface => |p| decl(p, Owner::Interface),
        _ => |p| decl(p, Owner::Impl),
    };
    p.lines(Holds::Body, Some(RBrace), member);
}

/// `type` Name, then `=` and a type when `value`: an associated type, which an `impl` fixes
/// and an `interface` declares (`grammar.impl_item`, `grammar.interface_decl`).
fn assoc_type(p: &mut Parser, value: bool) {
    p.bump();
    if !name(p, "a type name") || !value {
        return;
    }
    if !p.at_or_join(Eq) {
        return p.expected("`=`");
    }
    p.bump();
    types::type_(p);
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
    if p.at_or_join(LBrace) {
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
    if !p.at_or_join(LBrace) {
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
    if !p.at_or_join(Eq) {
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
