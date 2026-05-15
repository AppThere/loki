// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Table mapper: `w:tbl` → `Block::Table`.

use std::collections::{HashMap, HashSet};

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::table::col::{ColAlignment, ColSpec, ColWidth, TableWidth};
use loki_doc_model::content::table::core::{Table, TableBody, TableCaption, TableFoot, TableHead};
use loki_doc_model::content::table::row::{
    Cell, CellProps, CellTextDirection, CellVerticalAlign, Row,
};
use loki_primitives::units::Points;

use crate::docx::model::styles::{
    DocxTableModel, DocxTableRow, DocxTextDirection, DocxVAlign, DocxVMerge,
};

use super::document::MappingContext;
use super::paragraph::map_paragraph;
use super::props::map_border_edge;

/// Maps a `w:tbl` to a `Block::Table`.
///
/// ## Vertical merge
/// A two-pass algorithm resolves `w:vMerge` spans: restart cells receive
/// the correct `row_span` count and continuation cells are removed from the
/// output (OOXML §17.4.84).
///
/// ## Column widths
/// `w:tblGrid` widths are converted from twips to points when present;
/// otherwise `ColWidth::Default` is used.
pub(crate) fn map_table(t: &DocxTableModel, ctx: &mut MappingContext<'_>) -> Block {
    let col_specs = build_col_specs(t);

    // Pre-compute row spans and the set of continuation cells to remove.
    let (span_map, skip_set) = compute_v_merge_spans(&t.rows);

    let mut head_rows: Vec<Row> = Vec::new();
    let mut body_rows: Vec<Row> = Vec::new();

    for (row_idx, tr) in t.rows.iter().enumerate() {
        let is_header = tr.tr_pr.as_ref().is_some_and(|p| p.is_header);

        let mut grid_col: usize = 0;
        let mut cells: Vec<Cell> = Vec::new();
        for (cell_idx, tc) in tr.cells.iter().enumerate() {
            let col_span = tc
                .tc_pr
                .as_ref()
                .and_then(|p| p.grid_span)
                .unwrap_or(1)
                .max(1) as usize;
            // NOTE: continuation cells are removed from output. The spanning
            // cell above carries row_span = N covering all removed rows.
            // loki-layout must account for row_span when placing cell content.
            if !skip_set.contains(&(row_idx, cell_idx)) {
                let mut cell = map_cell(tc, ctx);
                cell.row_span = span_map.get(&(row_idx, grid_col)).copied().unwrap_or(1);
                cells.push(cell);
            }
            grid_col += col_span;
        }

        let row = Row::new(cells);
        if is_header {
            head_rows.push(row);
        } else {
            body_rows.push(row);
        }
    }

    let head = if head_rows.is_empty() {
        TableHead::empty()
    } else {
        TableHead {
            attr: NodeAttr::default(),
            rows: head_rows,
        }
    };

    let body = TableBody::from_rows(body_rows);

    let width = map_tbl_width(t);

    let table = Table {
        attr: NodeAttr::default(),
        caption: TableCaption::default(),
        width,
        col_specs,
        head,
        bodies: vec![body],
        foot: TableFoot::empty(),
    };

    Block::Table(Box::new(table))
}

/// Converts `w:tblW` to [`TableWidth`].
///
/// COMPAT(microsoft): w:tblW @w:type="pct" uses fiftieths of a percent,
/// not hundredths — divide by 50 to get 0.0–100.0 range.
#[allow(clippy::cast_precision_loss)] // twip values are small; f32 precision is sufficient
fn map_tbl_width(t: &DocxTableModel) -> Option<TableWidth> {
    let w = t.tbl_pr.as_ref()?.width.as_ref()?;
    Some(match w.w_type.as_str() {
        "dxa" => TableWidth::Fixed(w.w as f32 / 20.0),
        "pct" => TableWidth::Percent(w.w as f32 / 50.0),
        _ => TableWidth::Auto, // "auto" | "nil" | unknown
    })
}

