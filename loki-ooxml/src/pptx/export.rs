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
//! The output round-trips through [`super::import`] and includes the slide
//! master, slide layout, and theme parts that strict `PowerPoint` requires to
//! open a file. The master/layout/theme are generic defaults (see
//! [`super::write_master`] / [`super::write_theme`]); deriving real ones from
//! the document, plus image/group export, are tracked follow-ups.

use std::io::{Seek, Write};

use loki_opc::{Package, PartData, PartName, Relationship, TargetMode};
use loki_presentation_model::Presentation;

use super::write_master::{slide_layout_xml, slide_master_xml};
use super::write_presentation::presentation_xml;
use super::write_slide::slide_xml;
use super::write_theme::theme_xml;
use crate::constants::REL_OFFICE_DOCUMENT;
use crate::error::OoxmlError;

const PRESENTATION_CT: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml";
const SLIDE_CT: &str = "application/vnd.openxmlformats-officedocument.presentationml.slide+xml";
const MASTER_CT: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml";
const LAYOUT_CT: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml";
const THEME_CT: &str = "application/vnd.openxmlformats-officedocument.theme+xml";

const REL_BASE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/";

fn rel_type(name: &str) -> String {
    format!("{REL_BASE}{name}")
}

fn internal_rel(id: impl Into<String>, rel: &str, target: impl Into<String>) -> Relationship {
    Relationship {
        id: id.into(),
        rel_type: rel_type(rel),
        target: target.into(),
        target_mode: TargetMode::Internal,
    }
}

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

        write_scaffold(&mut pkg)?;

        // One part + presentation relationship per slide. The rId (rId{n}) and
        // the slide id (256 + i) line up with `presentation_xml`. Each slide
        // also references the shared layout (required by PowerPoint).
        for (i, slide) in pres.slides.iter().enumerate() {
            let n = i + 1;
            let slide_name = PartName::new(format!("/ppt/slides/slide{n}.xml"))?;
            pkg.content_type_map_mut()
                .add_override(&slide_name, SLIDE_CT);
            pkg.part_relationships_mut(&slide_name).add(internal_rel(
                "rId1",
                "slideLayout",
                "../slideLayouts/slideLayout1.xml",
            ))?;
            pkg.set_part(
                slide_name,
                PartData::new(slide_xml(slide).into_bytes(), SLIDE_CT),
            );
            pkg.part_relationships_mut(&pres_name).add(internal_rel(
                format!("rId{n}"),
                "slide",
                format!("slides/slide{n}.xml"),
            ))?;
        }

        // The slide master is registered after the slides, so its id is
        // rId{slide_count + 1} — matching the master reference in
        // presentation.xml.
        let master_rid = format!("rId{}", pres.slides.len() + 1);
        pkg.part_relationships_mut(&pres_name).add(internal_rel(
            master_rid,
            "slideMaster",
            "slideMasters/slideMaster1.xml",
        ))?;

        pkg.write(writer)?;
        Ok(())
    }
}

/// Adds the theme, slide master, and slide layout parts (plus their content
/// types and cross-relationships) required for a `PowerPoint`-openable package.
fn write_scaffold(pkg: &mut Package) -> Result<(), OoxmlError> {
    let theme_name = PartName::new("/ppt/theme/theme1.xml")?;
    pkg.content_type_map_mut()
        .add_override(&theme_name, THEME_CT);
    pkg.set_part(
        theme_name,
        PartData::new(theme_xml().as_bytes().to_vec(), THEME_CT),
    );

    // slideMaster1.xml → slideLayout1 (rId1) + theme1 (rId2).
    let master_name = PartName::new("/ppt/slideMasters/slideMaster1.xml")?;
    pkg.content_type_map_mut()
        .add_override(&master_name, MASTER_CT);
    pkg.part_relationships_mut(&master_name).add(internal_rel(
        "rId1",
        "slideLayout",
        "../slideLayouts/slideLayout1.xml",
    ))?;
    pkg.part_relationships_mut(&master_name).add(internal_rel(
        "rId2",
        "theme",
        "../theme/theme1.xml",
    ))?;
    pkg.set_part(
        master_name,
        PartData::new(slide_master_xml().into_bytes(), MASTER_CT),
    );

    // slideLayout1.xml → slideMaster1 (rId1).
    let layout_name = PartName::new("/ppt/slideLayouts/slideLayout1.xml")?;
    pkg.content_type_map_mut()
        .add_override(&layout_name, LAYOUT_CT);
    pkg.part_relationships_mut(&layout_name).add(internal_rel(
        "rId1",
        "slideMaster",
        "../slideMasters/slideMaster1.xml",
    ))?;
    pkg.set_part(
        layout_name,
        PartData::new(slide_layout_xml().into_bytes(), LAYOUT_CT),
    );

    Ok(())
}

#[cfg(test)]
#[path = "export_tests.rs"]
mod tests;
