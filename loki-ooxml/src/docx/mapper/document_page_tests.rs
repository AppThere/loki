// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for `document_page` (extracted for the 300-line file ceiling).

use super::*;
use crate::docx::model::paragraph::DocxBorderEdge;
use crate::docx::model::section::DocxPgBorders;

fn edge() -> DocxBorderEdge {
    DocxBorderEdge {
        val: "single".into(),
        sz: Some(8),
        color: Some("4472C4".into()),
        space: Some(24),
    }
}

#[test]
fn maps_page_borders_onto_the_layout() {
    let sect = DocxSectPr {
        pg_borders: Some(DocxPgBorders {
            top: Some(edge()),
            bottom: Some(edge()),
            left: Some(edge()),
            right: Some(edge()),
            offset_from_text: false,
        }),
        ..Default::default()
    };
    let layout = map_page_layout(Some(&sect));
    let pb = layout.page_border.expect("page border mapped");
    assert!(pb.top.is_some() && pb.left.is_some() && pb.bottom.is_some() && pb.right.is_some());
    assert!(!pb.offset_from_text);
}

#[test]
fn all_none_edges_map_to_no_border() {
    let sect = DocxSectPr {
        pg_borders: Some(DocxPgBorders::default()),
        ..Default::default()
    };
    assert!(map_page_layout(Some(&sect)).page_border.is_none());
}

#[test]
fn maps_line_numbering_with_defaults_and_distance() {
    use crate::docx::model::section::DocxLnNumType;
    let sect = DocxSectPr {
        ln_num_type: Some(DocxLnNumType {
            count_by: None, // → 1
            start: Some(1),
            restart: Some("newPage".into()),
            distance: Some(360), // twips → 18 pt
        }),
        ..Default::default()
    };
    let ln = map_page_layout(Some(&sect))
        .line_numbering
        .expect("line numbering mapped");
    assert_eq!(ln.count_by, 1);
    assert_eq!(ln.start, 1);
    assert_eq!(ln.restart, LineNumberRestart::NewPage);
    assert!((ln.distance.unwrap().value() - 18.0).abs() < 0.01);
}

#[test]
fn maps_line_numbering_restart_and_count_by() {
    use crate::docx::model::section::DocxLnNumType;
    let sect = DocxSectPr {
        ln_num_type: Some(DocxLnNumType {
            count_by: Some(5),
            start: Some(10),
            restart: Some("continuous".into()),
            distance: None,
        }),
        ..Default::default()
    };
    let ln = map_page_layout(Some(&sect)).line_numbering.unwrap();
    assert_eq!(ln.count_by, 5);
    assert_eq!(ln.start, 10);
    assert_eq!(ln.restart, LineNumberRestart::Continuous);
    assert!(ln.distance.is_none());
}

#[test]
fn no_line_numbering_maps_to_none() {
    assert!(
        map_page_layout(Some(&DocxSectPr::default()))
            .line_numbering
            .is_none()
    );
}
