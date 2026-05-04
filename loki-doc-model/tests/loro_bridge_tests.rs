use loki_doc_model::document::Document;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::{Inline, StyledRun};
use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::style::props::char_props::{CharProps, UnderlineStyle};
use loki_doc_model::layout::page::{PageMargins, PageSize};
use loki_doc_model::layout::header_footer::{HeaderFooter, HeaderFooterKind};
use loki_doc_model::layout::section::Section;
use loki_doc_model::loro_bridge::{document_to_loro, loro_to_document};
use loki_primitives::units::Points;
use loki_primitives::color::DocumentColor;

fn doc_to_json_string(doc: &Document) -> String {
    let loro = document_to_loro(doc).expect("should convert");
    let val = loro.get_deep_value();
    serde_json::to_string(&val).unwrap()
}

#[test]
fn test_hello_world_para() {
    let mut doc = Document::new();
    let para = Block::Para(vec![Inline::Str("Hello world".into())]);
    if let Some(sec) = doc.first_section_mut() {
        sec.blocks.push(para);
    }
    
    let json = doc_to_json_string(&doc);
    assert!(json.contains("Hello world"));
    assert!(json.contains("\"type\":\"para\""));
}

#[test]
fn test_bold_run() {
    let mut doc = Document::new();
    
    let run1 = Inline::Str("Normal ".into());
    let run2 = Inline::Strong(vec![Inline::Str("bold".into())]);
    let para = Block::Para(vec![run1, run2]);
    
    if let Some(sec) = doc.first_section_mut() {
        sec.blocks.push(para);
    }
    
    let json = doc_to_json_string(&doc);
    assert!(json.contains("Normal bold"));
    // Since bold mark is an annotation it might appear in op logs or delta, we just ensure it didn't panic.
}

#[test]
fn test_heading_level_2() {
    let mut doc = Document::new();
    let heading = Block::Heading(2, NodeAttr::default(), vec![Inline::Str("My Title".into())]);
    
    if let Some(sec) = doc.first_section_mut() {
        sec.blocks.push(heading);
    }
    
    let json = doc_to_json_string(&doc);
    assert!(json.contains("\"type\":\"heading\""));
    assert!(json.contains("\"level\":2"));
}

#[test]
fn test_three_paragraphs() {
    let mut doc = Document::new();
    let p1 = Block::Para(vec![Inline::Str("P1".into())]);
    let p2 = Block::Para(vec![Inline::Str("P2".into())]);
    let p3 = Block::Para(vec![Inline::Str("P3".into())]);
    
    if let Some(sec) = doc.first_section_mut() {
        sec.blocks.extend(vec![p1, p2, p3]);
    }
    
    let json = doc_to_json_string(&doc);
    assert!(json.contains("P1"));
    assert!(json.contains("P2"));
    assert!(json.contains("P3"));
}

#[test]
fn test_coloured_run() {
    let mut doc = Document::new();
    
    let props = CharProps {
        color: Some(DocumentColor::from_hex("#FF0000").unwrap()),
        ..Default::default()
    };
    
    let run = StyledRun {
        style_id: None,
        direct_props: Some(Box::new(props)),
        content: vec![Inline::Str("color text".into())],
        attr: NodeAttr::default(),
    };
    
    let para = Block::Para(vec![Inline::StyledRun(run)]);
    
    if let Some(sec) = doc.first_section_mut() {
        sec.blocks.push(para);
    }
    
    let json = doc_to_json_string(&doc);
    assert!(json.contains("color text"));
}

#[test]
fn test_roundtrip_hello_world_para() {
    let mut doc = Document::new();
    let para = Block::Para(vec![Inline::Str("Roundtrip text".into())]);
    if let Some(sec) = doc.first_section_mut() {
        sec.blocks.push(para);
    }
    
    let loro = document_to_loro(&doc).expect("convert to loro");
    let doc2 = loro_to_document(&loro).expect("convert from loro");
    
    let restored_blocks = &doc2.sections.first().unwrap().blocks;
    assert_eq!(restored_blocks.len(), 1);
    
    if let Block::Para(inlines) = &restored_blocks[0] {
        if let Inline::Str(text) = &inlines[0] {
            assert_eq!(text, "Roundtrip text");
        } else {
            panic!("Expected Inline::Str");
        }
    } else {
        panic!("Expected Block::Para");
    }
}

#[test]
fn test_roundtrip_bold_mark() {
    let mut doc = Document::new();
    
    let mut props = CharProps::default();
    props.bold = Some(true);
    let run = StyledRun {
        style_id: None,
        direct_props: Some(Box::new(props)),
        content: vec![Inline::Str("bold run".into())],
        attr: NodeAttr::default(),
    };
    
    let para = Block::Para(vec![Inline::StyledRun(run)]);
    if let Some(sec) = doc.first_section_mut() {
        sec.blocks.push(para);
    }
    
    let loro = document_to_loro(&doc).expect("convert to loro");
    let doc2 = loro_to_document(&loro).expect("convert from loro");
    
    let restored_blocks = &doc2.sections.first().unwrap().blocks;
    if let Block::Para(inlines) = &restored_blocks[0] {
        if let Inline::StyledRun(run) = &inlines[0] {
            assert_eq!(run.direct_props.as_ref().unwrap().bold, Some(true));
            if let Inline::Str(text) = &run.content[0] {
                assert_eq!(text, "bold run");
            }
        } else {
            panic!("Expected Inline::StyledRun");
        }
    }
}

