//! Parses each input and checks the parser's invariants, which `quince_syntax::fuzz` holds so
//! that CI runs them too.
#![no_main]

libfuzzer_sys::fuzz_target!(|bytes: &[u8]| quince_syntax::fuzz::check_parse(bytes));
