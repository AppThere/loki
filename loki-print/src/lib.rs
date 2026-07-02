// SPDX-License-Identifier: Apache-2.0

//! IPP print dispatch (headless spec ADR-C023).
//!
//! The print path is render → PDF → IPP: the PDF intermediate keeps print
//! fidelity identical to on-screen and archive output, and one IPP code
//! path covers essentially all modern office printers (optionally through a
//! CUPS server, which speaks IPP too — ratified decision §5.3).
//!
//! v1 supports explicit printer URIs, job options (copies, duplex, media,
//! colour), and job status polling. DNS-SD printer discovery is deliberately
//! deferred:
// TODO(headless-c023-discovery): IPP / DNS-SD printer discovery.

#![forbid(unsafe_code)]

mod client;
mod error;
mod options;

pub use client::{IppPrinter, PrintJobState};
pub use error::PrintError;
pub use options::{ColorMode, Duplex, PrintOptions};
