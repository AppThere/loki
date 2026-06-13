// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! PPTX (`PresentationML`) export.
//!
//! Serialises a [`loki_presentation_model::Presentation`] to a `PowerPoint`
//! package built on [`loki_opc`]. This is the inverse of [`super::import`] and
//! covers the same surface: slide size, the slide list, and per-slide geometry
//! shapes (transform, preset geometry, solid fill, line stroke, text,
//! placeholder role).
//!
//! The output round-trips through [`super::import`]. It is intentionally
//! minimal — it omits slide masters, layouts, and a theme, which strict
//! `PowerPoint` requires to open a file; adding those (plus image/group export)
//! is a tracked follow-up.

use std::io::{Seek, Write};

use loki_opc::{Package, PartData, PartName, Relationship, TargetMode};
use loki_presentation_model::Presentation;

use super::write_presentation::presentation_xml;
use super::write_slide::slide_xml;
use crate::constants::REL_OFFICE_DOCUMENT;
use crate::error::OoxmlError;

const PRESENTATION_CT: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml";
const SLIDE_CT: &str = "application/vnd.openxmlformats-officedocument.presentationml.slide+xml";
const SLIDE_REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide";

/// Options controlling PPTX export behaviour.
#[derive(Debug, Clone, Default)]
pub struct PptxExportOptions {}

/// Unit struct implementing PPTX (`PresentationML`) export.
pub struct PptxExport;

impl PptxExport {
    /// Exports `pres` to `writer` as a PPTX package.
    pub fn export(pres: &Presentation, writer: impl Write + Seek) -> Result<(), OoxmlError> {
        Self::export_with_options(pres, writer, PptxExportOptions::default())
    }

    /// Exports `pres` with explicit options.
    pub fn export_with_options(
        pres: &Presentation,
        writer: impl Write + Seek,
        _options: PptxExportOptions,
    ) -> Result<(), OoxmlError> {
        let mut pkg = Package::new();

        // presentation.xml + its content type + the package officeDocument rel.
        let pres_name = PartName::new("/ppt/presentation.xml")?;
        pkg.content_type_map_mut()
            .add_override(&pres_name, PRESENTATION_CT);
        pkg.set_part(
            pres_name.clone(),
            PartData::new(presentation_xml(pres).into_bytes(), PRESENTATION_CT),
        );
        pkg.relationships_mut().add(Relationship {
            id: "rId1".to_owned(),
            rel_type: REL_OFFICE_DOCUMENT.to_owned(),
            target: "ppt/presentation.xml".to_owned(),
            target_mode: TargetMode::Internal,
        })?;

        // One part + presentation relationship per slide. The rId (rId{n}) and
        // the slide id (256 + i) line up with `presentation_xml`.
        for (i, slide) in pres.slides.iter().enumerate() {
            let n = i + 1;
            let slide_name = PartName::new(format!("/ppt/slides/slide{n}.xml"))?;
            pkg.content_type_map_mut()
                .add_override(&slide_name, SLIDE_CT);
            pkg.set_part(
                slide_name,
                PartData::new(slide_xml(slide).into_bytes(), SLIDE_CT),
            );
            pkg.part_relationships_mut(&pres_name).add(Relationship {
                id: format!("rId{n}"),
                rel_type: SLIDE_REL.to_owned(),
                target: format!("slides/slide{n}.xml"),
                target_mode: TargetMode::Internal,
            })?;
        }

        pkg.write(writer)?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "export_tests.rs"]
mod tests;
