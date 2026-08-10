// SPDX-License-Identifier: Apache-2.0

//! Tests for the dialog's style-target resolution — the display-key →
//! catalog-id mapping whose absence made the Paragraph button a no-op.

use loki_doc_model::style::{ParagraphStyle, StyleCatalog, StyleId};

use super::resolve_target;

/// Applies the seed (when one was produced) and returns the resolved id plus
/// whether seeding happened — the shape the caller acts on.
fn resolve(catalog: &mut StyleCatalog, key: &str, stored: Option<&str>) -> Option<(String, bool)> {
    let (id, seed) = resolve_target(catalog, key, stored.map(str::to_string))?;
    let seeded = seed.is_some();
    if let Some(mutate) = seed {
        mutate(catalog);
    }
    Some((id, seeded))
}

fn blank_catalog() -> StyleCatalog {
    // The same catalog `Document::new_blank` seeds: Heading1–6, no default
    // paragraph style.
    let mut c = StyleCatalog::default();
    for level in 1..=6u8 {
        let s = ParagraphStyle::builtin_heading(level);
        c.paragraph_styles.insert(s.id.clone(), s);
    }
    c
}

/// A styled paragraph's id passes straight through when the catalog defines it.
#[test]
fn a_catalog_id_resolves_to_itself() {
    let mut c = blank_catalog();
    assert_eq!(
        resolve(&mut c, "Heading1", None),
        Some(("Heading1".into(), false))
    );
}

/// The display key `"Heading N"` (with the space) is not a catalog id — it must
/// map to the canonical `HeadingN` definition, not miss and render nothing.
#[test]
fn a_heading_display_key_maps_to_the_canonical_id() {
    let mut c = blank_catalog();
    assert_eq!(
        resolve(&mut c, "Heading 2", None),
        Some(("Heading2".into(), false))
    );
}

/// A heading whose block stores its own style name (ODF `text:style-name`)
/// edits that definition — the one the layout resolver consults first.
#[test]
fn a_stored_heading_style_wins_over_the_canonical_id() {
    let mut c = blank_catalog();
    let s = ParagraphStyle {
        id: StyleId::new("Heading_20_1"),
        ..ParagraphStyle::builtin_heading(1)
    };
    c.paragraph_styles.insert(s.id.clone(), s);
    assert_eq!(
        resolve(&mut c, "Heading 1", Some("Heading_20_1")),
        Some(("Heading_20_1".into(), false))
    );
}

/// A stored heading name with no definition is seeded **empty** under that id —
/// not with invented built-in properties the document never had.
#[test]
fn a_missing_stored_heading_style_is_seeded_empty() {
    let mut c = blank_catalog();
    let (id, seeded) = resolve(&mut c, "Heading 1", Some("Heading_20_9")).unwrap();
    assert_eq!(id, "Heading_20_9");
    assert!(seeded);
    let s = &c.paragraph_styles[&StyleId::new("Heading_20_9")];
    assert_eq!(s.char_props.bold, None);
    assert!(s.is_custom);
}

/// A heading level whose canonical definition is missing (imported catalogs)
/// is seeded from the built-in table, so the dialog edits what renders.
#[test]
fn a_missing_canonical_heading_is_seeded_from_the_builtin_table() {
    let mut c = StyleCatalog::default();
    let (id, seeded) = resolve(&mut c, "Heading 3", None).unwrap();
    assert_eq!(id, "Heading3");
    assert!(seeded);
    let s = &c.paragraph_styles[&StyleId::new("Heading3")];
    assert_eq!(s, &ParagraphStyle::builtin_heading(3));
}

/// An unstyled paragraph resolves through the catalog's default paragraph
/// style when one is installed.
#[test]
fn the_default_display_key_follows_the_catalog_default() {
    let mut c = blank_catalog();
    let def = ParagraphStyle {
        id: StyleId::new("Standard"),
        is_default: true,
        ..ParagraphStyle::builtin_default_paragraph()
    };
    c.default_paragraph_style = Some(def.id.clone());
    c.paragraph_styles.insert(def.id.clone(), def);
    assert_eq!(
        resolve(&mut c, "Default Paragraph Style", None),
        Some(("Standard".into(), false))
    );
}

/// A blank document has no default paragraph style at all — the resolution
/// seeds one **and installs it as the catalog default**, or editing it would
/// never affect unstyled paragraphs (they resolve through
/// `default_paragraph_style`, not through a name).
#[test]
fn a_missing_default_style_is_seeded_and_installed_as_the_default() {
    let mut c = blank_catalog();
    let (id, seeded) = resolve(&mut c, "Default Paragraph Style", None).unwrap();
    assert_eq!(id, "DefaultParagraphStyle");
    assert!(seeded);
    assert_eq!(c.default_paragraph_style, Some(StyleId::new(&id)));
    assert!(c.paragraph_styles.contains_key(&StyleId::new(&id)));
}

/// A `default_paragraph_style` pointing at a deleted definition must reseed,
/// not return the dangling id (the dialog's missing-style guard would close).
#[test]
fn a_dangling_catalog_default_is_reseeded() {
    let mut c = blank_catalog();
    c.default_paragraph_style = Some(StyleId::new("Gone"));
    let (id, seeded) = resolve(&mut c, "Default Paragraph Style", None).unwrap();
    assert_eq!(id, "DefaultParagraphStyle");
    assert!(seeded);
}

/// A styled paragraph referencing an undefined style gets an empty definition
/// under its own id, so the dialog edits the id the block references.
#[test]
fn an_undefined_styled_para_id_is_seeded_empty() {
    let mut c = blank_catalog();
    let (id, seeded) = resolve(&mut c, "Quotation", None).unwrap();
    assert_eq!(id, "Quotation");
    assert!(seeded);
    assert!(c.paragraph_styles[&StyleId::new("Quotation")].is_custom);
}

/// No block under the cursor produces no target — the empty key is the "no
/// cursor" sentinel `get_block_style_name` documents.
#[test]
fn resolution_never_invents_a_target_for_no_block() {
    // `dialog_style_target` guards the empty key before calling
    // `resolve_target`; the table itself treats it as an ordinary missing id,
    // so this pins the guard's necessity rather than the table.
    let mut c = blank_catalog();
    let (id, seeded) = resolve(&mut c, "", None).unwrap();
    assert!(seeded);
    assert!(id.is_empty());
}
