// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Paragraph, character, and cell property mappers.
//!
//! Converts [`OdfParaProps`] → [`ParaProps`], [`OdfTextProps`] → [`CharProps`],
//! and [`OdfCellProps`] → [`CellProps`].
//! All ODF measurement values are length strings (e.g. `"2.5cm"`, `"12pt"`);
//! conversion uses [`crate::xml_util::parse_length`].

mod cell;
mod char;
mod para;

pub(crate) use cell::map_cell_props;
pub(crate) use char::map_text_props;
pub(crate) use para::map_para_props;
