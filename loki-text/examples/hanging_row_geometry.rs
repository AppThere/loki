// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! **The boxes and the breaks, as numbers** — ADR-0017 §5.9's one-pixel residual.
//!
//! §5.9 left the DOM path breaking as though its column were one CSS pixel wider
//! than the canvas path's, at every swept width where the two differ. Every
//! instrument up to here reads a *screenshot*: it can say a paragraph took one
//! line too many, not what width Taffy gave the box it wrapped in.
//!
//! This lays the tree out through the same engine the app renders with — the
//! vendored `blitz-dom`, driven headlessly by `blitz-html` — and prints each
//! box's resolved geometry and each line's advance and text. No window, no
//! Xvfb, no pixels.
//!
//! # What it renders, and against what
//!
//! Two paragraphs carrying **the same text at the same intended measure**:
//!
//! 1. the hanging-marker **row** the list path emits ([`style::hanging_row_css`]
//!    and friends), and
//! 2. a **control**: one plain `<p>` whose indent is a `padding-inline-start`,
//!    which is what every non-list paragraph gets — and non-list fixtures agree
//!    with the canvas path at every swept width.
//!
//! So the two shapes bracket the suspect. If the row's `<p>` breaks where the
//! control's does, the flex row is not the cause and the residual is in what
//! both share (parley 0.6's line breaking against parley 0.10's). If they break
//! differently, it is the row, and the printed box widths say by how much.
//!
//! Then the same paragraph twice more, through `loki_layout::layout_paragraph`:
//! once as the flow lays it out, and once **with the hanging indent and the
//! marker taken away**. The second is the inversion — the disagreement must be
//! absent there, or the first line's budget is not the whole story.
//!
//! # The doctype is load-bearing
//!
//! Without it html5ever reports a parse error and the document is in quirks
//! mode. The measurement came out the same either way, which is worth knowing
//! and not worth relying on: a probe that prints an unexplained error is a
//! probe nobody trusts.
//!
//! The CSS comes from the view's own emitters rather than being written out
//! here: a probe with its own copy would agree with the view exactly as far as
//! the copy did.
//!
//! ```text
//! LB_FIXTURE=lists LB_WIDTH=278 cargo run -p loki-text \
//!     --example hanging_row_geometry
//! ```

use blitz_dom::{DocumentConfig, Node};
use blitz_html::HtmlDocument;
use blitz_traits::shell::{ColorScheme, Viewport};
use loki_layout::{FontResources, SharedFontResources};
use loki_text::routes::editor::dom_reflow;

// The document under test, shared with both halves of the comparison.
#[path = "common/fixture.rs"]
mod fixture;

/// CSS px → points, the ratio Blitz resolves `pt` at.
const PX_TO_PT: f32 = 72.0 / 96.0;

