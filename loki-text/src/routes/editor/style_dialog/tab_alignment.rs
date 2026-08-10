// SPDX-License-Identifier: Apache-2.0

//! The **Alignment** tab.

use appthere_ui::{AtDialogNotice, AtField, AtNoticeTone, AtSegmented, DialogPosture};
use dioxus::prelude::*;
use loki_doc_model::style::props::para_props::ParagraphAlignment;
use loki_doc_model::style::{StyleCatalog, StyleId};
use loki_i18n::fl;

use super::fields::{DraftSignal, OpenSignal, body_grid_style, full_width, provenance_line};
use super::rows::resolve_row;

/// The alignments offered, in display order.
const ALIGNMENTS: [ParagraphAlignment; 5] = [
    ParagraphAlignment::Left,
    ParagraphAlignment::Center,
    ParagraphAlignment::Right,
    ParagraphAlignment::Justify,
    ParagraphAlignment::Distribute,
];

/// The localized label for an alignment.
fn label(a: ParagraphAlignment) -> String {
    match a {
        ParagraphAlignment::Center => fl!("style-dialog-align-centre"),
        ParagraphAlignment::Right => fl!("style-dialog-align-right"),
        ParagraphAlignment::Justify => fl!("style-dialog-align-justified"),
        ParagraphAlignment::Distribute => fl!("style-dialog-align-distributed"),
        _ => fl!("style-dialog-align-left"),
    }
}

/// Renders the Alignment tab body.
pub(super) fn body(
    catalog: &StyleCatalog,
    id: &StyleId,
    draft: DraftSignal,
    open_style: OpenSignal,
    posture: DialogPosture,
) -> Element {
    let Some(current) = draft.read().clone() else {
        return rsx! {};
    };
    // The control highlights the **resolved** alignment, not just the local
    // one: a style that inherits `Left` must show Left selected, or the tab
    // reads as though the paragraph has no alignment at all.
    let resolved = current
        .style
        .para_props
        .alignment
        .or_else(|| {
            catalog
                .resolve_para_chain(id, |s| s.para_props.alignment)
                .and_then(|r| r.value)
        })
        .unwrap_or(ParagraphAlignment::Left);
    let selected = ALIGNMENTS
        .iter()
        .position(|a| *a == resolved)
        .unwrap_or(usize::MAX);

    let row = resolve_row(catalog, id, |s| s.para_props.alignment, |a| label(*a));

    rsx! {
        div {
            style: body_grid_style(posture),

            AtField {
                label: fl!("style-dialog-align-horizontal"),
                extra_style: full_width(posture),
                control: rsx! {
                    AtSegmented {
                        options: ALIGNMENTS.iter().map(|a| label(*a)).collect::<Vec<_>>(),
                        selected,
                        min_touch_px: posture.min_touch_px,
                        on_select: move |idx: usize| {
                            let mut draft = draft;
                            let mut next = draft.read().clone();
                            if let (Some(d), Some(a)) = (next.as_mut(), ALIGNMENTS.get(idx)) {
                                d.style.para_props.alignment = Some(*a);
                            }
                            draft.set(next);
                        },
                    }
                },
                footnote: row
                    .as_ref()
                    .map(|(source, value)| {
                        provenance_line(
                            source,
                            value.as_deref(),
                            posture,
                            open_style,
                            draft,
                            |d| d.style.para_props.alignment = None,
                        )
                    })
                    .unwrap_or_else(|| rsx! {}),
            }

            // `Distribute` stretches the last line too; the note is here rather
            // than in a tooltip because the difference from Justify is not
            // visible until a paragraph happens to end short.
            if resolved == ParagraphAlignment::Distribute {
                AtDialogNotice {
                    tone: AtNoticeTone::Info,
                    extra_style: full_width(posture),
                    message: rsx! { { fl!("style-dialog-align-distributed-note") } },
                }
            }

            // The design's remaining alignment controls — last line of a
            // justified paragraph, text-to-text vertical, snap to text grid,
            // expand single word — have no fields in `ParaProps`. They are
            // named here rather than drawn as controls that would do nothing.
            // TODO(para-props-alignment-detail): add `justify_last_line`,
            // `vertical_alignment`, `snap_to_grid` and `expand_single_word` to
            // `ParaProps`, then wire them through the ODF/OOXML mappers.
            AtDialogNotice {
                tone: AtNoticeTone::Info,
                extra_style: full_width(posture),
                message: rsx! { { fl!("style-dialog-align-unsupported") } },
            }
        }
    }
}
