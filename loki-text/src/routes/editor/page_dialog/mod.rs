// SPDX-License-Identifier: Apache-2.0

//! The page style editor dialog (design section 3).
//!
//! # A non-inheriting family, so no provenance chips
//!
//! Page styles have no `basedOn` parent in either ODF or OOXML (design note
//! 11), so there is nothing to inherit from and no chain to walk. The ↳ line
//! under the paper control names the **paper preset** instead — the only
//! upstream a page geometry has — and the body carries an explicit impact line
//! naming the sections that will change.

mod body;
mod preview;
mod tab_borders;
mod tab_columns;
mod tab_headfoot;
mod tab_margins;
mod tab_page;
mod tabs;

use std::sync::{Arc, Mutex};

use appthere_ui::responsive::use_breakpoint;
use appthere_ui::{
    AtDialogButton, AtDialogShell, AtDialogTabStrip, DialogPosture, DialogWidth, tokens,
};
use dioxus::prelude::*;
use loki_doc_model::layout::page::PageLayout;
use loki_doc_model::loki_primitives::units::MeasurementUnit;
use loki_i18n::fl;

use super::editor_defaults::PanelSettings;
use super::editor_style_editor::StyleEditorSync;
use crate::editing::state::DocumentState;
use tabs::PageTab;

/// The page geometry being edited, plus the numeric input buffers.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct PageDialogDraft {
    /// The name of the page style being edited.
    pub name: String,
    /// The geometry as it will be committed.
    pub layout: PageLayout,
    /// The geometry as it was when the dialog opened, so Cancel is a discard
    /// and Apply can be inert on an untouched draft.
    pub original: PageLayout,
    /// Buffers for the numeric inputs, keyed the way [`body`] reads them.
    pub buffers: PageBuffers,
    /// The user has asked for a custom size, so the width and height boxes stay
    /// editable even while the numbers still match a catalogued paper.
    ///
    /// Without this there is no way out of a preset: the boxes are read-only
    /// whenever the size matches, so the size can never stop matching.
    pub custom_paper: bool,
}

/// Text buffers for the dialog's measurement inputs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct PageBuffers {
    /// Page width.
    pub width: String,
    /// Page height.
    pub height: String,
    /// Top margin.
    pub margin_top: String,
    /// Bottom margin.
    pub margin_bottom: String,
    /// Start-edge margin (inner when mirrored, left otherwise).
    pub margin_start: String,
    /// End-edge margin (outer when mirrored, right otherwise).
    pub margin_end: String,
    /// Binding gutter.
    pub gutter: String,
    /// Header band height.
    pub header: String,
    /// Footer band height.
    pub footer: String,
    /// Inter-column gap.
    pub column_gap: String,
}

impl PageDialogDraft {
    /// Opens a draft over `layout`, formatting every buffer in `unit`.
    #[must_use]
    pub fn new(name: String, layout: PageLayout, unit: MeasurementUnit) -> Self {
        let buffers = Self::buffers_for(&layout, unit);
        Self {
            name,
            original: layout.clone(),
            layout,
            buffers,
            custom_paper: false,
        }
    }

    /// Re-formats every buffer from the layout in the current unit.
    ///
    /// Called when the measurement unit changes: the buffers hold *display*
    /// numbers, so leaving them alone after a unit switch would show
    /// millimetres labelled as inches.
    #[must_use]
    pub fn buffers_for(layout: &PageLayout, unit: MeasurementUnit) -> PageBuffers {
        let u = unit;
        let m = &layout.margins;
        PageBuffers {
            width: u.format_bare(layout.page_size.width),
            height: u.format_bare(layout.page_size.height),
            margin_top: u.format_bare(m.top),
            margin_bottom: u.format_bare(m.bottom),
            margin_start: u.format_bare(m.left),
            margin_end: u.format_bare(m.right),
            gutter: u.format_bare(m.gutter),
            header: u.format_bare(m.header),
            footer: u.format_bare(m.footer),
            column_gap: layout
                .columns
                .as_ref()
                .map(|c| u.format_bare(c.gap))
                .unwrap_or_default(),
        }
    }

    /// `true` when the user has changed the geometry.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.layout != self.original
    }
}

