// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! System print-dialog capability (usage audit §6), mirroring
//! `loki-file-access`'s per-platform shape: one portable entry point, with
//! each platform backend either implemented or a **typed**
//! [`PrintDialogError::Unsupported`] — never a silent no-op.
//!
//! Implemented backends:
//! - **Linux**: `org.freedesktop.portal.Print` (via `ashpd`) — the portal
//!   presents the printer dialog itself, and the portal route keeps Flatpak
//!   working. The futures are runtime-agnostic (zbus async-io), so callers
//!   may await them on any executor.
//!
//! Deferred backends, each returning `Unsupported` with its platform name:
//! macOS (`NSPrintOperation`), Windows (`PrintDlgEx`/WinRT), Android
//! (`PrintManager` over the JNI trampoline), iOS
//! (`UIPrintInteractionController`, blocked on the §5 harness).
//!
//! This crate takes **rendered PDF bytes** — the caller owns rendering (the
//! editor renders through `loki_pdf::build_pdf` from its live layout). The
//! IPP direct-URI path deliberately stays in `loki-print`: that is a
//! no-dialog office-deployment route, not a system dialog.

#![forbid(unsafe_code)]

mod platform;

/// Errors from presenting the system print dialog.
#[derive(Debug, thiserror::Error)]
pub enum PrintDialogError {
    /// No print-dialog backend exists for this platform yet.
    #[error("system print dialog is not supported on {platform}")]
    Unsupported {
        /// The platform lacking a backend.
        platform: &'static str,
    },
    /// The platform backend failed (portal unreachable, D-Bus error, …).
    #[error("print dialog failed: {message}")]
    Platform {
        /// Human-readable description from the backend.
        message: String,
    },
    /// Staging the document for the dialog failed (temp file I/O).
    #[error("could not stage document for printing: {0}")]
    Io(#[from] std::io::Error),
}

/// What the user did with the dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrintDialogOutcome {
    /// The job was handed to the platform's print system.
    Printed,
    /// The user dismissed the dialog.
    Cancelled,
}

/// The system print dialog.
///
/// Stateless; construct per use. On Linux the portal owns every job option
/// (printer, copies, pages, duplex) inside its own dialog, which is why this
/// API takes no options struct — offering one here would promise knobs the
/// dialog would then contradict.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemPrintDialog;

impl SystemPrintDialog {
    /// Creates the dialog handle.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Presents the platform print dialog for `pdf`, titled `job_title`.
    ///
    /// Resolves once the user has confirmed (the job is with the platform's
    /// print system) or cancelled. The future is runtime-agnostic.
    ///
    /// # Errors
    ///
    /// [`PrintDialogError::Unsupported`] on platforms with no backend;
    /// [`PrintDialogError::Platform`] / [`PrintDialogError::Io`] when the
    /// backend or its staging fails.
    pub async fn print_pdf(
        &self,
        pdf: Vec<u8>,
        job_title: &str,
    ) -> Result<PrintDialogOutcome, PrintDialogError> {
        platform::print_pdf(pdf, job_title).await
    }
}
