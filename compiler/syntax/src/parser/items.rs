//! Declarations: the file, imports, `fn`, `test`, `const`, `impl`, `interface`, `effect`,
//! `foreign`, structs, newtypes, enums and type aliases, with their doc comments and
//! attributes, and their parts.

use super::input::Break;
use super::{Holds, Parser, Step, Sync, exprs, types};
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

/// What holds a declaration: the file, or the body of an `impl`, an `interface`, an `effect`
/// or a `foreign` block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Owner {
    File,
    Impl,
    Interface,
    Effect,
    Foreign,
}

impl Owner {
    /// What its members can start with, after their doc comments and attributes, for the
    /// message after a `pub` none of them takes.
    fn unpublished(self) -> &'static str {
        match self {
            Owner::Impl => "`type`",
            Owner::Effect => "`fn`",
            _ => "`fn` or `type`",
        }
    }

    /// What may start a line in it, for the message when nothing does.
    fn expected_member(self) -> &'static str {
        match self {
            Owner::File => "a declaration",
            Owner::Impl => "`fn`, `const`, `type` or `}`",
            Owner::Effect => "`fn` or `}`",
            Owner::Interface | Owner::Foreign => "`fn`, `type` or `}`",
        }
    }
}

/// A declaration in the file, or a member of an `impl`, an `interface`, an `effect` or a
/// `foreign` block, with its doc comments, its attributes and its `pub` (`grammar.decl`,
/// `grammar.impl_item`, `grammar.interface_decl`, `grammar.effect_decl`,
/// `grammar.foreign_decl`). Anything else is skipped as an error, so a `const` in an interface
/// is one (`r[item.const.assoc.no-interface]`). The item is chosen at its keyword, once what
/// comes before it is parsed.
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
        // Only the declarations of the file and an impl's methods and constants take `pub`.
        // Anywhere else it is an E0016, and the member is parsed on. Taking the `pub` ends
        // recovery, as a member start does, so the error is the member's own, unless the line
        // is no member.
        let wrong = match owner {
            Owner::File => false,
            Owner::Impl => p.nth_kind(1) == TypeKw,
            _ => true,
        };
        let error = wrong.then(|| p.unexpected(owner.unpublished(), false));
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
            fn_decl(p, Owner::Interface);
            FnDecl
        },
        (Owner::Effect, FnKw) => |p| {
            fn_decl(p, Owner::Effect);
            FnDecl
        },
        (Owner::Foreign, FnKw) => |p| {
            c_fn(p);
            FnDecl
        },
        (_, FnKw) => |p| {
            fn_decl(p, Owner::File);
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
        (Owner::Foreign, TypeKw) => |p| {
            p.bump();
            name(p, "a type name");
            ForeignType
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
        (Owner::File, EffectKw) => |p| {
            effect_decl(p);
            EffectDecl
        },
        (Owner::File, ForeignKw) => |p| {
            foreign_decl(p);
            ForeignDecl
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
            let what = owner.expected_member();
            if p.steps.len() == marker.0 + 1 {
                // Nothing before the item, so the skip's own node is enough.
                p.abandon(marker);
                return skip(what, file, p);
            }
            skip(what, file, p);
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

/// A line that starts no declaration or member: one E0016, and the line is skipped. A block
/// it opens is skipped to its `}` (decision 9). In the `file`, that block is skipped as an
/// `impl` body: a line that starts a member of one doesn't end it, and a line that only the
/// file holds closes it. In a body, the block is skipped as a block, which a line that starts
/// a member closes.
fn skip(what: &str, file: bool, p: &mut Parser) {
    p.expected_member(what);
    let first = p.pos;
    if p.kind() != LBrace {
        p.recover(Sync::Block, true);
    }
    if p.kind() == LBrace && (p.pos == first || !p.at_break()) {
        skip_block(p, if file { Holds::Body } else { Holds::Block });
    }
}

/// Skips a block from its `{` to its `}` after the E0016 for the line that opens it, as a
/// frame that `holds` the lines a member start doesn't close. A member start that closes it,
/// or the end of the file, ends the skip: the E0016 covers the missing `}`.
fn skip_block(p: &mut Parser, holds: Holds) {
    let marker = p.start();
    p.take();
    p.push_frame(holds, Some(RBrace));
    loop {
        if p.kind() == RBrace {
            p.pop_frame();
            p.take();
            break;
        }
        if let Some(stop) = p.stop() {
            p.close_frame(stop);
            break;
        }
        p.recover(Sync::Line, true);
    }
    p.complete(marker, Skipped);
}

/// `fn` name [ generics ] `(` params `)` [ `->` type ] [ `uses` effects ], its contracts, and a
/// block (`grammar.fn_decl`, `grammar.fn_sig`). An interface's methods have no block
/// (`r[item.iface.no-default]`), and an effect's operations neither a block nor contracts
/// (`grammar.effect_decl`). A line that starts with `->`, `uses`, `requires`, `ensures` or `{`
/// continues the header (`r[lex.continue.fn-header]`).
fn fn_decl(p: &mut Parser, owner: Owner) {
    let body = matches!(owner, Owner::File | Owner::Impl);
    let contracts = owner != Owner::Effect;
    p.bump();
    let outer_header = std::mem::replace(&mut p.in_fn_header, true);
    if name(p, "a function name")
        && (!p.at(Lt) || types::generics(p))
        && param_list(p, param, starts_param)
    {
        fn_rest(p, contracts, body);
    }
    p.in_fn_header = outer_header;
}

/// `fn` c_name `(` [ c_name `:` type { `,` c_name `:` type } [ `,` ] ] `)` [ `->` type ]
/// (`grammar.c_fn`): a C function in a `foreign` block, its names in their C spelling. A
/// keyword can't be one (`r[ffi.cbind.keyword-name]`). It has no generics, effects, contracts
/// or body: its effects are the block's. A line that starts with `->` continues the header
/// (`r[lex.continue.fn-header]`).
fn c_fn(p: &mut Parser) {
    p.bump();
    let outer_header = std::mem::replace(&mut p.in_fn_header, true);
    if name(p, "a function name") && param_list(p, c_param, starts_c_param) {
        // A `}` on the line closes the block the function is in.
        let ends = |p: &Parser| p.at_break() || p.kind() == RBrace;
        if p.at(Arrow) {
            let marker = p.start();
            p.bump();
            // A `uses` after a bare function type belongs to the C function, which takes
            // none (`r[effect.uses-return]`).
            let returns = types::return_type(p);
            p.complete(marker, RetType);
            if returns && !ends(p) {
                p.expected("a line break");
            }
        } else if !ends(p) {
            p.expected("`->` or a line break");
        }
    }
    p.in_fn_header = outer_header;
}

/// A C function's parameter: a name, `:` and a type.
fn c_param(p: &mut Parser) {
    if !starts_c_param(p.kind()) || p.at_break() {
        return p.expected("a parameter");
    }
    let marker = p.start();
    p.bump_node(Name);
    if p.at(Colon) {
        p.bump();
        types::type_(p);
    } else {
        p.expected("`:`");
    }
    p.complete(marker, Param);
}

fn starts_c_param(kind: SyntaxKind) -> bool {
    kind == Ident
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
    // A line that starts with `for` continues the header, as an E0018 (decision 4).
    if ok && path && p.at_or_join(ForKw) {
        p.bump();
        ok = types::type_(p);
        target = true;
    }
    let what = if target { "`{`" } else { "`for` or `{`" };
    body(p, Owner::Impl, ok.then_some(what));
}

/// `interface` Name [ generics ] `{` members `}` (`grammar.interface_decl`).
fn interface_decl(p: &mut Parser) {
    p.bump();
    let ok = name(p, "an interface name") && (!p.at(Lt) || types::generics(p));
    body(p, Owner::Interface, ok.then_some("`{`"));
}

/// `effect` Name, then [ `uses` effects ] and its operations in braces, or `=` and the path of
/// another effect (`grammar.effect_decl`). A line that starts with `uses` continues the header,
/// as an E0018 (decision 4).
fn effect_decl(p: &mut Parser) {
    p.bump();
    if !name(p, "an effect name") {
        return;
    }
    if p.at_or_join(Eq) {
        p.bump();
        if p.at(Ident) {
            types::bare_path(p);
        } else {
            p.expected("an effect");
        }
        return;
    }
    let uses = p.at_or_join(UsesKw);
    let ok = !uses || types::effects(p);
    let what = if uses { "`{`" } else { "`uses`, `{` or `=`" };
    body(p, Owner::Effect, ok.then_some(what));
}

/// `foreign` string [ `uses` effects ] `{` members `}` (`grammar.foreign_decl`): the library
/// and its C functions and types. A line that starts with `uses` continues the header, as an
/// E0018 (decision 4).
fn foreign_decl(p: &mut Parser) {
    p.bump();
    if !p.at(StringStart) {
        p.expected("the library's name, a string");
        return body(p, Owner::Foreign, None);
    }
    exprs::literal(p);
    let uses = p.at_or_join(UsesKw);
    let ok = !uses || types::effects(p);
    let what = if uses { "`{`" } else { "`uses` or `{`" };
    body(p, Owner::Foreign, ok.then_some(what));
}

/// The body of an `impl`, an `interface`, an `effect` or a `foreign` block: its members in
/// braces, one per line. `expected` is what the header takes next, `{` among it, when the
/// header parsed with no slip. After a slip in the header, the rest of its line is skipped up
/// to its `{`, so the body is still parsed. A header that parsed, its generics or type
/// arguments closed early at the break included, and ends its line where the `{` belongs is
/// one E0016 at the line break. The lines after it are its members, up to the `}`, as though
/// the `{` were there (decision 9). When that `}` comes and the body held no slip, the E0016
/// gets a fix that inserts the `{`.
fn body(p: &mut Parser, owner: Owner, expected: Option<&str>) {
    let mut assumed = false;
    // The E0016 at the line break where the `{` belongs, by its step, and where the `{` goes.
    let mut missing = None;
    if let Some(what) = expected
        && !p.at_or_join(LBrace)
    {
        let (at, end) = (p.steps.len(), p.input.tokens[p.pos - 1].range.end);
        assumed = p.at_break();
        p.expected(what);
        if assumed && matches!(p.steps.get(at), Some(Step::Error(_))) {
            missing = Some((at, end));
        }
    }
    skip_to_brace(p);
    let member: fn(&mut Parser) = match owner {
        Owner::Interface => |p| decl(p, Owner::Interface),
        Owner::Effect => |p| decl(p, Owner::Effect),
        Owner::Foreign => |p| decl(p, Owner::Foreign),
        _ => |p| decl(p, Owner::Impl),
    };
    if p.at(LBrace) {
        p.bump();
        p.lines(Holds::Body, Some(RBrace), member);
        return;
    }
    if !assumed {
        return;
    }
    p.push_assumed_frame(Holds::Body, RBrace);
    // The `{` counts as taken, so the first member reports its own slips, as after a real
    // one, unless a `;` or a lexer error before the break owns the slip.
    if missing.is_some() {
        p.recovering = false;
    }
    let slips = p.slips;
    let closed = p.lines_in(Some(RBrace), member);
    // The fix says the `{` is all that's missing, so a body with a slip of its own gets none.
    if let (true, true, Some((at, end))) = (closed, p.slips == slips, missing)
        && let Step::Error(error) = &mut p.steps[at]
    {
        let edit = (end..end, " {".to_string());
        *error = error
            .clone()
            .with_fix("insert the `{`", Applicability::Safe, vec![edit]);
    }
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
