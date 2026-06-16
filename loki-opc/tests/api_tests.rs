// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: MIT

//! Public-API tests for `loki-opc`: part names, content types, relationships,
//! the compatibility shims, target-reference resolution, and a full package
//! ZIP round-trip.

use loki_opc::{
    ContentTypeMap, Package, PartData, PartName, Relationship, RelationshipSet, TargetMode,
};

// ── PartName validation & accessors ─────────────────────────────────────────────

#[test]
fn part_name_accepts_valid_names() {
    for name in [
        "/word/document.xml",
        "/a",
        "/docProps/core.xml",
        "/_rels/.rels",
    ] {
        assert!(PartName::new(name).is_ok(), "{name} should be valid");
    }
}

#[test]
fn part_name_rejects_invalid_names() {
    for bad in [
        "",                      // empty
        "word/document.xml",     // no leading slash
        "/word/",                // trailing slash
        "/word//document.xml",   // empty segment
        "/word/./document.xml",  // '.' segment
        "/word/../secret.xml",   // '..' segment
        "/word/document.",       // segment ends with '.'
        "/word/doc%2fument.xml", // percent-encoded slash
    ] {
        assert!(PartName::new(bad).is_err(), "{bad:?} should be rejected");
    }
}

#[test]
fn part_name_extension_and_rels_name() {
    let n = PartName::new("/word/document.xml").unwrap();
    assert_eq!(n.extension(), Some("xml"));
    assert_eq!(
        n.relationships_part_name().as_str(),
        "/word/_rels/document.xml.rels"
    );
    assert_eq!(PartName::new("/word/noext").unwrap().extension(), None);
}

#[test]
fn part_name_compares_case_insensitively() {
    // §6.3.5: part names are compared case-insensitively but case-preserved.
    let a = PartName::new("/Word/Document.XML").unwrap();
    let b = PartName::new("/word/document.xml").unwrap();
    assert_eq!(a, b);
    assert_eq!(a.as_str(), "/Word/Document.XML"); // original case retained
}

// ── ContentTypeMap ──────────────────────────────────────────────────────────────

#[test]
fn content_type_map_resolves_defaults_case_insensitively() {
    let mut map = ContentTypeMap::default();
    map.add_default("XML", "application/xml");
    let name = PartName::new("/a/b.xml").unwrap();
    assert_eq!(map.resolve(&name), Some("application/xml"));
    // Unknown extension resolves to None.
    assert_eq!(map.resolve(&PartName::new("/a/b.png").unwrap()), None);
}

#[test]
fn content_type_override_takes_precedence_over_default() {
    let mut map = ContentTypeMap::default();
    map.add_default("xml", "application/xml");
    let special = PartName::new("/word/document.xml").unwrap();
    map.add_override(&special, "application/vnd.ms-word+xml");
    assert_eq!(map.resolve(&special), Some("application/vnd.ms-word+xml"));
    // A different .xml part still uses the default.
    assert_eq!(
        map.resolve(&PartName::new("/word/styles.xml").unwrap()),
        Some("application/xml")
    );
}

// ── RelationshipSet ─────────────────────────────────────────────────────────────

fn rel(id: &str, rel_type: &str, target: &str) -> Relationship {
    Relationship {
        id: id.to_string(),
        rel_type: rel_type.to_string(),
        target: target.to_string(),
        target_mode: TargetMode::Internal,
    }
}

#[test]
fn relationship_set_add_get_remove() {
    let mut set = RelationshipSet::default();
    assert!(set.is_empty());
    set.add(rel("rId1", "type/a", "a.xml")).unwrap();
    set.add(rel("rId2", "type/b", "b.xml")).unwrap();
    assert_eq!(set.len(), 2);
    assert_eq!(set.get("rId2").unwrap().target, "b.xml");
    assert!(set.get("missing").is_none());

    let removed = set.remove("rId1").unwrap();
    assert_eq!(removed.id, "rId1");
    assert_eq!(set.len(), 1);
    assert!(set.remove("rId1").is_none());
}

#[test]
fn relationship_set_by_type_filters() {
    let mut set = RelationshipSet::default();
    set.add(rel("rId1", "type/image", "a.png")).unwrap();
    set.add(rel("rId2", "type/image", "b.png")).unwrap();
    set.add(rel("rId3", "type/hyperlink", "http://e")).unwrap();
    let images: Vec<_> = set.by_type("type/image").map(|r| r.id.as_str()).collect();
    assert_eq!(images, ["rId1", "rId2"]);
}

#[test]
fn relationship_set_next_id_increments_past_max() {
    let mut set = RelationshipSet::default();
    assert_eq!(set.next_id(), "rId1");
    set.add(rel("rId1", "t", "a")).unwrap();
    set.add(rel("rId7", "t", "b")).unwrap();
    assert_eq!(set.next_id(), "rId8");
}

#[test]
fn relationship_set_add_external_assigns_id_and_mode() {
    let mut set = RelationshipSet::default();
    let id = set.add_external("type/hyperlink", "https://example.com");
    assert_eq!(id, "rId1");
    let r = set.get("rId1").unwrap();
    assert_eq!(r.target_mode, TargetMode::External);
    assert_eq!(r.target, "https://example.com");
}

// ── Relationships .rels round-trip ──────────────────────────────────────────────

