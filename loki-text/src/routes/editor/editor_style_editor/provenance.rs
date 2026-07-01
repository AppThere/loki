// SPDX-License-Identifier: Apache-2.0

//! Read-only resolved-vs-overridden inspector column (Spec 05 M2).
//!
//! [`StyleProvenanceList`] renders one line per applicable style property — its
//! resolved value and where it comes from (Local / Inherited · ⟨ancestor⟩ /
//! Default / Auto) — from the rows built by
//! [`super::super::style_inspector::paragraph_inspector_rows`]. Every property
//! appears, not just locally-set ones, so a user can see at a glance which
//! properties this style *owns* versus passively receives (the fix for the old
//! panel's local-only blindness). Edit / reset affordances land in a later M2
//! increment; this pass makes provenance visible.
//!
//! A `#[component]` (ADR-0013) so it owns its hook scope — ready to read the
//! breakpoint for Compact posture without prop threading in a future pass.

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_i18n::fl;

use loki_doc_model::style::StyleId;

use super::super::style_inspector::{InspectorRow, RowProvenance, StyleProperty};

/// Renders the provenance inspector column for the selected style's rows.
///
/// `on_reset` is invoked with the property whose local override the user wants
/// to clear (reset to inherited); the reset control appears only on rows that
/// are set locally. `on_jump` is invoked with a source ancestor's id when the
/// user clicks an *inherited* row's provenance chip — the jump-to-ancestor
/// workflow ("identify where a property is set, change it for all dependents").
#[component]
pub(super) fn StyleProvenanceList(
    rows: Vec<InspectorRow>,
    on_reset: EventHandler<StyleProperty>,
    on_jump: EventHandler<StyleId>,
) -> Element {
    rsx! {
        div {
            style: format!(
                "width: 220px; min-width: 220px; overflow-y: auto; \
                 border-left: 1px solid {border}; display: flex; \
                 flex-direction: column; gap: {gap}px; padding: {p}px; \
                 font-family: {ff};",
                border = tokens::COLOR_BORDER_CHROME,
                gap = tokens::SPACE_2,
                p = tokens::SPACE_3,
                ff = tokens::FONT_FAMILY_UI,
            ),

            div {
                style: format!(
                    "font-size: {fs}px; font-weight: {fw}; color: {fg}; \
                     margin-bottom: {mb}px;",
                    fs = tokens::FONT_SIZE_LABEL,
                    fw = tokens::FONT_WEIGHT_MEDIUM,
                    fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                    mb = tokens::SPACE_1,
                ),
                { fl!("style-inspector-heading") }
            }

            for row in rows.iter() {
                {
                    let local = row.provenance.is_local();
                    let property = row.property;
                    // Inherited rows link to the ancestor that sets the value.
                    let jump_target = match &row.provenance {
                        RowProvenance::Inherited { ancestor_id, .. } => Some(ancestor_id.clone()),
                        _ => None,
                    };
                    rsx! {
                        div {
                            key: "{row.property:?}",
                            style: "display: flex; flex-direction: column;",

                            // Property label + resolved value.
                            div {
                                style: "display: flex; justify-content: space-between; gap: 8px;",
                                span {
                                    style: format!(
                                        "font-size: {fs}px; color: {fg}; font-weight: {fw};",
                                        fs = tokens::FONT_SIZE_LABEL,
                                        fg = tokens::COLOR_TEXT_ON_CHROME,
                                        // Bold the properties this style actually owns.
                                        fw = if local {
                                            tokens::FONT_WEIGHT_SEMIBOLD
                                        } else {
                                            tokens::FONT_WEIGHT_REGULAR
                                        },
                                    ),
                                    { property_label(row.property) }
                                }
                                span {
                                    style: format!(
                                        "font-size: {fs}px; color: {fg};",
                                        fs = tokens::FONT_SIZE_LABEL,
                                        fg = tokens::COLOR_TEXT_ON_CHROME,
                                    ),
                                    {
                                        row.value_display
                                            .clone()
                                            .unwrap_or_else(|| fl!("style-inspector-unset"))
                                    }
                                }
                            }

                            // Provenance line — accented when local — plus a
                            // reset-to-inherited control on locally-set rows.
                            div {
                                style: "display: flex; align-items: center; justify-content: space-between; gap: 8px;",
                                // Inherited rows: the chip is a link to the source
                                // ancestor. Other rows: a plain, accent-if-local chip.
                                if let Some(target) = jump_target {
                                    button {
                                        style: format!(
                                            "background: transparent; border: none; cursor: pointer; \
                                             padding: 0; text-align: left; text-decoration: underline; \
                                             font-family: {ff}; font-size: {fs}px; color: {fg};",
                                            ff = tokens::FONT_FAMILY_UI,
                                            fs = tokens::FONT_SIZE_XS,
                                            fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                                        ),
                                        aria_label: fl!("style-jump-aria"),
                                        onclick: move |_| on_jump.call(target.clone()),
                                        { provenance_label(&row.provenance) }
                                    }
                                } else {
                                    span {
                                        style: format!(
                                            "font-size: {fs}px; color: {fg};",
                                            fs = tokens::FONT_SIZE_XS,
                                            fg = if local {
                                                tokens::COLOR_TAB_ACTIVE_INDICATOR
                                            } else {
                                                tokens::COLOR_TEXT_ON_CHROME_SECONDARY
                                            },
                                        ),
                                        { provenance_label(&row.provenance) }
                                    }
                                }
                                // Reset to inherited (local rows only). Compact
                                // affordance in a dense inspector; the row's
                                // click area carries the intent.
                                if local {
                                    button {
                                        style: format!(
                                            "background: transparent; border: none; cursor: pointer; \
                                             padding: {p}px; font-size: {fs}px; color: {fg};",
                                            p = tokens::SPACE_1,
                                            fs = tokens::FONT_SIZE_XS,
                                            fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                                        ),
                                        aria_label: fl!("style-reset-aria"),
                                        onclick: move |_| on_reset.call(property),
                                        "\u{21A9}"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Localized label for a style property.
fn property_label(property: StyleProperty) -> String {
    match property {
        StyleProperty::FontFamily => fl!("style-prop-font-family"),
        StyleProperty::FontSize => fl!("style-prop-font-size"),
        StyleProperty::Bold => fl!("style-prop-bold"),
        StyleProperty::Italic => fl!("style-prop-italic"),
        StyleProperty::Alignment => fl!("style-prop-alignment"),
        StyleProperty::IndentStart => fl!("style-prop-indent-start"),
        StyleProperty::IndentEnd => fl!("style-prop-indent-end"),
        StyleProperty::IndentFirstLine => fl!("style-prop-indent-first-line"),
        StyleProperty::SpaceBefore => fl!("style-prop-space-before"),
        StyleProperty::SpaceAfter => fl!("style-prop-space-after"),
        StyleProperty::LineHeight => fl!("style-prop-line-height"),
    }
}

/// Localized provenance label; inherited rows name the source ancestor.
fn provenance_label(provenance: &RowProvenance) -> String {
    match provenance {
        RowProvenance::Local => fl!("style-provenance-local"),
        RowProvenance::Inherited {
            ancestor_display, ..
        } => fl!(
            "style-provenance-inherited",
            ancestor = ancestor_display.clone()
        ),
        RowProvenance::Default => fl!("style-provenance-default"),
        RowProvenance::FormatDefault => fl!("style-provenance-engine"),
    }
}
