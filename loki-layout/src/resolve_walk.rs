// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The recursive inline tree walker (`walk_inlines`), split out of `resolve.rs`
//! for the 300-line ceiling. Builds the flattened `buf` + `spans` from an
//! `[Inline]` tree, threading link URLs and toggling the effective `CharProps`
//! in place. Leaf/text emission and image collection live in the sibling
//! `inlines` submodule; char-span helpers in `char_span`.

use loki_doc_model::content::inline::Inline;
use loki_doc_model::style::catalog::StyleCatalog;
use loki_doc_model::style::props::char_props::{
    CharProps, StrikethroughStyle as DocStrikethroughStyle, UnderlineStyle as DocUnderlineStyle,
    VerticalAlign as DocVerticalAlign,
};

use crate::para::StyleSpan;

use super::char_span::{char_props_to_style_span, effective_run_char_props};
use super::inlines::{collect_inline_image, field_display_text, push_text, superscript_mark};
use super::{CollectedImage, CollectedNote};

/// Recursively collect text from an [`Inline`] tree, building `buf` + `spans`.
///
/// `active_link_url` carries the URL of the enclosing `Inline::Link`, if any;
/// it is threaded through recursive calls so all text inside a link gets
/// `StyleSpan::link_url` set. `images` collects any `Inline::Image` nodes
/// encountered for post-Parley placement (gap #9). `notes` collects footnotes
/// and endnotes; `note_counter` is incremented for each note (gap #2).
#[allow(clippy::too_many_arguments)]
pub(super) fn walk_inlines(
    inlines: &[Inline],
    effective: &mut CharProps,
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
                let mut p = effective_run_char_props(run, catalog, effective);
                walk_inlines(
                    &run.content,
                    &mut p,
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
                // Toggle the single flag in place and restore it after recursing,
                // instead of cloning the whole CharProps (which heap-allocates its
                // font-name Strings) for every formatting span.
                let prev = effective.bold.replace(true);
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
                effective.bold = prev;
            }
            Inline::Emph(ch) => {
                let prev = effective.italic.replace(true);
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
                effective.italic = prev;
            }
            Inline::Underline(ch) => {
                let prev = effective.underline.replace(DocUnderlineStyle::Single);
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
                effective.underline = prev;
            }
            Inline::Strikeout(ch) => {
                let prev = effective
                    .strikethrough
                    .replace(DocStrikethroughStyle::Single);
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
                effective.strikethrough = prev;
            }
            // Superscript (gap #3): set vertical_align on the effective props.
            Inline::Superscript(ch) => {
                let prev = effective
                    .vertical_align
                    .replace(DocVerticalAlign::Superscript);
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
                effective.vertical_align = prev;
            }
            // Subscript (gap #3): set vertical_align on the effective props.
            Inline::Subscript(ch) => {
                let prev = effective
                    .vertical_align
                    .replace(DocVerticalAlign::Subscript);
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
                effective.vertical_align = prev;
            }
            // SmallCaps (gap #15): set small_caps so StyleSpan gets FontVariant::SmallCaps.
            Inline::SmallCaps(ch) => {
                let prev = effective.small_caps.replace(true);
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
                effective.small_caps = prev;
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
            // Link (gap #11): thread the resolved URL into child spans (hint +
            // hit-test + Ctrl/Cmd+click open all ride on it; feature 5.11).
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
            // TODO(inline-image-flow): Parley has no inline box support; images placed
            //   as block-level prefix after layout_paragraph returns.
            // Image (gap #9): collect for post-Parley placement; do not emit text.
            Inline::Image(attr, alt_inlines, target) => {
                collect_inline_image(attr, alt_inlines, target, effective, catalog, images);
            }
            // Floating text box: collect like an image (post-Parley placement),
            // carrying its interior blocks + fill/border; emits no inline text.
            Inline::TextBox(attr, blocks) => {
                super::inlines::collect_textbox(attr, blocks, images);
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
                let prev = effective
                    .vertical_align
                    .replace(DocVerticalAlign::Superscript);
                push_text(buf, spans, &mark, effective, active_link_url);
                effective.vertical_align = prev;
                notes.push(CollectedNote {
                    number: *note_counter,
                    kind: *kind,
                    blocks: blocks.clone(),
                    // Set by `flow_paragraph` after collection.
                    owner_block_index: 0,
                    note_in_block: 0,
                });
            }
            // Math (gap): record an empty-range placeholder span carrying the
            // MathML; `layout_paragraph` typesets it and places it inline via a
            // Parley inline box. No text is emitted into `buf`.
            Inline::Math(_, mathml) => {
                let at = buf.len();
                let mut span = char_props_to_style_span(effective, at..at);
                span.math = Some(std::sync::Arc::from(mathml.as_str()));
                spans.push(span);
            }
            // RawInline, Comment, Bookmark, and any future #[non_exhaustive]
            // variants are not text runs — skip.
            _ => {}
        }
    }
}
