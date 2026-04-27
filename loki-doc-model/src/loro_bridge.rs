// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Bridge between Loki document model and Loro CRDT document.

use loro::{LoroDoc, LoroMap, LoroText, LoroMovableList};
use crate::document::Document;
use crate::content::block::Block;
use crate::content::inline::Inline;
use crate::style::props::char_props::CharProps;
use crate::style::props::para_props::ParaProps;
use crate::loro_schema::*;

/// Errors that can occur during document translation to/from Loro.
#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    /// An error returned by the underlying Loro library.
    #[error("Loro error: {0}")]
    Loro(String),
    /// The block type at the given index is not currently supported by the bridge.
    #[error("Unsupported block at index {index}: {detail}")]
    UnsupportedBlock {
        /// The index of the unsupported block.
        index: usize,
        /// Description of the block type.
        detail: String,
    },
}

impl From<loro::LoroError> for BridgeError {
    fn from(e: loro::LoroError) -> Self {
        BridgeError::Loro(e.to_string())
    }
}

/// Converts a Loki `Document` snapshot into a fresh `LoroDoc` CRDT.
pub fn document_to_loro(doc: &Document) -> Result<LoroDoc, BridgeError> {
    let loro_doc = LoroDoc::new();
    
    // Configure text styles for all handled marks
    use loro::ExpandType;
    let mut style_config = loro::StyleConfigMap::new();
    style_config.insert(loro::InternalString::from(MARK_BOLD), loro::StyleConfig { expand: ExpandType::After });
    style_config.insert(loro::InternalString::from(MARK_ITALIC), loro::StyleConfig { expand: ExpandType::After });
    style_config.insert(loro::InternalString::from(MARK_UNDERLINE), loro::StyleConfig { expand: ExpandType::After });
    style_config.insert(loro::InternalString::from(MARK_STRIKETHROUGH), loro::StyleConfig { expand: ExpandType::After });
    style_config.insert(loro::InternalString::from(MARK_COLOR), loro::StyleConfig { expand: ExpandType::After });
    style_config.insert(loro::InternalString::from(MARK_HIGHLIGHT_COLOR), loro::StyleConfig { expand: ExpandType::After });
    style_config.insert(loro::InternalString::from(MARK_FONT_FAMILY), loro::StyleConfig { expand: ExpandType::After });
    style_config.insert(loro::InternalString::from(MARK_FONT_SIZE_PT), loro::StyleConfig { expand: ExpandType::After });
    style_config.insert(loro::InternalString::from(MARK_VERTICAL_ALIGN), loro::StyleConfig { expand: ExpandType::After });
    style_config.insert(loro::InternalString::from(MARK_LINK_URL), loro::StyleConfig { expand: ExpandType::After });
    style_config.insert(loro::InternalString::from(MARK_LANGUAGE), loro::StyleConfig { expand: ExpandType::After });
    style_config.insert(loro::InternalString::from(MARK_LETTER_SPACING), loro::StyleConfig { expand: ExpandType::After });
    style_config.insert(loro::InternalString::from(MARK_WORD_SPACING), loro::StyleConfig { expand: ExpandType::After });
    style_config.insert(loro::InternalString::from(MARK_SCALE), loro::StyleConfig { expand: ExpandType::After });
    style_config.insert(loro::InternalString::from(MARK_SMALL_CAPS), loro::StyleConfig { expand: ExpandType::After });
    style_config.insert(loro::InternalString::from(MARK_ALL_CAPS), loro::StyleConfig { expand: ExpandType::After });
    style_config.insert(loro::InternalString::from(MARK_SHADOW), loro::StyleConfig { expand: ExpandType::After });
    style_config.insert(loro::InternalString::from(MARK_KERNING), loro::StyleConfig { expand: ExpandType::After });
    style_config.insert(loro::InternalString::from(MARK_OUTLINE), loro::StyleConfig { expand: ExpandType::After });
    loro_doc.config_text_style(style_config);
    
    // Map Meta
    let meta_map = loro_doc.get_map(KEY_METADATA);
    if let Some(title) = &doc.meta.title {
        meta_map.insert("title", title.as_str())?;
    }
    
    let sections_list = loro_doc.get_list(KEY_SECTIONS);
    for (s_idx, section) in doc.sections.iter().enumerate() {
        let sec_map = sections_list.insert_container(s_idx, LoroMap::new())?;
        let blocks_list = sec_map.insert_container(KEY_BLOCKS, LoroMovableList::new())?;
        for (b_idx, block) in section.blocks.iter().enumerate() {
            let block_map = blocks_list.insert_container(b_idx, LoroMap::new())?;
            map_block(block, b_idx, &block_map)?;
        }
    }

    Ok(loro_doc)
}

