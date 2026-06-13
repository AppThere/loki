// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the PPTX importer, driven by synthetic in-memory packages.

use super::*;
use loki_graphics::{Fill, Geometry, PresetShape};
use loki_presentation_model::PlaceholderKind;

// ── Synthetic .pptx package construction ────────────────────────────────────

const PRESENTATION_XML: &str = r#"<?xml version="1.0"?>
<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
                xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:sldIdLst><p:sldId id="256" r:id="rId1"/></p:sldIdLst>
  <p:sldSz cx="12192000" cy="6858000" type="screen16x9"/>
</p:presentation>"#;

const SLIDE_XML: &str = r#"<?xml version="1.0"?>
<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
       xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
  <p:cSld><p:spTree>
    <p:nvGrpSpPr/>
    <p:grpSpPr/>
    <p:sp>
      <p:nvSpPr>
        <p:cNvPr id="2" name="Title 1"/>
        <p:cNvSpPr/>
        <p:nvPr><p:ph type="title"/></p:nvPr>
      </p:nvSpPr>
      <p:spPr>
        <a:xfrm><a:off x="838200" y="365125"/><a:ext cx="10515600" cy="1325563"/></a:xfrm>
        <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
        <a:solidFill><a:srgbClr val="FF0000"/></a:solidFill>
      </p:spPr>
      <p:txBody>
        <a:bodyPr/>
        <a:p><a:pPr algn="ctr"/><a:r><a:rPr b="1" sz="4400"/><a:t>Hello</a:t></a:r></a:p>
      </p:txBody>
    </p:sp>
    <p:sp>
      <p:nvSpPr><p:cNvPr id="3" name="Oval 2"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>
      <p:spPr>
        <a:xfrm><a:off x="0" y="0"/><a:ext cx="1270000" cy="1270000"/></a:xfrm>
        <a:prstGeom prst="ellipse"><a:avLst/></a:prstGeom>
      </p:spPr>
    </p:sp>
    <p:graphicFrame><p:nvGraphicFramePr/></p:graphicFrame>
  </p:spTree></p:cSld>
</p:sld>"#;

fn build_pptx() -> Vec<u8> {
    use loki_opc::{PartData, Relationship, TargetMode};

    let mut pkg = Package::new();
    pkg.content_type_map_mut()
        .add_default("xml", "application/xml");
    pkg.content_type_map_mut().add_default(
        "rels",
        "application/vnd.openxmlformats-package.relationships+xml",
    );

    let pres = PartName::new("/ppt/presentation.xml").unwrap();
    pkg.set_part(
        pres.clone(),
        PartData::new(PRESENTATION_XML.as_bytes().to_vec(), "application/xml"),
    );
    let slide = PartName::new("/ppt/slides/slide1.xml").unwrap();
    pkg.set_part(
        slide.clone(),
        PartData::new(SLIDE_XML.as_bytes().to_vec(), "application/xml"),
    );

    // Package → presentation.xml
    pkg.relationships_mut()
        .add(Relationship {
            id: "rIdP".to_owned(),
            rel_type: REL_OFFICE_DOCUMENT.to_owned(),
            target: "ppt/presentation.xml".to_owned(),
            target_mode: TargetMode::Internal,
        })
        .unwrap();
    // presentation.xml → slide1.xml (rId1)
    pkg.part_relationships_mut(&pres)
        .add(Relationship {
            id: "rId1".to_owned(),
            rel_type: "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide"
                .to_owned(),
            target: "slides/slide1.xml".to_owned(),
            target_mode: TargetMode::Internal,
        })
        .unwrap();

    let mut buf = std::io::Cursor::new(Vec::new());
    pkg.write(&mut buf).unwrap();
    buf.into_inner()
}

#[test]
fn imports_slide_size_and_count() {
    let bytes = build_pptx();
    let p = PptxImport::import(std::io::Cursor::new(bytes), PptxImportOptions::default())
        .expect("import");
    assert_eq!(p.slide_count(), 1);
    // 12 192 000 EMU = 960 pt; 6 858 000 EMU = 540 pt.
    assert!((p.slide_size.width - 960.0).abs() < 1e-6);
    assert!((p.slide_size.height - 540.0).abs() < 1e-6);
}

#[test]
fn imports_title_shape_with_text_geometry_fill_and_placeholder() {
    let bytes = build_pptx();
    let p = PptxImport::import(std::io::Cursor::new(bytes), PptxImportOptions::default()).unwrap();
    let slide = &p.slides[0];

    // Two shapes (title + oval); the graphicFrame is skipped.
    assert_eq!(slide.drawing.shapes.len(), 2);

    let title = &slide.drawing.shapes[0];
    assert_eq!(title.id.as_str(), "2");
    assert_eq!(title.name.as_deref(), Some("Title 1"));
    // 838200 EMU = 66 pt, 365125 EMU = 28.75 pt.
    assert!((title.transform.frame.x - 66.0).abs() < 1e-6);
    if let ShapeKind::Geometry(g) = &title.kind {
        assert!(matches!(
            g.geometry,
            Geometry::Preset(PresetShape::Rectangle)
        ));
        assert!(matches!(g.fill, Fill::Solid(_)));
        let text = g.text.as_ref().unwrap();
        assert_eq!(text.paragraphs[0].text(), "Hello");
        assert!(text.paragraphs[0].runs[0].props.bold);
        assert!((text.paragraphs[0].runs[0].props.font_size_pt.unwrap() - 44.0).abs() < 1e-6);
    } else {
        panic!("expected geometry");
    }

    // Placeholder role recorded against the title shape.
    assert_eq!(
        slide
            .placeholder(PlaceholderKind::Title)
            .map(loki_graphics::ShapeId::as_str),
        Some("2")
    );
}

#[test]
fn unsupported_shapes_are_reported() {
    let bytes = build_pptx();
    let result =
        PptxImport::import_with_warnings(std::io::Cursor::new(bytes), PptxImportOptions::default())
            .unwrap();
    assert!(result.warnings.iter().any(
        |w| matches!(w, OoxmlWarning::Unsupported { feature } if feature == "p:graphicFrame")
    ));
}

#[test]
fn second_shape_is_ellipse_with_no_fill() {
    let bytes = build_pptx();
    let p = PptxImport::import(std::io::Cursor::new(bytes), PptxImportOptions::default()).unwrap();
    let oval = &p.slides[0].drawing.shapes[1];
    assert_eq!(oval.id.as_str(), "3");
    if let ShapeKind::Geometry(g) = &oval.kind {
        assert!(matches!(g.geometry, Geometry::Preset(PresetShape::Ellipse)));
        assert!(matches!(g.fill, Fill::None));
        assert!(g.text.is_none());
    } else {
        panic!("expected geometry");
    }
}
