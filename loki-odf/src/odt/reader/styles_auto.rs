// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Automatic-styles fast reader for `content.xml`, split out of `styles.rs`
//! for the 300-line ceiling. `read_auto_styles` is re-exported `pub(crate)`
//! (called by `odt/import.rs`); `parse_style_family` is re-exported for
//! `read_stylesheet`. Style-property parsing is reached via
//! `super::parse_style_props`.

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::error::{OdfError, OdfResult};
use crate::odt::model::styles::{OdfStyle, OdfStyleFamily};
use crate::xml_util::local_attr_val;

use super::parse_style_props;

/// Parse the `office:automatic-styles` section of `content.xml`, returning
/// only the `style:style` elements found within it, all marked
/// `is_automatic = true`.
///
/// Stops reading as soon as the closing `</office:automatic-styles>` tag is
/// encountered, so the (potentially large) `office:body` section is never
/// touched.
pub(crate) fn read_auto_styles(xml: &[u8]) -> OdfResult<Vec<OdfStyle>> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);

    let mut buf = Vec::new();
    let mut styles: Vec<OdfStyle> = Vec::new();
    let mut inside = false;

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"automatic-styles" => inside = true,
                    b"style" if inside => {
                        let name = local_attr_val(e, b"name").unwrap_or_default();
                        let display_name = local_attr_val(e, b"display-name");
                        let family = parse_style_family(
                            local_attr_val(e, b"family").as_deref().unwrap_or(""),
                        );
                        let parent_name = local_attr_val(e, b"parent-style-name");
                        let list_style_name = local_attr_val(e, b"list-style-name");
                        let master_page_name = local_attr_val(e, b"master-page-name");
                        drop(e);
                        let props = parse_style_props(&mut reader, b"style")?;
                        styles.push(OdfStyle {
                            name,
                            display_name,
                            family,
                            parent_name,
                            list_style_name,
                            para_props: props.para_props,
                            text_props: props.text_props,
                            col_width: props.col_width,
                            cell_props: props.cell_props,
                            graphic_wrap: props.graphic_wrap,
                            table_props: props.table_props,
                            is_automatic: true,
                            master_page_name,
                        });
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                if inside && e.local_name().into_inner() == b"style" {
                    let name = local_attr_val(e, b"name").unwrap_or_default();
                    let display_name = local_attr_val(e, b"display-name");
                    let family =
                        parse_style_family(local_attr_val(e, b"family").as_deref().unwrap_or(""));
                    let parent_name = local_attr_val(e, b"parent-style-name");
                    let list_style_name = local_attr_val(e, b"list-style-name");
                    let master_page_name = local_attr_val(e, b"master-page-name");
                    styles.push(OdfStyle {
                        name,
                        display_name,
                        family,
                        parent_name,
                        list_style_name,
                        para_props: None,
                        text_props: None,
                        col_width: None,
                        cell_props: None,
                        graphic_wrap: None,
                        table_props: None,
                        is_automatic: true,
                        master_page_name,
                    });
                }
            }
            Ok(Event::End(ref e)) => {
                if e.local_name().into_inner() == b"automatic-styles" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OdfError::Xml {
                    part: "content.xml".to_string(),
                    source: e,
                });
            }
            _ => {}
        }
    }

    Ok(styles)
}

// ── Shared helpers ─────────────────────────────────────────────────────────────

pub(super) fn parse_style_family(s: &str) -> OdfStyleFamily {
    match s {
        "paragraph" => OdfStyleFamily::Paragraph,
        "text" => OdfStyleFamily::Text,
        "table" => OdfStyleFamily::Table,
        "table-row" => OdfStyleFamily::TableRow,
        "table-cell" => OdfStyleFamily::TableCell,
        "graphic" => OdfStyleFamily::Graphic,
        _ => OdfStyleFamily::Unknown,
    }
}
