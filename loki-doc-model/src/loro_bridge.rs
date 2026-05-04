// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Bridge between Loki document model and Loro CRDT document.

use loro::{LoroDoc, LoroMap, LoroText, LoroMovableList};
use crate::document::Document;
use crate::content::block::Block;
use crate::content::inline::Inline;
use crate::style::props::char_props::{CharProps, UnderlineStyle, StrikethroughStyle, VerticalAlign};
use crate::style::props::para_props::{ParaProps, ParagraphAlignment, Spacing, LineHeight};
use crate::style::list_style::ListId;
use loki_primitives::units::Points;
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
            if let Some(char_props) = &para.direct_char_props {
                let props_map = map.insert_container(KEY_DIRECT_CHAR_PROPS, LoroMap::new())?;
                map_char_props_to_map(char_props, &props_map)?;
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
                if start < end && let Some(props) = &run.direct_props {
                    apply_char_props_marks(props, start, end, text)?;
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
                let text_str = extract_plain_text(std::slice::from_ref(inline));
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
    if let Some(v) = &props.alignment { map.insert(PROP_ALIGNMENT, encode_alignment(v))?; }
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

    if let Some(v) = &props.space_before { map.insert(PROP_SPACE_BEFORE_PT, encode_spacing(v))?; }
    if let Some(v) = &props.space_after { map.insert(PROP_SPACE_AFTER_PT, encode_spacing(v))?; }
    if let Some(v) = &props.line_height { map.insert(PROP_LINE_HEIGHT, encode_line_height(v))?; }
    if let Some(v) = &props.list_id { map.insert(PROP_LIST_ID, v.as_str())?; }
    if let Some(v) = &props.tab_stops { map.insert(PROP_TAB_STOPS, format!("{:?}", v))?; }
    if let Some(v) = &props.background_color { map.insert("background_color", format!("{:?}", v))?; }

    Ok(())
}

fn encode_alignment(a: &ParagraphAlignment) -> &'static str {
    match a {
        ParagraphAlignment::Left => "Left",
        ParagraphAlignment::Right => "Right",
        ParagraphAlignment::Center => "Center",
        ParagraphAlignment::Justify => "Justify",
        ParagraphAlignment::Distribute => "Distribute",
    }
}

fn decode_alignment(s: &str) -> Option<ParagraphAlignment> {
    match s {
        "Left" => Some(ParagraphAlignment::Left),
        "Right" => Some(ParagraphAlignment::Right),
        "Center" => Some(ParagraphAlignment::Center),
        "Justify" => Some(ParagraphAlignment::Justify),
        "Distribute" => Some(ParagraphAlignment::Distribute),
        _ => None,
    }
}

// Encode Spacing as "Exact:<f64>" or "Percent:<f32>"
fn encode_spacing(s: &Spacing) -> String {
    match s {
        Spacing::Exact(pts) => format!("Exact:{}", pts.value()),
        Spacing::Percent(pct) => format!("Percent:{}", pct),
    }
}

fn decode_spacing(s: &str) -> Option<Spacing> {
    if let Some(rest) = s.strip_prefix("Exact:") {
        rest.parse::<f64>().ok().map(|v| Spacing::Exact(Points::new(v)))
    } else if let Some(rest) = s.strip_prefix("Percent:") {
        rest.parse::<f32>().ok().map(Spacing::Percent)
    } else {
        None
    }
}

// Encode LineHeight as "Exact:<f64>", "AtLeast:<f64>", or "Multiple:<f32>"
fn encode_line_height(lh: &LineHeight) -> String {
    match lh {
        LineHeight::Exact(pts) => format!("Exact:{}", pts.value()),
        LineHeight::AtLeast(pts) => format!("AtLeast:{}", pts.value()),
        LineHeight::Multiple(m) => format!("Multiple:{}", m),
    }
}

fn decode_line_height(s: &str) -> Option<LineHeight> {
    if let Some(rest) = s.strip_prefix("Exact:") {
        rest.parse::<f64>().ok().map(|v| LineHeight::Exact(Points::new(v)))
    } else if let Some(rest) = s.strip_prefix("AtLeast:") {
        rest.parse::<f64>().ok().map(|v| LineHeight::AtLeast(Points::new(v)))
    } else if let Some(rest) = s.strip_prefix("Multiple:") {
        rest.parse::<f32>().ok().map(LineHeight::Multiple)
    } else {
        None
    }
}