fn map_block(block: &Block, index: usize, map: &LoroMap) -> Result<(), BridgeError> {
    match block {
        Block::Para(inlines) => {
            map.insert(KEY_TYPE, BLOCK_TYPE_PARA)?;
            let content = map.insert_container(KEY_CONTENT, LoroText::new())?;
            map_inlines(inlines, &content)?;
        }
        Block::StyledPara(para) => {
            map.insert(KEY_TYPE, BLOCK_TYPE_STYLED_PARA)?;
            if let Some(style_id) = &para.style_id {
                map.insert("style_id", style_id.as_str())?;
            }
            if let Some(para_props) = &para.direct_para_props {
                let props_map = map.insert_container(KEY_PARA_PROPS, LoroMap::new())?;
                map_para_props(para_props, &props_map)?;
            }
            // Add direct_char_props if present
            if let Some(char_props) = &para.direct_char_props {
                let _props_map = map.insert_container(KEY_DIRECT_CHAR_PROPS, LoroMap::new())?;
                // In full implementation we might map CharProps flat into here.
            }
            
            let content = map.insert_container(KEY_CONTENT, LoroText::new())?;
            map_inlines(&para.inlines, &content)?;
        }
        Block::Heading(level, _, inlines) => {
            map.insert(KEY_TYPE, BLOCK_TYPE_HEADING)?;
            map.insert("level", *level as i32)?;
            let content = map.insert_container(KEY_CONTENT, LoroText::new())?;
            map_inlines(inlines, &content)?;
        }
        Block::BulletList(_) => {
            tracing::debug!("stub: bullet list at index {}", index);
        }
        Block::OrderedList(_, _) => {
            tracing::debug!("stub: ordered list at index {}", index);
        }
        Block::Table(_) => {
            tracing::debug!("stub: table at index {}", index);
        }
        Block::Figure(_, _, _) => {
            tracing::debug!("stub: figure at index {}", index);
        }
        Block::CodeBlock(_, content_str) => {
            map.insert(KEY_TYPE, BLOCK_TYPE_CODE_BLOCK)?;
            let content = map.insert_container(KEY_CONTENT, LoroText::new())?;
            content.insert(0, content_str)?;
        }
        Block::HorizontalRule => {
            map.insert(KEY_TYPE, BLOCK_TYPE_HR)?;
        }
        Block::BlockQuote(_) => {
            tracing::debug!("stub: block quote at index {}", index);
        }
        Block::Plain(inlines) => {
            map.insert(KEY_TYPE, BLOCK_TYPE_PARA)?;
            let content = map.insert_container(KEY_CONTENT, LoroText::new())?;
            map_inlines(inlines, &content)?;
        }
        _ => {
            tracing::debug!("stub: other block variant at index {}", index);
        }
    }
    Ok(())
}

fn map_inlines(inlines: &[Inline], text: &LoroText) -> Result<(), BridgeError> {
    for inline in inlines {
        let start = text.len_unicode();
        match inline {
            Inline::Str(s) => {
                text.insert(start, s)?;
            }
            Inline::Space => {
                text.insert(start, " ")?;
            }
            Inline::SoftBreak | Inline::LineBreak => {
                text.insert(start, "\n")?;
            }
            Inline::StyledRun(run) => {
                let text_str = extract_plain_text(&run.content);
                text.insert(start, &text_str)?;
                let end = text.len_unicode();
                if start < end {
                    if let Some(props) = &run.direct_props {
                        apply_char_props_marks(props, start, end, text)?;
                    }
                }
            }
            Inline::Emph(inner) => {
                let text_str = extract_plain_text(inner);
                text.insert(start, &text_str)?;
                let end = text.len_unicode();
                if start < end {
                    text.mark(start..end, MARK_ITALIC, true)?;
                }
            }
            Inline::Strong(inner) => {
                let text_str = extract_plain_text(inner);
                text.insert(start, &text_str)?;
                let end = text.len_unicode();
                if start < end {
                    text.mark(start..end, MARK_BOLD, true)?;
                }
            }
            Inline::Underline(inner) => {
                let text_str = extract_plain_text(inner);
                text.insert(start, &text_str)?;
                let end = text.len_unicode();
                if start < end {
                    text.mark(start..end, MARK_UNDERLINE, "Single")?;
                }
            }
            _ => {
                // Ignore unsupported inline contents for this stub (or extract plain text implicitly)
                let text_str = extract_plain_text(&[inline.clone()]);
                if !text_str.is_empty() {
                    text.insert(start, &text_str)?;
                }
            }
        }
    }
    Ok(())
}

