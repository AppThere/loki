// SPDX-License-Identifier: Apache-2.0

//! Publish ribbon tab: export to PDF/X and EPUB 3.3.
//!
//! [`publish_tab_content`] builds the ribbon button row; [`publish_panel`]
//! renders the inline PDF/X conformance-level picker above the ribbon. Both the
//! direct EPUB export and the PDF export run through [`run_export`], which
//! picks a destination via the platform save dialog and writes the serialised
//! bytes in one shot.

use std::io::{Cursor, Write};
use std::sync::{Arc, Mutex};

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_doc_model::io::DocumentExport;
use loki_epub::{EpubExport, EpubOptions};
use loki_file_access::{FilePicker, SaveOptions};
use loki_i18n::fl;
use loki_pdf::{PdfXLevel, PdfXOptions};

use crate::editing::state::DocumentState;
use crate::utils::display_title_from_path;

/// Output format selected from the Publish tab.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum PublishFormat {
    /// PDF/X at the chosen conformance level.
    Pdf(PdfXLevelChoice),
    /// EPUB 3.3.
    Epub,
}

/// Copyable mirror of [`PdfXLevel`] so it can live in a `Signal`.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum PdfXLevelChoice {
    #[default]
    X1a,
    X3,
    X4,
}

impl From<PdfXLevelChoice> for PdfXLevel {
    fn from(c: PdfXLevelChoice) -> Self {
        match c {
            PdfXLevelChoice::X1a => PdfXLevel::X1a,
            PdfXLevelChoice::X3 => PdfXLevel::X3,
            PdfXLevelChoice::X4 => PdfXLevel::X4,
        }
    }
}

/// Inline panel for picking the PDF/X conformance level and exporting.
pub(super) fn publish_panel(
    doc_state: Arc<Mutex<DocumentState>>,
    path_signal: Signal<String>,
    save_message: Signal<Option<String>>,
    mut is_publish_panel_open: Signal<bool>,
    mut pdf_level: Signal<PdfXLevelChoice>,
) -> Element {
    if !is_publish_panel_open() {
        return rsx! {};
    }
    let ds_export = Arc::clone(&doc_state);
    let levels = [
        (PdfXLevelChoice::X1a, fl!("publish-level-x1a")),
        (PdfXLevelChoice::X3, fl!("publish-level-x3")),
        (PdfXLevelChoice::X4, fl!("publish-level-x4")),
    ];

    rsx! {
        div {
            style: format!(
                "display: flex; flex-direction: column; flex-shrink: 0; gap: {g}px; \
                 padding: {p}px {p2}px; background: {bg}; border-top: 1px solid {border};",
                g = tokens::SPACE_2,
                p = tokens::SPACE_2,
                p2 = tokens::SPACE_4,
                bg = tokens::COLOR_SURFACE_1,
                border = tokens::COLOR_BORDER_CHROME,
            ),
            span {
                style: format!(
                    "font-family: {ff}; font-size: {fs}px; font-weight: {fw}; color: {fg};",
                    ff = tokens::FONT_FAMILY_UI,
                    fs = tokens::FONT_SIZE_LABEL,
                    fw = tokens::FONT_WEIGHT_MEDIUM,
                    fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                ),
                { fl!("publish-panel-title") }
            }
            div {
                style: "display: flex; flex-direction: row; align-items: center; gap: 8px; flex-wrap: wrap;",
                for (choice, text) in levels {
                    button {
                        style: level_button_style(pdf_level() == choice),
                        onclick: move |_| pdf_level.set(choice),
                        "{text}"
                    }
                }
            }
            div {
                style: "display: flex; flex-direction: row; align-items: center; gap: 8px;",
                button {
                    style: panel_action_style(true),
                    onclick: move |_| {
                        let level = pdf_level();
                        run_export(
                            &ds_export,
                            PublishFormat::Pdf(level),
                            &path_signal.peek(),
                            save_message,
                        );
                        is_publish_panel_open.set(false);
                    },
                    { fl!("publish-export-button") }
                }
                button {
                    style: panel_action_style(false),
                    onclick: move |_| is_publish_panel_open.set(false),
                    { fl!("publish-cancel-button") }
                }
            }
        }
    }
}

