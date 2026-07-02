// SPDX-License-Identifier: Apache-2.0

//! Typed print errors.

/// Print dispatch failures.
#[derive(Debug, thiserror::Error)]
pub enum PrintError {
    /// The printer URI could not be parsed.
    #[error("invalid printer URI {uri:?}: {reason}")]
    InvalidPrinterUri {
        /// The rejected URI.
        uri: String,
        /// Why it was rejected.
        reason: String,
    },
    /// A job option could not be encoded as an IPP attribute.
    #[error("invalid print option: {0}")]
    InvalidOption(String),
    /// The IPP exchange failed (network, protocol, TLS).
    #[error("ipp error: {0}")]
    Ipp(#[from] ipp::error::IppError),
    /// The printer answered with a non-success IPP status code.
    #[error("printer rejected the request with IPP status {status:#06x}")]
    PrinterStatus {
        /// The raw IPP status code.
        status: u16,
    },
    /// The Print-Job response carried no `job-id`.
    #[error("printer accepted the job but returned no job-id")]
    MissingJobId,
    /// The printer reported the job failed (aborted or canceled).
    #[error("print job {job_id} ended in state {state:?}")]
    JobFailed {
        /// The IPP job id.
        job_id: i32,
        /// The terminal state reported by the printer.
        state: crate::client::PrintJobState,
    },
    /// The job did not reach a terminal state within the polling deadline.
    #[error("print job {job_id} did not complete within {timeout_secs}s")]
    JobTimeout {
        /// The IPP job id.
        job_id: i32,
        /// The polling deadline that elapsed.
        timeout_secs: u64,
    },
}