fn extract_plain_text(inlines: &[Inline]) -> String {
    let mut out = String::new();
    for inline in inlines {
        match inline {
            Inline::Str(s) => out.push_str(s),
            Inline::Space => out.push(' '),
            Inline::SoftBreak | Inline::LineBreak => out.push('\n'),
            Inline::StyledRun(run) => out.push_str(&extract_plain_text(&run.content)),
            Inline::Emph(inner) => out.push_str(&extract_plain_text(inner)),
            Inline::Strong(inner) => out.push_str(&extract_plain_text(inner)),
            Inline::Underline(inner) => out.push_str(&extract_plain_text(inner)),
            _ => {}
        }
    }
    out
}

fn apply_char_props_marks(props: &CharProps, start: usize, end: usize, text: &LoroText) -> Result<(), BridgeError> {
    if let Some(v) = props.bold { text.mark(start..end, MARK_BOLD, v)?; }
    if let Some(v) = props.italic { text.mark(start..end, MARK_ITALIC, v)?; }
    if let Some(v) = props.outline { text.mark(start..end, MARK_OUTLINE, v)?; }
    if let Some(v) = props.shadow { text.mark(start..end, MARK_SHADOW, v)?; }
    if let Some(v) = props.small_caps { text.mark(start..end, MARK_SMALL_CAPS, v)?; }
    if let Some(v) = props.all_caps { text.mark(start..end, MARK_ALL_CAPS, v)?; }
    if let Some(v) = props.kerning { text.mark(start..end, MARK_KERNING, v)?; }
    
    if let Some(v) = &props.font_name { text.mark(start..end, MARK_FONT_FAMILY, v.clone())?; }
    if let Some(v) = &props.font_size { text.mark(start..end, MARK_FONT_SIZE_PT, v.value())?; }
    if let Some(v) = &props.scale { text.mark(start..end, MARK_SCALE, f64::from(*v))?; }
    if let Some(v) = &props.letter_spacing { text.mark(start..end, MARK_LETTER_SPACING, v.value())?; }
    if let Some(v) = &props.word_spacing { text.mark(start..end, MARK_WORD_SPACING, v.value())?; }
    
    // Convert formatable types via format! macro
    if let Some(v) = &props.underline { text.mark(start..end, MARK_UNDERLINE, format!("{:?}", v))?; }
    if let Some(v) = &props.strikethrough { text.mark(start..end, MARK_STRIKETHROUGH, format!("{:?}", v))?; }
    if let Some(v) = &props.vertical_align { text.mark(start..end, MARK_VERTICAL_ALIGN, format!("{:?}", v))?; }
    if let Some(v) = &props.color { text.mark(start..end, MARK_COLOR, format!("{:?}", v))?; }
    if let Some(v) = &props.highlight_color { text.mark(start..end, MARK_HIGHLIGHT_COLOR, format!("{:?}", v))?; }
    
    // language and hyperlink can just be string
    // language might be a LanguageTag or String, use format
    if let Some(v) = &props.language { text.mark(start..end, MARK_LANGUAGE, format!("{:?}", v))?; }
    if let Some(v) = &props.hyperlink { text.mark(start..end, MARK_LINK_URL, v.clone())?; }
    
    Ok(())
}

