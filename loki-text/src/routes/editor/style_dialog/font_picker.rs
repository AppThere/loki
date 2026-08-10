// SPDX-License-Identifier: Apache-2.0

//! The font-family field: the shared searchable picker
//! ([`FontFamilyPicker`](super::super::font_family_field::FontFamilyPicker))
//! wrapped in this dialog's field chrome — label, draft plumbing and the
//! provenance footnote (design notes 07, 08 and 10).

use std::rc::Rc;

use appthere_ui::{AtField, DialogPosture};
use dioxus::prelude::*;
use loki_doc_model::style::{StyleCatalog, StyleId};
use loki_i18n::fl;

use super::super::font_family_field::{FontFamilyPicker, FontFamilyPickerProps};
use super::fields::{DraftSignal, OpenSignal, provenance_line};
use super::rows::resolve_row;

/// Renders the font-family field.
pub(super) fn field(
    catalog: &StyleCatalog,
    id: &StyleId,
    draft: DraftSignal,
    open_style: OpenSignal,
    posture: DialogPosture,
    font_families: Rc<Vec<String>>,
) -> Element {
    let selected = draft
        .read()
        .as_ref()
        .and_then(|d| d.style.char_props.font_name.clone());
    let row = resolve_row(
        catalog,
        id,
        |s| s.char_props.font_name.clone(),
        Clone::clone,
    );

    rsx! {
        AtField {
            label: fl!("style-dialog-font-family"),
            extra_style: super::fields::full_width(posture),
            control: rsx! {
                FontFamilyPicker {
                    ..FontFamilyPickerProps {
                        selected,
                        placeholder: fl!("style-dialog-font-family-inherit"),
                        font_families,
                        posture,
                        on_pick: EventHandler::new(move |name: String| {
                            let mut draft = draft;
                            let mut next = draft.read().clone();
                            if let Some(d) = next.as_mut() {
                                d.style.char_props.font_name = Some(name);
                            }
                            draft.set(next);
                        }),
                    }
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
                        |d| d.style.char_props.font_name = None,
                    )
                })
                .unwrap_or_else(|| rsx! {}),
        }
    }
}
