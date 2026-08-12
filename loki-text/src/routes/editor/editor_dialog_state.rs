// SPDX-License-Identifier: Apache-2.0

//! Open state for the six tabbed dialogs added in design sections 2–7.
//!
//! # One bundle, not six fields on `EditorState`
//!
//! These signals travel together: `editor_inner` hands the whole group to the
//! ribbon tabs that open them and to [`super::editor_modals`], which mounts
//! them. Bundling also keeps `editor_state` — a hundred lines of field and
//! initialiser pairs already — under the 300-line ceiling.
//!
//! The paragraph style dialog is deliberately **not** here: it predates this
//! group and `editor_state::EditorState::paragraph_style_dialog` is already
//! threaded through the Write tab and `editor_modals`.

use dioxus::prelude::*;

use super::link_dialog::LinkDraft;
use super::table_dialog::TableSpec;

/// Whether each tabbed dialog is open, and with what.
///
/// `Copy`, because every field is a [`Signal`] — so this can be passed to a
/// ribbon tab and captured by its `on_click` closures without cloning.
#[derive(Clone, Copy, PartialEq)]
pub(in crate::routes::editor) struct DialogSignals {
    /// Span-level formatting (design section 2). `true` while open.
    pub span_format: Signal<bool>,
    /// The page style open in the page dialog (section 3); `None` = closed.
    pub page_style: Signal<Option<String>>,
    /// Document properties (section 4). `true` while open.
    pub metadata: Signal<bool>,
    /// The link being composed (section 5); `None` = closed.
    pub insert_link: Signal<Option<LinkDraft>>,
    /// The table being composed (section 6); `None` = closed.
    pub insert_table: Signal<Option<TableSpec>>,
    /// Publish EPUB 3 (section 7). `true` while open.
    pub publish_epub: Signal<bool>,
    /// The Print dialog (§6). `true` while open.
    pub print: Signal<bool>,
}

/// Initialises every dialog's open state, all closed.
///
/// Acts as a Dioxus custom hook — called unconditionally from
/// [`super::editor_state::use_editor_state`], so hook order is stable.
pub(in crate::routes::editor) fn use_dialog_signals() -> DialogSignals {
    DialogSignals {
        span_format: use_signal(|| false),
        page_style: use_signal(|| None),
        metadata: use_signal(|| false),
        insert_link: use_signal(|| None),
        insert_table: use_signal(|| None),
        publish_epub: use_signal(|| false),
        print: use_signal(|| false),
    }
}
