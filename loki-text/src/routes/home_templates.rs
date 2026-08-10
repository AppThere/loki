// SPDX-License-Identifier: Apache-2.0

//! Template catalog + accepted MIME types, shared by the Home screen's gallery
//! and the editor ribbon's New menu (usage audit §4). `MIME_TYPES` gates the
//! file picker; `make_templates` builds the localized builtin-template gallery
//! cards.
//!
//! # `loki_templates::TEMPLATES` is the id list
//!
//! The gallery order and the template ids come from the crate that owns them —
//! this module holds only the *localisation* of each known id (the `fl!` macro
//! needs literal keys, so the id→key mapping is a match). A template added to
//! `TEMPLATES` therefore appears in the gallery and the New menu immediately,
//! under its English name until the match learns its key — previously the id
//! list was duplicated here card-by-card, a second source of truth that showed
//! nothing at all for an unmatched template.

use appthere_ui::BuiltinTemplate;
use loki_i18n::fl;

// ── MIME types accepted by the file picker ────────────────────────────────────

pub(super) const MIME_TYPES: &[&str] = &[
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "application/vnd.oasis.opendocument.text",
    // Templates — opened as fresh, detached documents.
    "application/vnd.openxmlformats-officedocument.wordprocessingml.template", // .dotx
    "application/vnd.ms-word.template.macroEnabled.12",                        // .dotm
    "application/vnd.oasis.opendocument.text-template",                        // .ott
];

/// MIME types the **Browse templates** picker accepts — the template subset of
/// [`MIME_TYPES`], so that dialog offers only files this app would open as a
/// detached copy.
///
/// `.dotm` rides along with `.dotx`: it is the same template, macro-enabled,
/// and `is_template_name` already treats the two identically — a filter that
/// omitted it would hide files the open path handles perfectly well.
pub(super) const TEMPLATE_MIME_TYPES: &[&str] = &[
    "application/vnd.openxmlformats-officedocument.wordprocessingml.template", // .dotx
    "application/vnd.ms-word.template.macroEnabled.12",                        // .dotm
    "application/vnd.oasis.opendocument.text-template",                        // .ott
];

// ── Template data ─────────────────────────────────────────────────────────────

/// The localized display name for a bundled template id, falling back to the
/// crate's English name (and, for an id the crate does not know, the id) so an
/// unmatched template is still *named* rather than invisible.
pub(super) fn template_display_name(id: &str) -> String {
    match id {
        "markdown" => fl!("home-template-markdown"),
        "apa" => fl!("home-template-apa"),
        "mla" => fl!("home-template-mla"),
        "screenplay" => fl!("home-template-screenplay"),
        "resume" => fl!("home-template-resume"),
        _ => loki_templates::TEMPLATES
            .iter()
            .find(|t| t.id == id)
            .map_or_else(|| id.to_string(), |t| t.name.to_string()),
    }
}

/// The localized gallery description for a bundled template id (empty for an
/// id the match does not know — the card renders without a description).
fn template_description(id: &str) -> String {
    match id {
        "markdown" => fl!("home-template-markdown-description"),
        "apa" => fl!("home-template-apa-description"),
        "mla" => fl!("home-template-mla-description"),
        "screenplay" => fl!("home-template-screenplay-description"),
        "resume" => fl!("home-template-resume-description"),
        _ => String::new(),
    }
}

/// Gallery card 0 is the plain Blank document; cards `1..` are
/// `loki_templates::TEMPLATES`, in its order — `Home::on_template_select`
/// indexes the same list, so the two cannot disagree.
pub(super) fn make_templates() -> Vec<BuiltinTemplate> {
    let mut cards = vec![BuiltinTemplate {
        name: fl!("home-template-blank"),
        description: fl!("home-template-blank-description"),
        format_label: fl!("home-template-blank-format"),
    }];
    cards.extend(loki_templates::TEMPLATES.iter().map(|t| BuiltinTemplate {
        name: template_display_name(t.id),
        description: template_description(t.id),
        format_label: fl!("home-template-format-dotx"),
    }));
    cards
}

#[cfg(test)]
#[path = "home_templates_tests.rs"]
mod tests;
