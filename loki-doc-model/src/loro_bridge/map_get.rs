// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Typed value accessors for [`LoroMap`] entries, shared by the bridge read
//! path (`read.rs`, `props_read.rs`).
//!
//! Split from `props_read.rs` to keep individual files under the 300-line
//! ceiling.

use loro::LoroMap;

pub(super) fn get_str_from_map(map: &LoroMap, key: &str) -> Option<String> {
    map.get(key)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_string().ok())
        .map(|s| s.to_string())
}

pub(super) fn get_f64_from_map(map: &LoroMap, key: &str) -> Option<f64> {
    map.get(key)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_double().ok())
}

pub(super) fn get_bool_from_map(map: &LoroMap, key: &str) -> Option<bool> {
    map.get(key)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_bool().ok())
}

pub(super) fn get_i64_from_map(map: &LoroMap, key: &str) -> Option<i64> {
    map.get(key)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_i64().ok())
}
