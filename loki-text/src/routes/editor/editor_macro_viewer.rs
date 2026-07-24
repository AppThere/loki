// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Read-only macro source viewer (macro spec §9.6, Phase 3).
//!
//! Lists the document's macro modules and shows the selected module's source in
//! a monospace, **read-only** block. This is visibility, not execution: Loki
//! has no execution surface yet, and the source shown for VBA is extracted from
//! the decompressed source streams only (never p-code). A tamper warning is
//! surfaced above the source when the project appears VBA-stomped.
//!
//! Rendered in-flow above the ribbon (like the font-substitution panel); no
//! `position: fixed`. All strings via `fl!()`.

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_i18n::fl;

use super::editor_macro_notice::MacroView;

/// Monospace stack for source display (generic families; no bundled font
/// dependency).
const MONO: &str = "ui-monospace, 'Cascadia Code', Consolas, 'Courier New', monospace";

/// The read-only macro viewer panel.
///
/// Touch targets: the module tabs and Close button meet the 44×44 logical-pixel
/// minimum (WCAG 2.5.8) via `min-height`/padding.
#[component]
pub(super) fn MacroViewerPanel(view: MacroView, on_close: EventHandler<()>) -> Element {
    let mut selected = use_signal(|| 0usize);
    let modules = view.modules.clone();
    let idx = selected().min(modules.len().saturating_sub(1));
    let source = modules
        .get(idx)
        .map(|m| m.source.clone())
        .unwrap_or_default();

    let container = format!(
        "display: flex; flex-direction: column; gap: {gap}px; padding: {pv}px {ph}px; \
         background: {bg}; border-top: 1px solid {border}; border-bottom: 1px solid {border}; \
         font-family: {ff}; color: {fg}; flex-shrink: 0; max-height: 45vh;",
        gap = tokens::SPACE_2,
        pv = tokens::SPACE_2,
        ph = tokens::SPACE_4,
        bg = tokens::COLOR_SURFACE_2,
        border = tokens::COLOR_BORDER_CHROME,
        ff = tokens::FONT_FAMILY_UI,
        fg = tokens::COLOR_TEXT_ON_CHROME,
    );
    let close_btn = format!(
        "min-height: {th}px; padding: {pv}px {ph}px; background: {bg}; \
         border: 1px solid {border}; border-radius: {r}px; color: {fg}; \
         font-size: {size}px; cursor: pointer; flex-shrink: 0;",
        th = tokens::TOUCH_MIN,
        pv = tokens::SPACE_1,
        ph = tokens::SPACE_2,
        bg = tokens::COLOR_SURFACE_3,
        border = tokens::COLOR_BORDER_CHROME,
        r = tokens::RADIUS_SM,
        fg = tokens::COLOR_TEXT_ON_CHROME,
        size = tokens::FONT_SIZE_LABEL,
    );

    rsx! {
        div { style: "{container}",
            // Header: title + close.
            div {
                style: "display: flex; flex-direction: row; align-items: center; gap: 8px;",
                span {
                    style: format!("font-weight: bold; font-size: {}px;", tokens::FONT_SIZE_MD),
                    {fl!("macros-viewer-title")}
                }
                div { style: "flex: 1;" }
                button {
                    style: "{close_btn}",
                    onclick: move |_| on_close.call(()),
                    {fl!("macros-viewer-close")}
                }
            }

            // Tamper warning (VBA stomping), if any.
            if view.tamper.is_some() {
                div {
                    style: format!(
                        "border-left: 3px solid {c}; padding: {p}px {p2}px; color: {c}; font-size: {s}px;",
                        c = tokens::COLOR_CONTEXTUAL_TAB, p = tokens::SPACE_1, p2 = tokens::SPACE_2,
                        s = tokens::FONT_SIZE_LABEL,
                    ),
                    "⚠ {fl!(\"macros-viewer-tamper\")}"
                }
            }

            if modules.is_empty() {
                span {
                    style: format!("font-size: {}px; color: {};", tokens::FONT_SIZE_LABEL, tokens::COLOR_TEXT_ON_CHROME_SECONDARY),
                    {fl!("macros-viewer-empty")}
                }
            } else {
                // Module tabs.
                div {
                    style: "display: flex; flex-direction: row; flex-wrap: wrap; gap: 6px;",
                    for (i, m) in modules.iter().enumerate() {
                        button {
                            key: "{i}",
                            style: module_tab_style(i == idx),
                            onclick: move |_| selected.set(i),
                            "{m.name}"
                        }
                    }
                }
                // Read-only source.
                pre {
                    style: format!(
                        "margin: 0; overflow: auto; white-space: pre; \
                         font-family: {mono}; font-size: {size}px; color: {fg}; \
                         background: {bg}; border: 1px solid {border}; border-radius: {r}px; \
                         padding: {p}px; flex: 1;",
                        mono = MONO, size = tokens::FONT_SIZE_LABEL, fg = tokens::COLOR_TEXT_ON_CHROME,
                        bg = tokens::COLOR_SURFACE_1, border = tokens::COLOR_BORDER_CHROME,
                        r = tokens::RADIUS_SM, p = tokens::SPACE_2,
                    ),
                    "{source}"
                }
            }
        }
    }
}

fn module_tab_style(active: bool) -> String {
    let border = if active {
        tokens::COLOR_TAB_ACTIVE_INDICATOR
    } else {
        tokens::COLOR_BORDER_CHROME
    };
    format!(
        "min-height: {th}px; padding: {pv}px {ph}px; background: {bg}; \
         border: 1px solid {border}; border-radius: {r}px; color: {fg}; \
         font-size: {size}px; cursor: pointer;",
        th = tokens::TOUCH_MIN,
        pv = tokens::SPACE_1,
        ph = tokens::SPACE_2,
        bg = tokens::COLOR_SURFACE_3,
        border = border,
        r = tokens::RADIUS_SM,
        fg = tokens::COLOR_TEXT_ON_CHROME,
        size = tokens::FONT_SIZE_LABEL,
    )
}
