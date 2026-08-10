// SPDX-License-Identifier: Apache-2.0

//! Resolving one property's provenance and phrasing it for the line under a
//! control (design notes 01 and 02).
//!
//! # The provenance shown is the *committed* one
//!
//! Resolution reads the catalog, not the draft. While a property is staged the
//! line keeps naming the source it currently resolves from and the row is
//! marked pending — a line that flipped to "set here" on the first keystroke
//! would be describing an override the document does not yet have, and Cancel
//! would leave the user having read a lie.

use appthere_ui::AtProvenanceKind;
use loki_doc_model::style::ParagraphStyle;
use loki_doc_model::style::{Provenance, StyleCatalog, StyleId};
use loki_i18n::fl;

/// One property's resolved provenance, ready for [`appthere_ui::AtProvenanceLine`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RowSource {
    /// Which level the value resolved from.
    pub kind: AtProvenanceKind,
    /// The ancestor that owns the value, for the "edit at the source" jump.
    /// `Some` only for [`AtProvenanceKind::Inherited`].
    pub ancestor: Option<StyleId>,
    /// The ancestor's display name, if there is one.
    pub ancestor_display: Option<String>,
}

impl RowSource {
    /// The localized sentence for the line, given the property's resolved value
    /// (already formatted; `None` when the engine decides).
    #[must_use]
    pub fn text(&self, value: Option<&str>) -> String {
        let value = value.unwrap_or_default();
        match self.kind {
            AtProvenanceKind::Local => fl!("style-dialog-prov-local"),
            AtProvenanceKind::Inherited => {
                let source = self
                    .ancestor_display
                    .clone()
                    .unwrap_or_else(|| fl!("style-dialog-prov-source-unnamed"));
                if value.is_empty() {
                    fl!("style-dialog-prov-inherited-bare", source = source)
                } else {
                    fl!(
                        "style-dialog-prov-inherited",
                        source = source,
                        value = value.to_string()
                    )
                }
            }
            AtProvenanceKind::Default => {
                if value.is_empty() {
                    fl!("style-dialog-prov-document-bare")
                } else {
                    fl!("style-dialog-prov-document", value = value.to_string())
                }
            }
            AtProvenanceKind::Engine => {
                if value.is_empty() {
                    fl!("style-dialog-prov-engine-bare")
                } else {
                    fl!("style-dialog-prov-engine", value = value.to_string())
                }
            }
        }
    }

    /// The "edit at the source" label, on inherited rows only. Following it
    /// opens the ancestor that actually owns the value, so a change there
    /// reaches every dependent (design note 02).
    #[must_use]
    pub fn jump_label(&self) -> Option<String> {
        self.ancestor
            .as_ref()
            .map(|_| fl!("style-dialog-prov-edit-there"))
    }

    /// The reset label, on locally-set rows only.
    #[must_use]
    pub fn reset_label(&self) -> Option<String> {
        self.kind.is_local().then(|| fl!("style-dialog-prov-reset"))
    }
}

/// Maps a model [`Provenance`] to a [`RowSource`], resolving the ancestor's
/// display name.
#[must_use]
pub(super) fn to_row_source(catalog: &StyleCatalog, p: Provenance) -> RowSource {
    match p {
        Provenance::Local => RowSource {
            kind: AtProvenanceKind::Local,
            ancestor: None,
            ancestor_display: None,
        },
        Provenance::Inherited(id) => {
            let display = catalog
                .paragraph_styles
                .get(&id)
                .and_then(|s| s.display_name.clone())
                .unwrap_or_else(|| id.as_str().to_string());
            RowSource {
                kind: AtProvenanceKind::Inherited,
                ancestor: Some(id),
                ancestor_display: Some(display),
            }
        }
        Provenance::Default => RowSource {
            kind: AtProvenanceKind::Default,
            ancestor: None,
            ancestor_display: None,
        },
        Provenance::FormatDefault => RowSource {
            kind: AtProvenanceKind::Engine,
            ancestor: None,
            ancestor_display: None,
        },
    }
}

/// Resolves one property of `id` over its chain, returning its source and the
/// resolved value formatted by `fmt`.
///
/// Returns `None` only when `id` is not a paragraph style in the catalog — the
/// caller renders the control with no line rather than inventing one.
#[must_use]
pub(super) fn resolve_row<T: Clone>(
    catalog: &StyleCatalog,
    id: &StyleId,
    get: impl Fn(&loki_doc_model::style::ParagraphStyle) -> Option<T>,
    fmt: impl Fn(&T) -> String,
) -> Option<(RowSource, Option<String>)> {
    let resolved = catalog.resolve_para_chain(id, get)?;
    Some((
        to_row_source(catalog, resolved.provenance),
        resolved.value.as_ref().map(fmt),
    ))
}

/// How many properties would re-resolve if `style` were re-parented onto
/// `new_parent` — the number the General tab states *before* the user commits
/// (design section 1b).
///
/// Counts the properties the style does **not** set locally, since those are
/// exactly the ones whose resolved value can move when the chain changes. A
/// locally-set property is unaffected by re-parenting, which is the point the
/// General tab's notice is making.
#[must_use]
pub(super) fn inherited_property_count(style: &loki_doc_model::style::ParagraphStyle) -> usize {
    let p = &style.para_props;
    let c = &style.char_props;
    let flags = [
        p.alignment.is_none(),
        p.indent_start.is_none(),
        p.indent_end.is_none(),
        p.indent_first_line.is_none(),
        p.space_before.is_none(),
        p.space_after.is_none(),
        p.line_height.is_none(),
        p.keep_together.is_none(),
        p.keep_with_next.is_none(),
        p.orphan_control.is_none(),
        p.widow_control.is_none(),
        p.tab_stops.is_none(),
        c.font_name.is_none(),
        c.font_size.is_none(),
        c.bold.is_none(),
        c.italic.is_none(),
        c.color.is_none(),
        c.language.is_none(),
    ];
    flags.into_iter().filter(|unset| *unset).count()
}

/// How many properties this style sets locally — the complement of
/// [`inherited_property_count`] over the same property set.
#[must_use]
pub(super) fn local_property_count(style: &loki_doc_model::style::ParagraphStyle) -> usize {
    PROPERTY_COUNT - inherited_property_count(style)
}

/// The size of the property set both counters range over. Named once so the
/// two cannot disagree.
pub(super) const PROPERTY_COUNT: usize = 18;

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;

/// What `style` would resolve this property to if it set nothing itself.
///
/// **Resolution starts at the draft's parent, not at the style.**
/// `StyleCatalog::resolve_para_chain` checks the style it is given first and
/// returns that style's own committed value as `Local`. Feeding it the style's
/// own id therefore makes a cleared override fall back onto the value that was
/// just cleared: Reset would leave the control showing the old value, and
/// clicking that value "again" would quietly un-dirty the draft.
///
/// Reading `parent` off the **draft** rather than the catalog matters too — a
/// re-parent staged on the General tab has to move what the other tabs inherit.
#[must_use]
pub(super) fn resolve_inherited<T: Clone>(
    catalog: &StyleCatalog,
    style: &ParagraphStyle,
    get: impl Fn(&ParagraphStyle) -> Option<T>,
) -> Option<T> {
    let parent = style.parent.as_ref()?;
    catalog
        .resolve_para_chain(parent, get)
        .and_then(|r| r.value)
}
