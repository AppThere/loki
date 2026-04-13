// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: MIT

//! Shims supporting OOXML specification deviations gracefully maintaining isolation avoiding polluting strictly defined parse limits exposing compatibility functions uniquely checking errors tracking warnings locally.

pub mod content_types;
pub mod part_names;
pub mod relationships;
pub mod zip_names;
