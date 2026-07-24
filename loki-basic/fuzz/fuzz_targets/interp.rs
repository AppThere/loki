// SPDX-License-Identifier: Apache-2.0
#![no_main]
use libfuzzer_sys::fuzz_target;
use loki_basic::host::FuelBudget;
use loki_basic::parser::Parser;
use loki_basic::{Dialect, Interp};

// Parsing then interpreting arbitrary input must terminate (fuel-bounded) and
// never panic — only return Ok/Err (macro spec §8, §12).
fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        if let Ok(module) = Parser::parse_module(s, Dialect::Vba) {
            if let Ok(mut interp) = Interp::new(&module, FuelBudget::new(100_000)) {
                // Try to run the first procedure, if any is named "main"/"f".
                for name in ["main", "f", "test"] {
                    let _ = interp.call(name, Vec::new());
                }
            }
        }
    }
});