fn decode_underline(s: &str) -> Option<UnderlineStyle> {
    match s {
        "Single" => Some(UnderlineStyle::Single),
        "Double" => Some(UnderlineStyle::Double),
        "Dotted" => Some(UnderlineStyle::Dotted),
        "Dash" => Some(UnderlineStyle::Dash),
        "Wave" => Some(UnderlineStyle::Wave),
        "Thick" => Some(UnderlineStyle::Thick),
        _ => None,
    }
}

fn decode_strikethrough(s: &str) -> Option<StrikethroughStyle> {
    match s {
        "Single" => Some(StrikethroughStyle::Single),
        "Double" => Some(StrikethroughStyle::Double),
        _ => None,
    }
}

fn decode_vertical_align(s: &str) -> Option<VerticalAlign> {
    match s {
        "Superscript" => Some(VerticalAlign::Superscript),
        "Subscript" => Some(VerticalAlign::Subscript),
        "Baseline" => Some(VerticalAlign::Baseline),
        _ => None,
    }
}

// ── Loro map value accessors ──────────────────────────────────────────────────

fn get_str_from_map(map: &LoroMap, key: &str) -> Option<String> {
    map.get(key)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_string().ok())
        .map(|s| s.to_string())
}

fn get_f64_from_map(map: &LoroMap, key: &str) -> Option<f64> {
    map.get(key)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_double().ok())
}

fn get_bool_from_map(map: &LoroMap, key: &str) -> Option<bool> {
    map.get(key)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_bool().ok())
}

fn get_i64_from_map(map: &LoroMap, key: &str) -> Option<i64> {
    map.get(key)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_i64().ok())
}

// ── ParaProps reconstruction ──────────────────────────────────────────────────

fn reconstruct_para_props(block_map: &LoroMap) -> Option<ParaProps> {
    let props_map = block_map
        .get(KEY_PARA_PROPS)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())?;

    let mut props = ParaProps::default();
    let mut any = false;

    if let Some(s) = get_str_from_map(&props_map, PROP_ALIGNMENT) {
        if let Some(a) = decode_alignment(&s) { props.alignment = Some(a); any = true; }
    }
    if let Some(v) = get_f64_from_map(&props_map, PROP_INDENT_LEFT) {
        props.indent_start = Some(Points::new(v)); any = true;
    }
    if let Some(v) = get_f64_from_map(&props_map, PROP_INDENT_RIGHT) {
        props.indent_end = Some(Points::new(v)); any = true;
    }
    if let Some(v) = get_f64_from_map(&props_map, PROP_INDENT_FIRST_LINE) {
        props.indent_first_line = Some(Points::new(v)); any = true;
    }
    if let Some(v) = get_f64_from_map(&props_map, PROP_INDENT_HANGING) {
        props.indent_hanging = Some(Points::new(v)); any = true;
    }
    if let Some(v) = get_bool_from_map(&props_map, PROP_KEEP_TOGETHER) {
        props.keep_together = Some(v); any = true;
    }
    if let Some(v) = get_bool_from_map(&props_map, PROP_KEEP_WITH_NEXT) {
        props.keep_with_next = Some(v); any = true;
    }
    if let Some(v) = get_bool_from_map(&props_map, PROP_PAGE_BREAK_AFTER) {
        props.page_break_after = Some(v); any = true;
    }
    if let Some(v) = get_bool_from_map(&props_map, PROP_BIDI) {
        props.bidi = Some(v); any = true;
    }
    if let Some(v) = get_i64_from_map(&props_map, PROP_WIDOW_CONTROL) {
        props.widow_control = Some(v as u8); any = true;
    }
    if let Some(v) = get_i64_from_map(&props_map, PROP_LIST_LEVEL) {
        props.list_level = Some(v as u8); any = true;
    }
    if let Some(s) = get_str_from_map(&props_map, PROP_SPACE_BEFORE_PT) {
        if let Some(sp) = decode_spacing(&s) { props.space_before = Some(sp); any = true; }
    }
    if let Some(s) = get_str_from_map(&props_map, PROP_SPACE_AFTER_PT) {
        if let Some(sp) = decode_spacing(&s) { props.space_after = Some(sp); any = true; }
    }
    if let Some(s) = get_str_from_map(&props_map, PROP_LINE_HEIGHT) {
        if let Some(lh) = decode_line_height(&s) { props.line_height = Some(lh); any = true; }
    }
    if let Some(s) = get_str_from_map(&props_map, PROP_LIST_ID) {
        props.list_id = Some(ListId::new(s)); any = true;
    }
    // tab_stops and background_color are complex; skip deserialization for now

    if any { Some(props) } else { None }
}