fn main() {
    let width_px: f32 = std::env::var("LB_WIDTH")
        .ok()
        .and_then(|w| w.parse().ok())
        .unwrap_or(278.0);
    let (name, doc) = fixture::from_env();
    let fonts = SharedFontResources::new_ready(FontResources::new());
    let families = dom_reflow::resolve_families(&fonts, &doc);
    println!(
        "fixture {name} at {width_px} px ({:.2} pt)",
        width_px * PX_TO_PT
    );

    let Some(item) = first_list_item(&doc) else {
        eprintln!("this fixture has no list item to measure");
        std::process::exit(1);
    };
    // The item's own indent, in points: `NESTED_INDENT_PT` for a top-level item
    // and twice that for one nested inside another list. It is a knob because
    // the two levels are different measures, and §5.9's disagreement at 278 px
    // was the *nested* item — measuring only the top-level one would have said
    // the paths agree.
    let list_indent: f32 = std::env::var("LB_INDENT")
        .ok()
        .and_then(|i| i.parse().ok())
        .unwrap_or(loki_layout::flow::NESTED_INDENT_PT);
    let marker = loki_layout::flow::list_marker(None, 0);
    let para = loki_layout::flow::synthesize_list_item_para(item, &marker, list_indent);
    let resolved = loki_layout::resolve::resolve_para_props(&para, &doc.styles);
    let mut notes = 0u32;
    let (text, spans, ..) = loki_layout::resolve::flatten_paragraph_with_base(
        &para,
        &doc.styles,
        &mut notes,
        None,
        loki_layout::RevisionDisplay::default(),
    );
    let Some(span) = spans.first() else {
        eprintln!("the item resolved to no runs");
        std::process::exit(1);
    };
    let run_css = dom_reflow::style::span_css(span, &families);
    let features = dom_reflow::style::span_font_features(span);
    // The marker split off the front, exactly as `content_para::split_runs`
    // does it — the tab is the canvas path's tab stop and is not text here.
    let (marker_text, body_text) = text.split_at(marker.len());
    let marker_text = marker_text.trim_end_matches('\t');

    // The row, then the control. `indent_start` stays on the control and its
    // hanging goes to zero, so both carry the same measure by different means.
    let mut control = resolved.clone();
    control.indent_hanging = 0.0;
    control.space_before = 0.0;
    control.space_after = 0.0;
    let html = format!(
        "<!DOCTYPE html><html><body style=\"margin: 0;\"><div style=\"width: {width_px}px;\">\
           <div id=\"row\" style=\"{row}\">\
             <div id=\"cell\" style=\"{cell}\"><span style=\"{run_css}\" \
               data-font-features='{features}'>{marker_text}</span></div>\
             <div id=\"body\" style=\"{body}\">\
               <p id=\"rowp\" style=\"{inner}\"><span style=\"{run_css}\" \
                 data-font-features='{features}'>{body_text}</span></p>\
             </div>\
           </div>\
           <p id=\"ctrl\" style=\"{ctrl}\"><span style=\"{run_css}\" \
             data-font-features='{features}'>{body_text}</span></p>\
         </div></body></html>",
        row = dom_reflow::style::hanging_row_css(&resolved),
        cell = dom_reflow::style::hanging_marker_css(&resolved),
        body = dom_reflow::style::HANGING_BODY_CSS,
        inner =
            dom_reflow::style::resolved_para_css(&dom_reflow::style::hanging_body_props(&resolved)),
        ctrl = dom_reflow::style::resolved_para_css(&control),
    );

    if std::env::var("LB_DUMP_HTML").is_ok() {
        println!("{html}");
    }

    let mut document = HtmlDocument::from_html(
        &html,
        DocumentConfig {
            viewport: Some(Viewport::new(
                // Wide enough that the viewport never binds: the inner div's
                // own `width` is the measure under test, and a viewport that
                // clipped it would be measuring this line instead.
                (width_px as u32) + 400,
                4000,
                1.0,
                ColorScheme::Light,
            )),
            // **The same registration `main.rs` does.** Without it every family
            // resolves to Blitz's default face and the breaks are the fallback's
            // (ADR-0017 §5.3).
            extra_fonts: loki_fonts::ui_font_blobs(),
            ..DocumentConfig::default()
        },
    );
    document.resolve(0.0);

    for id in ["row", "cell", "body", "rowp", "ctrl"] {
        report(&document, id);
    }

    canvas_report("canvas", &text, &spans, &resolved, width_px);

    // **The inversion.** The same text at the same measure with the hanging
    // indent taken away — and with it the marker, whose tab is only a tab stop
    // because of the hanging indent. If the engines still disagree here, the
    // first line's budget is not the whole story and the attribution above is
    // incomplete; if they agree line for line, the hanging indent is the only
    // thing between them.
    //
    // The spans are re-resolved rather than re-ranged: dropping the marker
    // moves every byte, and a span range left pointing at the old text would
    // shape a different string than the one measured.
    let mut flat = resolved.clone();
    flat.indent_hanging = 0.0;
    let mut notes = 0u32;
    let (plain_text, plain_spans, ..) = loki_layout::resolve::flatten_paragraph_with_base(
        item,
        &doc.styles,
        &mut notes,
        None,
        loki_layout::RevisionDisplay::default(),
    );
    canvas_report(
        "canvas (no hanging, no marker)",
        &plain_text,
        &plain_spans,
        &flat,
        width_px,
    );
}

