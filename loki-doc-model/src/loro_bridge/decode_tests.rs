// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Unit tests for the border codec's v2 layout and v1 back-compat decode.

use loki_primitives::color::{DocumentColor, ThemeColorSlot};

use super::{decode_border, encode_border};
use crate::style::props::border::{Border, BorderStyle};
use loki_primitives::units::Points;

#[test]
fn v1_border_strings_still_decode() {
    // Pre-migration layout: Style:width:color:spacing (color = auto | #hex).
    let b = decode_border("Solid:1:#FF0000:2").expect("v1 hex decodes");
    assert_eq!(b.style, BorderStyle::Solid);
    assert_eq!(
        b.color.as_ref().and_then(|c| c.to_hex()).as_deref(),
        Some("#FF0000")
    );
    assert!((b.spacing.map_or(0.0, |s| s.value()) - 2.0).abs() < 1e-9);

    let b = decode_border("Dashed:0.5:auto:0").expect("v1 auto decodes");
    assert_eq!(b.style, BorderStyle::Dashed);
    assert!(b.color.is_none());
    assert!(b.spacing.is_none());
}

#[test]
fn v2_border_round_trips_theme_color() {
    let border = Border {
        style: BorderStyle::Double,
        width: Points::new(1.5),
        color: Some(DocumentColor::Theme {
            slot: ThemeColorSlot::Accent3,
            tint: 0.5,
        }),
        spacing: Some(Points::new(4.0)),
    };
    let s = encode_border(&border);
    let back = decode_border(&s).expect("v2 decodes");
    assert_eq!(back.style, BorderStyle::Double);
    match back.color {
        Some(DocumentColor::Theme { slot, tint }) => {
            assert_eq!(slot, ThemeColorSlot::Accent3);
            assert!((tint - 0.5).abs() < 1e-6);
        }
        other => panic!("theme color must survive, got {other:?}"),
    }
    assert!((back.spacing.map_or(0.0, |s| s.value()) - 4.0).abs() < 1e-9);
}
