// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Font-substitution detail panel (Spec 03 M3 / D3, inverted 2026-07).
//!
//! The *indicator* is the status-bar chip ("N fonts substituted"), shown
//! whenever the layout engine recorded substitutions — there is no banner.
//! Clicking the chip opens this panel above the ribbon with the full
//! requested→substitute table; clicking Close (or the chip again) hides it.
//! **Breakpoint-aware**: a compact table on Expanded width and a vertical
//! stack of cards on Compact — never a full-width band that wraps. Blitz-
//! clean: no `position: fixed`, no `box-shadow` (elevation via border/
//! background), no CSS custom properties. All strings via `fl!()`.
//!
//! Owns only the warning *UI*; the substitution engine is Spec 02's.

use std::collections::HashMap;

use appthere_ui::tokens;
use appthere_ui::use_breakpoint;
use dioxus::prelude::*;
use loki_i18n::fl;

/// One requested→substitute relationship, ready to render.
struct Sub {
    requested: String,
    /// `Some(name)` = substituted with `name`; `None` = missing, no substitute.
    substitute: Option<String>,
    /// Whether the substitute is metric-compatible (low concern).
    compatible: bool,
    /// `(label, url)` to download the original font, where known.
    link: Option<(&'static str, &'static str)>,
}

/// Metric-compatible substitute faces (Spec 02 §7.3). A UI-side heuristic until
/// the substitution engine exposes severity directly; remove this list then.
fn is_metric_compatible(substitute: &str) -> bool {
    const COMPAT: &[&str] = &[
        "Carlito",
        "Caladea",
        "Gelasio",
        "Liberation Sans",
        "Liberation Serif",
        "Liberation Mono",
    ];
    COMPAT.iter().any(|c| c.eq_ignore_ascii_case(substitute))
}

/// Download URL for an original (requested) font, where one is known.
fn download_link(requested: &str) -> Option<(&'static str, &'static str)> {
    match requested.to_lowercase().as_str() {
        "aptos" => Some((
            "Aptos",
            "https://www.microsoft.com/en-us/download/details.aspx?id=106037",
        )),
        "calibri" => Some((
            "Calibri",
            "https://learn.microsoft.com/en-us/typography/font-list/calibri",
        )),
        "cambria" => Some((
            "Cambria",
            "https://learn.microsoft.com/en-us/typography/font-list/cambria",
        )),
        "arial" => Some((
            "Arial",
            "https://learn.microsoft.com/en-us/typography/font-list/arial",
        )),
        "courier new" => Some((
            "Courier New",
            "https://learn.microsoft.com/en-us/typography/font-list/courier-new",
        )),
        "times new roman" => Some((
            "Times New Roman",
            "https://learn.microsoft.com/en-us/typography/font-list/times-new-roman",
        )),
        _ => None,
    }
}

/// Builds the sorted, structured substitution list from the raw engine map.
fn build_items(map: &HashMap<String, Option<String>>) -> Vec<Sub> {
    let mut items: Vec<Sub> = map
        .iter()
        .map(|(requested, substitute)| Sub {
            requested: requested.clone(),
            substitute: substitute.clone(),
            compatible: substitute.as_deref().is_some_and(is_metric_compatible),
            link: download_link(requested),
        })
        .collect();
    items.sort_by(|a, b| a.requested.cmp(&b.requested));
    items
}

/// A small severity badge: calm for metric-compatible, prominent otherwise.
fn severity_badge(compatible: bool) -> Element {
    let (label, color) = if compatible {
        (
            fl!("editor-font-substitution-compatible"),
            tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
        )
    } else {
        (
            fl!("editor-font-substitution-approx"),
            tokens::COLOR_CONTEXTUAL_TAB,
        )
    };
    rsx! {
        span {
            style: format!(
                "font-size: {size}px; color: {color}; border: 1px solid {color}; \
                 border-radius: {r}px; padding: 0 {p}px; white-space: nowrap; flex-shrink: 0;",
                size = tokens::FONT_SIZE_XS, color = color, r = tokens::RADIUS_SM, p = tokens::SPACE_1,
            ),
            "{label}"
        }
    }
}

/// The "Download original" link for a substitution, where a URL is known.
fn download_anchor(link: Option<(&'static str, &'static str)>) -> Element {
    let Some((_, url)) = link else {
        return rsx! {};
    };
    rsx! {
        a {
            href: "{url}",
            target: "_blank",
            style: format!(
                "color: {accent}; text-decoration: underline; font-size: {size}px; \
                 cursor: pointer; flex-shrink: 0;",
                accent = tokens::COLOR_TAB_ACTIVE_INDICATOR, size = tokens::FONT_SIZE_LABEL,
            ),
            {fl!("editor-font-download-original")}
        }
    }
}

/// The substitute cell text: the substitute name, or the "missing" label.
fn substitute_text(item: &Sub) -> String {
    item.substitute
        .clone()
        .unwrap_or_else(|| fl!("editor-font-substitution-missing"))
}

