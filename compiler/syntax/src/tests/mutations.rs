//! Seeded mutations of the `lex/` fixtures, which check the lexer's and the parser's invariants
//! on inputs no test wrote.

use super::errors::check_error_ranges;
use super::repo_root;
use crate::fuzz::check_lossless;
use crate::{codes, decode_source, lex, parse};
use std::collections::BTreeSet;
use std::panic::{AssertUnwindSafe, catch_unwind};

/// The conformance fixtures of the lexer, relative to the repo root.
const LEX_FIXTURES_DIR: &str = "tests/conformance/testdata/lex";

/// Mutants made from each fixture. With 38 fixtures, lexing and parsing them takes about 1 s in
/// debug, so the test adds nothing to the crate's run, which the exhaustive `every_short_*`
/// tests set.
const MUTANTS_PER_SEED: usize = 120;

/// Each mutant applies between one and this many mutations to its seed.
const MAX_MUTATIONS: usize = 3;

/// Byte strings an insertion picks from: the bytes that open, close or escape a token, and a
/// few that start an error, such as a lone UTF-8 continuation byte, BEL and U+202E.
const INSERTED: [&[u8]; 22] = [
    b"\"",
    b"\"\"\"",
    b"'",
    b"{",
    b"}",
    b"\\",
    b"\\u{",
    b"/",
    b"*",
    b"#",
    b"\n",
    b"\r",
    b"\t",
    b" ",
    b"r",
    b"b",
    b"0x",
    b"1e",
    b".",
    b"\x80",
    b"\x07",
    b"\xe2\x80\xae",
];

/// Seeded mutations of the `lex/` fixtures keep the invariants, the lexer's and the parser's.
/// They are bytes, so they reach invalid UTF-8 through `decode_source`, `lex` and `parse`.
/// Together they report every lexer code, which the fixtures guarantee, since every code has
/// one and `cargo xtask diagnostics` requires it.
#[test]
fn mutations_keep_the_invariants() {
    let dir = repo_root().join(LEX_FIXTURES_DIR);
    let fixtures = std::fs::read_dir(&dir).unwrap();
    let mut fixtures: Vec<_> = fixtures.map(|entry| entry.unwrap().path()).collect();
    fixtures.sort();
    assert!(
        fixtures.len() >= 30,
        "found only {} fixtures in {LEX_FIXTURES_DIR}",
        fixtures.len()
    );

    let mut reported = BTreeSet::new();
    for path in &fixtures {
        let name = path.file_name().unwrap().to_string_lossy();
        let seed = std::fs::read(path).unwrap();
        // Each fixture has its own stream, so editing another fixture doesn't change its mutants.
        let mut rng = Rng(0x5157_494e_4345 ^ fnv1a(&seed));
        for n in 0..MUTANTS_PER_SEED {
            let mut bytes = seed.clone();
            for _ in 0..=rng.below(MAX_MUTATIONS) {
                mutate(&mut bytes, &mut rng);
            }
            let checked = catch_unwind(AssertUnwindSafe(|| {
                let decoded = decode_source(&bytes).unwrap();
                let lexed = lex(&decoded);
                check_error_ranges(&decoded.text, &lexed);
                let parsed = parse(&decoded);
                check_lossless(&decoded.text, &lexed, &parsed.syntax());
                crate::fuzz::check_error_ranges(&decoded.text, parsed.errors());
                lexed.errors
            }));
            let Ok(errors) = checked else {
                panic!(
                    "mutant {n} of {LEX_FIXTURES_DIR}/{name} broke an invariant: b\"{}\"",
                    bytes.escape_ascii()
                );
            };
            reported.extend(errors.iter().map(|e| e.code));
        }
    }
    let missing: Vec<_> = codes::LEXER
        .iter()
        .filter(|c| !reported.contains(*c))
        .collect();
    assert!(missing.is_empty(), "no mutant reported {missing:?}");
}

/// Inserts a byte string from [`INSERTED`] or a random byte, deletes a byte, or swaps two.
fn mutate(bytes: &mut Vec<u8>, rng: &mut Rng) {
    let len = bytes.len();
    // An empty text can only grow.
    let mutation = if len == 0 { 0 } else { rng.below(4) };
    match mutation {
        0 => {
            let at = rng.below(len + 1);
            bytes.splice(at..at, INSERTED[rng.below(INSERTED.len())].iter().copied());
        }
        1 => bytes.insert(rng.below(len + 1), rng.next() as u8),
        2 => drop(bytes.remove(rng.below(len))),
        _ => bytes.swap(rng.below(len), rng.below(len)),
    }
}

/// The 64-bit FNV-1a hash of `bytes`.
fn fnv1a(bytes: &[u8]) -> u64 {
    (bytes.iter()).fold(0xcbf2_9ce4_8422_2325, |hash, &b| {
        (hash ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// SplitMix64: a fixed seed gives the same mutants on every run, with no dependency.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A number in `0..n`, for `n` > 0.
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}
