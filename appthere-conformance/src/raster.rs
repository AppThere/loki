// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The shared, pinned PDF→PNG rasterizer (Spec 02 **D3** / B-5).
//!
//! Both reference apps produce their goldens *via PDF* (Word COM → PDF,
//! `soffice --headless` → PDF), and every PDF is rasterized to PNG through
//! **this one wrapper** at the fixed conformance DPI — so the golden and
//! candidate sides differ only in the layout/render engine, never in the PNG
//! encoder, DPI, or anti-aliasing settings.
//!
//! The backend is poppler's `pdftoppm`. Its availability and version are
//! checked explicitly and *recorded* — a missing binary fails loudly (the
//! same policy as the schema axis' `xmllint`), and the version string is
//! written into golden/calibration metadata so a poppler upgrade that shifts
//! the noise floor is visible in the record, not folklore.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The fixed DPI every conformance render and golden uses (Spec 02 §7.1: a
/// fixed DPI and pinned anti-aliasing settings). 144 dpi = 2× CSS pixel
/// density: crisp enough for glyph-level diffs, small enough to commit.
pub const CONFORMANCE_DPI: u32 = 144;

/// Errors from the pinned rasterizer.
#[derive(Debug, thiserror::Error)]
pub enum RasterError {
    /// `pdftoppm` was not found on `PATH`. Fails loudly, never skips.
    #[error(
        "pdftoppm (poppler-utils) not found on PATH — install poppler-utils; \
         golden rasterization must not be silently skipped"
    )]
    PdftoppmNotFound,
    /// Spawning or running `pdftoppm` failed.
    #[error("failed to run pdftoppm: {0}")]
    Spawn(#[source] std::io::Error),
    /// `pdftoppm` exited unsuccessfully.
    #[error("pdftoppm failed (code {code:?}): {stderr}")]
    Failed {
        /// Process exit code, if any.
        code: Option<i32>,
        /// Captured stderr.
        stderr: String,
    },
    /// The run produced no pages.
    #[error("pdftoppm produced no PNG pages for {0}")]
    NoPages(PathBuf),
    /// Filesystem error handling inputs/outputs.
    #[error("rasterizer I/O error: {0}")]
    Io(#[source] std::io::Error),
}

/// The pinned PDF→PNG stage. Construct once; the captured version string
/// belongs in golden/calibration metadata.
pub struct PdfRasterizer {
    version: String,
}

impl PdfRasterizer {
    /// Locates `pdftoppm` and captures its version. Errors if absent.
    pub fn new() -> Result<Self, RasterError> {
        let out = Command::new("pdftoppm")
            .arg("-v")
            .output()
            .map_err(|_| RasterError::PdftoppmNotFound)?;
        // pdftoppm prints its version banner on stderr.
        let banner = String::from_utf8_lossy(&out.stderr);
        let version = banner
            .lines()
            .next()
            .unwrap_or("pdftoppm (unknown version)")
            .trim()
            .to_string();
        Ok(Self { version })
    }

    /// The recorded backend version (e.g. `pdftoppm version 24.02.0`).
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Rasterizes every page of `pdf` to `out_dir/<stem>-N.png` at
    /// [`CONFORMANCE_DPI`], with anti-aliasing pinned on. Returns the page
    /// PNGs in page order.
    pub fn rasterize(
        &self,
        pdf: &Path,
        out_dir: &Path,
        stem: &str,
    ) -> Result<Vec<PathBuf>, RasterError> {
        std::fs::create_dir_all(out_dir).map_err(RasterError::Io)?;
        let prefix = out_dir.join(stem);
        let output = Command::new("pdftoppm")
            .arg("-png")
            .args(["-r", &CONFORMANCE_DPI.to_string()])
            // Pin the anti-aliasing settings explicitly (Spec 02 §7.1) so a
            // poppler default change cannot silently move the noise floor.
            .args(["-aa", "yes", "-aaVector", "yes"])
            .arg(pdf)
            .arg(&prefix)
            .output()
            .map_err(RasterError::Spawn)?;
        if !output.status.success() {
            return Err(RasterError::Failed {
                code: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }

        // pdftoppm names pages `<stem>-1.png`, `-2.png`, … (zero-padded once
        // page counts grow); collect and sort them for a stable page order.
        let mut pages: Vec<PathBuf> = std::fs::read_dir(out_dir)
            .map_err(RasterError::Io)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.extension().is_some_and(|x| x == "png")
                    && p.file_stem()
                        .and_then(|s| s.to_str())
                        .is_some_and(|s| s.starts_with(&format!("{stem}-")))
            })
            .collect();
        pages.sort();
        if pages.is_empty() {
            return Err(RasterError::NoPages(pdf.to_path_buf()));
        }
        Ok(pages)
    }
}

#[cfg(test)]
#[path = "raster_tests.rs"]
mod tests;
