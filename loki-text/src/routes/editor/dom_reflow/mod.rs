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
//! # What this is, and is not, wired for editing
//!
//! **Click-to-place-caret, typing, Backspace/Delete/Enter, and Ctrl
//! shortcuts work** (`editing.rs`, `keydown.rs`) — the first slice of
//! `TODO(dom-reflow-editing)`, reusing the canvas-reflow path's own
//! hit-test/caret-geometry primitives (`ContinuousLayout::hit_test`,
//! `cursor_rect_canvas`) rather than a second implementation.
//!
//! **Still not built:** drag-selection and shift-extend (no selection
//! highlight is painted), arrow/Home/End caret navigation
//! (`TODO(dom-reflow-caret-nav)`, see `keydown.rs`'s module docs), hit-testing
//! into nested containers (table cells, note bodies), spell squiggles, and
//! revision marks. Those are painted from `PositionedItem`s on the canvas
//! path and each needs its own DOM equivalent — ADR-0017 §3.2 lists them.

use std::sync::{Arc, Mutex};

use dioxus::prelude::*;

use crate::editing::cursor::CursorState;
use crate::editing::state::DocumentState;

pub mod content;
mod content_para;
mod editing;
mod image;
mod keydown;
mod list;
mod oversized;
pub mod style;
mod table;

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

/// The mutation-side signals [`dom_reflow_view`] needs beyond `doc_state` and
/// `cursor_state`, bundled into one parameter so the call site in
/// `editor_canvas.rs` — a baselined 300-line-ceiling file
/// (`scripts/file-ceiling-baseline.txt`) that must not grow — stays three
/// arguments instead of seven.
#[derive(Clone, Copy)]
pub(super) struct EditorSignals {
    pub(super) loro_doc: Signal<Option<loro::LoroDoc>>,
    pub(super) undo_manager: Signal<Option<loro::UndoManager>>,
    pub(super) can_undo: Signal<bool>,
    pub(super) can_redo: Signal<bool>,
    pub(super) save_request: Signal<u32>,
}

/// The DOM reflow view: the document's blocks in a centred reading column,
/// with click-to-place-caret, typing and the caret overlay wired in (see the
/// module docs for what is and is not covered).
///
/// A plain function, not a `#[component]`: `Arc<Mutex<DocumentState>>` has no
/// meaningful `PartialEq`, which component props require, and the editor's
/// other view functions (`render_canvas_area`) are plain for the same reason.
///
/// # Touch target
///
/// Discrete controls: the fit/expand toggle on an oversized element
/// ([`oversized::AtOversized`], 44 × 44). The reading column itself is a
/// text-editing surface, not a discrete control — WCAG 2.5.8 does not apply
/// to continuous text input, the same reasoning the canvas path's own text
/// area uses.
pub(super) fn dom_reflow_view(
    doc_state: &Arc<Mutex<DocumentState>>,
    cursor_state: Signal<CursorState>,
    sig: EditorSignals,
) -> Element {
    let EditorSignals {
        loro_doc,
        undo_manager,
        can_undo,
        can_redo,
        save_request,
    } = sig;
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
    let ctx = editing::EditingCtx {
        doc_state: Arc::clone(doc_state),
        cursor_state,
        loro_doc,
        undo_manager,
        can_undo,
        can_redo,
        save_request,
    };
    let click_ctx = ctx.clone();

    rsx! {
        // The scroll container. `overflow-x: hidden` is T7.3's rule stated
        // in one declaration: whatever an element does inside, the document
        // itself does not scroll sideways.
        div {
            style: "flex: 1; overflow-y: auto; overflow-x: hidden; \
                    background: #ffffff; color: #000000;",
            div {
                // The reading column: the measure, centred. This is the line
                // ADR-0017 is about — on the canvas path the same decision needs
                // a resolved cap in a process-wide static, read by four call
                // sites across two crates.
                style: format!("max-width: {max_w}pt; margin: 0 auto; padding: 24pt 18pt;"),
                div {
                    // `position: relative` makes this the containing block for
                    // the caret overlay, and (with no padding of its own) its
                    // border edge is exactly `ContinuousLayout`'s (0, 0) — see
                    // `editing.rs`'s module docs for why that means the click
                    // handler needs no origin computation.
                    // No `font-family` here on purpose. The column is a box, not
                    // a typeface: the document's own character properties supply
                    // the face per run (`style::char_css`), and hardcoding one
                    // would override every document with whatever this line
                    // happened to say — which is the r94 defect in miniature.
                    style: "position: relative; font-size: 12pt;",
                    // 44×44 touch target: N/A — this is a text-editing surface,
                    // not a discrete control; WCAG 2.5.8 does not apply to
                    // continuous text input.
                    tabindex: "0",
                    autofocus: true,
                    onclick: move |evt: MouseEvent| {
                        let c = evt.element_coordinates();
                        editing::place_caret_at_click(&click_ctx, c.x as f32, c.y as f32, max_w);
                    },
                    onkeydown: keydown::make_dom_reflow_keydown_handler(ctx.clone()),
                    { blocks_el(doc, &families) }
                    { editing::caret_el(&ctx, max_w) }
                }
            }
        }
    }
}

/// The reading column for one document, at one column width.
///
/// Used by the styled-document line-break comparison
/// (`loki-text/examples/styled_linebreak_probe.rs`, ADR-0017 §5.3 step 3),
/// which renders a document with no editor around it — so this stays a plain
/// read-only shell. [`blocks_el`] is the part it shares with
/// [`dom_reflow_view`]: rendering through **this** code rather than through a
/// second emitter is what lets a comparison between the two paths be a
/// comparison of *rendering*, not of two copies of the CSS.
pub fn document_view(
    doc: &loki_doc_model::document::Document,
    families: &content::FamilyMap,
    max_w: f32,
) -> Element {
    rsx! {
        div {
            style: "flex: 1; overflow-y: auto; overflow-x: hidden; \
                    background: #ffffff; color: #000000;",
            div {
                style: format!(
                    "max-width: {max_w}pt; margin: 0 auto; padding: 24pt 18pt; \
                     font-size: 12pt;"
                ),
                { blocks_el(doc, families) }
            }
        }
    }
}

/// Every block of every section, in document order — the part
/// [`document_view`] and [`dom_reflow_view`] share.
fn blocks_el(doc: &loki_doc_model::document::Document, families: &content::FamilyMap) -> Element {
    rsx! {
        for (si, section) in doc.sections.iter().enumerate() {
            for (bi, block) in section.blocks.iter().enumerate() {
                { rsx! { div { key: "{si}-{bi}", { content::block_el(block, &doc.styles, families) } } } }
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