/// Builds column specifications from `w:tblGrid` column widths.
fn build_col_specs(t: &DocxTableModel) -> Vec<ColSpec> {
    if t.col_widths.is_empty() {
        // Fall back: infer column count from the widest row.
        let num_cols = t.rows.iter().map(|r| r.cells.len()).max().unwrap_or(0);
        (0..num_cols)
            .map(|_| ColSpec {
                alignment: ColAlignment::Default,
                width: ColWidth::Default,
            })
            .collect()
    } else {
        t.col_widths
            .iter()
            .map(|&w| ColSpec {
                alignment: ColAlignment::Default,
                width: if w > 0 {
                    ColWidth::Fixed(Points::new(f64::from(w) / 20.0))
                } else {
                    ColWidth::Default
                },
            })
            .collect()
    }
}

/// Maps a `w:tc` table cell.
fn map_cell(tc: &crate::docx::model::styles::DocxTableCell, ctx: &mut MappingContext<'_>) -> Cell {
    let col_span = tc.tc_pr.as_ref().and_then(|p| p.grid_span).unwrap_or(1);
    let blocks: Vec<Block> = tc
        .paragraphs
        .iter()
        .flat_map(|p| map_paragraph(p, ctx))
        .collect();

    let mut props = CellProps::default();
    if let Some(tc_pr) = tc.tc_pr.as_ref() {
        // Cell background from `w:shd @w:fill`.
        if let Some(ref hex) = tc_pr.shd_fill
            && let Some(rgb) = crate::xml_util::hex_color(hex)
        {
            use loki_primitives::color::DocumentColor;
            props.background_color = Some(DocumentColor::Rgb(rgb));
        }
        // Cell borders from `w:tcBorders`.
        if let Some(ref borders) = tc_pr.tc_borders {
            props.border_top = borders.top.as_ref().map(map_border_edge);
            props.border_bottom = borders.bottom.as_ref().map(map_border_edge);
            props.border_left = borders.left.as_ref().map(map_border_edge);
            props.border_right = borders.right.as_ref().map(map_border_edge);
        }
        // Cell padding from `w:tcMar`. COMPAT(ooxml-dxa): twips ÷ 20 = points.
        if let Some(ref m) = tc_pr.tc_margins {
            props.padding_top = m.top.map(|v| Points::new(f64::from(v) / 20.0));
            props.padding_bottom = m.bottom.map(|v| Points::new(f64::from(v) / 20.0));
            props.padding_left = m.left.map(|v| Points::new(f64::from(v) / 20.0));
            props.padding_right = m.right.map(|v| Points::new(f64::from(v) / 20.0));
        }
        // Vertical alignment from `w:vAlign`.
        props.vertical_align = tc_pr.v_align.map(|v| match v {
            DocxVAlign::Top => CellVerticalAlign::Top,
            DocxVAlign::Center => CellVerticalAlign::Middle,
            DocxVAlign::Bottom => CellVerticalAlign::Bottom,
        });
        // Text direction from `w:textDirection`.
        props.text_direction = tc_pr.text_direction.map(|d| match d {
            DocxTextDirection::LrTb => CellTextDirection::LrTb,
            DocxTextDirection::TbRl => CellTextDirection::TbRl,
            DocxTextDirection::TbLr => CellTextDirection::TbLr,
            DocxTextDirection::BtLr => CellTextDirection::BtLr,
        });
    }

    Cell {
        attr: NodeAttr::default(),
        alignment: ColAlignment::Default,
        row_span: 1, // overridden by map_table after compute_v_merge_spans
        col_span,
        blocks,
        props,
    }
}

// ── vMerge two-pass algorithm ─────────────────────────────────────────────────

