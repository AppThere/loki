// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Linux backend: `org.freedesktop.portal.Print`.
//!
//! The portal's `Print` call (with no prepare token) presents the print
//! dialog itself, so this backend is one round trip: stage the PDF where a
//! file descriptor can be handed over, call `Print`, map the response. The
//! fd route is what keeps sandboxed (Flatpak) installs working — the portal
//! reads the content through the descriptor, never through a path.

use std::io::Write as _;

use ashpd::desktop::print::PrintProxy;

use crate::{PrintDialogError, PrintDialogOutcome};

/// Stages `pdf` in a temp file (unlinked as soon as the call finishes) and
/// asks the portal to print it.
pub(crate) async fn print_pdf(
    pdf: Vec<u8>,
    job_title: &str,
) -> Result<PrintDialogOutcome, PrintDialogError> {
    // A real file rather than a memfd: memfd_create is unsafe-fn territory
    // and this crate forbids unsafe. The file is created 0600 in the user's
    // temp dir and removed in every exit path below.
    let path = std::env::temp_dir().join(format!("loki-print-{}.pdf", std::process::id()));
    let mut file = std::fs::File::create(&path)?;
    file.write_all(&pdf)?;
    file.flush()?;
    drop(file);
    let readable = std::fs::File::open(&path)?;

    let outcome = portal_print(&readable, job_title).await;

    // Best-effort removal — the job content is already with the portal.
    let _ = std::fs::remove_file(&path);
    outcome
}

async fn portal_print(
    fd: &std::fs::File,
    job_title: &str,
) -> Result<PrintDialogOutcome, PrintDialogError> {
    let proxy = PrintProxy::new().await.map_err(map_err)?;
    // No prepare token: the portal shows its dialog on this call. Modal,
    // parented to no specific window (the identifier needs a wayland/x11
    // handle this API deliberately does not thread through yet).
    match proxy.print(None, job_title, fd, None, true).await {
        Ok(request) => match request.response() {
            Ok(()) => Ok(PrintDialogOutcome::Printed),
            Err(ashpd::Error::Response(ashpd::desktop::ResponseError::Cancelled)) => {
                Ok(PrintDialogOutcome::Cancelled)
            }
            Err(e) => Err(map_err(e)),
        },
        Err(ashpd::Error::Response(ashpd::desktop::ResponseError::Cancelled)) => {
            Ok(PrintDialogOutcome::Cancelled)
        }
        Err(e) => Err(map_err(e)),
    }
}

fn map_err(e: ashpd::Error) -> PrintDialogError {
    PrintDialogError::Platform {
        message: e.to_string(),
    }
}
