// SPDX-License-Identifier: Apache-2.0
#![no_main]
use libfuzzer_sys::fuzz_target;

// Source-only write-back invariant (macro spec §3.4): compressing arbitrary
// bytes and decompressing them again must reproduce the input exactly, so a
// macro-editor save can never corrupt module source.
fuzz_target!(|data: &[u8]| {
    let packed = loki_vba::compress(data);
    let back = loki_vba::decompress(&packed).expect("our own container decompresses");
    assert_eq!(back, data);
});
