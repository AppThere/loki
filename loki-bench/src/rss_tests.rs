// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `/proc/self/status` parsing (Spec 06 M5). Pure and headless; the live reader
//! is exercised by the `device_rss` bench.

use super::*;

const SAMPLE: &str = "\
Name:\tloki-bench
VmPeak:\t 3548 kB
VmHWM:\t 1952 kB
VmRSS:\t 1952 kB
Threads:\t 1
";

#[test]
fn parses_peak_and_current() {
    assert_eq!(parse_status_kib(SAMPLE, "VmHWM"), Some(1952));
    assert_eq!(parse_status_kib(SAMPLE, "VmRSS"), Some(1952));
    assert_eq!(parse_status_kib(SAMPLE, "VmPeak"), Some(3548));
}

#[test]
fn missing_key_is_none() {
    assert_eq!(parse_status_kib(SAMPLE, "VmSwap"), None);
}

#[test]
fn prefix_collision_is_not_a_false_match() {
    // "Vm" is a prefix of several fields but must not match on its own.
    assert_eq!(parse_status_kib(SAMPLE, "Vm"), None);
}
