// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: MIT

//! Serialises a [`RelationshipSet`] to a `.rels` part.

use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, Event};

use crate::constants::RELATIONSHIPS_NS;
use crate::error::OpcResult;
use crate::relationships::{RelationshipSet, TargetMode};

/// Serialises `set` to `.rels` XML bytes: a `<Relationships>` root with one
/// `<Relationship>` element per entry (emitting `TargetMode="External"` only
/// for external targets).
pub fn write_relationships_part(set: &RelationshipSet) -> OpcResult<Vec<u8>> {
    let mut writer = Writer::new_with_indent(Vec::new(), b' ', 2);

    writer.write_event(Event::Decl(BytesDecl::new(
        "1.0",
        Some("UTF-8"),
        Some("yes"),
    )))?;

    let mut relationships_tag = BytesStart::new("Relationships");
    relationships_tag.push_attribute(("xmlns", RELATIONSHIPS_NS));

    writer.write_event(Event::Start(relationships_tag))?;

    for rel in set.iter() {
        let mut rel_tag = BytesStart::new("Relationship");
        rel_tag.push_attribute(("Id", rel.id.as_str()));
        rel_tag.push_attribute(("Type", rel.rel_type.as_str()));
        rel_tag.push_attribute(("Target", rel.target.as_str()));

        if rel.target_mode == TargetMode::External {
            rel_tag.push_attribute(("TargetMode", "External"));
        }

        writer.write_event(Event::Empty(rel_tag))?;
    }

    writer.write_event(Event::End(BytesEnd::new("Relationships")))?;

    Ok(writer.into_inner())
}
