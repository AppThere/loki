// SPDX-License-Identifier: Apache-2.0

//! The table the Insert table dialog will build (design section 6).
//!
//! Pure: [`TableSpec`] describes what the user asked for and [`build_table`]
//! turns it into a model [`Table`], with no Dioxus scope in sight — so the
//! grid's arithmetic and the header-row promotion are testable directly.

use loki_doc_model::content::inline::Inline;
use loki_doc_model::content::table::col::TableWidth;
use loki_doc_model::content::table::core::Table;
use loki_i18n::fl;

/// The largest grid the drag picker offers, and the ceiling the steppers clamp
/// to. Not a model limit — rows and columns are `usize` — but a table wider
/// than the text area lays out nothing readable, and the picker has to stop
/// somewhere the pointer can still reach.
pub(super) const MAX_GRID_COLS: usize = 8;
/// Rows offered by the drag picker. The steppers reach further.
pub(super) const MAX_GRID_ROWS: usize = 6;
/// The stepper ceiling — past this, a user wants a spreadsheet.
pub(super) const MAX_ROWS: usize = 100;
/// The column ceiling.
pub(super) const MAX_COLS: usize = 32;

/// How the table sizes itself in the text area.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::routes::editor) enum WidthChoice {
    /// Fill the text area edge to edge.
    Fill,
    /// Size the columns to their content.
    Auto,
}

impl WidthChoice {
    /// Both choices, in display order.
    pub const ALL: [WidthChoice; 2] = [WidthChoice::Fill, WidthChoice::Auto];

    /// The localized label.
    #[must_use]
    pub fn label(self) -> String {
        match self {
            WidthChoice::Fill => fl!("table-dialog-width-fill"),
            WidthChoice::Auto => fl!("table-dialog-width-auto"),
        }
    }

    /// The model width this maps to.
    #[must_use]
    pub fn to_model(self) -> TableWidth {
        match self {
            WidthChoice::Fill => TableWidth::Percent(100.0),
            WidthChoice::Auto => TableWidth::Auto,
        }
    }

    /// The choice at `index`, saturating.
    #[must_use]
    pub fn from_index(index: usize) -> Self {
        WidthChoice::ALL
            .get(index)
            .copied()
            .unwrap_or(WidthChoice::Fill)
    }

    /// This choice's position in the control.
    #[must_use]
    pub fn index(self) -> usize {
        WidthChoice::ALL
            .iter()
            .position(|w| *w == self)
            .unwrap_or(0)
    }
}

/// What the dialog will insert.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::routes::editor) struct TableSpec {
    /// Body rows requested, **including** the header row when `header_row` is
    /// set — the grid shows what the table will look like, and a header is a
    /// row on screen.
    pub rows: usize,
    /// Columns requested.
    pub cols: usize,
    /// The caption text; empty inserts no caption.
    pub caption: String,
    /// Promote the first row to a table header.
    pub header_row: bool,
    /// How the table sizes itself.
    pub width: WidthChoice,
    /// The table style's id, if one is chosen.
    pub style: Option<String>,
}

impl Default for TableSpec {
    fn default() -> Self {
        Self {
            rows: 4,
            cols: 3,
            caption: String::new(),
            // On by default: an unheaded table is inaccessible, and the export
            // pipeline has no way to guess a header row later (design note 24).
            header_row: true,
            width: WidthChoice::Fill,
            style: None,
        }
    }
}

impl TableSpec {
    /// Clamps the requested size into what the dialog will actually build.
    ///
    /// A table needs at least one row and one column; a zero of either is not a
    /// smaller table but no table at all.
    #[must_use]
    pub fn clamped(&self) -> Self {
        Self {
            rows: self.rows.clamp(1, MAX_ROWS),
            cols: self.cols.clamp(1, MAX_COLS),
            ..self.clone()
        }
    }

    /// How many rows of body content remain once the header is taken.
    ///
    /// A one-row table asked to have a header keeps its single row as the
    /// header and has no body — which is a legitimate (if unusual) table, and
    /// better than silently dropping the header the user asked for.
    #[must_use]
    pub fn body_rows(&self) -> usize {
        let rows = self.clamped().rows;
        if self.header_row {
            rows.saturating_sub(1)
        } else {
            rows
        }
    }
}

/// Builds the model table `spec` describes.
///
/// The header row is **promoted out of the body**, not added to it: the grid
/// and the steppers count the rows the user sees, so a 4×3 table with a header
/// is one header row and three body rows — not four body rows plus a fifth.
#[must_use]
pub(super) fn build_table(spec: &TableSpec) -> Table {
    let spec = spec.clamped();
    let mut table = Table::grid(spec.rows, spec.cols);

    if spec.header_row
        && let Some(body) = table.bodies.first_mut()
        && !body.body_rows.is_empty()
    {
        let head_row = body.body_rows.remove(0);
        table.head.rows.push(head_row);
    }

    let caption = spec.caption.trim();
    if !caption.is_empty() {
        table.caption.full = vec![Inline::Str(caption.to_string())];
    }
    table.width = Some(spec.width.to_model());
    if let Some(style) = spec.style.as_ref().filter(|s| !s.trim().is_empty()) {
        table.set_style_name(Some(style.clone()));
    }
    table
}

#[cfg(test)]
#[path = "spec_tests.rs"]
mod tests;
