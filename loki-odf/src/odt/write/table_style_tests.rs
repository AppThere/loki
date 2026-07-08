// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Unit tests for the named ODT table-style writer.

use super::*;
use loki_doc_model::content::attr::ExtensionBag;
use loki_doc_model::style::catalog::StyleId;
use loki_primitives::color::{DocumentColor, RgbColor};
use loki_primitives::units::Points;

fn style_with(props: TableProps) -> StyleCatalog {
    let mut catalog = StyleCatalog::new();
    catalog.table_styles.insert(
        StyleId::new("Banded"),
        TableStyle {
            id: StyleId::new("Banded"),
            display_name: Some("Banded".into()),
            parent: None,
            table_props: props,
            conditional: Default::default(),
            extensions: ExtensionBag::default(),
        },
    );
    catalog
}

fn render(catalog: &StyleCatalog) -> String {
    let mut out = String::new();
    write_table_styles(&mut out, catalog);
    out
}

#[test]
fn emits_width_alignment_and_background() {
    let catalog = style_with(TableProps {
        width: Some(TableWidth::Absolute(Points::new(340.0))),
        alignment: Some(TableAlignment::Center),
        background_color: Some(DocumentColor::Rgb(RgbColor::new(1.0, 1.0, 1.0))),
        ..TableProps::default()
    });
    let xml = render(&catalog);
    assert!(xml.contains(r#"<style:style style:name="Banded""#));
    assert!(xml.contains(r#"style:family="table""#));
    assert!(xml.contains(r#"style:width="340pt""#));
    assert!(xml.contains(r#"table:align="center""#));
    assert!(xml.contains(r##"fo:background-color="#FFFFFF""##));
}

#[test]
fn percent_width_uses_rel_width() {
    let catalog = style_with(TableProps {
        width: Some(TableWidth::Percent(80.0)),
        ..TableProps::default()
    });
    let xml = render(&catalog);
    assert!(xml.contains(r#"style:rel-width="80%""#));
    assert!(!xml.contains("style:width="));
}

#[test]
fn a_style_with_no_geometry_omits_table_properties() {
    let catalog = style_with(TableProps::default());
    let xml = render(&catalog);
    assert!(xml.contains(r#"style:family="table""#));
    assert!(!xml.contains("style:table-properties"));
}

#[test]
fn synthetic_styles_are_skipped() {
    let mut catalog = StyleCatalog::new();
    catalog.table_styles.insert(
        StyleId::new("__DefaultTable"),
        TableStyle {
            id: StyleId::new("__DefaultTable"),
            display_name: None,
            parent: None,
            table_props: TableProps::default(),
            conditional: Default::default(),
            extensions: ExtensionBag::default(),
        },
    );
    assert!(render(&catalog).is_empty());
}
