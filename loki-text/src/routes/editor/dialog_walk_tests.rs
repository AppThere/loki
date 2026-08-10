// SPDX-License-Identifier: Apache-2.0

//! Tests for the shared document walk.
//!
//! Both structures pinned here were missed by all four hand-written walkers
//! this module replaced.

use super::*;
use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::StyledParagraph;

fn para(text: &str) -> Block {
    Block::Para(vec![Inline::Str(text.to_string())])
}

/// A 1x1 table whose single cell says `text`, built through the public
/// constructor so the test cannot drift from the model's own shape.
fn one_cell_table(text: &str) -> Table {
    let mut t = Table::grid(1, 1);
    if let Some(body) = t.bodies.first_mut()
        && let Some(row) = body.body_rows.first_mut()
        && let Some(cell) = row.cells.first_mut()
    {
        cell.blocks = vec![para(text)];
    }
    t
}

/// Moves `table`'s only body row into its head, the way `build_table` does.
fn promote_head(table: &mut Table) {
    if let Some(body) = table.bodies.first_mut()
        && !body.body_rows.is_empty()
    {
        let head = body.body_rows.remove(0);
        table.head.rows.push(head);
    }
}

fn texts(blocks: &[Block]) -> Vec<String> {
    let mut out = Vec::new();
    visit_blocks(blocks, &mut |b| {
        for inlines in block_inlines(b) {
            for i in inlines {
                if let Inline::Str(s) = i {
                    out.push(s.clone());
                }
            }
        }
    });
    out
}

/// The header row is where `build_table` puts the first row, so a walk that
/// visits only `bodies` cannot see the one structure the Insert table dialog
/// creates by default.
#[test]
fn a_table_header_row_is_walked() {
    // Exactly what the Insert table dialog produces: the first row promoted
    // out of the body into `head`.
    let mut table = one_cell_table("heading");
    promote_head(&mut table);
    let mut body = one_cell_table("body");
    table.bodies = std::mem::take(&mut body.bodies);
    let foot = one_cell_table("footer");
    table.foot.rows = foot.bodies.into_iter().flat_map(|b| b.body_rows).collect();
    table.caption.full = vec![Inline::Str("caption".to_string())];

    let found = texts(&[Block::Table(Box::new(table))]);

    for expected in ["heading", "body", "footer", "caption"] {
        assert!(
            found.contains(&expected.to_string()),
            "missing {expected}: {found:?}"
        );
    }
}

/// A styled paragraph is still a paragraph. Handling only `Block::Para` made
/// the statistics tab report zero paragraphs for a styled document.
#[test]
fn a_styled_paragraph_is_walked_like_a_plain_one() {
    let styled = Block::StyledPara(StyledParagraph {
        style_id: None,
        direct_para_props: None,
        direct_char_props: None,
        inlines: vec![Inline::Str("styled".to_string())],
        attr: NodeAttr::default(),
    });

    assert_eq!(texts(&[styled]), vec!["styled".to_string()]);
}

/// Blocks reachable only through an inline — footnote bodies and text boxes —
/// are part of the document and must be visited too.
#[test]
fn blocks_inside_notes_and_text_boxes_are_walked() {
    let boxed = Block::Para(vec![Inline::TextBox(
        NodeAttr::default(),
        vec![para("in a box")],
    )]);

    assert!(texts(&[boxed]).contains(&"in a box".to_string()));
}

/// Inline nesting the old walkers stopped short of: an image inside an
/// underlined run inside a link is still an image.
#[test]
fn deeply_nested_inlines_are_visited() {
    let nested = vec![Inline::Underline(vec![Inline::Strikeout(vec![
        Inline::Str("deep".to_string()),
    ])])];

    let mut found = Vec::new();
    visit_inlines(&nested, &mut |i| {
        if let Inline::Str(s) = i {
            found.push(s.clone());
        }
    });

    assert_eq!(found, vec!["deep".to_string()]);
}
