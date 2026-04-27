use loki_doc_model::document::Document;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::{Inline, StyledRun};
use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::style::props::char_props::CharProps;
use loki_doc_model::loro_bridge::document_to_loro;
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
    let doc2 = loki_doc_model::loro_bridge::loro_to_document(&loro).expect("convert from loro");
    
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
    let doc2 = loki_doc_model::loro_bridge::loro_to_document(&loro).expect("convert from loro");
    
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
    let doc2 = loki_doc_model::loro_bridge::loro_to_document(&loro).expect("convert from loro");
    
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
    let doc2 = loki_doc_model::loro_bridge::loro_to_document(&loro).expect("convert from loro");
    
    assert_eq!(doc2.sections.len(), 1);
    assert!(doc2.sections.first().unwrap().blocks.is_empty());
}
