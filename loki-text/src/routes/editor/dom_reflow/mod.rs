// SPDX-License-Identifier: Apache-2.0

//! The **DOM reflow view** (ADR-0017 sequencing step 2) — the reading view
//! rendered as DOM rather than painted into the canvas.
//!
//! # Why it exists alongside the canvas one
//!
//! ADR-0017 decides the reflow view should be DOM: T7.3's per-element
//! horizontal scroll container becomes `overflow-x: auto` on an element (and
//! probe P1 already measured that Blitz routes nested scrolling correctly), and
//! T7.2's reading measure becomes a CSS `max-width` instead of process-wide
//! ambient state crossing from the layout pass to the paint pass.
//!
//! The ADR sequences this second, *after* the line-break comparison — which
//! passed, on plain Latin text in one face. It is built **beside** the canvas
//! path rather than replacing it, because the remaining risks (mixed style runs,
//! virtualisation cost, spell/selection/caret) are things you compare rather
//! than reason about, and comparing needs both paths live.
//!
//! # How to reach it
//!
//! `LOKI_REFLOW_DOM=1`, then switch the view mode to Reflow. An environment
//! variable rather than a settings control, deliberately: this path renders a
//! subset of the document (see [`content`]) and is not something to offer a
//! reader in a menu until it renders all of it.
//!
//! # What this is not
//!
//! **Read-only.** No caret, no selection, no hit-testing, no spell squiggles, no
//! revision marks. Those are painted from `PositionedItem`s on the canvas path
//! and each needs a DOM equivalent — ADR-0017 §3.2 lists them, and none is done.
//! Editing while this view is active edits nothing. `TODO(dom-reflow-editing)`.

use std::sync::{Arc, Mutex};

use dioxus::prelude::*;

use crate::editing::state::DocumentState;

pub mod content;
mod style;

/// Whether the DOM reflow path is selected.
///
/// Read on every render rather than cached: it costs an environment lookup and
/// it means a developer comparing the two paths does not have to reason about
/// when the value was captured.
#[must_use]
pub(super) fn enabled() -> bool {
    std::env::var("LOKI_REFLOW_DOM").is_ok_and(|v| v == "1")
}

/// The reading column's width, in points, from T7.2's measure.
///
/// This is the whole of ADR-0017's "the measure becomes CSS": the resolved
/// character count is a `max-width` on the column, applied by the same layout
/// that paints it. No ambient state, no ordering hazard between the pass that
/// resolves it and the pass that reads it — the two are one pass.
///
/// Falls back to the canvas path's fixed cap when no measure has been resolved,
/// so the two views are comparable rather than differing for a reason that is
/// not about rendering.
fn column_max_width_pt() -> f32 {
    loki_renderer::measure::content_cap_pt().unwrap_or(
        // `MAX_REFLOW_TILE_PX` minus its insets, in points — the same column the
        // canvas path would use, read from the same constants.
        (loki_renderer::render_layout::MAX_REFLOW_TILE_PX
            - 2.0 * loki_renderer::render_layout::REFLOW_PADDING_PT
                / loki_renderer::render_layout::PX_TO_PT)
            * loki_renderer::render_layout::PX_TO_PT,
    )
}

/// The DOM reflow view: the document's blocks in a centred reading column.
///
/// A plain function, not a `#[component]`: `Arc<Mutex<DocumentState>>` has no
/// meaningful `PartialEq`, which component props require, and the editor's
/// other view functions (`render_canvas_area`) are plain for the same reason.
///
/// # Touch target
///
/// No interactive controls — this view is read-only (see the module docs), so
/// WCAG 2.5.8 has nothing to measure here. The scroll container is the whole
/// view.
pub(super) fn dom_reflow_view(
    doc_state: &Arc<Mutex<DocumentState>>,
    cursor_state: Signal<crate::editing::cursor::CursorState>,
) -> Element {
    // **The read is the subscription.** `doc_state` is a mutex, not reactive
    // state, so reading the document through it does not tell Dioxus to
    // re-render when the document changes — this view would paint what it first
    // saw and never update. `post_mutation_sync` writes the document generation
    // into `cursor_state` after every mutation, so reading it here is what
    // subscribes this scope to them. The value itself is not needed.
    let _generation = cursor_state.read().document_generation;

    let Ok(state) = doc_state.lock() else {
        return rsx! {};
    };
    let Some(doc) = state.document.as_ref() else {
        return rsx! {};
    };
    let max_w = column_max_width_pt();
    // Substitution is `loki-layout`'s, applied here so both paths pick the same
    // face for a font the host does not have. Resolved once per render into a
    // map rather than per run: `resolve_font_name` takes `&mut FontResources`,
    // and holding that lock across the render would put a shaping mutex in the
    // middle of the UI thread's tree build.
    let families = resolve_families(&state.shared_font_resources, doc);

    document_view(doc, &families, max_w)
}

