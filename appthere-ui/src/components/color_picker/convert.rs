// SPDX-License-Identifier: Apache-2.0

//! Pure colour-space conversions for the custom-colour entry of
//! [`AtColorPicker`](super::AtColorPicker).
//!
//! All functions convert *to* sRGB bytes, because the picker's output is a
//! `#RRGGBB` hex string. The CMYK conversion is the naive complement formula —
//! adequate for an on-screen preview and for authoring an RGB document colour;
//! print-accurate CMYK (ICC transforms) is the PDF exporter's concern, not the
//! picker's.

/// Parses `#RRGGBB` (leading `#` optional, case-insensitive) into RGB bytes.
pub fn parse_hex(s: &str) -> Option<(u8, u8, u8)> {
    let hex = s.trim().trim_start_matches('#');
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some((r, g, b))
}

/// Formats RGB bytes as an uppercase `#RRGGBB` string.
pub fn rgb_to_hex(r: u8, g: u8, b: u8) -> String {
    format!("#{r:02X}{g:02X}{b:02X}")
}

fn channel(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Converts HSL (hue 0–360, saturation/lightness 0–100) to RGB bytes.
pub fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let h = h.rem_euclid(360.0);
    let s = (s / 100.0).clamp(0.0, 1.0);
    let l = (l / 100.0).clamp(0.0, 1.0);
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let (r1, g1, b1) = hue_sector(h, c);
    let m = l - c / 2.0;
    (channel(r1 + m), channel(g1 + m), channel(b1 + m))
}

/// Converts HSV (hue 0–360, saturation/value 0–100) to RGB bytes.
pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let h = h.rem_euclid(360.0);
    let s = (s / 100.0).clamp(0.0, 1.0);
    let v = (v / 100.0).clamp(0.0, 1.0);
    let c = v * s;
    let (r1, g1, b1) = hue_sector(h, c);
    let m = v - c;
    (channel(r1 + m), channel(g1 + m), channel(b1 + m))
}

/// The shared hue-sector step of the HSL/HSV formulas: chroma distributed over
/// the two dominant channels for `h`'s 60° sector.
fn hue_sector(h: f32, c: f32) -> (f32, f32, f32) {
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    }
}

/// Converts CMYK (each 0–100) to RGB bytes via the naive complement formula.
pub fn cmyk_to_rgb(c: f32, m: f32, y: f32, k: f32) -> (u8, u8, u8) {
    let c = (c / 100.0).clamp(0.0, 1.0);
    let m = (m / 100.0).clamp(0.0, 1.0);
    let y = (y / 100.0).clamp(0.0, 1.0);
    let k = (k / 100.0).clamp(0.0, 1.0);
    (
        channel((1.0 - c) * (1.0 - k)),
        channel((1.0 - m) * (1.0 - k)),
        channel((1.0 - y) * (1.0 - k)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trip_and_forgiving_parse() {
        assert_eq!(parse_hex("#C0392B"), Some((0xC0, 0x39, 0x2B)));
        assert_eq!(parse_hex("c0392b"), Some((0xC0, 0x39, 0x2B)));
        assert_eq!(parse_hex(" #c0392b "), Some((0xC0, 0x39, 0x2B)));
        assert_eq!(parse_hex("#c0392"), None);
        assert_eq!(parse_hex("#c0392g"), None);
        assert_eq!(rgb_to_hex(0xC0, 0x39, 0x2B), "#C0392B");
    }

    #[test]
    fn hsl_primaries_and_greys() {
        assert_eq!(hsl_to_rgb(0.0, 100.0, 50.0), (255, 0, 0));
        assert_eq!(hsl_to_rgb(120.0, 100.0, 50.0), (0, 255, 0));
        assert_eq!(hsl_to_rgb(240.0, 100.0, 50.0), (0, 0, 255));
        assert_eq!(hsl_to_rgb(0.0, 0.0, 100.0), (255, 255, 255));
        assert_eq!(hsl_to_rgb(0.0, 0.0, 0.0), (0, 0, 0));
        // 360 wraps to 0; out-of-range saturation clamps.
        assert_eq!(hsl_to_rgb(360.0, 150.0, 50.0), (255, 0, 0));
    }

    #[test]
    fn hsv_primaries_and_value_scale() {
        assert_eq!(hsv_to_rgb(0.0, 100.0, 100.0), (255, 0, 0));
        assert_eq!(hsv_to_rgb(120.0, 100.0, 100.0), (0, 255, 0));
        assert_eq!(hsv_to_rgb(240.0, 100.0, 100.0), (0, 0, 255));
        assert_eq!(hsv_to_rgb(0.0, 0.0, 100.0), (255, 255, 255));
        assert_eq!(hsv_to_rgb(60.0, 100.0, 50.0), (128, 128, 0));
    }

    #[test]
    fn cmyk_complements() {
        assert_eq!(cmyk_to_rgb(0.0, 0.0, 0.0, 0.0), (255, 255, 255));
        assert_eq!(cmyk_to_rgb(0.0, 0.0, 0.0, 100.0), (0, 0, 0));
        assert_eq!(cmyk_to_rgb(100.0, 0.0, 0.0, 0.0), (0, 255, 255));
        assert_eq!(cmyk_to_rgb(0.0, 100.0, 100.0, 0.0), (255, 0, 0));
    }
}
