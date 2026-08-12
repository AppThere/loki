// SPDX-License-Identifier: Apache-2.0

//! The Print dialog (usage audit §6): two routes to paper.
//!
//! - **System print dialog** — renders the live document to PDF and hands it
//!   to [`loki_print_dialog::SystemPrintDialog`]. On Linux the portal shows
//!   the printer/copies/pages dialog itself; on platforms without a backend
//!   the typed `Unsupported` error surfaces as an honest status message.
//! - **Network printer (IPP)** — the office-deployment route: a printer URI
//!   plus copies/pages/duplex, dispatched through `loki-print`'s blocking
//!   client. The POST runs inline in the spawned task, the same bounded
//!   tradeoff `run_export` takes for its serialize-and-write.
//!
//! Both routes render through the same serializer the Publish tab exports
//! with, so what prints is what a PDF/X export would contain.

use std::sync::{Arc, Mutex};

use appthere_ui::{AtCheckRow, AtDialogButton, AtDialogShell, AtField, DialogWidth, tokens};
use dioxus::prelude::*;
use loki_i18n::fl;
use loki_print::{Duplex, IppPrinter, PrintOptions};
use loki_print_dialog::{PrintDialogOutcome, SystemPrintDialog};

use super::editor_publish::{PdfXLevelChoice, PublishFormat, serialize};
use super::editor_state::SaveStatus;
use crate::editing::state::DocumentState;
use crate::utils::display_title_from_path;

/// Builds the IPP job options from the dialog's field strings. Pure, so the
/// mapping is testable: copies parse loosely (blank = 1), the page-range
/// string is passed through verbatim — `loki-print` validates it segment by
/// segment at attribute-build time and refuses the job with a typed error
/// before any bytes reach a printer.
pub(super) fn build_ipp_options(
    copies: &str,
    pages: &str,
    duplex: bool,
    job_title: String,
) -> PrintOptions {
    PrintOptions {
        copies: copies.trim().parse().unwrap_or(1),
        duplex: if duplex {
            Duplex::LongEdge
        } else {
            Duplex::Simplex
        },
        media: None,
        color: loki_print::ColorMode::Auto,
        job_title: Some(job_title),
        page_ranges: {
            let trimmed = pages.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        },
    }
}

/// Renders the current document to print-ready PDF bytes (PDF/X-3, the
/// Publish tab's default level).
fn render_pdf(doc_state: &Arc<Mutex<DocumentState>>) -> Result<Vec<u8>, String> {
    let doc = doc_state
        .lock()
        .ok()
        .and_then(|s| s.document.clone())
        .ok_or_else(|| "no document".to_string())?;
    serialize(&doc, PublishFormat::Pdf(PdfXLevelChoice::X3))
}

/// Props for [`PrintDialog`].
#[derive(Clone, Props)]
pub(super) struct PrintDialogProps {
    pub(super) doc_state: Arc<Mutex<DocumentState>>,
    /// `true` while the dialog is open.
    pub(super) open: Signal<bool>,
    /// The document's path, for the job title.
    pub(super) path: Signal<String>,
    /// Status-banner sink.
    pub(super) save_message: Signal<Option<SaveStatus>>,
}

impl PartialEq for PrintDialogProps {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.doc_state, &other.doc_state)
            && self.open == other.open
            && self.path == other.path
            && self.save_message == other.save_message
    }
}