/// The reading column for one document, at one column width.
///
/// Split out from [`dom_reflow_view`] so the *tree* can be built without the
/// editor's state around it. That is what lets the styled-document line-break
/// comparison (`loki-text/examples/styled_linebreak_probe.rs`, ADR-0017 §5.3
/// step 3) render through **this** code rather than through a second emitter:
/// a probe with its own copy of the CSS would agree with the canvas path
/// exactly as far as the copy did, which is the one thing the comparison must
/// not depend on.
pub fn document_view(
    doc: &loki_doc_model::document::Document,
    families: &content::FamilyMap,
    max_w: f32,
) -> Element {
    rsx! {
        div {
            // The scroll container. `overflow-x: hidden` is T7.3's rule stated
            // in one declaration: whatever an element does inside, the document
            // itself does not scroll sideways.
            style: "flex: 1; overflow-y: auto; overflow-x: hidden; \
                    background: #ffffff; color: #000000;",
            div {
                // The reading column: the measure, centred. This is the line
                // ADR-0017 is about — on the canvas path the same decision needs
                // a resolved cap in a process-wide static, read by four call
                // sites across two crates.
                // No `font-family` here on purpose. The column is a box, not a
                // typeface: the document's own character properties supply the
                // face per run (`style::char_css`), and hardcoding one would
                // override every document with whatever this line happened to
                // say — which is the r94 defect in miniature.
                style: format!(
                    "max-width: {max_w}pt; margin: 0 auto; padding: 24pt 18pt; \
                     font-size: 12pt;"
                ),
                for (si, section) in doc.sections.iter().enumerate() {
                    for (bi, block) in section.blocks.iter().enumerate() {
                        { rsx! { div { key: "{si}-{bi}", { content::block_el(block, &doc.styles, families) } } } }
                    }
                }
            }
        }
    }
}

/// Resolves every family the document asks for through `loki-layout`'s
/// substitution, so a font the host lacks lands on the same face here as on the
/// canvas path.
///
/// Returns an empty map when the font lock is unavailable: the requested names
/// are then emitted as-is, which is the pre-substitution behaviour and visibly
/// wrong in the same way for every run, rather than wrong for some.
///
/// # The resolved name has to be resolvable *here* too
///
/// `resolve_font_name` answers with a family from `loki-layout`'s collection.
/// Blitz has its own, and a name only the first can resolve is a name the DOM
/// path renders in Blitz's default face — measured, on this very document, when
/// a probe launched without the registration `main.rs` does: the canvas path set
/// the screenplay in Cousine and the DOM path in a proportional sans, and the
/// two disagreed at 14 of 18 widths (ADR-0017 §5.3).
///
/// It holds today because `main.rs` passes `loki_fonts::ui_font_blobs()` to
/// `Config::with_fonts`, and those blobs are the same bundled faces
/// `resolve_font_name` substitutes. It does **not** hold for a face
/// `FontResources::new` picks up from the executable-relative `assets/fonts/`
/// directory, which Blitz never scans. `TODO(dom-reflow-fonts)`: the DOM path
/// should emit the face it will actually get, not a name resolved against a
/// collection the renderer does not share.
pub fn resolve_families(
    fonts: &loki_layout::SharedFontResources,
    doc: &loki_doc_model::document::Document,
) -> content::FamilyMap {
    let requested = content::requested_families(doc);
    if requested.is_empty() {
        return content::FamilyMap::new();
    }
    let mut fonts = fonts.lock();
    requested
        .into_iter()
        .map(|name| {
            let resolved = fonts.resolve_font_name(&name);
            (name, resolved)
        })
        .collect()
}
