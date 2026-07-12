// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Golden/candidate render discovery, promoted from `loki-acid` (Spec 02 B-8)
//! and generalised: the roots are parameters, so any consumer's tree layout
//! works — `loki-acid` pairs `goldens/<stem>/` with `renders/<stem>/`, the
//! shared corpus pairs `goldens/<format>/<id>/` with a caller-chosen candidate
//! root.
//!
//! The convention is one PNG per page, identically named on both sides;
//! [`golden_pages`] lists the golden side and [`candidate_for`] maps a golden
//! page to its candidate path.

use std::path::{Path, PathBuf};

use image::RgbaImage;

/// Returns the sorted page PNGs under `<goldens_root>/<stem>/` (possibly
/// empty — discovery never errors, so suites stay green until references are
/// supplied).
#[must_use]
pub fn golden_pages(goldens_root: &Path, stem: &str) -> Vec<PathBuf> {
    let dir = goldens_root.join(stem);
    let mut pages: Vec<PathBuf> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("png")))
        .collect();
    pages.sort();
    pages
}

/// Maps a golden page path to the identically-named candidate page under
/// `candidates_root`, preserving the per-fixture subdirectory.
#[must_use]
pub fn candidate_for(candidates_root: &Path, golden: &Path) -> Option<PathBuf> {
    let file = golden.file_name()?;
    let stem = golden.parent()?.file_name()?;
    Some(candidates_root.join(stem).join(file))
}

/// Loads a PNG as an RGBA image.
///
/// # Errors
///
/// A human-readable message naming the path on decode/I/O failure.
pub fn load_png(path: &Path) -> Result<RgbaImage, String> {
    image::open(path)
        .map(|img| img.to_rgba8())
        .map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_path_mirrors_golden_path() {
        let golden = Path::new("/g/para-carlito/page-3.png");
        let cand = candidate_for(Path::new("/r"), golden).unwrap();
        assert_eq!(cand, Path::new("/r/para-carlito/page-3.png"));
    }

    #[test]
    fn missing_golden_dir_yields_empty() {
        assert!(golden_pages(Path::new("/nonexistent-goldens"), "x").is_empty());
    }
}