/// The Print dialog (see the module docs).
///
/// # Touch target
///
/// Both action buttons and every field meet the 44 px minimum via
/// [`AtDialogButton`] / [`AtField`]'s own guarantees.
#[allow(non_snake_case)]
pub(super) fn PrintDialog(props: PrintDialogProps) -> Element {
    let PrintDialogProps {
        doc_state,
        mut open,
        path,
        mut save_message,
    } = props;

    let mut ipp_uri = use_signal(String::new);
    let mut ipp_copies = use_signal(|| "1".to_string());
    let mut ipp_pages = use_signal(String::new);
    let mut ipp_duplex = use_signal(|| false);

    let job_title = display_title_from_path(&path.read());
    let ds_system = Arc::clone(&doc_state);
    let ds_ipp = Arc::clone(&doc_state);
    let title_system = job_title.clone();
    let title_ipp = job_title.clone();

    rsx! {
        AtDialogShell {
            title: fl!("print-dialog-title"),
            subtitle: rsx! {},
            close_aria_label: fl!("print-dialog-close-aria"),
            width: DialogWidth::Narrow,
            on_close: move |_| open.set(false),
            tabs: rsx! {},
            body: rsx! {
                div {
                    style: format!(
                        "display: flex; flex-direction: column; gap: {g}px; padding: {p}px;",
                        g = tokens::SPACE_3,
                        p = tokens::SPACE_3,
                    ),

                    // ── System print dialog ───────────────────────────────────
                    div {
                        style: format!(
                            "font-size: {fs}px; color: {fg};",
                            fs = tokens::FONT_SIZE_LABEL,
                            fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                        ),
                        { fl!("print-system-hint") }
                    }
                    AtDialogButton {
                        label: fl!("print-system-button"),
                        primary: true,
                        min_touch_px: tokens::TOUCH_MIN,
                        on_click: move |_| {
                            let ds = Arc::clone(&ds_system);
                            let title = title_system.clone();
                            spawn(async move {
                                let pdf = match render_pdf(&ds) {
                                    Ok(b) => b,
                                    Err(e) => {
                                        save_message.set(Some(SaveStatus::error(
                                            fl!("print-error", reason = e),
                                        )));
                                        return;
                                    }
                                };
                                match SystemPrintDialog::new().print_pdf(pdf, &title).await {
                                    Ok(PrintDialogOutcome::Printed) => {
                                        save_message.set(Some(SaveStatus::ok(fl!("print-sent"))));
                                        open.set(false);
                                    }
                                    Ok(PrintDialogOutcome::Cancelled) => {
                                        save_message.set(Some(SaveStatus::ok(
                                            fl!("print-cancelled"),
                                        )));
                                    }
                                    Err(e) => {
                                        save_message.set(Some(SaveStatus::error(
                                            fl!("print-error", reason = e.to_string()),
                                        )));
                                    }
                                }
                            });
                        },
                    }

                    // ── Network printer (IPP) ─────────────────────────────────
                    div {
                        style: format!(
                            "border-top: 1px solid {b}; padding-top: {p}px; \
                             font-size: {fs}px; color: {fg};",
                            b = tokens::COLOR_BORDER_CHROME,
                            p = tokens::SPACE_2,
                            fs = tokens::FONT_SIZE_LABEL,
                            fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                        ),
                        { fl!("print-ipp-hint") }
                    }
                    AtField {
                        label: fl!("print-ipp-uri-label"),
                        control: rsx! {
                            input {
                                style: field_style(),
                                value: ipp_uri.read().clone(),
                                placeholder: "ipp://printer.local/ipp/print",
                                oninput: move |evt| ipp_uri.set(evt.value()),
                            }
                        },
                    }
                    div {
                        style: format!("display: flex; flex-direction: row; gap: {}px;", tokens::SPACE_2),
                        AtField {
                            label: fl!("print-ipp-copies"),
                            control: rsx! {
                                input {
                                    style: field_style(),
                                    value: ipp_copies.read().clone(),
                                    oninput: move |evt| ipp_copies.set(evt.value()),
                                }
                            },
                        }
                        AtField {
                            label: fl!("print-ipp-pages"),
                            control: rsx! {
                                input {
                                    style: field_style(),
                                    value: ipp_pages.read().clone(),
                                    placeholder: "1-3,5",
                                    oninput: move |evt| ipp_pages.set(evt.value()),
                                }
                            },
                        }
                    }
                    AtCheckRow {
                        checked: *ipp_duplex.read(),
                        disabled: false,
                        min_touch_px: tokens::TOUCH_MIN,
                        aria_label: fl!("print-ipp-duplex"),
                        label: rsx! { { fl!("print-ipp-duplex") } },
                        on_toggle: move |v: bool| ipp_duplex.set(v),
                    }
                }
            },
            footer: rsx! {
                AtDialogButton {
                    label: fl!("print-dialog-cancel"),
                    min_touch_px: tokens::TOUCH_MIN,
                    on_click: move |_| open.set(false),
                }
                AtDialogButton {
                    label: fl!("print-ipp-button"),
                    primary: true,
                    disabled: ipp_uri.read().trim().is_empty(),
                    min_touch_px: tokens::TOUCH_MIN,
                    on_click: move |_| {
                        let ds = Arc::clone(&ds_ipp);
                        let uri = ipp_uri.read().trim().to_string();
                        let options = build_ipp_options(
                            &ipp_copies.read(),
                            &ipp_pages.read(),
                            *ipp_duplex.read(),
                            title_ipp.clone(),
                        );
                        spawn(async move {
                            let pdf = match render_pdf(&ds) {
                                Ok(b) => b,
                                Err(e) => {
                                    save_message.set(Some(SaveStatus::error(
                                        fl!("print-error", reason = e),
                                    )));
                                    return;
                                }
                            };
                            // Blocking POST inline — the bounded tradeoff
                            // run_export takes; see the module docs.
                            let sent = IppPrinter::connect(&uri)
                                .and_then(|p| p.print_pdf(pdf, &options));
                            match sent {
                                Ok(job) => {
                                    save_message.set(Some(SaveStatus::ok(
                                        fl!("print-ipp-sent", job = i64::from(job)),
                                    )));
                                    open.set(false);
                                }
                                Err(e) => {
                                    save_message.set(Some(SaveStatus::error(
                                        fl!("print-error", reason = e.to_string()),
                                    )));
                                }
                            }
                        });
                    },
                }
            },
        }
    }
}

/// The dialog's text-input styling.
fn field_style() -> String {
    format!(
        "width: 100%; padding: {p}px; border-radius: 3px; border: 1px solid {border}; \
         background: {bg}; color: {fg}; font-size: {fs}px;",
        p = tokens::SPACE_1,
        border = tokens::COLOR_BORDER_CHROME,
        bg = tokens::COLOR_SURFACE_2,
        fg = tokens::COLOR_TEXT_ON_CHROME,
        fs = tokens::FONT_SIZE_LABEL,
    )
}

#[cfg(test)]
#[path = "print_dialog_tests.rs"]
mod tests;
