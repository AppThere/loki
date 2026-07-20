// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `w:shd` shading resolution, split from `xml_util.rs` for the 300-line
//! ceiling. `resolve_shading` flattens a texture to a solid tint;
//! `resolve_shading_pattern` preserves the hatch geometry + colours so the
//! renderer can draw the actual lines. Both re-exported from `xml_util`.

use appthere_color::RgbColor;

use super::hex_color;

/// Resolves an OOXML `w:shd` shading element to an effective solid fill colour.
///
/// `w:shd` (ECMA-376 §17.3.5) layers a pattern foreground (`@w:color`) over a
/// background (`@w:fill`) at a coverage implied by `@w:val`:
/// - `clear` / absent → the background `fill` shows through unchanged.
/// - `solid` → the foreground `color` fully covers the cell.
/// - `pctN` → `N`% of `color` blended over `fill` (e.g. `pct25` = 25 % color).
/// - `nil` / `none` → no shading at all.
///
/// Texture patterns (`horzStripe`, `diagCross`, …) are approximated by their
/// background `fill` — the dominant colour — until per-pixel pattern fills are
/// supported. `auto` fill resolves to white and `auto` color to black, matching
/// Word's automatic-colour resolution for body shading.
///
/// Returns `None` when the element contributes no visible fill (so the caller
/// leaves `background_color` unset rather than painting white over the page).
#[must_use]
pub fn resolve_shading(
    fill: Option<&str>,
    val: Option<&str>,
    color: Option<&str>,
) -> Option<RgbColor> {
    let fill_rgb = fill.and_then(hex_color);
    let color_rgb = color.and_then(hex_color);
    match val.unwrap_or("clear") {
        "nil" | "none" => None,
        "solid" => color_rgb.or(fill_rgb),
        v if v.starts_with("pct") => {
            let pct: f32 = v[3..].parse().ok()?;
            let frac = (pct / 100.0).clamp(0.0, 1.0);
            // `auto` fill → white background; `auto` color → black foreground.
            let bg = fill_rgb.unwrap_or_else(|| RgbColor::new(1.0, 1.0, 1.0));
            let fg = color_rgb.unwrap_or_else(|| RgbColor::new(0.0, 0.0, 0.0));
            Some(blend_rgb(bg, fg, frac))
        }
        // Line/cross texture patterns (`diagStripe`, `horzStripe`, `diagCross`,
        // …): `resolve_shading` flattens the pattern to a tint of the foreground
        // `@w:color` over `@w:fill` at the pattern's rough ink coverage — the
        // fallback for consumers that cannot draw the actual hatch lines.
        v if texture_coverage(v).is_some() => {
            let frac = texture_coverage(v)?;
            let bg = fill_rgb.unwrap_or_else(|| RgbColor::new(1.0, 1.0, 1.0));
            let fg = color_rgb.unwrap_or_else(|| RgbColor::new(0.0, 0.0, 0.0));
            Some(blend_rgb(bg, fg, frac))
        }
        // `clear` or unknown → background fill only.
        _ => fill_rgb,
    }
}

/// Approximate ink coverage (`0.0..1.0`) of a `w:shd` line/cross texture
/// pattern, used to flatten it to a solid tint. `None` for non-texture values
/// (`clear`, `solid`, `pctN`, unknown). Densities are eyeballed to Word:
/// `thin*` variants are lighter, cross hatches a touch denser than single
/// stripes.
fn texture_coverage(val: &str) -> Option<f32> {
    let cov = match val {
        "horzStripe" | "vertStripe" | "diagStripe" | "reverseDiagStripe" => 0.5,
        "horzCross" | "diagCross" => 0.6,
        "thinHorzStripe" | "thinVertStripe" | "thinDiagStripe" | "thinReverseDiagStripe" => 0.25,
        "thinHorzCross" | "thinDiagCross" => 0.35,
        _ => return None,
    };
    Some(cov)
}

/// Resolves a `w:shd` line/cross texture `@w:val` to a `ShadingPattern`
/// (hatch geometry + colours), or `None` for non-texture values (`clear`,
/// `solid`, `pctN`, unknown).
///
/// The companion of [`resolve_shading`]: that flattens the texture to a solid
/// tint (the fallback stored on `background_color`); this preserves the pattern
/// so the renderer can draw the actual hatch lines. `@w:color` defaults to black
/// (the hatch line colour); `@w:fill` is carried as the background fill and left
/// `None` when absent or `auto`.
#[must_use]
pub fn resolve_shading_pattern(
    fill: Option<&str>,
    val: Option<&str>,
    color: Option<&str>,
) -> Option<loki_doc_model::style::props::shading::ShadingPattern> {
    use loki_doc_model::style::props::shading::{HatchPattern, ShadingPattern};
    use loki_primitives::color::DocumentColor;

    let v = val?;
    let thin = v.starts_with("thin");
    // `strip_prefix("thin")` leaves the pattern stem capitalised
    // (`thinHorzStripe` → `HorzStripe`); match both cases.
    let base = v.strip_prefix("thin").unwrap_or(v);
    let pattern = match base {
        "horzStripe" | "HorzStripe" => HatchPattern::Horizontal,
        "vertStripe" | "VertStripe" => HatchPattern::Vertical,
        "diagStripe" | "DiagStripe" => HatchPattern::DiagUp,
        "reverseDiagStripe" | "ReverseDiagStripe" => HatchPattern::DiagDown,
        "horzCross" | "HorzCross" => HatchPattern::Cross,
        "diagCross" | "DiagCross" => HatchPattern::DiagCross,
        _ => return None,
    };
    let fg = color
        .and_then(hex_color)
        .map_or(DocumentColor::Rgb(RgbColor::new(0.0, 0.0, 0.0)), |c| {
            DocumentColor::Rgb(c)
        });
    let bg = fill
        .filter(|f| *f != "auto")
        .and_then(hex_color)
        .map(DocumentColor::Rgb);
    Some(ShadingPattern {
        pattern,
        thin,
        color: fg,
        fill: bg,
    })
}

/// Linearly blends `fg` over `bg` at coverage `t` in `[0, 1]`.
#[must_use]
fn blend_rgb(bg: RgbColor, fg: RgbColor, t: f32) -> RgbColor {
    let mix = |a: f32, b: f32| a * (1.0 - t) + b * t;
    RgbColor::new(
        mix(bg.red(), fg.red()),
        mix(bg.green(), fg.green()),
        mix(bg.blue(), fg.blue()),
    )
}