// ── CharProps map serialization / reconstruction ───────────────────────────────

fn map_char_props_to_map(props: &CharProps, map: &LoroMap) -> Result<(), BridgeError> {
    if let Some(v) = props.bold { map.insert("bold", v)?; }
    if let Some(v) = props.italic { map.insert("italic", v)?; }
    if let Some(v) = props.outline { map.insert("outline", v)?; }
    if let Some(v) = props.shadow { map.insert("shadow", v)?; }
    if let Some(v) = props.small_caps { map.insert("small_caps", v)?; }
    if let Some(v) = props.all_caps { map.insert("all_caps", v)?; }
    if let Some(v) = props.kerning { map.insert("kerning", v)?; }
    if let Some(v) = &props.font_name { map.insert("font_name", v.as_str())?; }
    if let Some(v) = &props.font_name_complex { map.insert("font_name_complex", v.as_str())?; }
    if let Some(v) = &props.font_name_east_asian { map.insert("font_name_east_asian", v.as_str())?; }
    if let Some(v) = &props.font_size { map.insert("font_size", v.value())?; }
    if let Some(v) = &props.font_size_complex { map.insert("font_size_complex", v.value())?; }
    if let Some(v) = props.scale { map.insert("scale", f64::from(v))?; }
    if let Some(v) = &props.letter_spacing { map.insert("letter_spacing", v.value())?; }
    if let Some(v) = &props.word_spacing { map.insert("word_spacing", v.value())?; }
    if let Some(v) = &props.underline { map.insert("underline", format!("{:?}", v))?; }
    if let Some(v) = &props.strikethrough { map.insert("strikethrough", format!("{:?}", v))?; }
    if let Some(v) = &props.vertical_align { map.insert("vertical_align", format!("{:?}", v))?; }
    if let Some(v) = &props.hyperlink { map.insert("hyperlink", v.as_str())?; }
    // color, background_color, highlight_color, language* — complex types, skip for now
    Ok(())
}

fn reconstruct_char_props_from_map(block_map: &LoroMap) -> Option<CharProps> {
    let props_map = block_map
        .get(KEY_DIRECT_CHAR_PROPS)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())?;

    let mut props = CharProps::default();
    let mut any = false;

    macro_rules! set_bool {
        ($field:ident, $key:literal) => {
            if let Some(v) = get_bool_from_map(&props_map, $key) {
                props.$field = Some(v);
                any = true;
            }
        };
    }
    set_bool!(bold, "bold");
    set_bool!(italic, "italic");
    set_bool!(outline, "outline");
    set_bool!(shadow, "shadow");
    set_bool!(small_caps, "small_caps");
    set_bool!(all_caps, "all_caps");
    set_bool!(kerning, "kerning");

    if let Some(s) = get_str_from_map(&props_map, "font_name") { props.font_name = Some(s); any = true; }
    if let Some(s) = get_str_from_map(&props_map, "font_name_complex") { props.font_name_complex = Some(s); any = true; }
    if let Some(s) = get_str_from_map(&props_map, "font_name_east_asian") { props.font_name_east_asian = Some(s); any = true; }
    if let Some(v) = get_f64_from_map(&props_map, "font_size") { props.font_size = Some(Points::new(v)); any = true; }
    if let Some(v) = get_f64_from_map(&props_map, "font_size_complex") { props.font_size_complex = Some(Points::new(v)); any = true; }
    if let Some(v) = get_f64_from_map(&props_map, "scale") { props.scale = Some(v as f32); any = true; }
    if let Some(v) = get_f64_from_map(&props_map, "letter_spacing") { props.letter_spacing = Some(Points::new(v)); any = true; }
    if let Some(v) = get_f64_from_map(&props_map, "word_spacing") { props.word_spacing = Some(Points::new(v)); any = true; }
    if let Some(s) = get_str_from_map(&props_map, "underline") {
        if let Some(u) = decode_underline(&s) { props.underline = Some(u); any = true; }
    }
    if let Some(s) = get_str_from_map(&props_map, "strikethrough") {
        if let Some(st) = decode_strikethrough(&s) { props.strikethrough = Some(st); any = true; }
    }
    if let Some(s) = get_str_from_map(&props_map, "vertical_align") {
        if let Some(va) = decode_vertical_align(&s) { props.vertical_align = Some(va); any = true; }
    }
    if let Some(s) = get_str_from_map(&props_map, "hyperlink") { props.hyperlink = Some(s); any = true; }

    if any { Some(props) } else { None }
}