fn map_para_props(props: &ParaProps, map: &LoroMap) -> Result<(), BridgeError> {
    if let Some(v) = &props.alignment { map.insert(PROP_ALIGNMENT, format!("{:?}", v))?; }
    if let Some(v) = &props.indent_start { map.insert(PROP_INDENT_LEFT, v.value())?; }
    if let Some(v) = &props.indent_end { map.insert(PROP_INDENT_RIGHT, v.value())?; }
    if let Some(v) = &props.indent_first_line { map.insert(PROP_INDENT_FIRST_LINE, v.value())?; }
    if let Some(v) = &props.indent_hanging { map.insert(PROP_INDENT_HANGING, v.value())?; }
    
    if let Some(v) = props.keep_together { map.insert(PROP_KEEP_TOGETHER, v)?; }
    if let Some(v) = props.keep_with_next { map.insert(PROP_KEEP_WITH_NEXT, v)?; }
    if let Some(v) = props.page_break_after { map.insert(PROP_PAGE_BREAK_AFTER, v)?; }
    if let Some(v) = props.bidi { map.insert(PROP_BIDI, v)?; }
    
    if let Some(v) = props.widow_control { map.insert(PROP_WIDOW_CONTROL, i32::from(v))?; }
    if let Some(v) = props.list_level { map.insert(PROP_LIST_LEVEL, i32::from(v))?; }
    
    if let Some(v) = &props.space_before { map.insert(PROP_SPACE_BEFORE_PT, format!("{:?}", v))?; }
    if let Some(v) = &props.space_after { map.insert(PROP_SPACE_AFTER_PT, format!("{:?}", v))?; }
    if let Some(v) = &props.line_height { map.insert(PROP_LINE_HEIGHT, format!("{:?}", v))?; }
    if let Some(v) = &props.list_id { map.insert(PROP_LIST_ID, format!("{:?}", v))?; }
    if let Some(v) = &props.tab_stops { map.insert(PROP_TAB_STOPS, format!("{:?}", v))?; }
    if let Some(v) = &props.background_color { map.insert("background_color", format!("{:?}", v))?; }

    Ok(())
}

/// Derives a Document snapshot from a LoroDoc for layout and rendering.
/// Re-derive when the LoroDoc changes — not kept in sync automatically.
pub fn loro_to_document(loro: &LoroDoc) -> Result<Document, BridgeError> {
    let mut doc = Document::new();
    
    // Map Meta
    if let Some(meta_map) = loro.get_map(KEY_METADATA).into() {
        // Fallback or read as needed; simple match for title
        if let Some(title) = meta_map.get("title").and_then(|v| v.into_value().ok()).and_then(|v| v.into_string().ok()) {
            doc.meta.title = Some(title.to_string());
        }
    }
    
    // Read sections and blocks
    let sections_list = loro.get_list(KEY_SECTIONS);
    doc.sections.clear(); // Clear the default section
    
    for i in 0..sections_list.len() {
        if let Some(sec_val) = sections_list.get(i) {
            if let Some(sec_map) = sec_val.into_container().ok().and_then(|c| c.into_map().ok()) {
                let mut section = crate::layout::section::Section::new();
                
                if let Some(blocks_val) = sec_map.get(KEY_BLOCKS) {
                    if let Some(blocks_list) = blocks_val.into_container().ok().and_then(|c| c.into_movable_list().ok()) {
                        for j in 0..blocks_list.len() {
                            if let Some(block_val) = blocks_list.get(j) {
                                if let Some(block_map) = block_val.into_container().ok().and_then(|c| c.into_map().ok()) {
                                    if let Ok(block) = map_loro_block(&block_map) {
                                        section.blocks.push(block);
                                    }
                                }
                            }
                        }
                    }
                }
                
                doc.sections.push(section);
            }
        }
    }
    
    // If we loaded no sections, push a default one so it renders properly
    if doc.sections.is_empty() {
        doc.sections.push(crate::layout::section::Section::new());
    }
    
    Ok(doc)
}

