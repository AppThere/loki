// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Pattern shading (`w:shd` line/cross textures).
//!
//! A texture `w:shd` (`diagStripe`, `horzCross`, …) is not a solid fill: it is a
//! set of hatch lines in the foreground `@w:color` over the `@w:fill`
//! background. [`ShadingPattern`] carries the pattern kind and both colours so
//! the layout/renderer can draw the actual lines, rather than collapsing them to
//! a single flattened tint. The flattened tint is still stored on
//! `background_color` as a fallback for consumers that cannot draw the hatch
//! (ODF/EPUB export, the reflow paths).
//!
//! ECMA-376 §17.18.78 (`ST_Shd`).

use loki_primitives::color::DocumentColor;

/// The geometry of a `w:shd` line/cross texture.
///
/// Direction names follow Word: `DiagUp` is the `/` of `diagStripe`, `DiagDown`
/// the `\` of `reverseDiagStripe`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HatchPattern {
    /// Horizontal lines (`horzStripe`).
    Horizontal,
    /// Vertical lines (`vertStripe`).
    Vertical,
    /// `/` diagonal lines (`diagStripe`).
    DiagUp,
    /// `\` diagonal lines (`reverseDiagStripe`).
    DiagDown,
    /// Horizontal + vertical grid (`horzCross`).
    Cross,
    /// Both diagonals, an `X` grid (`diagCross`).
    DiagCross,
}

/// A `w:shd` texture pattern: hatch geometry plus its foreground/background
/// colours. Drawn by the layout as a background fill overlaid with hatch lines.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ShadingPattern {
    /// The hatch geometry.
    pub pattern: HatchPattern,
    /// `true` for the `thin*` variants — closer, lighter lines.
    pub thin: bool,
    /// Foreground `@w:color` — the hatch line colour.
    pub color: DocumentColor,
    /// Background `@w:fill` — the fill drawn behind the lines. `None` leaves the
    /// surface (page/cell) unpainted behind the hatch.
    pub fill: Option<DocumentColor>,
}
