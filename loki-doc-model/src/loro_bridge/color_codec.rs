// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Codec for [`DocumentColor`] values in the Loro CRDT schema.
//!
//! Unlike [`DocumentColor::to_hex`], this codec is total: every variant —
//! including `Cmyk`, `Theme`, and `Transparent` — has a stable string
//! encoding, so a color survives a document_to_loro → loro_to_document
//! round-trip without collapsing to Rgb or being dropped.
//!
//! Encodings:
//! - `Rgb`         → `#RRGGBB`
//! - `Cmyk`        → `cmyk:C:M:Y:K` (components in `[0, 1]`)
//! - `Theme`       → `theme:Slot:tint`
//! - `Transparent` → `transparent`

use loki_primitives::color::{CmykColor, DocumentColor, ThemeColorSlot};

/// Encode any [`DocumentColor`] as a compact schema string.
pub(super) fn encode_document_color(c: &DocumentColor) -> String {
    match c {
        DocumentColor::Rgb(_) => c.to_hex().unwrap_or_else(|| String::from("transparent")),
        DocumentColor::Cmyk(cmyk) => format!(
            "cmyk:{}:{}:{}:{}",
            cmyk.cyan(),
            cmyk.magenta(),
            cmyk.yellow(),
            cmyk.key()
        ),
        DocumentColor::Theme { slot, tint } => {
            format!("theme:{}:{}", encode_theme_slot(*slot), tint)
        }
        DocumentColor::Transparent => String::from("transparent"),
        // `DocumentColor` is #[non_exhaustive]; a variant this codec does not
        // know yet cannot be encoded faithfully. Encode as transparent rather
        // than panicking — extending the codec is the real fix when the enum
        // grows.
        _ => String::from("transparent"),
    }
}

/// Decode a string produced by [`encode_document_color`].
pub(super) fn decode_document_color(s: &str) -> Option<DocumentColor> {
    if s == "transparent" {
        return Some(DocumentColor::Transparent);
    }
    if let Some(rest) = s.strip_prefix("cmyk:") {
        let mut parts = rest.splitn(4, ':');
        let c: f32 = parts.next()?.parse().ok()?;
        let m: f32 = parts.next()?.parse().ok()?;
        let y: f32 = parts.next()?.parse().ok()?;
        let k: f32 = parts.next()?.parse().ok()?;
        return Some(DocumentColor::Cmyk(CmykColor::new(c, m, y, k)));
    }
    if let Some(rest) = s.strip_prefix("theme:") {
        let mut parts = rest.splitn(2, ':');
        let slot = decode_theme_slot(parts.next()?)?;
        let tint: f32 = parts.next()?.parse().ok()?;
        return Some(DocumentColor::Theme { slot, tint });
    }
    DocumentColor::from_hex(s).ok()
}

fn encode_theme_slot(slot: ThemeColorSlot) -> &'static str {
    match slot {
        ThemeColorSlot::Dark1 => "Dark1",
        ThemeColorSlot::Dark2 => "Dark2",
        ThemeColorSlot::Light1 => "Light1",
        ThemeColorSlot::Light2 => "Light2",
        ThemeColorSlot::Accent1 => "Accent1",
        ThemeColorSlot::Accent2 => "Accent2",
        ThemeColorSlot::Accent3 => "Accent3",
        ThemeColorSlot::Accent4 => "Accent4",
        ThemeColorSlot::Accent5 => "Accent5",
        ThemeColorSlot::Accent6 => "Accent6",
        ThemeColorSlot::Hyperlink => "Hyperlink",
        ThemeColorSlot::FollowedHyperlink => "FollowedHyperlink",
        // #[non_exhaustive] guard for slots added upstream — map to Dark1
        // (text default) until the codec learns the new slot.
        _ => "Dark1",
    }
}

fn decode_theme_slot(s: &str) -> Option<ThemeColorSlot> {
    match s {
        "Dark1" => Some(ThemeColorSlot::Dark1),
        "Dark2" => Some(ThemeColorSlot::Dark2),
        "Light1" => Some(ThemeColorSlot::Light1),
        "Light2" => Some(ThemeColorSlot::Light2),
        "Accent1" => Some(ThemeColorSlot::Accent1),
        "Accent2" => Some(ThemeColorSlot::Accent2),
        "Accent3" => Some(ThemeColorSlot::Accent3),
        "Accent4" => Some(ThemeColorSlot::Accent4),
        "Accent5" => Some(ThemeColorSlot::Accent5),
        "Accent6" => Some(ThemeColorSlot::Accent6),
        "Hyperlink" => Some(ThemeColorSlot::Hyperlink),
        "FollowedHyperlink" => Some(ThemeColorSlot::FollowedHyperlink),
        _ => None,
    }
}
