// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! PPTX importer entry point: orchestrates package reading and slide assembly.

use std::io::{Read, Seek};

use loki_graphics::{Drawing, ImageFormat, ImageRef, ImageShape, ShapeKind, Size};
use loki_opc::{Package, PartName, RelationshipSet};
use loki_presentation_model::{Placeholder, Presentation, Slide, SlideId};
use quick_xml::Reader;
use quick_xml::events::Event;

use super::presentation_part::parse_presentation;
use super::shapes::parse_sptree;
use super::units::emu_to_pt;
use crate::constants::REL_OFFICE_DOCUMENT;
use crate::error::{OoxmlError, OoxmlWarning};
use crate::xml_util::local_name;

/// Options controlling PPTX import behaviour.
#[derive(Debug, Clone, Default)]
pub struct PptxImportOptions {}

/// The result of a PPTX import: the presentation plus any non-fatal warnings.
#[derive(Debug)]
pub struct PptxImportResult {
    /// The imported presentation model.
    pub presentation: Presentation,
    /// Non-fatal warnings (unsupported constructs, unresolved images, …).
    pub warnings: Vec<OoxmlWarning>,
}

/// Unit struct implementing PPTX (`PresentationML`) import.
pub struct PptxImport;

impl PptxImport {
    /// Imports a PPTX file, returning the presentation (warnings discarded).
    pub fn import(
        reader: impl Read + Seek,
        options: PptxImportOptions,
    ) -> Result<Presentation, OoxmlError> {
        Ok(Self::import_with_warnings(reader, options)?.presentation)
    }

    /// Imports a PPTX file, returning the presentation and collected warnings.
    pub fn import_with_warnings(
        reader: impl Read + Seek,
        _options: PptxImportOptions,
    ) -> Result<PptxImportResult, OoxmlError> {
        let package = Package::open(reader)?;
        let mut warnings = Vec::new();

        // 1. Locate ppt/presentation.xml via the package officeDocument relationship.
        let pres_rel = rels_by_type(package.relationships(), REL_OFFICE_DOCUMENT)
            .next()
            .ok_or_else(|| OoxmlError::MissingPart {
                relationship_type: REL_OFFICE_DOCUMENT.to_owned(),
            })?
            .clone();
        let pres_part_name = resolve_part_name("/", &pres_rel.target)?;
        let pres_part = package
            .part(&pres_part_name)
            .ok_or_else(|| OoxmlError::MissingPart {
                relationship_type: pres_part_name.as_str().to_owned(),
            })?;

        // 2. Slide size + ordered slide relationship ids.
        let info = parse_presentation(&pres_part.bytes).map_err(|e| xml_err(&pres_part_name, e))?;
        let slide_size = Size::new(emu_to_pt(info.width_emu), emu_to_pt(info.height_emu));

        let mut presentation = Presentation::new(slide_size);

        // 3. Resolve each slide relationship and assemble its drawing.
        let pres_rels = package.part_relationships(&pres_part_name);
        for (i, rid) in info.slide_rids.iter().enumerate() {
            let Some(slide_part_name) =
                resolve_rel_target(pres_rels, rid, pres_part_name.as_str())?
            else {
                warnings.push(OoxmlWarning::UnresolvedRelationship {
                    id: rid.clone(),
                    context: "presentation slide list".to_owned(),
                });
                continue;
            };
            let Some(slide_part) = package.part(&slide_part_name) else {
                continue;
            };

            let parsed = parse_slide(&slide_part.bytes, &slide_part_name, &mut warnings)?;
            let slide = assemble_slide(
                format!("slide{}", i + 1),
                slide_size,
                parsed,
                &package,
                &slide_part_name,
                &mut warnings,
            );
            presentation.add_slide(slide);
        }

        Ok(PptxImportResult {
            presentation,
            warnings,
        })
    }
}

