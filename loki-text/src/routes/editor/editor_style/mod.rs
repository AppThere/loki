// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Inline paragraph-style picker panel for the document editor.
//!
//! Public API is re-exported from submodules so external callers see no change.

mod helpers;
mod panel;

pub use panel::style_picker_panel;
