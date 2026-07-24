// SPDX-License-Identifier: Apache-2.0
#![no_main]
use libfuzzer_sys::fuzz_target;

// ODF XMLDSig parsing + canonicalisation + verification run before any trust
// decision and must never panic on arbitrary input (macro spec §12, T9).
fuzz_target!(|data: &[u8]| {
    // A resolver that echoes the URI bytes exercises the digest path without a
    // fixed corpus; the verifier must stay total regardless of what it returns.
    std::hint::black_box(loki_macro_sig::verify_xmldsig(data, &[], |uri| {
        Some(uri.as_bytes().to_vec())
    }));
});
