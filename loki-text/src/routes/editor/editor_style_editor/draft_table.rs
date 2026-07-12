// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The table-style edit draft (Spec 05 M6 table family, 4a.3) and its
//! catalog conversions. Sibling of `draft.rs` (kept separate for the
//! 300-line ceiling).
//!
//! The form edits the *identity and table-level* fields (name, based-on,
//! alignment, band sizes); everything it does not edit — width, background,
//! the conditional/banding region map — is preserved from the existing
//! catalog entry, so applying the form never wipes a style's banding.

use loki_doc_model::style::StyleId;
use loki_doc_model::style::table_style::{TableAlignment, TableStyle};

/// Editable working copy of a [`TableStyle`]'s form fields.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct TableStyleDraft {
    /// Catalog id (immutable through the form).
    pub id: String,
    /// Display name.
    pub name: String,
    /// Based-on parent id ("" = none).
    pub parent: String,
    /// Table alignment, `None` = inherit/unset.
    pub alignment: Option<TableAlignment>,
    /// Row band size as typed (empty = unset; non-numeric rejected on apply).
    pub row_band_str: String,
    /// Column band size as typed.
    pub col_band_str: String,
}

/// Seeds a draft from a catalog style.
pub(crate) fn table_style_to_draft(style: &TableStyle) -> TableStyleDraft {
    TableStyleDraft {
        id: style.id.as_str().to_string(),
        name: style
            .display_name
            .clone()
            .unwrap_or_else(|| style.id.as_str().to_string()),
        parent: style
            .parent
            .as_ref()
            .map(|p| p.as_str().to_string())
            .unwrap_or_default(),
        alignment: style.table_props.alignment,
        row_band_str: style
            .table_props
            .row_band_size
            .map(|n| n.to_string())
            .unwrap_or_default(),
        col_band_str: style
            .table_props
            .col_band_size
            .map(|n| n.to_string())
            .unwrap_or_default(),
    }
}

/// Applies the draft's form fields onto `base` (the current catalog entry, or
/// a fresh style when absent), preserving the unedited fields — notably the
/// conditional/banding map and any width/background.
pub(crate) fn draft_apply_to_table_style(
    draft: &TableStyleDraft,
    base: Option<TableStyle>,
) -> TableStyle {
    let mut style = base.unwrap_or_else(|| TableStyle {
        id: StyleId::new(&draft.id),
        display_name: None,
        parent: None,
        table_props: Default::default(),
        conditional: Default::default(),
        extensions: Default::default(),
    });
    style.id = StyleId::new(&draft.id);
    style.display_name = if draft.name.is_empty() || draft.name == draft.id {
        None
    } else {
        Some(draft.name.clone())
    };
    style.parent = if draft.parent.is_empty() {
        None
    } else {
        Some(StyleId::new(&draft.parent))
    };
    style.table_props.alignment = draft.alignment;
    style.table_props.row_band_size = draft.row_band_str.trim().parse().ok();
    style.table_props.col_band_size = draft.col_band_str.trim().parse().ok();
    style
}

#[cfg(test)]
mod tests {
    use super::*;
    use loki_doc_model::style::table_style::{TableConditionalFormat, TableRegion};

    fn styled() -> TableStyle {
        let mut s = TableStyle {
            id: StyleId::new("Banded"),
            display_name: Some("Banded Grid".into()),
            parent: Some(StyleId::new("Base")),
            table_props: Default::default(),
            conditional: Default::default(),
            extensions: Default::default(),
        };
        s.table_props.alignment = Some(TableAlignment::Center);
        s.table_props.row_band_size = Some(2);
        s.conditional.insert(
            TableRegion::FirstRow,
            TableConditionalFormat {
                background_color: None,
                char_props: Default::default(),
            },
        );
        s
    }

    #[test]
    fn draft_round_trips_the_form_fields() {
        let style = styled();
        let draft = table_style_to_draft(&style);
        assert_eq!(draft.name, "Banded Grid");
        assert_eq!(draft.parent, "Base");
        assert_eq!(draft.alignment, Some(TableAlignment::Center));
        assert_eq!(draft.row_band_str, "2");
        let back = draft_apply_to_table_style(&draft, Some(style.clone()));
        assert_eq!(back, style);
    }

    #[test]
    fn apply_preserves_the_conditional_map_and_edits_fields() {
        let style = styled();
        let mut draft = table_style_to_draft(&style);
        draft.alignment = Some(TableAlignment::Right);
        draft.row_band_str = "3".into();
        draft.parent.clear();
        let back = draft_apply_to_table_style(&draft, Some(style));
        assert_eq!(back.table_props.alignment, Some(TableAlignment::Right));
        assert_eq!(back.table_props.row_band_size, Some(3));
        assert_eq!(back.parent, None);
        assert!(
            back.conditional.contains_key(&TableRegion::FirstRow),
            "the banding map the form does not edit must survive"
        );
    }

    #[test]
    fn non_numeric_band_input_clears_rather_than_garbles() {
        let mut draft = table_style_to_draft(&styled());
        draft.row_band_str = "abc".into();
        let back = draft_apply_to_table_style(&draft, Some(styled()));
        assert_eq!(back.table_props.row_band_size, None);
    }
}