/// Finds the `p:spTree` in a slide part and parses it into shapes.
fn parse_slide(
    bytes: &[u8],
    part: &PartName,
    warnings: &mut Vec<OoxmlWarning>,
) -> Result<Vec<super::ParsedShape>, OoxmlError> {
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    loop {
        match reader
            .read_event_into(&mut buf)
            .map_err(|e| xml_err(part, e))?
        {
            Event::Start(e) if local_name(&e) == b"spTree" => {
                return parse_sptree(&mut reader, warnings).map_err(|e| xml_err(part, e));
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(Vec::new())
}

/// Builds a [`Slide`] from parsed shapes, resolving picture image bytes and
/// collecting placeholder roles.
fn assemble_slide(
    id: String,
    size: Size,
    parsed: Vec<super::ParsedShape>,
    package: &Package,
    slide_part_name: &PartName,
    warnings: &mut Vec<OoxmlWarning>,
) -> Slide {
    let slide_rels = package.part_relationships(slide_part_name);
    let mut drawing = Drawing::new(size);
    let mut placeholders = Vec::new();

    for ps in parsed {
        let mut shape = ps.shape;
        if let ShapeKind::Image(img) = &shape.kind {
            shape.kind = resolve_image(img, package, slide_rels, slide_part_name, warnings);
        }
        if let Some(kind) = ps.placeholder {
            placeholders.push(Placeholder {
                kind,
                shape_id: shape.id.clone(),
            });
        }
        drawing.push(shape);
    }

    Slide {
        id: SlideId::new(id),
        name: None,
        drawing,
        placeholders,
        notes: None,
    }
}

/// Resolves a picture's `External(rId)` reference to embedded image bytes when
/// the relationship and part can be found; otherwise leaves it external and
/// records a warning.
fn resolve_image(
    img: &ImageShape,
    package: &Package,
    slide_rels: Option<&RelationshipSet>,
    slide_part_name: &PartName,
    warnings: &mut Vec<OoxmlWarning>,
) -> ShapeKind {
    let loki_graphics::ImageSource::External(rid) = &img.image.source else {
        return ShapeKind::Image(img.clone());
    };

    let resolved = resolve_rel_target(slide_rels, rid, slide_part_name.as_str())
        .ok()
        .flatten()
        .and_then(|name| package.part(&name).map(|p| (name, p.bytes.clone())));

    if let Some((name, bytes)) = resolved {
        ShapeKind::Image(ImageShape {
            image: ImageRef {
                format: format_from_ext(name.extension()),
                source: loki_graphics::ImageSource::Embedded(bytes),
            },
        })
    } else {
        warnings.push(OoxmlWarning::UnresolvedImage {
            rel_id: rid.clone(),
        });
        ShapeKind::Image(img.clone())
    }
}

fn format_from_ext(ext: Option<&str>) -> ImageFormat {
    match ext.map(str::to_ascii_lowercase).as_deref() {
        Some("png") => ImageFormat::Png,
        Some("jpg" | "jpeg") => ImageFormat::Jpeg,
        Some("gif") => ImageFormat::Gif,
        Some("svg") => ImageFormat::Svg,
        Some(other) => ImageFormat::Other(other.to_owned()),
        None => ImageFormat::Other(String::new()),
    }
}

fn xml_err(part: &PartName, source: quick_xml::Error) -> OoxmlError {
    OoxmlError::Xml {
        part: part.as_str().to_owned(),
        source,
    }
}

/// Resolves a relationship id against a part's relationship set into a [`PartName`].
fn resolve_rel_target(
    rels: Option<&RelationshipSet>,
    rid: &str,
    base: &str,
) -> Result<Option<PartName>, OoxmlError> {
    let Some(rels) = rels else { return Ok(None) };
    let Some(rel) = rels.get(rid) else {
        return Ok(None);
    };
    Ok(Some(resolve_part_name(base, &rel.target)?))
}

/// Resolves a relationship target (absolute or relative to `base`) to a [`PartName`].
fn resolve_part_name(base: &str, target: &str) -> Result<PartName, OoxmlError> {
    if target.starts_with('/') {
        return PartName::new(normalize_part_path(target)).map_err(OoxmlError::Opc);
    }
    let dir = base.rfind('/').map_or("/", |i| &base[..=i]);
    let raw = format!("{dir}{target}");
    PartName::new(normalize_part_path(&raw)).map_err(OoxmlError::Opc)
}

/// Collapses `.` and `..` segments in an OPC part path, preserving the leading `/`.
fn normalize_part_path(path: &str) -> String {
    let mut segments: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                segments.pop();
            }
            s => segments.push(s),
        }
    }
    format!("/{}", segments.join("/"))
}

fn rels_by_type<'a>(
    rels: &'a RelationshipSet,
    transitional_type: &str,
) -> impl Iterator<Item = &'a loki_opc::Relationship> {
    let strict = transitional_type.replace(
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/",
        "http://purl.oclc.org/ooxml/officeDocument/relationships/",
    );
    let trans = transitional_type.to_owned();
    rels.iter()
        .filter(move |r| r.rel_type == trans || r.rel_type == strict)
}

#[cfg(test)]
#[path = "import_tests.rs"]
mod tests;
