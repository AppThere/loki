// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Inline content serialization for the Loro bridge (write direction).
//!
//! Deserialization lives in `inlines_read.rs`; the two files are split to
//! stay under the 300-line ceiling.

use super::BridgeError;
use crate::content::inline::Inline;
use crate::loro_schema::*;
use crate::style::props::char_props::CharProps;
use loro::LoroText;

// ── Serialization ─────────────────────────────────────────────────────────────

/// Inserts `inlines`' plain text at the end of `text` and returns the
/// `(start, end)` unicode range it occupies.
fn insert_inline_text(text: &LoroText, inlines: &[Inline]) -> Result<(usize, usize), BridgeError> {
    let start = text.len_unicode();
    let text_str = extract_plain_text(inlines);
    if !text_str.is_empty() {
        text.insert(start, &text_str)?;
    }
    Ok((start, text.len_unicode()))
}

pub(super) fn map_inlines(inlines: &[Inline], text: &LoroText) -> Result<(), BridgeError> {
    for inline in inlines {
        let start = text.len_unicode();
        match inline {
            Inline::Str(s) => {
                text.insert(start, s)?;
            }
            Inline::Space => {
                text.insert(start, " ")?;
            }
            Inline::SoftBreak | Inline::LineBreak => {
                text.insert(start, "\n")?;
            }
            Inline::StyledRun(run) => {
                let (start, end) = insert_inline_text(text, &run.content)?;
                if start < end {
                    if let Some(props) = &run.direct_props {
                        apply_char_props_marks(props, start, end, text)?;
                    }
                    if let Some(style_id) = &run.style_id {
                        text.mark(start..end, MARK_CHAR_STYLE_ID, style_id.0.as_str())?;
                    }
                }
            }
            Inline::Emph(inner) => {
                let (start, end) = insert_inline_text(text, inner)?;
                if start < end {
                    text.mark(start..end, MARK_ITALIC, true)?;
                }
            }
            Inline::Strong(inner) => {
                let (start, end) = insert_inline_text(text, inner)?;
                if start < end {
                    text.mark(start..end, MARK_BOLD, true)?;
                }
            }
            Inline::Underline(inner) => {
                let (start, end) = insert_inline_text(text, inner)?;
                if start < end {
                    text.mark(start..end, MARK_UNDERLINE, "Single")?;
                }
            }
            Inline::Strikeout(inner) => {
                let (start, end) = insert_inline_text(text, inner)?;
                if start < end {
                    text.mark(start..end, MARK_STRIKETHROUGH, "Single")?;
                }
            }
            Inline::Superscript(inner) => {
                let (start, end) = insert_inline_text(text, inner)?;
                if start < end {
                    text.mark(start..end, MARK_VERTICAL_ALIGN, "Superscript")?;
                }
            }
            Inline::Subscript(inner) => {
                let (start, end) = insert_inline_text(text, inner)?;
                if start < end {
                    text.mark(start..end, MARK_VERTICAL_ALIGN, "Subscript")?;
                }
            }
            Inline::SmallCaps(inner) => {
                let (start, end) = insert_inline_text(text, inner)?;
                if start < end {
                    text.mark(start..end, MARK_SMALL_CAPS, true)?;
                }
            }
            Inline::Link(_, inner, target) => {
                let (start, end) = insert_inline_text(text, inner)?;
                if start < end {
                    text.mark(start..end, MARK_LINK_URL, target.url.as_str())?;
                }
            }
            // Inline objects anchored natively (placeholder char + data mark) so
            // they stay live, positioned, deletable inlines. Only *top-level*
            // objects reach here — one nested in a wrapper keeps its block
            // opaque (see `opaque.rs`). Without `serde` they fall through to the
            // catch-all and their block is preserved opaquely instead.
            #[cfg(feature = "serde")]
            Inline::Image(..) => {
                write_inline_object(inline, text, MARK_IMAGE)?;
            }
            #[cfg(feature = "serde")]
            Inline::Note(..) => {
                write_inline_object(inline, text, MARK_NOTE)?;
            }
            // Text-bearing wrappers without a dedicated mark: keep the text.
            // Quote type / span attrs / citation metadata are not yet carried
            // through the CRDT — TODO(loro-bridge).
            _ => {
                let text_str = extract_plain_text(std::slice::from_ref(inline));
                if !text_str.is_empty() {
                    text.insert(start, &text_str)?;
                }
            }
        }
    }
    Ok(())
}

/// Writes a structured inline object (`Inline::Image`, `Inline::Note`, …) as an
/// [`OBJECT_REPLACEMENT_CHAR`] anchor carrying the object's `serde`-JSON
/// snapshot in `mark_key`.
///
/// The anchor is a single Unicode scalar, so the object becomes a discrete,
/// positioned, deletable inline rather than collapsing the whole paragraph to
/// an opaque snapshot. The object's payload (an image's geometry, a footnote's
/// body) rides along in the mark and round-trips losslessly. Reconstructed by
/// `inlines_read::reconstruct_inlines`.
#[cfg(feature = "serde")]
fn write_inline_object(
    inline: &Inline,
    text: &LoroText,
    mark_key: &str,
) -> Result<(), BridgeError> {
    match serde_json::to_string(inline) {
        Ok(json) => {
            let start = text.len_unicode();
            text.insert(start, OBJECT_REPLACEMENT_STR)?;
            let end = text.len_unicode();
            text.mark(start..end, mark_key, json)?;
        }
        Err(err) => {
            // Unreachable in practice: `Inline` derives Serialize. Drop the
            // object rather than leave a bare anchor with no backing data.
            tracing::warn!("loro bridge: failed to encode inline object ({mark_key}): {err}");
        }
    }
    Ok(())
}

