// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! List-marker synthesis for the flow engine: prepends the label glyph(s) as a
//! `Inline::Str` + tab, and — for **picture bullets** (feature 5.4) — reports
//! the bullet image so `flow_paragraph` can place it out-of-band (Parley cannot
//! inline an image). Extracted from `flow_para.rs` (300-line ceiling).

use loki_doc_model::content::block::StyledParagraph;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::style::list_style::{BulletChar, ListLevelKind};

use crate::geometry::LayoutRect;
use crate::items::{PositionedImage, PositionedItem};
use crate::para::{ParagraphLayout, ResolvedParaProps, format_list_marker};

use super::FlowState;

/// The result of synthesising a paragraph's list marker.
pub(super) struct ListMarker {
    /// A clone of the paragraph with the marker text prepended, or `None` for a
    /// non-list paragraph (use the original).
    pub owned: Option<StyledParagraph>,
    /// A picture-bullet image reference, when the level's bullet is an image.
    /// `flow_paragraph` places it out-of-band via [`picture_bullet_item`].
    pub bullet_src: Option<String>,
}

/// Advance the list counters and prepend the marker label to `para`.
///
/// For a text bullet / number the label is inserted as `Inline::Str("<label>\t")`
/// (the tab positions the content at the indent). For a **picture bullet** the
/// label text is empty (so only a leading tab is prepended) and `bullet_src` is
/// returned for out-of-band image placement.
/// Apply the numbering level's `pPr` indent as a fallback when the paragraph
/// carries none (both indents 0.0) — e.g. `w:ind` set only on the abstract num.
pub(super) fn apply_level_indent_fallback(state: &FlowState, resolved: &mut ResolvedParaProps) {
    if let Some(ref lm) = resolved.list_marker
        && resolved.indent_start == 0.0
        && resolved.indent_hanging == 0.0
        && let Some(list_style) = state.catalog.list_styles.get(&lm.list_id)
        && let Some(level_def) = list_style.levels.get(lm.level as usize)
    {
        let level_indent = crate::resolve::pts_to_f32(level_def.indent_start);
        let level_hanging = crate::resolve::pts_to_f32(level_def.hanging_indent);
        if level_indent > 0.0 || level_hanging > 0.0 {
            resolved.indent_start = level_indent;
            resolved.indent_hanging = level_hanging;
        }
    }
}

pub(super) fn synthesize(
    state: &mut FlowState,
    para: &StyledParagraph,
    resolved: &ResolvedParaProps,
) -> ListMarker {
    let Some(lm) = &resolved.list_marker else {
        state.prev_list_id = None;
        return ListMarker {
            owned: None,
            bullet_src: None,
        };
    };
    let Some(list_style) = state.catalog.list_styles.get(&lm.list_id) else {
        state.prev_list_id = None;
        return ListMarker {
            owned: None,
            bullet_src: None,
        };
    };
    let Some(level_def) = list_style.levels.get(lm.level as usize) else {
        state.prev_list_id = None;
        return ListMarker {
            owned: None,
            bullet_src: None,
        };
    };

    let start_value = match &level_def.kind {
        ListLevelKind::Numbered { start_value, .. } => *start_value,
        _ => 1,
    };
    // New-list detection: a different list_id starts a fresh list, so its
    // counters are cleared.
    if state.prev_list_id.as_ref() != Some(&lm.list_id) {
        state.list_counters.remove(&lm.list_id);
    }
    state.prev_list_id = Some(lm.list_id.clone());
    state.advance_counter(&lm.list_id, lm.level, start_value);
    let counters = state
        .list_counters
        .get(&lm.list_id)
        .copied()
        .unwrap_or([1u32; 9]);

    let marker_text = format_list_marker(&list_style.levels, lm.level, &counters);
    let bullet_src = match &level_def.kind {
        ListLevelKind::Bullet {
            char: BulletChar::Image { src },
            ..
        } => Some(src.clone()),
        _ => None,
    };

    let mut cloned = para.clone();
    cloned
        .inlines
        .insert(0, Inline::Str(format!("{marker_text}\t")));
    ListMarker {
        owned: Some(cloned),
        bullet_src,
    }
}

/// Build the out-of-band image item for a picture bullet, in paragraph-local
/// coordinates, or `None` for an empty paragraph.
///
/// The bullet is a square sized to the first line's height (capped to the label
/// box width), left-aligned in the label box `[indent_start − label_w,
/// indent_start]` and vertically centred on line 0. Non-left `LabelAlignment`
/// is a refinement.
pub(super) fn picture_bullet_item(
    src: &str,
    resolved: &ResolvedParaProps,
    para_layout: &ParagraphLayout,
) -> Option<PositionedItem> {
    let &(top, bottom) = para_layout.line_boundaries.first()?;
    let line_h = (bottom - top).max(1.0);
    let label_w = if resolved.indent_hanging > 0.5 {
        resolved.indent_hanging
    } else {
        line_h
    };
    let size = line_h.min(label_w).max(4.0);
    let x = resolved.indent_start - label_w;
    let y = top + (line_h - size) / 2.0;
    Some(PositionedItem::Image(PositionedImage {
        rect: LayoutRect::new(x, y, size, size),
        src: src.to_string(),
        alt: None,
    }))
}
