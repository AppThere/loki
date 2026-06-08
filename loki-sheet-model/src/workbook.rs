// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Format-neutral spreadsheet model types.

use std::collections::HashMap;

/// Metadata for a spreadsheet document.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DocumentMeta {
    /// Title of the document.
    pub title: Option<String>,
    /// Creator or author of the document.
    pub creator: Option<String>,
}

/// Cell horizontal alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CellAlign {
    /// Left aligned (default for text).
    #[default]
    Left,
    /// Centered.
    Center,
    /// Right aligned (default for numbers).
    Right,
}

impl CellAlign {
    /// Convert to string representation.
    pub fn as_str(self) -> &'static str {
        match self {
            CellAlign::Left => "left",
            CellAlign::Center => "center",
            CellAlign::Right => "right",
        }
    }
}

/// Cell number formatting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum NumberFormat {
    /// Unformatted or general format.
    #[default]
    General,
    /// Currency formatting (e.g. $10.00).
    Currency,
    /// Percentage formatting (e.g. 50.0%).
    Percent,
}

impl NumberFormat {
    /// Convert to string representation.
    pub fn as_str(self) -> &'static str {
        match self {
            NumberFormat::General => "general",
            NumberFormat::Currency => "currency",
            NumberFormat::Percent => "percent",
        }
    }
}

/// Formatting properties for a cell.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CellStyle {
    /// Bold text.
    pub bold: bool,
    /// Italic text.
    pub italic: bool,
    /// Underline text.
    pub underline: bool,
    /// Text horizontal alignment.
    pub align: CellAlign,
    /// Number format for values.
    pub num_format: NumberFormat,
}

/// A cell in a worksheet.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Cell {
    /// The string or evaluated numeric value of the cell.
    pub value: String,
    /// Optional formula (e.g. `=SUM(A1:A5)`).
    pub formula: Option<String>,
    /// Optional styling applied to the cell.
    pub style: Option<CellStyle>,
}

/// A single worksheet containing a grid of cells.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Worksheet {
    /// Name of the worksheet (e.g. "Sheet1").
    pub name: String,
    /// Grid of cells indexed by (row, col) coordinates (0-indexed).
    pub cells: HashMap<(u32, u32), Cell>,
}

impl Worksheet {
    /// Create a new worksheet with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            cells: HashMap::new(),
        }
    }

    /// Retrieve cell at row and col, if any.
    pub fn get_cell(&self, row: u32, col: u32) -> Option<&Cell> {
        self.cells.get(&(row, col))
    }

    /// Retrieve a mutable reference to cell at row and col, creating a default one if absent.
    pub fn get_cell_mut(&mut self, row: u32, col: u32) -> &mut Cell {
        self.cells.entry((row, col)).or_default()
    }
}

/// A workbook containing metadata and worksheet sheets.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Workbook {
    /// Document metadata.
    pub meta: DocumentMeta,
    /// The worksheet sheets.
    pub sheets: Vec<Worksheet>,
}

impl Workbook {
    /// Create a new workbook with a single default sheet.
    pub fn new() -> Self {
        Self {
            meta: DocumentMeta::default(),
            sheets: vec![Worksheet::new("Sheet1")],
        }
    }

    /// Retrieve sheet at index.
    pub fn get_sheet(&self, index: usize) -> Option<&Worksheet> {
        self.sheets.get(index)
    }

    /// Retrieve mutable sheet reference at index.
    pub fn get_sheet_mut(&mut self, index: usize) -> Option<&mut Worksheet> {
        self.sheets.get_mut(index)
    }
}

impl Default for Workbook {
    fn default() -> Self {
        Self::new()
    }
}
