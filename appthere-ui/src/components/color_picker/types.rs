// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The picker's data types — a swatch, and the panel's translated labels.
//!
//! Split from `mod.rs` when the document-colours group (Spec 08 T5.2) took that
//! file over the 300-line ceiling. They travel together because both are the
//! caller's side of the panel's contract: what to show, and what to call it.

/// One selectable colour: the opaque `value` reported on pick, the CSS `fill`
/// shown in the swatch square, and its accessible name.
#[derive(Clone, PartialEq)]
pub struct AtColorSwatch {
    /// Opaque value reported to `on_pick` (e.g. a hex string or variant name).
    pub value: String,
    /// CSS colour painted in the swatch square.
    pub fill: String,
    /// Accessible name of the swatch button.
    pub aria_label: String,
}

/// Translated prose labels for the panel's sections and actions.
#[derive(Clone, PartialEq)]
pub struct AtColorPickerLabels {
    /// Panel heading (e.g. "Font colour").
    pub title: String,
    /// Accessible name of the panel's close button.
    pub close: String,
    /// The "clear / automatic / none" action label.
    pub clear: String,
    /// Heading of the recent-colours section.
    pub recent_heading: String,
    /// Heading of the document-colours section.
    pub document_heading: String,
    /// Heading of the custom-colour section.
    pub custom_heading: String,
    /// Apply button label of the custom-colour section.
    pub apply: String,
    /// Accessible name of the saturation/value square.
    pub area: String,
    /// Accessible name of the hue strip.
    pub hue: String,
}
