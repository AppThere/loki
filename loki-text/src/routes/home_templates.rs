// SPDX-License-Identifier: Apache-2.0

//! Template catalog + accepted MIME types for the Home screen (split from
//! `home.rs` for the 300-line ceiling). `MIME_TYPES` gates the file picker;
//! `make_templates` builds the localized builtin-template gallery cards. Both
//! are re-imported by `home.rs`.

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

// ── Template data ─────────────────────────────────────────────────────────────

// Gallery card 0 is the plain Blank document; cards 1..=5 are the bundled
// templates, in the same order as the `on_template_select` match below.
pub(super) fn make_templates() -> Vec<BuiltinTemplate> {
    let dotx = || fl!("home-template-format-dotx");
    vec![
        BuiltinTemplate {
            name: fl!("home-template-blank"),
            description: fl!("home-template-blank-description"),
            format_label: fl!("home-template-blank-format"),
        },
        BuiltinTemplate {
            name: fl!("home-template-markdown"),
            description: fl!("home-template-markdown-description"),
            format_label: dotx(),
        },
        BuiltinTemplate {
            name: fl!("home-template-apa"),
            description: fl!("home-template-apa-description"),
            format_label: dotx(),
        },
        BuiltinTemplate {
            name: fl!("home-template-mla"),
            description: fl!("home-template-mla-description"),
            format_label: dotx(),
        },
        BuiltinTemplate {
            name: fl!("home-template-screenplay"),
            description: fl!("home-template-screenplay-description"),
            format_label: dotx(),
        },
        BuiltinTemplate {
            name: fl!("home-template-resume"),
            description: fl!("home-template-resume-description"),
            format_label: dotx(),
        },
    ]
}
