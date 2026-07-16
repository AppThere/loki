// SPDX-License-Identifier: Apache-2.0
#![no_main]
use libfuzzer_sys::fuzz_target;
use loki_basic::lexer::Lexer;

// The lexer must never panic on arbitrary input (macro spec §12, T9).
fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = Lexer::new(s).tokenize();
    }
});
