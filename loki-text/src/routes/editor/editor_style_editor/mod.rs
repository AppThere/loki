// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Paragraph style catalog editor panel for the document editor.
//!
//! Split into submodules:
//! - `conversions` — [`style_to_draft`] / [`draft_to_style`] helpers
//! - `panel` — [`style_editor_panel`] component and [`STYLE_EDITOR_HEIGHT_PX`]

mod conversions;
mod form;
mod panel;

pub(crate) use conversions::{draft_to_style, style_to_draft};
pub(crate) use panel::{STYLE_EDITOR_HEIGHT_PX, style_editor_panel};
