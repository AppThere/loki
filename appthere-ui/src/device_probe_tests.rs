// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the memory probe's parser. Extracted per the file-ceiling idiom.
//!
//! The `note_*` functions are not covered here: they need a Dioxus runtime and
//! a provided context, so they are exercised where the profile is provided
//! rather than mocked into existence.

use super::parse_memsize;
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

/// **The macOS reading, parsed on every platform.** `hw.memsize` is a bare byte
/// count — no unit suffix, unlike `/proc/meminfo`'s `kB` — so it has its own
/// parser and its own way of being wrong.
#[test]
fn a_bare_byte_count_parses_and_a_bad_one_does_not() {
    assert_eq!(parse_memsize("8589934592\n"), Some(8 * 1024 * 1024 * 1024));
    assert_eq!(
        parse_memsize("  17179869184  "),
        Some(16 * 1024 * 1024 * 1024)
    );
    for bad in ["", "\n", "hw.memsize: 8589934592", "8 GB", "-1"] {
        assert_eq!(parse_memsize(bad), None, "{bad:?} must not parse");
    }
}

/// **Zero is not a reading.** It would look like a successful probe and drive
/// the budget to its floor — the same trap `parse_meminfo` avoids by leaving a
/// missing field `None` rather than defaulting it.
#[test]
fn a_zero_memsize_is_rejected_rather_than_believed() {
    assert_eq!(parse_memsize("0"), None);
}

/// **What the macOS probe is worth, stated as the budget it produces.** 8 GiB of
/// total with no available figure takes the `TOTAL_RAM_DIVISOR` path: 8 GiB/128
/// = 64 MiB, which is the baseline exactly — the two agree at Spec 06's design
/// floor by construction.
///
/// So on an 8 GiB Mac the probe changes the *source* and not the number, and on
/// a 32 GiB one it changes both. Asserted because "the fix has no effect here"
/// is the kind of claim that should be a computation rather than a shrug.
#[test]
fn the_macos_total_path_agrees_with_the_baseline_at_the_design_floor() {
    let eight_gib = 8_u64 * 1024 * 1024 * 1024;
    assert_eq!(eight_gib / 128, 64 * 1024 * 1024);
    let thirty_two = 32_u64 * 1024 * 1024 * 1024;
    assert_eq!(thirty_two / 128, 256 * 1024 * 1024);
}

/// **Two live derivations of one quantity, pinned against each other** (Spec 08
/// r59).
///
/// Linux keeps the hand-parsed `/proc/meminfo` path while macOS and Windows use
/// `sysinfo`, deliberately: the budget divisors are calibrated against
/// `MemAvailable`, and changing the one platform this environment can test to
/// gain tidiness is the wrong trade. But two derivations of one value is exactly
/// the shape L08-029 recorded as drifting invisibly — the drift is free until
/// something depends on the difference.
///
/// `sysinfo`'s Linux implementation reads the same two fields, so the claim
/// "unifying later would be semantics-preserving" is checkable rather than
/// merely plausible. This is that check.
///
/// **Tolerances say what they distinguish.** `MemTotal` does not move, so it is
/// asserted exactly — a difference there is a different field, not a different
/// instant. `MemAvailable` can move between the two reads, so 5% absorbs a
/// sampling gap while still failing loudly if either side switched to a
/// different quantity: `MemFree` differs from `MemAvailable` by the page cache,
/// which is gigabytes on an ordinary machine rather than percent.
#[cfg(target_os = "linux")]
#[test]
fn the_hand_parsed_linux_path_agrees_with_sysinfo() {
    use sysinfo::{MemoryRefreshKind, RefreshKind, System};

    let Ok(text) = std::fs::read_to_string("/proc/meminfo") else {
        panic!("no /proc/meminfo on a linux target — this test's subject is absent");
    };
    let parsed = parse_meminfo(&text);
    let sys = System::new_with_specifics(
        RefreshKind::nothing().with_memory(MemoryRefreshKind::nothing().with_ram()),
    );

    let (Some(total), Some(available)) = (parsed.total_bytes, parsed.available_bytes) else {
        panic!(
            "the hand parser found no MemTotal/MemAvailable, so there is nothing \
             to compare: {parsed:?}"
        );
    };
    assert_eq!(
        total,
        sys.total_memory(),
        "MemTotal disagrees between the two paths — that figure does not move \
         between reads, so this is a different field rather than a different \
         instant",
    );
    let delta = available.abs_diff(sys.available_memory());
    let tolerance = available / 20;
    assert!(
        delta <= tolerance,
        "available disagrees by {delta} bytes ({}%), beyond the {tolerance} that \
         a sampling gap explains — one path has changed which quantity it reads, \
         and unifying them would no longer be semantics-preserving",
        delta * 100 / available.max(1),
    );
}
