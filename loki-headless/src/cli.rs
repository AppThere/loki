// SPDX-License-Identifier: Apache-2.0

//! Argument definitions (headless spec §3).

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

/// Headless Loki: print and convert office documents with no GPU.
#[derive(Debug, Parser)]
#[command(name = "loki-headless", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Convert a document to another format.
    Convert(ConvertArgs),
    /// Render a document to PDF (alias for `convert` with a PDF target).
    Render(ConvertArgs),
    /// Render a document to PDF and dispatch it to an IPP printer.
    Print(PrintArgs),
    /// Detect and fix schema problems that stop a DOCX opening in Microsoft
    /// Word (out-of-order elements a tolerant reader accepts but Word rejects).
    Repair(RepairArgs),
    /// List the supported conversion pairs.
    Formats,
}

#[derive(Debug, Args)]
pub struct RepairArgs {
    /// Input `.docx` file to check or repair.
    #[arg(long = "in", value_name = "FILE")]
    pub input: PathBuf,
    /// Output `.docx` file for the repaired document. Omit with nothing else to
    /// just report; the file is only written when this is given.
    #[arg(long = "out", value_name = "FILE")]
    pub output: Option<PathBuf>,
    /// Only report problems; never write an output file.
    #[arg(long)]
    pub check: bool,
}

#[derive(Debug, Args)]
pub struct ConvertArgs {
    /// Input file (format inferred from the extension unless --from is set).
    #[arg(long = "in", value_name = "FILE")]
    pub input: PathBuf,
    /// Output file (format inferred from the extension unless --to is set).
    #[arg(long = "out", value_name = "FILE")]
    pub output: PathBuf,
    /// Source format override (docx, odt, xlsx, ods, …).
    #[arg(long)]
    pub from: Option<String>,
    /// Target format override.
    #[arg(long)]
    pub to: Option<String>,
    /// PDF profile: pdf, pdf-x1a, pdf-x3, pdf-x4, or pdf-a2b.
    #[arg(long)]
    pub profile: Option<String>,
    /// Override the document title in the output metadata.
    #[arg(long)]
    pub title: Option<String>,
}

#[derive(Debug, Args)]
pub struct PrintArgs {
    /// Input file (printed as-is if already PDF, otherwise rendered first).
    #[arg(long = "in", value_name = "FILE")]
    pub input: PathBuf,
    /// Printer URI (`ipp://printer.local/ipp/print`).
    #[arg(long)]
    pub printer: String,
    /// Number of copies.
    #[arg(long, default_value_t = 1)]
    pub copies: u32,
    /// Double-sided (long-edge binding).
    #[arg(long)]
    pub duplex: bool,
    /// With --duplex: flip on the short edge instead.
    #[arg(long, requires = "duplex")]
    pub short_edge: bool,
    /// Media size: A3, A4, A5, letter, legal, or a raw IPP media keyword.
    #[arg(long)]
    pub media: Option<String>,
    /// Force monochrome output.
    #[arg(long)]
    pub mono: bool,
    /// Job title shown in the printer queue (defaults to the file name).
    #[arg(long)]
    pub title: Option<String>,
    /// Submit without waiting for the printer to finish the job.
    #[arg(long)]
    pub no_wait: bool,
    /// Seconds to wait for job completion before giving up.
    #[arg(long, default_value_t = 300)]
    pub timeout: u64,
}
