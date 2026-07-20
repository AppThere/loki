// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Inline flattening leaves, split out of `resolve.rs` for the 300-line
//! ceiling: the `flatten_paragraph_with_base` entry point, the text-emission
//! helpers (`push_text` / `push_small_caps` / `apply_link`), the field and
//! note-mark string builders, and `collect_inline_image`. The recursive tree
//! walk lives in the sibling `walk` submodule (`super::walk::walk_inlines`).

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::StyledParagraph;
use loki_doc_model::content::field::types::{Field, FieldKind};
use loki_doc_model::content::float::FloatWrap;
use loki_doc_model::content::inline::{Inline, LinkTarget};
use loki_doc_model::style::catalog::StyleCatalog;
use loki_doc_model::style::props::char_props::CharProps;

use crate::para::{StyleSpan, UnderlineStyle};

use super::char_span::char_props_to_style_span;
use super::walk::walk_inlines;
use super::{CollectedImage, CollectedNote};

/// [`flatten_paragraph`] with table-region character defaults (4a.3):
/// `region_base` is the cell's merged `w:tblStylePr/w:rPr`. Word precedence:
/// docDefaults < region < named styles < direct. The resolved chain folds
/// docDefaults in, so an *unstyled* paragraph (chain == docDefaults) merges
/// the region **over** it, a *styled* one **under** it (approximation for
/// the rare styled cell); direct formatting still wins in both.
pub fn flatten_paragraph_with_base(
    block: &StyledParagraph,
    catalog: &StyleCatalog,
    note_counter: &mut u32,
    region_base: Option<&CharProps>,
    revision_display: crate::options::RevisionDisplay,
) -> (
    String,
    Vec<StyleSpan>,
    Vec<CollectedImage>,
    Vec<CollectedNote>,
) {
    let base: CharProps = catalog
        .effective_paragraph_style(block.style_id.as_ref())
        .and_then(|id| catalog.resolve_char(id))
        .unwrap_or_default();
    let base = match region_base {
        Some(region) if block.style_id.is_none() => region.clone().merged_with_parent(&base),
        Some(region) => base.merged_with_parent(region),
        None => base,
    };
    let base = match &block.direct_char_props {
        Some(direct) => direct.as_ref().clone().merged_with_parent(&base),
        None => base,
    };
    // `walk_inlines` takes `&mut` and mutates this in place (restoring after each
    // styled run) to avoid cloning `CharProps` per formatting span; `base` is a
    // throwaway local, so the mutation is not observable outside this call.
    let mut base = base;
    // A tracked ¶-mark deletion on `direct_char_props` must not bleed onto the
    // runs: it belongs to the paragraph mark (struck ¶ marker), not the text.
    base.revision = None;
    let mut buf = String::new();
    let mut spans: Vec<StyleSpan> = Vec::new();
    let mut images: Vec<CollectedImage> = Vec::new();
    let mut notes: Vec<CollectedNote> = Vec::new();
    // Non-destructive tracked-change display: hide/normalise revision runs for
    // Final/Original modes (borrowed unchanged for All-Markup / no revisions).
    let inlines = crate::revision_filter::display_inlines(&block.inlines, revision_display);
    walk_inlines(
        &inlines,
        &mut base,
        catalog,
        &mut buf,
        &mut spans,
        None,
        &mut images,
        note_counter,
        &mut notes,
    );
    (buf, spans, images, notes)
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// Append `text` to `buf` and push a span; no-op for empty strings.
///
/// When `props.all_caps` is set, `text` is uppercased before appending
/// (gap #16 fallback — Parley has no `FontVariantCaps` property).
/// When `active_link_url` is `Some`, the span gets `link_url` set and an
/// auto-underline if not already underlined (gap #11).
#[inline]
pub(super) fn push_text(
    buf: &mut String,
    spans: &mut Vec<StyleSpan>,
    text: &str,
    props: &CharProps,
    active_link_url: Option<&str>,
) {
    if text.is_empty() {
        return;
    }

    // Small caps (gap #15): Parley exposes no `FontVariantCaps`, so synthesize
    // it — every letter is uppercased and the letters that were *lowercase* in
    // the source render at a reduced size, the classic small-caps look. This
    // splits `text` into case-homogeneous segments (handled below). `all_caps`,
    // if also set, takes precedence and uses the plain uppercasing path.
    if props.small_caps == Some(true) && props.all_caps != Some(true) {
        push_small_caps(buf, spans, text, props, active_link_url);
        return;
    }

    let start = buf.len();
    if props.all_caps == Some(true) {
        buf.push_str(&text.to_uppercase());
    } else {
        buf.push_str(text);
    }
    let mut span = char_props_to_style_span(props, start..buf.len());
    apply_link(&mut span, active_link_url);
    spans.push(span);
}

/// Fraction of the full cap size used for synthesized small capitals (letters
/// that were lowercase in the source). Matches the common ~0.8 synthetic ratio
/// applied when a font carries no real small-cap (`smcp`) glyphs.
const SMALL_CAPS_RATIO: f32 = 0.8;

/// Append `text` as synthesized small caps: uppercase every letter, splitting
/// into runs where the source was lowercase (rendered at [`SMALL_CAPS_RATIO`] of
/// the size) versus already-uppercase / non-letters (full size). Each run is its
/// own [`StyleSpan`] so Parley shapes it at the right size.
fn push_small_caps(
    buf: &mut String,
    spans: &mut Vec<StyleSpan>,
    text: &str,
    props: &CharProps,
    active_link_url: Option<&str>,
) {
    let mut chars = text.chars().peekable();
    while let Some(&first) = chars.peek() {
        let lower = first.is_lowercase();
        let start = buf.len();
        while let Some(&c) = chars.peek() {
            if c.is_lowercase() != lower {
                break;
            }
            // Uppercase the character (may expand, e.g. ß → SS).
            for u in c.to_uppercase() {
                buf.push(u);
            }
            chars.next();
        }
        let mut span = char_props_to_style_span(props, start..buf.len());
        if lower {
            span.font_size *= SMALL_CAPS_RATIO;
        }
        apply_link(&mut span, active_link_url);
        spans.push(span);
    }
}

/// Apply an active hyperlink URL to `span`, auto-underlining undecorated link
/// text (gap #11).
fn apply_link(span: &mut StyleSpan, active_link_url: Option<&str>) {
    if let Some(url) = active_link_url {
        span.link_url = Some(url.to_string());
        if span.underline.is_none() {
            span.underline = Some(UnderlineStyle::Single);
        }
    }
}

/// Return display text for a [`Field`]: use `current_value` if available,
/// otherwise fall back to a kind-specific placeholder.
pub(super) fn field_display_text(f: &Field) -> String {
    if let Some(ref v) = f.current_value {
        return v.clone();
    }
    match &f.kind {
        FieldKind::PageNumber => "1".to_string(),
        FieldKind::PageCount => "1".to_string(),
        FieldKind::Date { .. } => String::new(),
        FieldKind::Time { .. } => String::new(),
        FieldKind::Title | FieldKind::Author | FieldKind::Subject | FieldKind::FileName => {
            String::new()
        }
        FieldKind::WordCount => String::new(),
        FieldKind::CrossReference { .. } => String::new(),
        FieldKind::Raw { .. } => String::new(),
        _ => String::new(),
    }
}

/// Return the Unicode superscript string for note number `n`.
///
/// Uses Unicode superscript digits (U+00B9, U+00B2, U+00B3, U+2074–U+2079)
/// for n ≤ 9, and `[n]` for larger numbers.
pub(super) fn superscript_mark(n: u32) -> String {
    match n {
        1 => "\u{00B9}".to_string(),
        2 => "\u{00B2}".to_string(),
        3 => "\u{00B3}".to_string(),
        4 => "\u{2074}".to_string(),
        5 => "\u{2075}".to_string(),
        6 => "\u{2076}".to_string(),
        7 => "\u{2077}".to_string(),
        8 => "\u{2078}".to_string(),
        9 => "\u{2079}".to_string(),
        _ => format!("[{n}]"),
    }
}

/// Collect an `Inline::Image` for post-Parley placement (gap #9); emits no text.
///
/// Flattens the alt-text inlines into a plain string (no spans) and, when the
/// target URL is non-empty, pushes a [`CollectedImage`] carrying EMU dimensions
/// and the float-wrap configuration read from the image's `NodeAttr`.
pub(super) fn collect_inline_image(
    attr: &NodeAttr,
    alt_inlines: &[Inline],
    target: &LinkTarget,
    effective: &mut CharProps,
    catalog: &StyleCatalog,
    images: &mut Vec<CollectedImage>,
) {
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
            float: FloatWrap::read_or_class_default(attr),
            textbox: None,
        });
    }
}

/// Collect an `Inline::TextBox` (a floating `wps` text box) as a
/// [`CollectedImage`] whose [`textbox`](CollectedImage::textbox) carries the
/// interior blocks + fill/border; the flow engine renders it as a bordered box.
pub(super) fn collect_textbox(
    attr: &NodeAttr,
    blocks: &[loki_doc_model::content::block::Block],
    images: &mut Vec<CollectedImage>,
) {
    let get = |key: &str| {
        attr.kv
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
    };
    let emu = |key: &str| get(key).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
    images.push(CollectedImage {
        src: String::new(),
        alt: None,
        cx_emu: emu("cx_emu"),
        cy_emu: emu("cy_emu"),
        float: FloatWrap::read_or_class_default(attr),
        textbox: Some(crate::resolve::CollectedTextBox {
            blocks: blocks.to_vec(),
            fill: get("textbox-fill"),
            line: get("textbox-line"),
        }),
    });
}
