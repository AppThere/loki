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

/// Converts RGB bytes to HSV (hue 0–360, saturation/value 0–100).
///
/// # The inverse the SV square needs, and the reason it did not exist
///
/// The picker had only the *forward* conversions: the fields produce a colour,
/// and a colour is only ever written outward as hex. A two-dimensional
/// saturation/value square needs the other direction — given the colour the
/// document already has, *where does the handle go* — and without it the square
/// could only ever start from a corner.
///
/// # Hue is undefined for greys, and this returns 0 rather than pretending
///
/// A grey has no hue: every hue produces it at zero saturation. Returning 0 is a
/// choice, not a measurement, and it matters at the call site — a picker that
/// recomputed the hue strip's position from a grey would slam it to red the
/// moment the reader dragged saturation to zero. Callers keep the hue they had;
/// see `hue_preserving_hsv`.
pub fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let (rf, gf, bf) = (
        f32::from(r) / 255.0,
        f32::from(g) / 255.0,
        f32::from(b) / 255.0,
    );
    let max = rf.max(gf).max(bf);
    let min = rf.min(gf).min(bf);
    let delta = max - min;

    let hue = if delta <= f32::EPSILON {
        0.0
    } else if (max - rf).abs() < f32::EPSILON {
        60.0 * (((gf - bf) / delta) % 6.0)
    } else if (max - gf).abs() < f32::EPSILON {
        60.0 * ((bf - rf) / delta + 2.0)
    } else {
        60.0 * ((rf - gf) / delta + 4.0)
    };

    let saturation = if max <= f32::EPSILON {
        0.0
    } else {
        delta / max
    };
    (hue.rem_euclid(360.0), saturation * 100.0, max * 100.0)
}

/// [`rgb_to_hsv`], but keeping `previous_hue` when the colour has no hue of its
/// own.
///
/// A picker whose handle is at zero saturation or zero value is showing a colour
/// that names no hue. Recomputing from it would move the hue strip to red under
/// the reader's finger, and the movement would look like the control fighting
/// them. So the strip keeps the hue the reader last chose, which is the only
/// value that carries their intent.
pub fn hue_preserving_hsv(r: u8, g: u8, b: u8, previous_hue: f32) -> (f32, f32, f32) {
    let (h, s, v) = rgb_to_hsv(r, g, b);
    let hue = if s <= f32::EPSILON || v <= f32::EPSILON {
        previous_hue
    } else {
        h
    };
    (hue, s, v)
}

/// Converts RGB bytes to HSL (hue 0–360, saturation/lightness 0–100).
///
/// The HSL counterpart of [`rgb_to_hsv`], added for the same reason: the fields
/// have to be able to *show* a colour the square produced, not only accept one.
pub fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let (rf, gf, bf) = (
        f32::from(r) / 255.0,
        f32::from(g) / 255.0,
        f32::from(b) / 255.0,
    );
    let max = rf.max(gf).max(bf);
    let min = rf.min(gf).min(bf);
    let delta = max - min;
    let l = (max + min) / 2.0;

    if delta <= f32::EPSILON {
        return (0.0, 0.0, l * 100.0);
    }
    let s = delta / (1.0 - (2.0 * l - 1.0).abs());
    let (h, _, _) = rgb_to_hsv(r, g, b);
    (h, s.clamp(0.0, 1.0) * 100.0, l * 100.0)
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

    /// **The round trip is the property the SV square actually relies on.** The
    /// handle's position comes from `rgb_to_hsv`, and the colour it produces
    /// comes back through `hsv_to_rgb`; if the pair are not inverses the handle
    /// drifts a little every time the reader touches it.
    #[test]
    fn hsv_round_trips_through_rgb() {
        for (r, g, b) in [
            (0xC0, 0x39, 0x2B),
            (0x2E, 0xCC, 0x71),
            (0x34, 0x98, 0xDB),
            (0x00, 0x00, 0x00),
            (0xFF, 0xFF, 0xFF),
            (0x7F, 0x7F, 0x7F),
            (0xFF, 0x00, 0x00),
        ] {
            let (h, s, v) = rgb_to_hsv(r, g, b);
            let back = hsv_to_rgb(h, s, v);
            assert_eq!(back, (r, g, b), "round trip for #{r:02X}{g:02X}{b:02X}");
        }
    }

    /// The primaries land on the hues everyone expects — the anchor that says
    /// the sector arithmetic is right rather than merely self-consistent.
    #[test]
    fn the_primaries_have_their_textbook_hues() {
        assert!((rgb_to_hsv(0xFF, 0x00, 0x00).0 - 0.0).abs() < 0.5, "red");
        assert!(
            (rgb_to_hsv(0x00, 0xFF, 0x00).0 - 120.0).abs() < 0.5,
            "green"
        );
        assert!((rgb_to_hsv(0x00, 0x00, 0xFF).0 - 240.0).abs() < 0.5, "blue");
        assert!(
            (rgb_to_hsv(0xFF, 0xFF, 0x00).0 - 60.0).abs() < 0.5,
            "yellow"
        );
    }

    /// Hue is always reported inside one turn. A negative sector result is the
    /// classic slip here — reds just below 0 come out as -30 — and a handle
    /// positioned from it lands off the left end of the strip.
    #[test]
    fn hue_is_always_within_one_turn() {
        for (r, g, b) in [(0xFF, 0x00, 0x40), (0xFF, 0x00, 0x01), (0xFE, 0x00, 0x80)] {
            let h = rgb_to_hsv(r, g, b).0;
            assert!((0.0..360.0).contains(&h), "hue {h} out of range");
        }
    }

    /// Greys report zero saturation, whatever their brightness.
    #[test]
    fn greys_have_no_saturation() {
        for level in [0x00_u8, 0x40, 0x80, 0xC0, 0xFF] {
            let (_, s, v) = rgb_to_hsv(level, level, level);
            assert!(s.abs() < 0.01, "grey {level:#04X} reported saturation {s}");
            assert!(
                (v - f32::from(level) / 2.55).abs() < 0.5,
                "grey {level:#04X} value {v}",
            );
        }
    }

    /// **The hue a grey cannot supply is the reader's previous one.** Without
    /// this the hue strip jumps to red the moment saturation reaches zero — the
    /// control appearing to fight the finger dragging it.
    #[test]
    fn a_colourless_colour_keeps_the_hue_the_reader_chose() {
        let (h, s, _) = hue_preserving_hsv(0x80, 0x80, 0x80, 210.0);
        assert!((h - 210.0).abs() < 0.01, "grey kept hue, got {h}");
        assert!(s.abs() < 0.01);

        // Black is the other colourless case: zero *value* rather than zero
        // saturation, and it is a separate branch.
        assert!((hue_preserving_hsv(0, 0, 0, 210.0).0 - 210.0).abs() < 0.01);

        // The polarity: a colour that *does* name a hue overrides the previous
        // one, or the strip would never move at all.
        let (h, _, _) = hue_preserving_hsv(0x00, 0xFF, 0x00, 210.0);
        assert!((h - 120.0).abs() < 0.5, "a real hue must win, got {h}");
    }
}
