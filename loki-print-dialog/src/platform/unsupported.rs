// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Fallback backend for platforms without a print-dialog implementation yet:
//! a **typed** refusal naming the platform, so the caller can show an honest
//! message rather than a dialog that never appears (the `loki-file-access`
//! `delete` precedent).
//!
//! TODO(print-macos): `NSPrintOperation`. TODO(print-windows):
//! `PrintDlgEx`/WinRT. TODO(print-android): `PrintManager` over the JNI
//! trampoline. TODO(print-ios): `UIPrintInteractionController`, blocked on
//! the §5 harness.

use crate::{PrintDialogError, PrintDialogOutcome};

pub(crate) async fn print_pdf(
    _pdf: Vec<u8>,
    _job_title: &str,
) -> Result<PrintDialogOutcome, PrintDialogError> {
    Err(PrintDialogError::Unsupported {
        platform: std::env::consts::OS,
    })
}