#[test]
fn test_roundtrip_heading_level() {
    let mut doc = Document::new();
    let heading = Block::Heading(3, NodeAttr::default(), vec![Inline::Str("Level 3".into())]);
    if let Some(sec) = doc.first_section_mut() {
        sec.blocks.push(heading);
    }
    
    let loro = document_to_loro(&doc).expect("convert to loro");
    let doc2 = loro_to_document(&loro).expect("convert from loro");
    
    let restored_blocks = &doc2.sections.first().unwrap().blocks;
    if let Block::Heading(level, _, inlines) = &restored_blocks[0] {
        assert_eq!(*level, 3);
        if let Inline::Str(text) = &inlines[0] {
            assert_eq!(text, "Level 3");
        }
    } else {
        panic!("Expected Block::Heading");
    }
}

#[test]
fn test_roundtrip_empty_doc() {
    let doc = Document::new(); // Initially 1 empty section

    let loro = document_to_loro(&doc).expect("convert to loro");
    let doc2 = loro_to_document(&loro).expect("convert from loro");
    
    assert_eq!(doc2.sections.len(), 1);
    assert!(doc2.sections.first().unwrap().blocks.is_empty());
}

// ── PageLayout round-trip ─────────────────────────────────────────────────────

#[test]
fn roundtrip_page_layout_non_default_margins() {
    let mut doc = Document::new();
    if let Some(sec) = doc.first_section_mut() {
        sec.layout.page_size = PageSize { width: Points::new(595.28), height: Points::new(841.89) };
        sec.layout.margins = PageMargins {
            top: Points::new(56.7),
            bottom: Points::new(56.7),
            left: Points::new(42.5),
            right: Points::new(42.5),
            header: Points::new(28.3),
            footer: Points::new(28.3),
            gutter: Points::new(0.0),
        };
    }

    let loro = document_to_loro(&doc).expect("serialize");
    let doc2 = loro_to_document(&loro).expect("deserialize");

    let layout = &doc2.sections[0].layout;
    let eps = 0.01;
    assert!((layout.page_size.width.value() - 595.28).abs() < eps);
    assert!((layout.page_size.height.value() - 841.89).abs() < eps);
    assert!((layout.margins.top.value() - 56.7).abs() < eps);
    assert!((layout.margins.left.value() - 42.5).abs() < eps);
    assert!((layout.margins.header.value() - 28.3).abs() < eps);
    assert!((layout.margins.gutter.value() - 0.0).abs() < eps);
}

#[test]
fn roundtrip_landscape_orientation() {
    let mut doc = Document::new();
    if let Some(sec) = doc.first_section_mut() {
        sec.layout.orientation = loki_doc_model::layout::page::PageOrientation::Landscape;
    }

    let loro = document_to_loro(&doc).expect("serialize");
    let doc2 = loro_to_document(&loro).expect("deserialize");

    assert_eq!(
        doc2.sections[0].layout.orientation,
        loki_doc_model::layout::page::PageOrientation::Landscape,
    );
}

#[test]
fn roundtrip_section_with_header_content() {
    let mut doc = Document::new();
    if let Some(sec) = doc.first_section_mut() {
        let header_para = Block::Para(vec![Inline::Str("Page header text".into())]);
        sec.layout.header = Some(HeaderFooter {
            kind: HeaderFooterKind::Default,
            blocks: vec![header_para],
        });
        sec.blocks.push(Block::Para(vec![Inline::Str("Body".into())]));
    }

    let loro = document_to_loro(&doc).expect("serialize");
    let doc2 = loro_to_document(&loro).expect("deserialize");

    let hf = doc2.sections[0].layout.header.as_ref().expect("header present");
    assert_eq!(hf.blocks.len(), 1);
    if let Block::Para(inlines) = &hf.blocks[0] {
        if let Inline::Str(s) = &inlines[0] {
            assert_eq!(s, "Page header text");
        } else {
            panic!("expected Str inline in header");
        }
    } else {
        panic!("expected Para block in header");
    }
}

// ── Inline mark round-trip ────────────────────────────────────────────────────

#[test]
fn roundtrip_font_size_and_family() {
    let mut doc = Document::new();
    let props = CharProps {
        font_name: Some("Arial".into()),
        font_size: Some(Points::new(14.0)),
        ..Default::default()
    };
    let run = StyledRun {
        style_id: None,
        direct_props: Some(Box::new(props)),
        content: vec![Inline::Str("sized".into())],
        attr: NodeAttr::default(),
    };
    if let Some(sec) = doc.first_section_mut() {
        sec.blocks.push(Block::Para(vec![Inline::StyledRun(run)]));
    }

    let loro = document_to_loro(&doc).expect("serialize");
    let doc2 = loro_to_document(&loro).expect("deserialize");

    if let Block::Para(inlines) = &doc2.sections[0].blocks[0] {
        if let Inline::StyledRun(run) = &inlines[0] {
            let props = run.direct_props.as_ref().unwrap();
            assert_eq!(props.font_name.as_deref(), Some("Arial"));
            let size = props.font_size.expect("font_size present");
            assert!((size.value() - 14.0).abs() < 0.01);
        } else {
            panic!("expected StyledRun");
        }
    } else {
        panic!("expected Para");
    }
}

