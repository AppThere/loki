// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Discovery of golden reference renders and Loki renders for the pixel diff.
//!
//! Layout under the crate root:
//!
//! ```text
//! goldens/<fixture-stem>/page-001.png   ← canonical reference (O365 / LibreOffice)
//! renders/<fixture-stem>/page-001.png   ← Loki's own render of the same page
//! ```
//!
//! The harness pairs `goldens/.../page-NNN.png` with the identically-named file
//! under `renders/` and diffs them. Until a rasteriser produces the Loki
//! renders headlessly, the `renders/` tree is populated by an external step
//! (see `README.md`); the pixel test skips gracefully when either side is
//! absent.

use std::path::{Path, PathBuf};

use image::RgbaImage;

use crate::fixtures::Fixture;

/// The crate-root-relative directory holding canonical reference renders.
#[must_use]
pub fn goldens_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("goldens")
}

/// The crate-root-relative directory holding Loki's own renders.
#[must_use]
pub fn renders_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("renders")
}

/// The fixture stem used as the per-fixture subdirectory name (e.g.
/// `acid_docx`).
#[must_use]
pub fn fixture_stem(fixture: Fixture) -> &'static str {
    fixture
        .asset_name()
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(fixture.asset_name())
}

/// Returns the sorted golden page PNGs present for `fixture` (possibly empty).
/// Delegates to the shared discovery (promoted to `appthere-conformance`,
/// Spec 02 B-8) with this crate's roots.
#[must_use]
pub fn golden_pages(fixture: Fixture) -> Vec<PathBuf> {
    appthere_conformance::golden::golden_pages(&goldens_dir(), fixture_stem(fixture))
}

/// Maps a golden page path to the matching Loki render path under `renders/`.
#[must_use]
pub fn render_for(golden: &Path) -> Option<PathBuf> {
    appthere_conformance::golden::candidate_for(&renders_dir(), golden)
}

/// Loads a PNG as an RGBA image.
pub fn load_png(path: &Path) -> Result<RgbaImage, String> {
    appthere_conformance::golden::load_png(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stem_strips_extension() {
        assert_eq!(fixture_stem(Fixture::Docx), "acid_docx");
        assert_eq!(fixture_stem(Fixture::Ods), "acid_ods");
    }

    #[test]
    fn render_path_mirrors_golden_path() {
        let golden = goldens_dir().join("acid_docx").join("page-003.png");
        let render = render_for(&golden).unwrap();
        assert!(render.ends_with("renders/acid_docx/page-003.png"));
    }

    #[test]
    fn missing_goldens_yields_empty() {
        // No goldens are committed, so discovery returns an empty list rather
        // than erroring.
        assert!(golden_pages(Fixture::Docx).is_empty());
    }
}
