// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Paragraph and character property mappers.
//!
//! Converts [`DocxPPr`] → [`ParaProps`] and [`DocxRPr`] → [`CharProps`].
//! All OOXML measurements are in twentieths of a point (twips); all model
//! measurements are in [`loki_primitives::units::Points`].

mod border;
mod char;
mod helpers;
mod para;

#[cfg(test)]
mod tests;

pub(crate) use border::map_border_edge;
pub(crate) use char::map_rpr;
pub(crate) use para::map_ppr;
