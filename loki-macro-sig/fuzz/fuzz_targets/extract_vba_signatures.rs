// SPDX-License-Identifier: Apache-2.0
#![no_main]
use libfuzzer_sys::fuzz_target;

// Signature-stream extraction + the embedded-PKCS#7 locator run before any trust
// decision and must never panic on arbitrary input (macro spec §12, T9).
fuzz_target!(|data: &[u8]| {
    for sig in loki_macro_sig::extract_vba_signatures(data) {
        // `black_box` exercises the locator for panics without a discarded
        // binding (keeps `check-suppressions` clean) and stops the call being
        // optimised away.
        std::hint::black_box(sig.pkcs7_der());
    }
});
