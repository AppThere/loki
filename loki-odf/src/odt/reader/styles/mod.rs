// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2025 AppThere contributors

//! Reader for `styles.xml` and the `office:automatic-styles` section of
//! `content.xml`.  Returns an [`OdfStylesheet`].  ODF 1.3 §14–§16.
// Functions in this module are consumed by the document reader added in the
// next session; suppress premature dead-code lints.
#![allow(dead_code)]
// `drop(ref_binding)` is a deliberate NLL-boundary hint and does nothing at
// runtime; silence the compiler's suggestion to use `let _ = ...` instead.
#![allow(dropping_references)]

mod auto_styles;
mod list_level;
mod list_style;
mod page;
mod para_props;
mod props;
mod util;

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::error::{OdfError, OdfResult};
use crate::odt::model::document::OdfMasterPage;
use crate::odt::model::list_styles::OdfListStyle;
use crate::odt::model::styles::{OdfDefaultStyle, OdfStyle, OdfStylesheet};
use crate::xml_util::local_attr_val;

use list_style::parse_list_style;
use page::{parse_master_page, parse_page_layout};
use props::{parse_style_family, parse_style_props};

// ── Public entry points ────────────────────────────────────────────────────────

/// Parse `styles.xml` (pass `is_automatic = false`) or the
/// `office:automatic-styles` block from `content.xml`
/// (pass `is_automatic = true`).
///
/// When `is_automatic` is `false` the reader distinguishes between
/// `office:styles` (→ `named_styles`) and `office:automatic-styles`
/// (→ `auto_styles`), and also collects page layouts and master pages.
/// When `is_automatic` is `true` every `style:style` found anywhere in the
/// document goes into `auto_styles`.
#[allow(clippy::too_many_lines)]
// Function body is a single large match over XML events; splitting would reduce readability.
pub(crate) fn read_stylesheet(xml: &[u8], is_automatic: bool) -> OdfResult<OdfStylesheet> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);

    let mut buf = Vec::new();
    let mut sheet = OdfStylesheet::default();

    // Which section we are currently inside (only relevant when !is_automatic).
    let mut in_named = false; // inside office:styles
    let mut in_auto = false; // inside office:automatic-styles
    let mut in_master = false; // inside office:master-styles

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"styles" => {
                        // office:styles (named)
                        in_named = true;
                        in_auto = false;
                        in_master = false;
                    }
                    b"automatic-styles" => {
                        in_named = false;
                        in_auto = true;
                        in_master = false;
                    }
                    b"master-styles" => {
                        in_named = false;
                        in_auto = false;
                        in_master = true;
                    }
                    b"style" => {
                        // style:style
                        let name = local_attr_val(e, b"name").unwrap_or_default();
                        let display_name = local_attr_val(e, b"display-name");
                        let family = parse_style_family(
                            local_attr_val(e, b"family").as_deref().unwrap_or(""),
                        );
                        let parent_name = local_attr_val(e, b"parent-style-name");
                        let list_style_name = local_attr_val(e, b"list-style-name");
                        // COMPAT(odf): style:master-page-name on a paragraph
                        // style signals a master page transition. The new master
                        // page's layout applies from that paragraph onward.
                        let master_page_name = local_attr_val(e, b"master-page-name");
                        let auto = is_automatic || in_auto;
                        drop(e);
                        let (para_props, text_props, col_width, cell_props) =
                            parse_style_props(&mut reader, b"style")?;
                        let style = OdfStyle {
                            name,
                            display_name,
                            family,
                            parent_name,
                            list_style_name,
                            para_props,
                            text_props,
                            col_width,
                            cell_props,
                            is_automatic: auto,
                            master_page_name,
                        };
                        if auto {
                            sheet.auto_styles.push(style);
                        } else {
                            sheet.named_styles.push(style);
                        }
                    }
                    b"default-style" => {
                        let family = parse_style_family(
                            local_attr_val(e, b"family").as_deref().unwrap_or(""),
                        );
                        drop(e);
                        let (para_props, text_props, _col_width, _cell_props) =
                            parse_style_props(&mut reader, b"default-style")?;
                        sheet.default_styles.push(OdfDefaultStyle {
                            family,
                            para_props,
                            text_props,
                        });
                    }
                    b"list-style" => {
                        let name = local_attr_val(e, b"name").unwrap_or_default();
                        drop(e);
                        let list_style = parse_list_style(&mut reader, name)?;
                        sheet.list_styles.push(list_style);
                    }
                    b"page-layout" => {
                        let name = local_attr_val(e, b"name").unwrap_or_default();
                        drop(e);
                        let layout = parse_page_layout(&mut reader, name)?;
                        sheet.page_layouts.push(layout);
                    }
                    b"master-page" => {
                        let name = local_attr_val(e, b"name").unwrap_or_default();
                        let page_layout_name =
                            local_attr_val(e, b"page-layout-name").unwrap_or_default();
                        drop(e);
                        let master = parse_master_page(&mut reader, name, page_layout_name)?;
                        sheet.master_pages.push(master);
                    }
                    _ => {
                        // Skip unrecognised elements inside known sections.
                        // We only need to skip if we're not in a section we
                        // manage explicitly above.
                        drop(e);
                        if in_named || in_auto || in_master || is_automatic {
                            // don't skip – the enclosing section handler above
                            // will encounter the End tag and close its section
                        }
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"style" => {
                        // style:style with no children (no props)
                        let name = local_attr_val(e, b"name").unwrap_or_default();
                        let display_name = local_attr_val(e, b"display-name");
                        let family = parse_style_family(
                            local_attr_val(e, b"family").as_deref().unwrap_or(""),
                        );
                        let parent_name = local_attr_val(e, b"parent-style-name");
                        let list_style_name = local_attr_val(e, b"list-style-name");
                        let master_page_name = local_attr_val(e, b"master-page-name");
                        let auto = is_automatic || in_auto;
                        let style = OdfStyle {
                            name,
                            display_name,
                            family,
                            parent_name,
                            list_style_name,
                            para_props: None,
                            text_props: None,
                            col_width: None,
                            cell_props: None,
                            is_automatic: auto,
                            master_page_name,
                        };
                        if auto {
                            sheet.auto_styles.push(style);
                        } else {
                            sheet.named_styles.push(style);
                        }
                    }
                    b"list-style" => {
                        let name = local_attr_val(e, b"name").unwrap_or_default();
                        sheet.list_styles.push(OdfListStyle {
                            name,
                            levels: Vec::new(),
                        });
                    }
                    b"master-page" => {
                        // Self-closing <style:master-page .../> — no header/footer content.
                        // TODO(odf-master-page): style:master-page-name transitions not implemented.
                        let name = local_attr_val(e, b"name").unwrap_or_default();
                        let page_layout_name =
                            local_attr_val(e, b"page-layout-name").unwrap_or_default();
                        sheet.master_pages.push(OdfMasterPage {
                            name,
                            page_layout_name,
                            header: None,
                            footer: None,
                            header_first: None,
                            footer_first: None,
                            header_even: None,
                            footer_even: None,
                        });
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let local = e.local_name().into_inner();
                match local {
                    b"styles" => in_named = false,
                    b"automatic-styles" => in_auto = false,
                    b"master-styles" => in_master = false,
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OdfError::Xml {
                    part: "styles.xml".to_string(),
                    source: e,
                });
            }
            _ => {}
        }
    }

    Ok(sheet)
}

pub(crate) use auto_styles::read_auto_styles;

#[cfg(test)]
mod styles_tests;
