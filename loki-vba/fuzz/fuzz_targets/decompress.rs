// SPDX-License-Identifier: Apache-2.0
#![no_main]
use libfuzzer_sys::fuzz_target;

// MS-OVBA decompression must never panic and must respect its bomb guards
// (macro spec §8, §12, T9) on arbitrary input.
fuzz_target!(|data: &[u8]| {
    let _ = loki_vba::decompress(data);
});
