//! `match` and its arms (`grammar.match_expr`, `grammar.arm`), and the arms list a `select`
//! shares.

use super::{Completed, Holds, Parser, Sync, exprs, patterns};
use crate::SyntaxKind::{self, *};
use quince_diagnostics::Applicability;

/// `match` expr `{` { arm `,` } `}`. At the top of the subject, `{` starts the arms
/// (`r[expr.block.cond-brace]`), as it starts an `if`'s block.
pub(super) fn match_expr(p: &mut Parser) -> Completed {
    let marker = p.start();
    p.bump();
    exprs::condition_and(p, |p| arms(p, "a match arm", arm, MatchArmList));
    p.complete(marker, MatchExpr)
}

/// The arms in braces, each parsed by `arm`, as a `list_kind` node: a comma-separated list
/// (`r[lex.continue.lists]`) whose frame holds arms, so an arm's `=>` closes the brackets
/// opened in it (decision 9). Every arm ends with a `,`, the last one included
/// (`r[expr.match.arm-comma]`, `grammar.select`), so a `}` right after an arm is an E0016 whose
/// fix inserts it. After an arm whose line an unclosed string or char ends, the missing `,` is
/// part of that slip, and may be inside the literal, so it isn't reported, before the `}` or
/// before an arm on the next line. Every arm starts with a pattern.
pub(super) fn arms(p: &mut Parser, what_arm: &str, arm: fn(&mut Parser), list_kind: SyntaxKind) {
    let what = format!("{what_arm} or `}}`");
    let marker = p.start();
    p.bump();
    p.push_frame(Holds::Arms, Some(RBrace));
    loop {
        if p.kind() == RBrace {
            p.pop_frame();
            p.bump();
            break;
        }
        if let Some(stop) = p.stop() {
            p.close_frame(stop);
            break;
        }
        if p.unmatched_closer() {
            p.close_in_place(&what);
            break;
        }
        if patterns::starts_pattern(p.kind()) {
            arm(p);
        } else {
            p.expected(&what);
        }
        let end = p.input.tokens[p.pos - 1].range.end;
        let after_unclosed_line = p.after_unclosed_line();
        match p.kind() {
            Comma => p.bump(),
            RBrace => {
                if after_unclosed_line {
                    p.slip();
                } else {
                    let error = p.unexpected("`,`", false).with_fix(
                        "insert the missing `,`",
                        Applicability::Safe,
                        vec![(end..end, ",".to_string())],
                    );
                    p.report(error);
                }
            }
            _ if p.stop().is_some() => {}
            _ if p.unmatched_closer() => {
                p.close_in_place("`,`");
                break;
            }
            // A pattern always takes its first token, so the next arm moves the loop on.
            kind if patterns::starts_pattern(kind) && after_unclosed_line => p.slip(),
            _ => {
                p.expected_in_list("`,`", patterns::starts_pattern);
                p.recover(Sync::List, false);
            }
        }
    }
    p.complete(marker, list_kind);
}

/// pattern [ `if` expr ] `=>` ( block | expr ) (`r[expr.match.arm-syntax]`). In an arm, `{`
/// starts a block (`r[expr.match.arm-block]`).
fn arm(p: &mut Parser) {
    let marker = p.start();
    arm_parts(p);
    p.complete(marker, MatchArm);
}

fn arm_parts(p: &mut Parser) {
    if patterns::pattern(p).is_none() || p.recovering {
        return;
    }
    let mut what = "`if` or `=>`";
    if p.at(IfKw) {
        let guard = p.start();
        p.bump();
        exprs::expr(p);
        p.complete(guard, MatchGuard);
        what = "`=>`";
    }
    arrow_and_body(p, what);
}

/// An arm's `=>` and its body, an expression or a block, after what comes before them, unless
/// that held a slip. In an arm `{` starts a block (`r[expr.match.arm-block]`). `what` is what
/// may go where the `=>` is.
pub(super) fn arrow_and_body(p: &mut Parser, what: &str) {
    if p.recovering {
        return;
    }
    if !p.at_or_join(FatArrow) {
        return p.expected(what);
    }
    p.bump();
    if p.kind() == LBrace {
        exprs::block(p);
    } else {
        exprs::expr(p);
    }
}
