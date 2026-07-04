// SPDX-License-Identifier: Apache-2.0

#![forbid(unsafe_code)]

//! `loki-headless` — GPU-free printing and file conversion (headless spec
//! §3). The standalone CLI is the "office printing" deployment: no server,
//! no Postgres, just files in, PDF/converted files or IPP print jobs out.
//!
//! Deliberately deferred (recorded here so the gaps are visible):
// TODO(headless-c025): apalis worker mode consuming loki-server's job queue
// (idempotency keys, retries, dead-letter) and the optional HTTP endpoint.
// TODO(headless-c021): vello_cpu rasterisation for Thumbnail jobs; the
// print/convert path needs no rasteriser (layout → PDF is already CPU-only).
// TODO(headless-c027): fail-closed font policy for print jobs — needs
// loki-layout to report substitutions instead of silently falling back.
// TODO(headless-c028): SEV-SNP/TDX attestation-gated key release for Tier-1
// worker pools.

mod cli;
mod commands;

use clap::Parser;

fn main() -> std::process::ExitCode {
    let args = cli::Cli::parse();
    match commands::run(args) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
