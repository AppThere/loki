// SPDX-License-Identifier: Apache-2.0
#![no_main]
use libfuzzer_sys::fuzz_target;

// Reading an arbitrary (possibly hostile) vbaProject.bin must return a typed
// result, never panic — the container parse runs before any trust decision
// (macro spec §12, threat T9).
fuzz_target!(|data: &[u8]| {
    let _ = loki_vba::VbaProject::read(data);
});