/// Recursively extracts the visible text of `inlines`.
///
/// Covers every text-bearing variant; content-bearing variants that cannot be
/// flattened to text (`Note`, `Image`, `Field`, …) never reach this function
/// in the write path — their containing block is preserved as an opaque
/// snapshot instead (see `opaque.rs`).
pub(super) fn extract_plain_text(inlines: &[Inline]) -> String {
    let mut out = String::new();
    for inline in inlines {
        match inline {
            Inline::Str(s) => out.push_str(s),
            Inline::Space => out.push(' '),
            Inline::SoftBreak | Inline::LineBreak => out.push('\n'),
            Inline::Code(_, s) => out.push_str(s),
            // Math holds MathML markup, not display text; a block containing it
            // is preserved as an opaque snapshot rather than flat text.
            Inline::StyledRun(run) => out.push_str(&extract_plain_text(&run.content)),
            Inline::Emph(inner)
            | Inline::Strong(inner)
            | Inline::Underline(inner)
            | Inline::Strikeout(inner)
            | Inline::Superscript(inner)
            | Inline::Subscript(inner)
            | Inline::SmallCaps(inner)
            | Inline::Quoted(_, inner)
            | Inline::Cite(_, inner)
            | Inline::Span(_, inner)
            | Inline::Link(_, inner, _) => out.push_str(&extract_plain_text(inner)),
            _ => {}
        }
    }
    out
}

pub(super) fn apply_char_props_marks(
    props: &CharProps,
    start: usize,
    end: usize,
    text: &LoroText,
) -> Result<(), BridgeError> {
    if let Some(v) = props.bold {
        text.mark(start..end, MARK_BOLD, v)?;
    }
    if let Some(v) = props.italic {
        text.mark(start..end, MARK_ITALIC, v)?;
    }
    if let Some(v) = props.outline {
        text.mark(start..end, MARK_OUTLINE, v)?;
    }
    if let Some(v) = props.shadow {
        text.mark(start..end, MARK_SHADOW, v)?;
    }
    if let Some(v) = props.small_caps {
        text.mark(start..end, MARK_SMALL_CAPS, v)?;
    }
    if let Some(v) = props.all_caps {
        text.mark(start..end, MARK_ALL_CAPS, v)?;
    }
    if let Some(v) = props.kerning {
        text.mark(start..end, MARK_KERNING, v)?;
    }
    if let Some(v) = &props.font_name {
        text.mark(start..end, MARK_FONT_FAMILY, v.clone())?;
    }
    if let Some(v) = &props.font_size {
        text.mark(start..end, MARK_FONT_SIZE_PT, v.value())?;
    }
    if let Some(v) = props.scale {
        text.mark(start..end, MARK_SCALE, f64::from(v))?;
    }
    if let Some(v) = &props.letter_spacing {
        text.mark(start..end, MARK_LETTER_SPACING, v.value())?;
    }
    if let Some(v) = &props.word_spacing {
        text.mark(start..end, MARK_WORD_SPACING, v.value())?;
    }
    if let Some(v) = &props.underline {
        text.mark(start..end, MARK_UNDERLINE, format!("{:?}", v))?;
    }
    if let Some(v) = &props.strikethrough {
        text.mark(start..end, MARK_STRIKETHROUGH, format!("{:?}", v))?;
    }
    if let Some(v) = &props.vertical_align {
        text.mark(start..end, MARK_VERTICAL_ALIGN, format!("{:?}", v))?;
    }
    // Non-Rgb variants (Theme, Cmyk, Transparent) have no hex repr and are deferred — TODO(loro-bridge)
    if let Some(v) = &props.color
        && let Some(hex) = v.to_hex()
    {
        text.mark(start..end, MARK_COLOR, hex)?;
    }
    if let Some(v) = &props.highlight_color {
        text.mark(start..end, MARK_HIGHLIGHT_COLOR, format!("{v:?}"))?;
    }
    if let Some(v) = &props.language {
        text.mark(start..end, MARK_LANGUAGE, v.as_str())?;
    }
    if let Some(v) = &props.language_complex {
        text.mark(start..end, MARK_LANGUAGE_COMPLEX, v.as_str())?;
    }
    if let Some(v) = &props.language_east_asian {
        text.mark(start..end, MARK_LANGUAGE_EAST_ASIAN, v.as_str())?;
    }
    if let Some(v) = &props.hyperlink {
        text.mark(start..end, MARK_LINK_URL, v.clone())?;
    }
    Ok(())
}
