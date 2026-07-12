// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The on-disk corpus manifest (Spec 02 §9): the machine-readable record of
//! every fixture committed under `fixtures/<format>/`, with its feature,
//! applicable axes, reference application/version, and tolerance overrides.
//!
//! The layout is `fixtures/<format>/<id>.<ext>` with goldens under
//! `goldens/<format>/<id>/page-N.png` (see `goldens/CALIBRATION.md` for the
//! generation record). The manifest is code, like the catalog, so it is
//! versioned, reviewed, and impossible to desync from a sidecar file format.

use std::borrow::Cow;
use std::path::PathBuf;

use super::{Axis, Fixture, FixtureMeta, Format, Reference, Severity};

/// The LibreOffice reference that generated the committed ODT goldens
/// (pinned in `goldens/CALIBRATION.md` together with the rasterizer).
pub const LIBREOFFICE_24_2: Reference = Reference {
    app: "LibreOffice",
    version: "24.2",
};

/// All three axes apply (the common case for the visual baseline fixtures).
const ALL_AXES: &[Axis] = &[Axis::Schema, Axis::RoundTrip, Axis::Visual];

/// The committed on-disk corpus. Every entry's document must exist under
/// [`fixtures_root`]; entries carrying [`Axis::Visual`] also have a golden
/// directory under [`goldens_root`].
pub const MANIFEST: &[FixtureMeta] = &[
    FixtureMeta {
        id: "para-carlito",
        format: Format::Odt,
        feature: "Body paragraphs in Calibri (metric-compatible Carlito substitution)",
        severity: Severity::P1,
        axes: ALL_AXES,
        reference: Some(LIBREOFFICE_24_2),
        // The 2026-07-05 kerning fix brought this fixture inside the calibrated
        // band (see CALIBRATION.md's re-measured distributions) — no override.
        tolerance_override: None,
    },
    FixtureMeta {
        id: "para-gelasio",
        format: Format::Odt,
        feature: "Body paragraphs in Georgia (metric-compatible Gelasio substitution)",
        severity: Severity::P1,
        axes: ALL_AXES,
        reference: Some(LIBREOFFICE_24_2),
        tolerance_override: None,
    },
    FixtureMeta {
        id: "styles-tinos",
        format: Format::Odt,
        feature: "Named paragraph styles in Times New Roman (Tinos substitution)",
        severity: Severity::P1,
        axes: ALL_AXES,
        reference: Some(LIBREOFFICE_24_2),
        tolerance_override: None,
    },
];

/// The corpus root: `appthere-conformance/fixtures/`.
#[must_use]
pub fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

/// The goldens root: `appthere-conformance/goldens/`.
#[must_use]
pub fn goldens_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("goldens")
}

/// The on-disk file extension for a format.
#[must_use]
pub fn extension(format: Format) -> &'static str {
    match format {
        Format::Docx => "docx",
        Format::Xlsx => "xlsx",
        Format::Pptx => "pptx",
        Format::Odt => "odt",
        Format::Odp => "odp",
        Format::Odg => "odg",
        Format::Ods => "ods",
    }
}

/// The format's corpus subdirectory name (`fixtures/<dir>/…`).
#[must_use]
pub fn format_dir(format: Format) -> &'static str {
    extension(format)
}

/// An on-disk corpus fixture: manifest metadata plus lazily-loaded bytes.
#[derive(Debug, Clone, Copy)]
pub struct DiskFixture {
    meta: FixtureMeta,
}

impl DiskFixture {
    /// The fixture's document path under [`fixtures_root`].
    #[must_use]
    pub fn path(&self) -> PathBuf {
        fixtures_root()
            .join(format_dir(self.meta.format))
            .join(format!("{}.{}", self.meta.id, extension(self.meta.format)))
    }

    /// The fixture's golden directory under [`goldens_root`] (only meaningful
    /// for [`Axis::Visual`] fixtures).
    #[must_use]
    pub fn golden_dir(&self) -> PathBuf {
        goldens_root()
            .join(format_dir(self.meta.format))
            .join(self.meta.id)
    }
}

impl Fixture for DiskFixture {
    fn meta(&self) -> FixtureMeta {
        self.meta
    }

    /// Reads the committed document. Empty on I/O failure — the manifest test
    /// asserts existence, so an empty read only occurs outside the repo.
    fn bytes(&self) -> Cow<'_, [u8]> {
        Cow::Owned(std::fs::read(self.path()).unwrap_or_default())
    }
}

/// Every manifest entry as a loadable [`DiskFixture`].
#[must_use]
pub fn disk_fixtures() -> Vec<DiskFixture> {
    MANIFEST.iter().map(|&meta| DiskFixture { meta }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_manifest_entry_exists_on_disk_with_bytes() {
        for fixture in disk_fixtures() {
            let path = fixture.path();
            assert!(path.exists(), "missing corpus document: {}", path.display());
            assert!(
                !fixture.bytes().is_empty(),
                "empty corpus document: {}",
                path.display()
            );
        }
    }

    #[test]
    fn visual_entries_have_a_reference_and_a_golden_dir() {
        for fixture in disk_fixtures() {
            let meta = fixture.meta();
            if meta.axes.contains(&Axis::Visual) {
                assert!(
                    meta.reference.is_some(),
                    "{}: a visual fixture must record its reference app",
                    meta.id
                );
                assert!(
                    fixture.golden_dir().is_dir(),
                    "{}: missing golden dir {}",
                    meta.id,
                    fixture.golden_dir().display()
                );
            }
        }
    }

    #[test]
    fn ids_are_unique_and_axes_nonempty() {
        let mut ids: Vec<&str> = MANIFEST.iter().map(|m| m.id).collect();
        ids.sort_unstable();
        let n = ids.len();
        ids.dedup();
        assert_eq!(n, ids.len(), "duplicate fixture id in manifest");
        for m in MANIFEST {
            assert!(!m.axes.is_empty(), "{}: no axes recorded", m.id);
        }
    }
}
