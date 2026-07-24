// SPDX-License-Identifier: Apache-2.0
#![no_main]
use libfuzzer_sys::fuzz_target;
use loki_basic::parser::Parser;
use loki_basic::Dialect;

// The parser must never panic on arbitrary input (macro spec §12, T9).
fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = Parser::parse_module(s, Dialect::Vba);
        let _ = Parser::parse_module(s, Dialect::StarBasic);
    }
});