/// Picks a destination and writes the document in `format` to it.
pub(super) fn run_export(
    doc_state: &Arc<Mutex<DocumentState>>,
    format: PublishFormat,
    cur_path: &str,
    mut save_message: Signal<Option<String>>,
) {
    let doc = match doc_state.lock().ok().and_then(|s| s.document.clone()) {
        Some(d) => d,
        None => {
            save_message.set(Some(fl!("publish-export-error", reason = "no document")));
            return;
        }
    };
    let (mime, ext) = match format {
        PublishFormat::Pdf(_) => ("application/pdf", "pdf"),
        PublishFormat::Epub => ("application/epub+zip", "epub"),
    };
    let suggested = format!("{}.{ext}", display_title_from_path(cur_path));

    spawn(async move {
        let picker = FilePicker::new();
        let opts = SaveOptions {
            mime_type: Some(mime.to_string()),
            suggested_name: Some(suggested),
        };
        match picker.pick_file_to_save(opts).await {
            Ok(Some(token)) => {
                let bytes = match serialize(&doc, format) {
                    Ok(b) => b,
                    Err(e) => {
                        save_message.set(Some(fl!("publish-export-error", reason = e)));
                        return;
                    }
                };
                match token.open_write() {
                    Ok(mut w) => match w.write_all(&bytes) {
                        Ok(()) => save_message.set(Some(fl!("publish-export-success"))),
                        Err(e) => save_message
                            .set(Some(fl!("publish-export-error", reason = e.to_string()))),
                    },
                    Err(e) => {
                        save_message.set(Some(fl!("publish-export-error", reason = e.to_string())))
                    }
                }
            }
            Ok(None) => save_message.set(Some(fl!("publish-export-cancelled"))),
            Err(e) => save_message.set(Some(fl!("publish-export-error", reason = e.to_string()))),
        }
    });
}

/// Serialises `doc` into the chosen format's bytes.
fn serialize(doc: &loki_doc_model::Document, format: PublishFormat) -> Result<Vec<u8>, String> {
    match format {
        PublishFormat::Pdf(level) => {
            let opts = PdfXOptions {
                level: level.into(),
                ..Default::default()
            };
            let mut out = Vec::new();
            loki_pdf::export_document(doc, &opts, &mut out).map_err(|e| e.to_string())?;
            Ok(out)
        }
        PublishFormat::Epub => {
            let mut buf = Cursor::new(Vec::new());
            EpubExport::export(doc, &mut buf, EpubOptions::default()).map_err(|e| e.to_string())?;
            Ok(buf.into_inner())
        }
    }
}

fn level_button_style(selected: bool) -> String {
    let (bg, fg) = if selected {
        (tokens::COLOR_TAB_ACTIVE_BG, tokens::COLOR_TEXT_ACCENT)
    } else {
        (tokens::COLOR_SURFACE_2, tokens::COLOR_TEXT_ON_CHROME)
    };
    format!(
        "min-height: 28px; padding: 0 {p}px; background: {bg}; border: 1px solid {border}; \
         border-radius: {r}px; font-family: {ff}; font-size: {fs}px; color: {fg}; cursor: pointer;",
        p = tokens::SPACE_2,
        bg = bg,
        border = tokens::COLOR_BORDER_DEFAULT,
        r = tokens::RADIUS_SM,
        ff = tokens::FONT_FAMILY_UI,
        fs = tokens::FONT_SIZE_LABEL,
        fg = fg,
    )
}

fn panel_action_style(primary: bool) -> String {
    let (bg, fg) = if primary {
        (tokens::COLOR_TAB_ACTIVE_BG, tokens::COLOR_TEXT_ACCENT)
    } else {
        (tokens::COLOR_SURFACE_3, tokens::COLOR_TEXT_ON_CHROME)
    };
    format!(
        "min-height: 28px; padding: 0 {p}px; background: {bg}; border: 1px solid {border}; \
         border-radius: {r}px; font-family: {ff}; font-size: {fs}px; color: {fg}; cursor: pointer;",
        p = tokens::SPACE_3,
        bg = bg,
        border = tokens::COLOR_BORDER_CHROME,
        r = tokens::RADIUS_SM,
        ff = tokens::FONT_FAMILY_UI,
        fs = tokens::FONT_SIZE_LABEL,
        fg = fg,
    )
}