/// Computes `row_span` values for all vertically-merged cells in a table.
///
/// Returns:
/// - `span_map`: `(row_idx, grid_col)` → `row_span` for every `Restart` cell.
///   The key uses the cell's *starting* grid column (accounting for
///   `w:gridSpan` of preceding cells in the same row).
/// - `skip_set`: `(row_idx, cell_idx)` pairs that are `Continue` cells and
///   should be omitted from the output row.
///
/// OOXML §17.4.84: `w:vMerge` with no `w:val` is a continuation cell.
///
/// # Algorithm
///
/// **Pass 1** — build a `v_merge_grid[row][grid_col]` by expanding each cell
/// by its `w:gridSpan` so that multi-column cells fill multiple grid slots
/// with the same vMerge state.
///
/// **Pass 2** — for each grid column, scan down; on every `Restart` cell,
/// count consecutive `Continue` cells below and record the span length.
/// Each counted `Continue` cell is added to `skip_set`.
#[allow(clippy::type_complexity)] // Pre-existing pattern — structural refactor deferred
fn compute_v_merge_spans(
    rows: &[DocxTableRow],
) -> (HashMap<(usize, usize), u32>, HashSet<(usize, usize)>) {
    // Pass 1: expand cells into a flat grid indexed by grid column.
    let mut v_merge_grid: Vec<Vec<Option<DocxVMerge>>> = Vec::with_capacity(rows.len());
    let mut cell_idx_grid: Vec<Vec<usize>> = Vec::with_capacity(rows.len());

    for row in rows {
        let mut v_merge_row: Vec<Option<DocxVMerge>> = Vec::new();
        let mut cell_idx_row: Vec<usize> = Vec::new();
        for (cell_idx, cell) in row.cells.iter().enumerate() {
            let v_merge = cell.tc_pr.as_ref().and_then(|p| p.v_merge);
            let col_span = cell
                .tc_pr
                .as_ref()
                .and_then(|p| p.grid_span)
                .unwrap_or(1)
                .max(1) as usize;
            for _ in 0..col_span {
                v_merge_row.push(v_merge);
                cell_idx_row.push(cell_idx);
            }
        }
        v_merge_grid.push(v_merge_row);
        cell_idx_grid.push(cell_idx_row);
    }

    let num_rows = v_merge_grid.len();
    let num_cols = v_merge_grid.iter().map(Vec::len).max().unwrap_or(0);

    let mut span_map: HashMap<(usize, usize), u32> = HashMap::new();
    // COMPAT(microsoft): w:vMerge with no w:val attribute is a continuation
    // cell per OOXML §17.4.84, not a restart. Some non-Microsoft producers
    // incorrectly omit w:vMerge entirely for continuation cells — those will
    // still render as row_span=1.
    let mut skip_set: HashSet<(usize, usize)> = HashSet::new();

    // Pass 2: for each column, find restart cells and count their span.
    for col in 0..num_cols {
        for row in 0..num_rows {
            if v_merge_grid[row].get(col).copied() == Some(Some(DocxVMerge::Restart)) {
                let mut span = 1u32;
                let mut r = row + 1;
                while r < num_rows
                    && v_merge_grid[r].get(col).copied() == Some(Some(DocxVMerge::Continue))
                {
                    if let Some(&cell_idx) = cell_idx_grid[r].get(col) {
                        skip_set.insert((r, cell_idx));
                    }
                    span += 1;
                    r += 1;
                }
                span_map.insert((row, col), span);
            }
        }
    }

    (span_map, skip_set)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docx::import::DocxImportOptions;
    use crate::docx::model::paragraph::DocxParagraph;
    use crate::docx::model::styles::{DocxTableCell, DocxTableRow, DocxTcPr, DocxTrPr};
    use loki_doc_model::content::block::Block;
    use loki_doc_model::style::catalog::StyleCatalog;
    use loki_opc::PartData;
    use std::collections::HashMap;

    fn make_ctx<'a>(
        styles: &'a StyleCatalog,
        footnotes: &'a HashMap<i32, Vec<Block>>,
        endnotes: &'a HashMap<i32, Vec<Block>>,
        hyperlinks: &'a HashMap<String, String>,
        images: &'a HashMap<String, PartData>,
        options: &'a DocxImportOptions,
    ) -> MappingContext<'a> {
        MappingContext {
            styles,
            footnotes,
            endnotes,
            hyperlinks,
            images,
            options,
            warnings: Vec::new(),
        }
    }

    fn simple_cell(paragraphs: Vec<DocxParagraph>) -> DocxTableCell {
        DocxTableCell {
            tc_pr: None,
            paragraphs,
        }
    }

    fn simple_row(cells: Vec<DocxTableCell>) -> DocxTableRow {
        DocxTableRow { tr_pr: None, cells }
    }

    #[test]
    fn empty_table_produces_table_block() {
        let styles = StyleCatalog::default();
        let (fn_m, en_m, hl_m, img_m) = (
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
        );
        let opts = DocxImportOptions::default();
        let mut ctx = make_ctx(&styles, &fn_m, &en_m, &hl_m, &img_m, &opts);

        let t = DocxTableModel {
            tbl_pr: None,
            col_widths: vec![],
            rows: vec![],
        };
        let block = map_table(&t, &mut ctx);
        assert!(matches!(block, Block::Table(_)));
        if let Block::Table(tbl) = block {
            assert_eq!(tbl.col_specs.len(), 0);
            assert!(tbl.bodies[0].body_rows.is_empty());
        }
    }

    #[test]
    fn two_by_two_table() {
        let styles = StyleCatalog::default();
        let (fn_m, en_m, hl_m, img_m) = (
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
        );
        let opts = DocxImportOptions::default();
        let mut ctx = make_ctx(&styles, &fn_m, &en_m, &hl_m, &img_m, &opts);

        let t = DocxTableModel {
            tbl_pr: None,
            col_widths: vec![1440, 1440], // 72pt each
            rows: vec![
                simple_row(vec![
                    simple_cell(vec![DocxParagraph::default()]),
                    simple_cell(vec![DocxParagraph::default()]),
                ]),
                simple_row(vec![
                    simple_cell(vec![DocxParagraph::default()]),
                    simple_cell(vec![DocxParagraph::default()]),
                ]),
            ],
        };
        let block = map_table(&t, &mut ctx);
        if let Block::Table(tbl) = block {
            assert_eq!(tbl.col_specs.len(), 2);
            assert_eq!(tbl.bodies[0].body_rows.len(), 2);
            assert_eq!(tbl.bodies[0].body_rows[0].cells.len(), 2);
            // 1440 twips = 72 pt
            assert!(
                matches!(tbl.col_specs[0].width, ColWidth::Fixed(p) if (p.value() - 72.0).abs() < 0.01)
            );
        } else {
            panic!("expected Table");
        }
    }

    #[test]
    fn header_row_goes_to_head() {
        let styles = StyleCatalog::default();
        let (fn_m, en_m, hl_m, img_m) = (
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
        );
        let opts = DocxImportOptions::default();
        let mut ctx = make_ctx(&styles, &fn_m, &en_m, &hl_m, &img_m, &opts);

        let header_row = DocxTableRow {
            tr_pr: Some(DocxTrPr { is_header: true }),
            cells: vec![simple_cell(vec![])],
        };
        let body_row = simple_row(vec![simple_cell(vec![])]);
        let t = DocxTableModel {
            tbl_pr: None,
            col_widths: vec![],
            rows: vec![header_row, body_row],
        };
        let block = map_table(&t, &mut ctx);
        if let Block::Table(tbl) = block {
            assert_eq!(tbl.head.rows.len(), 1);
            assert_eq!(tbl.bodies[0].body_rows.len(), 1);
        } else {
            panic!("expected Table");
        }
    }

    #[test]
    fn cell_col_span_preserved() {
        let styles = StyleCatalog::default();
        let (fn_m, en_m, hl_m, img_m) = (
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
        );
        let opts = DocxImportOptions::default();
        let mut ctx = make_ctx(&styles, &fn_m, &en_m, &hl_m, &img_m, &opts);

        let cell_with_span = DocxTableCell {
            tc_pr: Some(DocxTcPr {
                grid_span: Some(3),
                v_merge: None,
                ..Default::default()
            }),
            paragraphs: vec![],
        };
        let t = DocxTableModel {
            tbl_pr: None,
            col_widths: vec![],
            rows: vec![simple_row(vec![cell_with_span])],
        };
        let block = map_table(&t, &mut ctx);
        if let Block::Table(tbl) = block {
            assert_eq!(tbl.bodies[0].body_rows[0].cells[0].col_span, 3);
        } else {
            panic!("expected Table");
        }
    }

    // ── compute_v_merge_spans unit tests ─────────────────────────────────────

    fn merge_cell(v_merge: DocxVMerge) -> DocxTableCell {
        DocxTableCell {
            tc_pr: Some(DocxTcPr {
                v_merge: Some(v_merge),
                ..Default::default()
            }),
            paragraphs: vec![],
        }
    }

    fn merge_cell_with_col_span(v_merge: DocxVMerge, col_span: u32) -> DocxTableCell {
        DocxTableCell {
            tc_pr: Some(DocxTcPr {
                v_merge: Some(v_merge),
                grid_span: Some(col_span),
                ..Default::default()
            }),
            paragraphs: vec![],
        }
    }

    /// Simple 2-row merge: 2×2 table, col 0 merged across rows 0-1.
    #[test]
    fn vmerge_simple_2_row_merge() {
        let rows = vec![
            simple_row(vec![merge_cell(DocxVMerge::Restart), simple_cell(vec![])]),
            simple_row(vec![merge_cell(DocxVMerge::Continue), simple_cell(vec![])]),
        ];
        let (span_map, skip_set) = compute_v_merge_spans(&rows);

        assert_eq!(span_map[&(0, 0)], 2, "restart cell should have row_span=2");
        assert!(
            skip_set.contains(&(1, 0)),
            "continuation cell (1,0) should be skipped"
        );
        assert!(
            !skip_set.contains(&(0, 0)),
            "restart cell must not be skipped"
        );
        assert!(
            !skip_set.contains(&(0, 1)),
            "col-1 cells must not be skipped"
        );
        assert!(
            !skip_set.contains(&(1, 1)),
            "col-1 cells must not be skipped"
        );
    }

    /// 3-row merge: col 0 merged across 3 rows.
    #[test]
    fn vmerge_3_row_merge() {
        let rows = vec![
            simple_row(vec![merge_cell(DocxVMerge::Restart), simple_cell(vec![])]),
            simple_row(vec![merge_cell(DocxVMerge::Continue), simple_cell(vec![])]),
            simple_row(vec![merge_cell(DocxVMerge::Continue), simple_cell(vec![])]),
        ];
        let (span_map, skip_set) = compute_v_merge_spans(&rows);

        assert_eq!(span_map[&(0, 0)], 3, "restart cell should have row_span=3");
        assert!(
            skip_set.contains(&(1, 0)),
            "row 1 continuation must be skipped"
        );
        assert!(
            skip_set.contains(&(2, 0)),
            "row 2 continuation must be skipped"
        );
    }

    /// No merge: table with no vMerge → all cells row_span=1, none removed.
    #[test]
    fn vmerge_no_merge() {
        let rows = vec![
            simple_row(vec![simple_cell(vec![]), simple_cell(vec![])]),
            simple_row(vec![simple_cell(vec![]), simple_cell(vec![])]),
        ];
        let (span_map, skip_set) = compute_v_merge_spans(&rows);

        assert!(span_map.is_empty(), "no spans expected");
        assert!(skip_set.is_empty(), "no cells to skip");
    }

    /// Multiple independent merges in different columns.
    #[test]
    fn vmerge_multiple_independent_merges() {
        // 3×2 table: col 0 merged rows 0-1, col 1 merged rows 1-2.
        let rows = vec![
            simple_row(vec![merge_cell(DocxVMerge::Restart), simple_cell(vec![])]),
            simple_row(vec![
                merge_cell(DocxVMerge::Continue),
                merge_cell(DocxVMerge::Restart),
            ]),
            simple_row(vec![simple_cell(vec![]), merge_cell(DocxVMerge::Continue)]),
        ];
        let (span_map, skip_set) = compute_v_merge_spans(&rows);

        assert_eq!(span_map[&(0, 0)], 2, "col-0 restart at row 0 → span 2");
        assert_eq!(span_map[&(1, 1)], 2, "col-1 restart at row 1 → span 2");
        assert!(
            skip_set.contains(&(1, 0)),
            "col-0 continuation (row 1) skipped"
        );
        assert!(
            skip_set.contains(&(2, 1)),
            "col-1 continuation (row 2) skipped"
        );
        assert!(!skip_set.contains(&(0, 1)), "col-1 row 0 is a plain cell");
        assert!(!skip_set.contains(&(2, 0)), "col-0 row 2 is a plain cell");
    }

    fn cell_with_props(tc_pr: DocxTcPr) -> DocxTableCell {
        DocxTableCell {
            tc_pr: Some(tc_pr),
            paragraphs: vec![],
        }
    }

    #[test]
    fn cell_padding_maps_to_points() {
        use crate::docx::model::styles::DocxCellMargins;
        use loki_primitives::units::Points;

        let styles = StyleCatalog::default();
        let (fn_m, en_m, hl_m, img_m) = (
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
        );
        let opts = DocxImportOptions::default();
        let mut ctx = make_ctx(&styles, &fn_m, &en_m, &hl_m, &img_m, &opts);

        let tc = cell_with_props(DocxTcPr {
            tc_margins: Some(DocxCellMargins {
                top: Some(100),    // 5pt
                bottom: Some(200), // 10pt
                left: Some(300),   // 15pt
                right: Some(400),  // 20pt
            }),
            ..Default::default()
        });
        let cell = map_cell(&tc, &mut ctx);
        assert_eq!(cell.props.padding_top, Some(Points::new(5.0)));
        assert_eq!(cell.props.padding_bottom, Some(Points::new(10.0)));
        assert_eq!(cell.props.padding_left, Some(Points::new(15.0)));
        assert_eq!(cell.props.padding_right, Some(Points::new(20.0)));
    }

    #[test]
    fn cell_valign_maps_correctly() {
        use crate::docx::model::styles::DocxVAlign;
        use loki_doc_model::content::table::row::CellVerticalAlign;

        let styles = StyleCatalog::default();
        let (fn_m, en_m, hl_m, img_m) = (
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
        );
        let opts = DocxImportOptions::default();
        let mut ctx = make_ctx(&styles, &fn_m, &en_m, &hl_m, &img_m, &opts);

        for (docx_val, expected) in [
            (DocxVAlign::Top, CellVerticalAlign::Top),
            (DocxVAlign::Center, CellVerticalAlign::Middle),
            (DocxVAlign::Bottom, CellVerticalAlign::Bottom),
        ] {
            let tc = cell_with_props(DocxTcPr {
                v_align: Some(docx_val),
                ..Default::default()
            });
            let cell = map_cell(&tc, &mut ctx);
            assert_eq!(cell.props.vertical_align, Some(expected));
        }
    }

    #[test]
    fn cell_text_direction_maps_correctly() {
        use crate::docx::model::styles::DocxTextDirection;
        use loki_doc_model::content::table::row::CellTextDirection;

        let styles = StyleCatalog::default();
        let (fn_m, en_m, hl_m, img_m) = (
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
        );
        let opts = DocxImportOptions::default();
        let mut ctx = make_ctx(&styles, &fn_m, &en_m, &hl_m, &img_m, &opts);

        for (docx_val, expected) in [
            (DocxTextDirection::LrTb, CellTextDirection::LrTb),
            (DocxTextDirection::TbRl, CellTextDirection::TbRl),
            (DocxTextDirection::TbLr, CellTextDirection::TbLr),
            (DocxTextDirection::BtLr, CellTextDirection::BtLr),
        ] {
            let tc = cell_with_props(DocxTcPr {
                text_direction: Some(docx_val),
                ..Default::default()
            });
            let cell = map_cell(&tc, &mut ctx);
            assert_eq!(cell.props.text_direction, Some(expected));
        }
    }

    /// col_span + vMerge: a restart cell with col_span=2 spans two grid columns.
    #[test]
    fn vmerge_with_col_span() {
        // 2×1 logical table: row 0 has a 2-wide restart, row 1 has a 2-wide continuation.
        let rows = vec![
            simple_row(vec![merge_cell_with_col_span(DocxVMerge::Restart, 2)]),
            simple_row(vec![merge_cell_with_col_span(DocxVMerge::Continue, 2)]),
        ];
        let (span_map, skip_set) = compute_v_merge_spans(&rows);

        // Grid col 0 (first of the two expanded columns) holds the span.
        assert_eq!(
            span_map[&(0, 0)],
            2,
            "wide restart cell should have row_span=2"
        );
        assert!(
            skip_set.contains(&(1, 0)),
            "wide continuation cell must be skipped"
        );
    }
}
