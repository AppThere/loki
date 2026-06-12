// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Recursive inline tree walker: flattens inline content into text + style spans.

use loki_doc_model::content::inline::Inline;
use loki_doc_model::style::catalog::StyleCatalog;
use loki_doc_model::style::props::char_props::{
    CharProps, StrikethroughStyle as DocStrikethroughStyle, UnderlineStyle as DocUnderlineStyle,
    VerticalAlign as DocVerticalAlign,
};

use crate::para::StyleSpan;

use super::char_props::{effective_run_char_props, push_text};
use super::inline_types::{
    field_display_text, superscript_mark, CollectedImage, CollectedNote,
};

/// Recursively collect text from an [`Inline`] tree, building `buf` + `spans`.
///
/// `active_link_url` carries the URL of the enclosing `Inline::Link`, if any;
/// it is threaded through recursive calls so all text inside a link gets
/// `StyleSpan::link_url` set. `images` collects any `Inline::Image` nodes
/// encountered for post-Parley placement (gap #9). `notes` collects footnotes
/// and endnotes; `note_counter` is incremented for each note (gap #2).
#[allow(clippy::too_many_arguments)]
pub(crate) fn walk_inlines(
    inlines: &[Inline],
    effective: &CharProps,
    catalog: &StyleCatalog,
    buf: &mut String,
    spans: &mut Vec<StyleSpan>,
    active_link_url: Option<&str>,
    images: &mut Vec<CollectedImage>,
    note_counter: &mut u32,
    notes: &mut Vec<CollectedNote>,
) {
    for inline in inlines {
        match inline {
            Inline::Str(s) => push_text(buf, spans, s, effective, active_link_url),
            Inline::Space => push_text(buf, spans, " ", effective, active_link_url),
            Inline::SoftBreak => push_text(buf, spans, " ", effective, active_link_url),
            Inline::LineBreak => push_text(buf, spans, "\n", effective, active_link_url),
            Inline::Code(_, s) => push_text(buf, spans, s, effective, active_link_url),
            Inline::StyledRun(run) => {
                let p = effective_run_char_props(run, catalog, effective);
                walk_inlines(
                    &run.content,
                    &p,
                    catalog,
                    buf,
                    spans,
                    active_link_url,
                    images,
                    note_counter,
                    notes,
                );
            }
            Inline::Strong(ch) => {
                let mut p = effective.clone();
                p.bold = Some(true);
                walk_inlines(
                    ch,
                    &p,
                    catalog,
                    buf,
                    spans,
                    active_link_url,
                    images,
                    note_counter,
                    notes,
                );
            }
            Inline::Emph(ch) => {
                let mut p = effective.clone();
                p.italic = Some(true);
                walk_inlines(
                    ch,
                    &p,
                    catalog,
                    buf,
                    spans,
                    active_link_url,
                    images,
                    note_counter,
                    notes,
                );
            }
            Inline::Underline(ch) => {
                let mut p = effective.clone();
                p.underline = Some(DocUnderlineStyle::Single);
                walk_inlines(
                    ch,
                    &p,
                    catalog,
                    buf,
                    spans,
                    active_link_url,
                    images,
                    note_counter,
                    notes,
                );
            }
            Inline::Strikeout(ch) => {
                let mut p = effective.clone();
                p.strikethrough = Some(DocStrikethroughStyle::Single);
                walk_inlines(
                    ch,
                    &p,
                    catalog,
                    buf,
                    spans,
                    active_link_url,
                    images,
                    note_counter,
                    notes,
                );
            }
            // Superscript (gap #3): set vertical_align on the effective props.
            Inline::Superscript(ch) => {
                let mut p = effective.clone();
                p.vertical_align = Some(DocVerticalAlign::Superscript);
                walk_inlines(
                    ch,
                    &p,
                    catalog,
                    buf,
                    spans,
                    active_link_url,
                    images,
                    note_counter,
                    notes,
                );
            }
            // Subscript (gap #3): set vertical_align on the effective props.
            Inline::Subscript(ch) => {
                let mut p = effective.clone();
                p.vertical_align = Some(DocVerticalAlign::Subscript);
                walk_inlines(
                    ch,
                    &p,
                    catalog,
                    buf,
                    spans,
                    active_link_url,
                    images,
                    note_counter,
                    notes,
                );
            }
            // SmallCaps (gap #15): set small_caps so StyleSpan gets FontVariant::SmallCaps.
            Inline::SmallCaps(ch) => {
                let mut p = effective.clone();
                p.small_caps = Some(true);
                walk_inlines(
                    ch,
                    &p,
                    catalog,
                    buf,
                    spans,
                    active_link_url,
                    images,
                    note_counter,
                    notes,
                );
            }
            Inline::Quoted(_, ch) | Inline::Span(_, ch) => {
                walk_inlines(
                    ch,
                    effective,
                    catalog,
                    buf,
                    spans,
                    active_link_url,
                    images,
                    note_counter,
                    notes,
                );
            }
            // Link (gap #11): thread the resolved URL into child spans.
            // TODO(link-click): interactive hit-testing deferred; only visual hint rendered.
            Inline::Link(_, ch, target) => {
                let url = target.url.as_str();
                walk_inlines(
                    ch,
                    effective,
                    catalog,
                    buf,
                    spans,
                    Some(url),
                    images,
                    note_counter,
                    notes,
                );
            }
            // Image (gap #9): collect for post-Parley placement; do not emit text.
            // TODO(floating-image): check NodeAttr.classes for "floating"; deferred (gap #12).
            // TODO(inline-image-flow): Parley has no inline box support; images placed
            //   as block-level prefix after layout_paragraph returns.
            Inline::Image(attr, alt_inlines, target) => {
                let cx_emu = attr
                    .kv
                    .iter()
                    .find(|(k, _)| k == "cx_emu")
                    .and_then(|(_, v)| v.parse::<u64>().ok())
                    .unwrap_or(0);
                let cy_emu = attr
                    .kv
                    .iter()
                    .find(|(k, _)| k == "cy_emu")
                    .and_then(|(_, v)| v.parse::<u64>().ok())
                    .unwrap_or(0);
                // Flatten alt-text inlines into a plain string (no spans, not main text).
                let mut alt_buf = String::new();
                let mut alt_spans: Vec<StyleSpan> = Vec::new();
                let mut alt_images: Vec<CollectedImage> = Vec::new();
                let mut dummy_counter = 0u32;
                let mut dummy_notes: Vec<CollectedNote> = Vec::new();
                walk_inlines(
                    alt_inlines,
                    effective,
                    catalog,
                    &mut alt_buf,
                    &mut alt_spans,
                    None,
                    &mut alt_images,
                    &mut dummy_counter,
                    &mut dummy_notes,
                );
                let alt = if alt_buf.is_empty() {
                    None
                } else {
                    Some(alt_buf)
                };
                if !target.url.is_empty() {
                    images.push(CollectedImage {
                        src: target.url.clone(),
                        alt,
                        cx_emu,
                        cy_emu,
                    });
                }
            }
            Inline::Cite(_, ch) => walk_inlines(
                ch,
                effective,
                catalog,
                buf,
                spans,
                active_link_url,
                images,
                note_counter,
                notes,
            ),
            // Field (gap #4): emit current_value snapshot, or a kind-based fallback.
            Inline::Field(f) => {
                let text = field_display_text(f);
                if !text.is_empty() {
                    push_text(buf, spans, &text, effective, active_link_url);
                }
            }
            // Note (gap #2): emit a superscript reference mark and collect the body.
            Inline::Note(kind, blocks) => {
                *note_counter += 1;
                let mark = superscript_mark(*note_counter);
                let mut mark_props = effective.clone();
                mark_props.vertical_align = Some(DocVerticalAlign::Superscript);
                push_text(buf, spans, &mark, &mark_props, active_link_url);
                notes.push(CollectedNote {
                    number: *note_counter,
                    kind: *kind,
                    blocks: blocks.clone(),
                });
            }
            // Math, RawInline, Comment, Bookmark, and any
            // future #[non_exhaustive] variants are not text runs — skip.
            _ => {}
        }
    }
}