/// The **canvas** path's answer for the same paragraph at the same column, so
/// the two engines are read off one page instead of two runs.
///
/// `layout_paragraph` is what `flow_para` calls, given the same resolved props
/// and the same `available_width` the flow would pass — the full column, with
/// the indents inside the props where the flow leaves them.
fn canvas_report(
    label: &str,
    text: &str,
    spans: &[loki_layout::para::StyleSpan],
    resolved: &loki_layout::para::ResolvedParaProps,
    width_px: f32,
) {
    let mut fonts = FontResources::new();
    let laid = loki_layout::layout_paragraph(
        &mut fonts,
        text,
        spans,
        resolved,
        width_px * PX_TO_PT,
        1.0,
        true,
    );
    let Some(parley) = laid.parley_layout.as_ref() else {
        println!("\n{label}: no retained Parley layout");
        return;
    };
    let continuation = (width_px * PX_TO_PT - resolved.indent_start - resolved.indent_end).max(0.0);
    println!(
        "\n{label}: available={:.4} pt ({:.4} px), indent_start={} hanging={}",
        width_px * PX_TO_PT,
        width_px,
        resolved.indent_start,
        resolved.indent_hanging,
    );
    // The budget the first line got against the one it is entitled to. A hanging
    // paragraph's first line starts `indent_hanging` further left, so it has that
    // much *more* room — `NOTE(indent-hanging-width)` in `para.rs` gives it the
    // same width as the rest instead. This is the residual: the marker then eats
    // the first line's text budget rather than the hanging space.
    println!(
        "  first line budget: {:.3} pt given, {:.3} pt entitled ({:+.3})",
        continuation,
        continuation + resolved.indent_hanging,
        resolved.indent_hanging,
    );
    for (i, line) in parley.lines().enumerate() {
        let m = line.metrics();
        let range = line.text_range();
        let slice = text.get(range.clone()).unwrap_or("<not a boundary>");
        println!(
            "  {i:>2} [{:>4}..{:>4}] advance={:>8.3} trailing_ws={:>6.3}  {slice:?}",
            range.start, range.end, m.advance, m.trailing_whitespace,
        );
    }
}

/// Prints one element's resolved box and, if it holds text, its lines.
fn report(document: &HtmlDocument, id: &str) {
    let Some(node) = find_by_id(document, document.root_element(), id) else {
        println!("\n#{id}: not found");
        return;
    };
    // Both layouts, because the question is whether rounding is involved: Taffy
    // lays out in floats and then rounds boxes to whole pixels, and text is
    // measured against the unrounded width.
    println!(
        "\n#{id}: x={:.4} w={:.4}  (unrounded x={:.4} w={:.4})",
        node.final_layout.location.x,
        node.final_layout.size.width,
        node.unrounded_layout.location.x,
        node.unrounded_layout.size.width,
    );
    let Some(inline) = node
        .element_data()
        .and_then(|el| el.inline_layout_data.as_deref())
    else {
        return;
    };
    println!("  parley width={:.4}", inline.layout.width());
    for (i, line) in inline.layout.lines().enumerate() {
        let m = line.metrics();
        let range = line.text_range();
        let slice = inline.text.get(range.clone()).unwrap_or("<not a boundary>");
        println!(
            "  {i:>2} [{:>4}..{:>4}] advance={:>8.3} trailing_ws={:>6.3}  {slice:?}",
            range.start, range.end, m.advance, m.trailing_whitespace,
        );
    }
}

/// The node carrying `id`, by a walk from the root rather than a selector match
/// — `BaseDocument`'s node arena is private, and a walk needs no more than the
/// public `children`.
fn find_by_id<'a>(document: &'a HtmlDocument, node: &'a Node, id: &str) -> Option<&'a Node> {
    if node
        .element_data()
        .and_then(|el| el.attr(blitz_dom::local_name!("id")))
        == Some(id)
    {
        return Some(node);
    }
    node.children.iter().find_map(|child| {
        let child = document.get_node(*child)?;
        find_by_id(document, child, id)
    })
}

/// The first list item's first paragraph in `doc`.
fn first_list_item(
    doc: &loki_doc_model::Document,
) -> Option<&loki_doc_model::content::block::StyledParagraph> {
    use loki_doc_model::content::block::Block;
    doc.sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .find_map(|block| match block {
            Block::OrderedList(_, items) | Block::BulletList(items) => {
                items.first().and_then(|item| match item.first() {
                    Some(Block::StyledPara(p)) => Some(p),
                    _ => None,
                })
            }
            _ => None,
        })
}