#[test]
fn roundtrip_underline_mark() {
    let mut doc = Document::new();
    let props = CharProps {
        underline: Some(UnderlineStyle::Single),
        ..Default::default()
    };
    let run = StyledRun {
        style_id: None,
        direct_props: Some(Box::new(props)),
        content: vec![Inline::Str("underlined".into())],
        attr: NodeAttr::default(),
    };
    if let Some(sec) = doc.first_section_mut() {
        sec.blocks.push(Block::Para(vec![Inline::StyledRun(run)]));
    }

    let loro = document_to_loro(&doc).expect("serialize");
    let doc2 = loro_to_document(&loro).expect("deserialize");

    if let Block::Para(inlines) = &doc2.sections[0].blocks[0] {
        if let Inline::StyledRun(run) = &inlines[0] {
            assert_eq!(
                run.direct_props.as_ref().unwrap().underline,
                Some(UnderlineStyle::Single),
            );
        } else {
            panic!("expected StyledRun");
        }
    } else {
        panic!("expected Para");
    }
}

#[test]
fn roundtrip_link_url() {
    let mut doc = Document::new();
    let props = CharProps {
        hyperlink: Some("https://example.com".into()),
        ..Default::default()
    };
    let run = StyledRun {
        style_id: None,
        direct_props: Some(Box::new(props)),
        content: vec![Inline::Str("link text".into())],
        attr: NodeAttr::default(),
    };
    if let Some(sec) = doc.first_section_mut() {
        sec.blocks.push(Block::Para(vec![Inline::StyledRun(run)]));
    }

    let loro = document_to_loro(&doc).expect("serialize");
    let doc2 = loro_to_document(&loro).expect("deserialize");

    if let Block::Para(inlines) = &doc2.sections[0].blocks[0] {
        if let Inline::StyledRun(run) = &inlines[0] {
            assert_eq!(
                run.direct_props.as_ref().unwrap().hyperlink.as_deref(),
                Some("https://example.com"),
            );
        } else {
            panic!("expected StyledRun");
        }
    } else {
        panic!("expected Para");
    }
}

#[test]
fn roundtrip_bold_italic_font_size_combined() {
    let mut doc = Document::new();
    let props = CharProps {
        bold: Some(true),
        italic: Some(true),
        font_size: Some(Points::new(18.0)),
        ..Default::default()
    };
    let run = StyledRun {
        style_id: None,
        direct_props: Some(Box::new(props)),
        content: vec![Inline::Str("combined".into())],
        attr: NodeAttr::default(),
    };
    if let Some(sec) = doc.first_section_mut() {
        sec.blocks.push(Block::Para(vec![Inline::StyledRun(run)]));
    }

    let loro = document_to_loro(&doc).expect("serialize");
    let doc2 = loro_to_document(&loro).expect("deserialize");

    if let Block::Para(inlines) = &doc2.sections[0].blocks[0] {
        if let Inline::StyledRun(run) = &inlines[0] {
            let p = run.direct_props.as_ref().unwrap();
            assert_eq!(p.bold, Some(true));
            assert_eq!(p.italic, Some(true));
            assert!((p.font_size.unwrap().value() - 18.0).abs() < 0.01);
        } else {
            panic!("expected StyledRun");
        }
    } else {
        panic!("expected Para");
    }
}

// ── Two-section round-trip ────────────────────────────────────────────────────

#[test]
fn roundtrip_two_sections_different_page_sizes() {
    let mut doc = Document::new();
    // First section: Letter
    doc.sections[0].layout.page_size = PageSize::letter();
    doc.sections[0].blocks.push(Block::Para(vec![Inline::Str("Sec1".into())]));
    // Second section: A4
    let mut sec2 = Section::new();
    sec2.layout.page_size = PageSize::a4();
    sec2.blocks.push(Block::Para(vec![Inline::Str("Sec2".into())]));
    doc.sections.push(sec2);

    let loro = document_to_loro(&doc).expect("serialize");
    let doc2 = loro_to_document(&loro).expect("deserialize");

    assert_eq!(doc2.sections.len(), 2);
    let eps = 0.1;
    assert!((doc2.sections[0].layout.page_size.width.value() - 612.0).abs() < eps);
    assert!((doc2.sections[1].layout.page_size.width.value() - 595.28).abs() < eps);
    if let Block::Para(inlines) = &doc2.sections[1].blocks[0] {
        if let Inline::Str(s) = &inlines[0] {
            assert_eq!(s, "Sec2");
        } else { panic!(); }
    } else { panic!(); }
}