#[test]
fn relationships_part_round_trip() {
    use loki_opc::relationships::{parse_relationships_part, write_relationships_part};

    let mut set = RelationshipSet::default();
    set.add(rel(
        "rId1",
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument",
        "word/document.xml",
    ))
    .unwrap();
    set.add_external("type/hyperlink", "https://example.com");

    let bytes = write_relationships_part(&set).unwrap();
    let mut warnings = Vec::new();
    let parsed = parse_relationships_part(&bytes, "/_rels/.rels", &mut warnings).unwrap();

    assert_eq!(parsed.len(), 2);
    let internal = parsed.iter().find(|r| r.id == "rId1").unwrap();
    assert_eq!(internal.target, "word/document.xml");
    assert_eq!(internal.target_mode, TargetMode::Internal);
    let external = parsed.iter().find(|r| r.id == "rId2").unwrap();
    assert_eq!(external.target_mode, TargetMode::External);
}

// ── Content-types XML round-trip ────────────────────────────────────────────────

#[test]
fn content_types_round_trip() {
    use loki_opc::content_types::{parse_content_types, write_content_types};

    let mut map = ContentTypeMap::default();
    map.add_default("xml", "application/xml");
    map.add_default("png", "image/png");
    let doc = PartName::new("/word/document.xml").unwrap();
    map.add_override(&doc, "application/vnd.ms-word+xml");

    let bytes = write_content_types(&map).unwrap();
    let mut warnings = Vec::new();
    let parsed = parse_content_types(&bytes, &mut warnings).unwrap();

    assert_eq!(parsed.resolve(&doc), Some("application/vnd.ms-word+xml"));
    assert_eq!(
        parsed.resolve(&PartName::new("/a/b.png").unwrap()),
        Some("image/png")
    );
}

// ── Compatibility shims (non-strict default build) ──────────────────────────────

#[cfg(not(feature = "strict"))]
#[test]
fn compat_normalizes_backslash_paths() {
    use loki_opc::compat::zip_names::normalize_backslash;
    let mut warnings = Vec::new();
    let out = normalize_backslash("word\\media\\image.png", &mut warnings);
    assert_eq!(out, "word/media/image.png");
    assert_eq!(warnings.len(), 1);

    // Conformant names pass through and record no warning.
    let mut warnings = Vec::new();
    assert_eq!(
        normalize_backslash("word/ok.png", &mut warnings),
        "word/ok.png"
    );
    assert!(warnings.is_empty());
}

#[cfg(not(feature = "strict"))]
#[test]
fn compat_resolve_duplicate_keeps_first_and_warns() {
    use loki_opc::compat::part_names::resolve_duplicate;
    let mut warnings = Vec::new();
    let names = ["/Doc.xml", "/doc.xml"];
    let kept = resolve_duplicate(&names, &mut warnings).unwrap();
    assert_eq!(kept, "/Doc.xml");
    assert_eq!(warnings.len(), 1);
}

#[test]
fn compat_fallback_media_types() {
    use loki_opc::compat::content_types::fallback_media_type;
    assert_eq!(fallback_media_type("png"), "image/png");
    assert_eq!(fallback_media_type("JPG"), "image/jpeg"); // case-insensitive
    assert_eq!(fallback_media_type("rels"), MEDIA_TYPE_RELS);
    assert_eq!(fallback_media_type("unknownext"), "");
}

const MEDIA_TYPE_RELS: &str = "application/vnd.openxmlformats-package.relationships+xml";

// ── Relationship target resolution ──────────────────────────────────────────────

#[test]
fn resolve_relative_reference_handles_paths() {
    use loki_opc::part::addressing::resolve_relative_reference;
    let base = PartName::new("/word/document.xml").unwrap();

    // Absolute target is taken as-is.
    assert_eq!(
        resolve_relative_reference(&base, "/customXml/item1.xml")
            .unwrap()
            .as_str(),
        "/customXml/item1.xml"
    );
    // Relative target is resolved against the base directory.
    assert_eq!(
        resolve_relative_reference(&base, "media/image1.png")
            .unwrap()
            .as_str(),
        "/word/media/image1.png"
    );
    // Parent traversal walks up out of the base directory.
    assert_eq!(
        resolve_relative_reference(&base, "../media/image1.png")
            .unwrap()
            .as_str(),
        "/media/image1.png"
    );
}

// ── Full package ZIP round-trip ─────────────────────────────────────────────────

#[test]
fn package_round_trip_preserves_parts_and_relationships() {
    let mut pkg = Package::new();
    pkg.content_type_map_mut()
        .add_default("xml", "application/xml");

    let doc = PartName::new("/word/document.xml").unwrap();
    let body = b"<document>hi</document>".to_vec();
    pkg.set_part(doc.clone(), PartData::new(body.clone(), "application/xml"));

    pkg.relationships_mut()
        .add(rel(
            "rId1",
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument",
            "word/document.xml",
        ))
        .unwrap();

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.docx");
    pkg.write_path(&path).unwrap();

    let reopened = Package::open_path(&path).unwrap();
    let part = reopened.part(&doc).expect("document part survives");
    assert_eq!(part.bytes, body);
    assert_eq!(reopened.content_type(&doc), Some("application/xml"));
    assert_eq!(
        reopened.relationships().get("rId1").unwrap().target,
        "word/document.xml"
    );
}
