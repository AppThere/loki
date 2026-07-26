// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the memory probe's parser. Extracted per the file-ceiling idiom.
//!
//! The `note_*` functions are not covered here: they need a Dioxus runtime and
//! a provided context, so they are exercised where the profile is provided
//! rather than mocked into existence.

use super::{parse_meminfo, SystemMemory};

/// A real `/proc/meminfo` prefix from a 16 GiB Linux machine, verbatim
/// including the column alignment the kernel emits.
const REAL: &str = "\
MemTotal:       16316576 kB
MemFree:         2103944 kB
MemAvailable:   12345678 kB
Buffers:          412332 kB
Cached:          9083548 kB
";

#[test]
fn parses_total_and_available_from_real_output() {
    let m = parse_meminfo(REAL);
    assert_eq!(m.total_bytes, Some(16_316_576 * 1024));
    assert_eq!(m.available_bytes, Some(12_345_678 * 1024));
    assert!(!m.is_unknown());
}

#[test]
fn a_missing_field_stays_unknown_rather_than_zero() {
    // Older kernels have no MemAvailable. Zero available RAM is a meaningful
    // value that would drive a budget straight to its floor, so absence must
    // not be spelled the same way (L9-009).
    let m = parse_meminfo("MemTotal:       16316576 kB\nMemFree: 2103944 kB\n");
    assert_eq!(m.total_bytes, Some(16_316_576 * 1024));
    assert_eq!(m.available_bytes, None);
}

#[test]
fn an_unrecognised_unit_is_refused_rather_than_read_as_bytes() {
    // If the kernel ever reported MiB, reading the number as bytes would
    // under-report by 1024x and silently set a tiny budget.
    let m = parse_meminfo("MemTotal:       15934 MB\n");
    assert_eq!(m.total_bytes, None);
}

#[test]
fn garbage_is_unknown_not_a_panic_and_not_a_guess() {
    for text in [
        "",
        "not meminfo at all",
        "MemTotal:\n",
        "MemTotal: abc kB\n",
    ] {
        let m = parse_meminfo(text);
        assert!(m.is_unknown(), "{text:?} must read as unknown");
    }
}

#[test]
fn a_prefix_match_does_not_capture_a_different_field() {
    // A longer field that merely starts the same way must not satisfy the
    // lookup. Put first so a prefix-only match would win and be observable.
    let m = parse_meminfo("MemAvailableHuge: 999 kB\nMemAvailable:   4096 kB\n");
    assert_eq!(m.available_bytes, Some(4096 * 1024));
}

#[test]
fn default_is_unknown() {
    assert!(SystemMemory::default().is_unknown());
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[test]
fn the_platform_probe_answers_on_linux() {
    // The point of T2.0: a probe that returns `Unknown` everywhere is why
    // L08-011 was asserted-but-not-true for three phases (R24). On a platform
    // where the probe is implemented, it must actually answer.
    let m = super::probe_system_memory();
    assert!(
        m.total_bytes.is_some_and(|b| b > 0),
        "expected a real MemTotal, got {m:?}"
    );
}