fn map_loro_block(map: &LoroMap) -> Result<Block, BridgeError> {
    let block_type = map.get(KEY_TYPE).and_then(|v| v.into_value().ok()).and_then(|v| v.into_string().ok()).map(|s| s.to_string()).unwrap_or_default();
    
    match block_type.as_str() {
        BLOCK_TYPE_PARA => {
            let inlines = reconstruct_inlines(map)?;
            Ok(Block::Para(inlines))
        }
        BLOCK_TYPE_STYLED_PARA => {
            let inlines = reconstruct_inlines(map)?;
            let style_id = map.get("style_id").and_then(|v| v.into_value().ok()).and_then(|v| v.into_string().ok()).map(|s| crate::style::catalog::StyleId(s.to_string()));
            let para_props = None; // Stub ParaProps reconstruction
            let direct_char_props = None;
            let styled_para = crate::content::block::StyledParagraph {
                style_id,
                direct_para_props: para_props,
                direct_char_props,
                inlines,
                attr: crate::content::attr::NodeAttr::default(),
            };
            Ok(Block::StyledPara(styled_para))
        }
        BLOCK_TYPE_HEADING => {
            let inlines = reconstruct_inlines(map)?;
            let level = map.get("level").and_then(|v| v.into_value().ok()).and_then(|v| {
                if v.is_i64() {
                    Some(v.into_i64().unwrap() as u8)
                } else if v.is_double() {
                    Some(v.into_double().unwrap() as u8)
                } else {
                    None
                }
            }).unwrap_or(1);
            Ok(Block::Heading(level, crate::content::attr::NodeAttr::default(), inlines))
        }
        BLOCK_TYPE_TABLE => {
            // TODO: reconstruct table logic
            tracing::debug!("TODO/stub: export bridge table");
            Ok(Block::HorizontalRule) 
        }
        BLOCK_TYPE_BULLET_LIST => {
            // TODO: reconstruct bullet list logic
            tracing::debug!("TODO/stub: export bridge bullet list");
            Ok(Block::HorizontalRule)
        }
        BLOCK_TYPE_ORDERED_LIST => {
            // TODO: reconstruct ordered list logic
            tracing::debug!("TODO/stub: export bridge ordered list");
            Ok(Block::HorizontalRule)
        }
        BLOCK_TYPE_FIGURE => {
            // TODO: reconstruct figure logic
            tracing::debug!("TODO/stub: export bridge figure");
            Ok(Block::HorizontalRule)
        }
        BLOCK_TYPE_CODE_BLOCK => {
            let text = map.get(KEY_CONTENT).and_then(|v| v.into_container().ok()).and_then(|c| c.into_text().ok()).map(|t| t.to_string()).unwrap_or_default();
            Ok(Block::CodeBlock(crate::content::attr::NodeAttr::default(), text))
        }
        BLOCK_TYPE_HR => {
            Ok(Block::HorizontalRule)
        }
        _ => {
            // Support fallback by reading inlines and wrapping as plain
            let inlines = reconstruct_inlines(map)?;
            if inlines.is_empty() {
                Ok(Block::HorizontalRule)
            } else {
                Ok(Block::Plain(inlines))
            }
        }
    }
}

fn reconstruct_inlines(map: &LoroMap) -> Result<Vec<Inline>, BridgeError> {
    let mut inlines = Vec::new();
    if let Some(content_val) = map.get(KEY_CONTENT) {
        if let Some(text_container) = content_val.into_container().ok().and_then(|c| c.into_text().ok()) {
            let delta = text_container.to_delta();
            for span in delta {
                // span might be an enum DeltaItem::Insert
                if let loro::TextDelta::Insert { insert, attributes } = span {
                    // Inspect marks
                    if let Some(attrs) = attributes {
                        let mut props = CharProps::default();
                        let mut has_props = false;
                        
                        // Try to parse boolean marks
                        if let Some(v) = attrs.get(MARK_BOLD) { 
                            props.bold = Some(true); // Since it was in the map as truthy
                            has_props = true;
                        }
                        if let Some(v) = attrs.get(MARK_ITALIC) { 
                            props.italic = Some(true);
                            has_props = true;
                        }
                        // Reconstruct other marks as possible / requested by the stub
                        
                        if has_props {
                            let run = crate::content::inline::StyledRun {
                                style_id: None,
                                direct_props: Some(Box::new(props)),
                                content: vec![Inline::Str(insert.to_string())],
                                attr: crate::content::attr::NodeAttr::default(),
                            };
                            inlines.push(Inline::StyledRun(run));
                        } else {
                            inlines.push(Inline::Str(insert.to_string()));
                        }
                    } else {
                        inlines.push(Inline::Str(insert.to_string()));
                    }
                }
            }
        }
    }
    Ok(inlines)
}
