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
#[must_use]
pub fn golden_pages(fixture: Fixture) -> Vec<PathBuf> {
    let dir = goldens_dir().join(fixture_stem(fixture));
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

/// Maps a golden page path to the matching Loki render path under `renders/`.
#[must_use]
pub fn render_for(golden: &Path) -> Option<PathBuf> {
    let file = golden.file_name()?;
    let stem = golden.parent()?.file_name()?;
    Some(renders_dir().join(stem).join(file))
}

/// Loads a PNG as an RGBA image.
pub fn load_png(path: &Path) -> Result<RgbaImage, String> {
    image::open(path)
        .map(|img| img.to_rgba8())
        .map_err(|e| format!("{}: {e}", path.display()))
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