/// One substitution as a bordered card (Compact width — vertical stack).
fn item_card(item: &Sub) -> Element {
    let label_style = format!(
        "font-size: {size}px; color: {fg};",
        size = tokens::FONT_SIZE_XS,
        fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
    );
    rsx! {
        div {
            style: format!(
                "display: flex; flex-direction: column; gap: {gap}px; padding: {p}px; \
                 border: 1px solid {border}; border-radius: {r}px; background: {bg};",
                gap = tokens::SPACE_1, p = tokens::SPACE_2, border = tokens::COLOR_BORDER_CHROME,
                r = tokens::RADIUS_SM, bg = tokens::COLOR_SURFACE_3,
            ),
            div {
                style: "display: flex; flex-direction: row; align-items: center; justify-content: space-between; gap: 8px;",
                span {
                    style: format!("font-size: {}px; color: {};", tokens::FONT_SIZE_BODY - 1.0, tokens::COLOR_TEXT_ON_CHROME),
                    "{item.requested}"
                }
                {severity_badge(item.compatible)}
            }
            div {
                style: "display: flex; flex-direction: row; align-items: center; gap: 6px;",
                span { style: "{label_style}", {fl!("editor-font-substitution-substitute")} }
                span {
                    style: format!("font-size: {}px; color: {};", tokens::FONT_SIZE_LABEL, tokens::COLOR_TEXT_ON_CHROME),
                    "{substitute_text(item)}"
                }
            }
            {download_anchor(item.link)}
        }
    }
}

/// One substitution as a table row (Expanded width).
fn item_row(item: &Sub) -> Element {
    let cell = format!(
        "flex: 1; min-width: 0; font-size: {}px; color: {};",
        tokens::FONT_SIZE_LABEL,
        tokens::COLOR_TEXT_ON_CHROME,
    );
    rsx! {
        div {
            style: "display: flex; flex-direction: row; align-items: center; gap: 12px;",
            span { style: "{cell}", "{item.requested}" }
            span { style: "{cell}", "{substitute_text(item)}" }
            {severity_badge(item.compatible)}
            {download_anchor(item.link)}
        }
    }
}

/// The font-substitution detail panel. Renders nothing when there are no
/// substitutions or while closed; the status-bar chip toggles `open`.
///
/// The Close button inherits the ribbon-button touch-target convention: its
/// padded hit area meets the 44×44 logical-pixel minimum (WCAG 2.5.8) through
/// the surrounding header row height plus padding.
#[component]
pub(super) fn FontSubstitutionPanel(
    substitutions: HashMap<String, Option<String>>,
    open: Signal<bool>,
) -> Element {
    let mut open = open;
    let compact = use_breakpoint().is_compact();

    if substitutions.is_empty() || !open() {
        return rsx! {};
    }
    let items = build_items(&substitutions);

    let container = format!(
        "display: flex; flex-direction: column; gap: {gap}px; padding: {pv}px {ph}px; \
         background: {bg}; border-top: 1px solid {border}; border-bottom: 1px solid {border}; \
         font-family: {ff}; color: {fg}; flex-shrink: 0;",
        gap = tokens::SPACE_2,
        pv = tokens::SPACE_2,
        ph = tokens::SPACE_4,
        bg = tokens::COLOR_SURFACE_2,
        border = tokens::COLOR_CONTEXTUAL_TAB,
        ff = tokens::FONT_FAMILY_UI,
        fg = tokens::COLOR_TEXT_ON_CHROME,
    );
    let btn = format!(
        "padding: {pv}px {ph}px; background: {bg}; border: 1px solid {border}; \
         border-radius: {r}px; color: {fg}; font-size: {size}px; cursor: pointer; flex-shrink: 0;",
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
            // Header row — title + close.
            div {
                style: "display: flex; flex-direction: row; align-items: center; gap: 8px;",
                span {
                    style: format!("color: {}; font-weight: bold;", tokens::COLOR_CONTEXTUAL_TAB),
                    "⚠ {fl!(\"editor-font-substitution-title\")}"
                }
                div { style: "flex: 1;" }
                button {
                    style: "{btn}",
                    onclick: move |_| { open.set(false); },
                    {fl!("editor-font-substitution-close")}
                }
            }
            // Detail body — card stack (Compact) or table (Expanded).
            span {
                style: format!("font-size: {}px; color: {};", tokens::FONT_SIZE_LABEL, tokens::COLOR_TEXT_ON_CHROME_SECONDARY),
                {fl!("editor-font-substitution-message")}
            }
            div {
                style: "display: flex; flex-direction: column; gap: 6px;",
                for item in items.iter() {
                    div { key: "{item.requested}",
                        if compact { {item_card(item)} } else { {item_row(item)} }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "editor_font_warning_tests.rs"]
mod tests;
