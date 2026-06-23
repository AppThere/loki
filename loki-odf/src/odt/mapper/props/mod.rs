// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Paragraph and character property mappers.
//!
//! Converts [`OdfParaProps`] → [`ParaProps`] and
//! [`OdfTextProps`] → [`CharProps`].
//! All ODF measurement values are length strings (e.g. `"2.5cm"`, `"12pt"`);
//! conversion uses [`crate::xml_util::parse_length`].
mod cell;
mod character;
mod paragraph;

pub(crate) use cell::map_cell_props;
pub(crate) use character::map_text_props;
pub(crate) use paragraph::map_para_props;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
