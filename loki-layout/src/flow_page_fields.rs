// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! PAGE / NUMPAGES field detection and substitution for headers/footers.
//! Split out of `flow.rs` (Phase 7.1); pure block/inline traversal with no
//! `FlowState`. The header/footer *layout* orchestrator (`assign_headers_footers`)
//! stays in `flow.rs` and calls these via the `page_fields` submodule.

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::layout::page::PageLayout;

/// Visit every inline vector reachable from `blocks` (paragraphs, headings,
/// list items, table cells, nested containers), calling `f` on each.
///
/// Shared traversal for page-field detection and substitution in
/// headers/footers.
fn visit_inline_vecs_mut(blocks: &mut [Block], f: &mut impl FnMut(&mut Vec<Inline>)) {
    use loki_doc_model::content::table::core::Table;

    fn visit_table(table: &mut Table, f: &mut impl FnMut(&mut Vec<Inline>)) {
        let rows = table
            .head
            .rows
            .iter_mut()
            .chain(table.foot.rows.iter_mut())
            .chain(
                table
                    .bodies
                    .iter_mut()
                    .flat_map(|b| b.head_rows.iter_mut().chain(b.body_rows.iter_mut())),
            );
        for row in rows {
            for cell in &mut row.cells {
                visit_inline_vecs_mut(&mut cell.blocks, f);
            }
        }
    }

    for block in blocks {
        match block {
            Block::Plain(inlines) | Block::Para(inlines) | Block::Heading(_, _, inlines) => {
                f(inlines)
            }
            Block::StyledPara(p) => f(&mut p.inlines),
            Block::LineBlock(lines) => {
                for line in lines {
                    f(line);
                }
            }
            Block::BlockQuote(ch) | Block::Div(_, ch) | Block::Figure(_, _, ch) => {
                visit_inline_vecs_mut(ch, f)
            }
            Block::OrderedList(_, items) | Block::BulletList(items) => {
                for item in items {
                    visit_inline_vecs_mut(item, f);
                }
            }
            Block::Table(table) => visit_table(table, f),
            _ => {}
        }
    }
}

/// `true` when any inline reachable from `inlines` is a PAGE or NUMPAGES
/// field.
fn inlines_contain_page_field(inlines: &[Inline]) -> bool {
    use loki_doc_model::content::field::types::FieldKind;
    inlines.iter().any(|inline| match inline {
        Inline::Field(field) => {
            matches!(field.kind, FieldKind::PageNumber | FieldKind::PageCount)
        }
        Inline::Strong(ch)
        | Inline::Emph(ch)
        | Inline::Underline(ch)
        | Inline::Strikeout(ch)
        | Inline::Superscript(ch)
        | Inline::Subscript(ch)
        | Inline::SmallCaps(ch)
        | Inline::Quoted(_, ch)
        | Inline::Span(_, ch)
        | Inline::Cite(_, ch) => inlines_contain_page_field(ch),
        Inline::Link(_, ch, _) => inlines_contain_page_field(ch),
        Inline::StyledRun(run) => inlines_contain_page_field(&run.content),
        _ => false,
    })
}

/// `true` when any of `pl`'s header/footer variants contains a PAGE / NUMPAGES
/// field. Used by incremental relayout: when a header references the page count,
/// a page-count change invalidates the headers on *reused* pages too, so the
/// fast path must re-run the header pass over all pages in that case.
pub(crate) fn page_layout_has_page_fields(pl: &PageLayout) -> bool {
    [
        &pl.header,
        &pl.header_first,
        &pl.header_even,
        &pl.footer,
        &pl.footer_first,
        &pl.footer_even,
    ]
    .into_iter()
    .flatten()
    .any(|hf| blocks_contain_page_field(&hf.blocks))
}

/// `true` when any inline in `blocks` is a PAGE or NUMPAGES field, in which
/// case the header/footer must be laid out per page rather than once.
pub(super) fn blocks_contain_page_field(blocks: &[Block]) -> bool {
    use loki_doc_model::content::table::core::Table;

    fn table_contains(table: &Table) -> bool {
        let rows = table.head.rows.iter().chain(table.foot.rows.iter()).chain(
            table
                .bodies
                .iter()
                .flat_map(|b| b.head_rows.iter().chain(b.body_rows.iter())),
        );
        rows.into_iter().any(|row| {
            row.cells
                .iter()
                .any(|c| blocks_contain_page_field(&c.blocks))
        })
    }

    blocks.iter().any(|block| match block {
        Block::Plain(i) | Block::Para(i) | Block::Heading(_, _, i) => inlines_contain_page_field(i),
        Block::StyledPara(p) => inlines_contain_page_field(&p.inlines),
        Block::LineBlock(lines) => lines.iter().any(|l| inlines_contain_page_field(l)),
        Block::BlockQuote(ch) | Block::Div(_, ch) | Block::Figure(_, _, ch) => {
            blocks_contain_page_field(ch)
        }
        Block::OrderedList(_, items) | Block::BulletList(items) => {
            items.iter().any(|i| blocks_contain_page_field(i))
        }
        Block::Table(table) => table_contains(table),
        _ => false,
    })
}

/// Replace every PAGE / NUMPAGES field reachable from `blocks` with a plain
/// text inline carrying its resolved value from `ctx`.
pub(super) fn substitute_page_fields(blocks: &mut [Block], ctx: &crate::FieldContext) {
    use loki_doc_model::content::field::types::FieldKind;

    fn substitute_inlines(inlines: &mut [Inline], ctx: &crate::FieldContext) {
        for inline in inlines.iter_mut() {
            match inline {
                Inline::Field(field) => {
                    let value = match field.kind {
                        // The PAGE field honours the section's number format
                        // (roman/alpha); NUMPAGES stays decimal.
                        FieldKind::PageNumber => Some(match ctx.number_format {
                            Some(scheme) => crate::para::format_counter(ctx.page_number, scheme),
                            None => ctx.page_number.to_string(),
                        }),
                        FieldKind::PageCount => Some(ctx.page_count.to_string()),
                        _ => None,
                    };
                    if let Some(v) = value {
                        *inline = Inline::Str(v);
                    }
                }
                Inline::Strong(ch)
                | Inline::Emph(ch)
                | Inline::Underline(ch)
                | Inline::Strikeout(ch)
                | Inline::Superscript(ch)
                | Inline::Subscript(ch)
                | Inline::SmallCaps(ch)
                | Inline::Quoted(_, ch)
                | Inline::Span(_, ch)
                | Inline::Cite(_, ch) => substitute_inlines(ch, ctx),
                Inline::Link(_, ch, _) => substitute_inlines(ch, ctx),
                Inline::StyledRun(run) => substitute_inlines(&mut run.content, ctx),
                _ => {}
            }
        }
    }

    visit_inline_vecs_mut(blocks, &mut |inlines| substitute_inlines(inlines, ctx));
}
