// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: MIT

//! Output generation explicitly formatting properties defining metadata matching specification boundaries correctly identifying items.

use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, Event};
use quick_xml::Writer;

use crate::error::OpcResult;
use crate::content_types::ContentTypeMap;

/// Structures mappings systematically isolating standard components serializing definitions safely internally tracking constraints universally.
pub fn write_content_types(map: &ContentTypeMap) -> OpcResult<Vec<u8>> {
    let mut writer = Writer::new_with_indent(Vec::new(), b' ', 2);
    
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), Some("yes"))))?;
    
    let mut types_tag = BytesStart::new("Types");
    types_tag.push_attribute(("xmlns", "http://schemas.openxmlformats.org/package/2006/content-types"));
    
    writer.write_event(Event::Start(types_tag))?;
    
    for (ext, ct) in map.defaults() {
        let mut def_tag = BytesStart::new("Default");
        def_tag.push_attribute(("Extension", ext.as_str()));
        def_tag.push_attribute(("ContentType", ct.as_str()));
        writer.write_event(Event::Empty(def_tag))?;
    }
    
    for (name, ct) in map.overrides() {
        let mut ovr_tag = BytesStart::new("Override");
        ovr_tag.push_attribute(("PartName", name.as_str()));
        ovr_tag.push_attribute(("ContentType", ct.as_str()));
        writer.write_event(Event::Empty(ovr_tag))?;
    }
    
    writer.write_event(Event::End(BytesEnd::new("Types")))?;
    
    Ok(writer.into_inner())
}