/// Props for [`PageStyleDialog`].
#[derive(Clone, Props)]
pub(super) struct PageStyleDialogProps {
    /// Shared document state.
    pub(super) doc_state: Arc<Mutex<DocumentState>>,
    /// The page style being edited; `None` closes the dialog.
    pub(super) open: Signal<Option<String>>,
    /// The style this mount is editing, read out of `open` by the caller.
    ///
    /// A prop rather than a read of `open` inside the body, so the draft can be
    /// seeded before any early return and the mount can be keyed on it.
    pub(super) style_name: String,
    /// Loro / undo plumbing.
    pub(super) sync: StyleEditorSync,
}

impl PartialEq for PageStyleDialogProps {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.doc_state, &other.doc_state)
            && self.open == other.open
            && self.style_name == other.style_name
            && self.sync == other.sync
    }
}

/// Renders the page style editor.
// PascalCase for rsx; `#[component]` cannot derive the props comparison.
#[allow(non_snake_case)]
pub(super) fn PageStyleDialog(props: PageStyleDialogProps) -> Element {
    let PageStyleDialogProps {
        doc_state,
        mut open,
        style_name,
        sync,
    } = props;
    let posture = DialogPosture::for_breakpoint(use_breakpoint());
    let settings = PanelSettings::load(sync.settings_generation);
    // Copied out of the settings so closures capture a `Copy` unit rather than
    // borrowing the whole (heap-carrying) settings snapshot.
    let unit = settings.unit;

    // Every hook runs before the first early return below: a `use_signal` that
    // some renders reach and others do not shifts every later hook's index.
    let mut draft = use_signal(|| {
        body::layout_for(&doc_state, &style_name)
            .map(|l| PageDialogDraft::new(style_name.clone(), l, unit))
    });
    let mut active_tab = use_signal(|| PageTab::Page);
    let menu_open = use_signal(|| false);

    let Some(current) = draft.read().clone() else {
        return rsx! {};
    };
    let tab = *active_tab.read();
    let dirty = current.is_dirty();
    let ds_apply = Arc::clone(&doc_state);
    let apply_name = style_name.clone();

    rsx! {
        AtDialogShell {
            title: fl!("page-dialog-title", name = current.name.clone()),
            subtitle: rsx! {
                div {
                    style: format!(
                        "font-size: {fs}px; color: {fg};",
                        fs = tokens::FONT_SIZE_META,
                        fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                    ),
                    // Stated in the header, because "why is there no
                    // Inherits-from field" is the first question this dialog
                    // raises for anyone arriving from the paragraph editor.
                    { fl!("page-dialog-subtitle") }
                }
            },
            close_aria_label: fl!("page-dialog-close-aria"),
            width: DialogWidth::Wide,
            on_close: move |_| open.set(None),

            tabs: rsx! {
                AtDialogTabStrip {
                    labels: PageTab::labels(),
                    active: tab.index(),
                    inline_at_medium: PageTab::INLINE_AT_MEDIUM,
                    menu_open,
                    more_label: fl!("page-dialog-tabs-more"),
                    on_select: move |idx: usize| active_tab.set(PageTab::from_index(idx)),
                }
            },

            body: rsx! { { body::tab_body(tab, &doc_state, draft, posture, &settings) } },

            footer: rsx! {
                AtDialogButton {
                    label: fl!("page-dialog-restore-preset"),
                    tertiary: true,
                    min_touch_px: posture.min_touch_px,
                    on_click: move |_| {
                        let mut next = draft.read().clone();
                        if let Some(d) = next.as_mut() {
                            d.layout = d.original.clone();
                            d.buffers = PageDialogDraft::buffers_for(&d.layout, unit);
                        }
                        draft.set(next);
                    },
                }
                div {
                    style: format!(
                        "display: flex; flex-direction: {dir}; gap: {gap}px;",
                        dir = if posture.stack_footer { "column-reverse" } else { "row" },
                        gap = tokens::SPACE_2,
                    ),
                    AtDialogButton {
                        label: fl!("page-dialog-cancel"),
                        min_touch_px: posture.min_touch_px,
                        on_click: move |_| open.set(None),
                    }
                    AtDialogButton {
                        label: fl!("page-dialog-apply"),
                        primary: true,
                        disabled: !dirty,
                        min_touch_px: posture.min_touch_px,
                        on_click: move |_| {
                            let Some(d) = draft.read().clone() else { return };
                            if body::commit(&ds_apply, &sync, &apply_name, &d.layout) {
                                draft.set(
                                    body::layout_for(&ds_apply, &apply_name).map(|l| {
                                        PageDialogDraft::new(apply_name.clone(), l, unit)
                                    }),
                                );
                            }
                        },
                    }
                }
            },
        }
    }
}
