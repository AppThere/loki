// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Unit tests for the certificate field helpers — pure, no fixtures.

use super::{serial_hex, unix_seconds};

#[test]
fn serial_hex_is_lowercase_and_zero_padded() {
    assert_eq!(serial_hex(&[0x00, 0x0a, 0xff]), "000aff");
    assert_eq!(serial_hex(&[0x01]), "01");
    assert_eq!(serial_hex(&[]), "");
}

#[test]
fn unix_seconds_clamps_overflow() {
    assert_eq!(unix_seconds(0), 0);
    assert_eq!(unix_seconds(1_600_000_000), 1_600_000_000);
    assert_eq!(unix_seconds(u64::MAX), i64::MAX);
}