/// Derives a stable Loro [`Cursor`] from a document position.
///
/// The cursor anchors to a Fugue element ID (not an integer offset) so it
/// remains valid after concurrent insertions and deletions.
///
/// # Parameters
///
/// * `loro` — the live [`LoroDoc`] for this editing session.
/// * `page_index` — zero-based page index (unused in current stub; pages are
///   layout concepts, not Loro container boundaries).
/// * `paragraph_index` — the paragraph's index within
///   [`PageEditingData::paragraph_layouts`] for the given page.  This is a
///   **page-local** index; the mapping to the global Loro block index requires
///   a table that the layout engine does not yet expose.
/// * `byte_offset` — byte offset into the paragraph's flattened text.
///
/// # Current limitations
///
/// This MVP returns `None` because the mapping from `(page_index,
/// paragraph_index)` to the corresponding Loro `LoroText` container requires
/// Derives a stable Loro [`Cursor`] from a document position identified by
/// `block_index` (paragraph index, section 0) and a UTF-8 `byte_offset`.
///
/// The cursor survives concurrent remote edits, making it suitable for
/// collaborative editing.  Returns `None` when the block or text container
/// cannot be found (e.g. a stub block type that has no `LoroText`).
///
/// # Session 3a limitation
///
/// Uses `block_index` as a direct index into section 0's block list.
/// Multi-section support (where `paragraph_index` must be mapped to a
/// `(section_idx, block_idx)` pair) is deferred to a later session once
/// `PageEditingData` carries stable block identifiers.
///
/// [`Cursor`]: loro::Cursor
pub fn derive_loro_cursor(
    loro: &LoroDoc,
    block_index: usize,
    byte_offset: usize,
) -> Option<loro::cursor::Cursor> {
    // Navigate sections[0].blocks[block_index]["content"] → LoroText.
    let sections_list = loro.get_list(KEY_SECTIONS);
    let sec_val = sections_list.get(0)?;
    let sec_map = sec_val
        .into_container()
        .ok()
        .and_then(|c| c.into_map().ok())?;

    let blocks_val = sec_map.get(KEY_BLOCKS)?;
    let blocks_list = blocks_val
        .into_container()
        .ok()
        .and_then(|c| c.into_movable_list().ok())?;

    if block_index >= blocks_list.len() {
        return None;
    }

    let block_val = blocks_list.get(block_index)?;
    let block_map = block_val
        .into_container()
        .ok()
        .and_then(|c| c.into_map().ok())?;

    let content_val = block_map.get(KEY_CONTENT)?;
    let text = content_val
        .into_container()
        .ok()
        .and_then(|c| c.into_text().ok())?;

    // Side::Right places the cursor just after the character at byte_offset.
    text.get_cursor(byte_offset, loro::cursor::Side::Right)
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
        if let Some(sec_val) = sections_list.get(i)
            && let Some(sec_map) = sec_val.into_container().ok().and_then(|c| c.into_map().ok())
        {
            let mut section = crate::layout::section::Section::new();
            
            if let Some(blocks_val) = sec_map.get(KEY_BLOCKS)
                && let Some(blocks_list) = blocks_val.into_container().ok().and_then(|c| c.into_movable_list().ok())
            {
                for j in 0..blocks_list.len() {
                    if let Some(block_val) = blocks_list.get(j)
                        && let Some(block_map) = block_val.into_container().ok().and_then(|c| c.into_map().ok())
                        && let Ok(block) = map_loro_block(&block_map)
                    {
                        section.blocks.push(block);
                    }
                }
            }
            
            doc.sections.push(section);
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
            let direct_para_props = reconstruct_para_props(map).map(Box::new);
            let direct_char_props = reconstruct_char_props_from_map(map).map(Box::new);
            let styled_para = crate::content::block::StyledParagraph {
                style_id,
                direct_para_props,
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
    if let Some(content_val) = map.get(KEY_CONTENT)
        && let Some(text_container) = content_val.into_container().ok().and_then(|c| c.into_text().ok())
    {
        let delta = text_container.to_delta();
            for span in delta {
                // span might be an enum DeltaItem::Insert
                if let loro::TextDelta::Insert { insert, attributes } = span {
                    // Inspect marks
                    if let Some(attrs) = attributes {
                        let mut props = CharProps::default();
                        let mut has_props = false;
                        
                        // Try to parse boolean marks
                        if let Some(_v) = attrs.get(MARK_BOLD) { 
                            props.bold = Some(true); // Since it was in the map as truthy
                            has_props = true;
                        }
                        if let Some(_v) = attrs.get(MARK_ITALIC) { 
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
    Ok(inlines)
}
