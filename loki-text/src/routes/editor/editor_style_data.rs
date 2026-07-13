// SPDX-License-Identifier: Apache-2.0

//! Data helpers for the paragraph-style picker (split from `editor_style.rs`
//! for the 300-line ceiling): `collect_style_names` enumerates the document's
//! `(display_name, style_key)` pairs (built-in headings + catalog styles) and
//! `style_preview_font` derives each chip's preview font. Re-used by
//! `style_picker_panel` in `editor_style.rs`.

use std::sync::{Arc, Mutex};

use appthere_ui::tokens;
use loki_doc_model::StyleId;

use crate::editing::state::DocumentState;

/// Returns (display_name, style_key) pairs for all styles available in the document.
///
/// `style_key` is the string that [`loki_doc_model::get_block_style_name`] returns
/// for a block carrying that style, and the string passed to [`set_block_style`]
/// (or the dispatch logic for built-in types).  `display_name` is shown in the UI.
///
/// For built-in headings and the default paragraph style the two are identical.
/// For catalog styles the key is the [`StyleId`] string and the display name is
/// `ParagraphStyle::display_name` (falling back to the id if unset).
pub fn collect_style_names(doc_state: &Arc<Mutex<DocumentState>>) -> Vec<(String, String)> {
    let mut entries: Vec<(String, String)> = vec![
        (
            "Default Paragraph Style".into(),
            "Default Paragraph Style".into(),
        ),
        ("Heading 1".into(), "Heading 1".into()),
        ("Heading 2".into(), "Heading 2".into()),
        ("Heading 3".into(), "Heading 3".into()),
        ("Heading 4".into(), "Heading 4".into()),
        ("Heading 5".into(), "Heading 5".into()),
        ("Heading 6".into(), "Heading 6".into()),
    ];

    if let Ok(state) = doc_state.lock()
        && let Some(doc) = &state.document
    {
        for (id, style) in &doc.styles.paragraph_styles {
            let display = style
                .display_name
                .clone()
                .unwrap_or_else(|| id.as_str().to_string());
            let key = id.as_str().to_string();
            // Skip if the key already appears in built-ins (e.g. a catalog style
            // whose id is literally "Heading 1").
            if !entries.iter().any(|(_, k)| k == &key) {
                entries.push((display, key));
            }
        }
    }
    entries
}

/// Returns (font_size_px, font_weight, italic) for a style preview chip.
///
/// `display_name` is matched against built-in names.  `style_key` is used to
/// look up custom styles in the catalog by [`StyleId`] (O(1), avoids display-
/// name mismatch with catalog IDs).
pub(super) fn style_preview_font(
    display_name: &str,
    style_key: &str,
    doc_state: &Arc<Mutex<DocumentState>>,
) -> (f32, &'static str, bool) {
    match display_name {
        "Default Paragraph Style" => (tokens::FONT_SIZE_LABEL, "400", false),
        "Heading 1" => (18.0, "700", false),
        "Heading 2" => (16.0, "700", false),
        "Heading 3" => (14.0, "600", false),
        "Heading 4" => (13.0, "600", true),
        "Heading 5" => (12.0, "600", true),
        "Heading 6" => (11.0, "600", true),
        _ => {
            if let Ok(state) = doc_state.lock()
                && let Some(doc) = &state.document
                && let Some(style) = doc.styles.paragraph_styles.get(&StyleId::new(style_key))
            {
                // 1 pt ≈ 1.333 CSS px (96/72). Cap at 18 px.
                let fs = style
                    .char_props
                    .font_size
                    .map(|s| (s.value() as f32 * (96.0 / 72.0)).min(18.0))
                    .unwrap_or(tokens::FONT_SIZE_LABEL);
                let fw = if style.char_props.bold == Some(true) {
                    "700"
                } else {
                    "400"
                };
                let fi = style.char_props.italic == Some(true);
                return (fs, fw, fi);
            }
            (tokens::FONT_SIZE_LABEL, "400", false)
        }
    }
}
