// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Shared XML parsing utilities for all OOXML part readers.
//!
//! # Why `trim_text(false)` must always be set
//!
//! `quick_xml` can strip leading/trailing whitespace from text nodes when
//! `trim_text(true)` is set. OOXML documents frequently use `xml:space="preserve"`
//! and rely on significant whitespace within runs (`<w:t xml:space="preserve"> </w:t>`).
//! Trimming would silently discard inter-word spaces, producing garbled text.
//! Every reader in this crate must call `reader.config_mut().trim_text(false)`
//! immediately after constructing the reader.

use appthere_color::RgbColor;
use quick_xml::events::{BytesRef, BytesStart, BytesText, Event};

/// Decodes and XML-entity-unescapes a text node's content.
///
/// `BytesText::unescape()` was removed in quick-xml 0.41 (COMPAT: split into
/// separate `decode()` + `escape::unescape()` steps); this restores the old
/// combined behaviour for the many call sites that just want plain text.
pub fn unescape_text(text: &BytesText<'_>) -> quick_xml::Result<String> {
    let decoded = text.decode()?;
    Ok(quick_xml::escape::unescape(&decoded)?.into_owned())
}

/// Resolves a `&entity;` / `&#N;` general reference to its literal string.
///
/// COMPAT(quick-xml-0.41): quick-xml 0.41 stopped folding entity/character
/// references into the surrounding `Event::Text`; they now arrive as their
/// own `Event::GeneralRef` between Text events (verified empirically: a run
/// containing `&quot;` is split into three events — Text, `GeneralRef`, Text —
/// and the `&quot;` is dropped entirely if `GeneralRef` isn't handled). Every
/// text-accumulation loop in this crate must match `Event::GeneralRef` next
/// to `Event::Text` and push this function's result, or embedded entities
/// (quotes in field-code instructions being the case that surfaced this)
/// silently vanish from imported text.
///
/// Named references are resolved via the five predefined XML entities
/// (`lt`, `gt`, `amp`, `apos`, `quot`); anything else (a custom DTD entity,
/// which OOXML/ODF documents do not define) falls back to the literal
/// `&name;` so no data is silently lost.
pub fn resolve_general_ref(r: &BytesRef<'_>) -> quick_xml::Result<String> {
    if let Some(ch) = r.resolve_char_ref()? {
        return Ok(ch.to_string());
    }
    let name = r.decode()?;
    Ok(quick_xml::escape::resolve_predefined_entity(&name)
        .map_or_else(|| format!("&{name};"), ToString::to_string))
}

/// Extracts text from an `Event::Text` or `Event::GeneralRef`; `None` for any
/// other event kind. One-call replacement for the `unescape_text`/
/// `resolve_general_ref` pair, for call sites matching both in a single arm.
pub fn event_text(event: &Event<'_>) -> quick_xml::Result<String> {
    match event {
        Event::Text(t) => unescape_text(t),
        Event::GeneralRef(r) => resolve_general_ref(r),
        _ => Ok(String::new()),
    }
}

/// Extracts the local name (without namespace prefix) from an element.
///
/// OOXML uses many namespace prefixes (`w:`, `wp:`, `a:`, `r:`, etc.).
/// This crate matches on local names only (ECMA-376 §L.5).
///
/// # Examples
///
/// ```ignore
/// // b"w:p" → b"p"
/// // b"a:blip" → b"blip"
/// // b"numFmt" → b"numFmt"
/// ```
#[must_use]
#[allow(dead_code)]
pub fn local_name<'a>(e: &'a BytesStart<'a>) -> &'a [u8] {
    let bytes = e.local_name().into_inner();
    if let Some(pos) = bytes.iter().position(|&b| b == b':') {
        &bytes[pos + 1..]
    } else {
        bytes
    }
}

/// Extracts the value of an attribute by its local name (without prefix).
///
/// Returns `None` if the attribute is absent or its value is not valid UTF-8.
///
/// # Examples
///
/// ```ignore
/// // <w:numFmt w:val="decimal"/> → local_attr_val(e, b"val") = Some("decimal")
/// ```
#[must_use]
#[allow(dead_code)]
pub fn local_attr_val(e: &BytesStart<'_>, local: &[u8]) -> Option<String> {
    e.attributes().flatten().find_map(|attr| {
        let key_bytes = attr.key.as_ref();
        let key_local = if let Some(pos) = key_bytes.iter().position(|&b| b == b':') {
            &key_bytes[pos + 1..]
        } else {
            key_bytes
        };
        if key_local == local {
            attr.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .ok()
                .map(std::borrow::Cow::into_owned)
        } else {
            None
        }
    })
}

/// Single-pass variant of [`local_attr_val`] for hot loops that need several
/// attributes of the same element: returns the values of the `N` requested
/// local attribute names after one scan of the attribute list, instead of one
/// full scan per attribute. First occurrence wins, matching
/// [`local_attr_val`]. Used by the XLSX sheet reader's per-cell `<c>` handling.
#[must_use]
// Sole caller today is behind the (non-default) `xlsx` feature.
#[cfg_attr(not(feature = "xlsx"), allow(dead_code))]
pub fn local_attr_vals<const N: usize>(
    e: &BytesStart<'_>,
    locals: [&[u8]; N],
) -> [Option<String>; N] {
    let mut out = [const { None }; N];
    for attr in e.attributes().flatten() {
        let key_bytes = attr.key.as_ref();
        let key_local = key_bytes
            .iter()
            .position(|&b| b == b':')
            .map_or(key_bytes, |pos| &key_bytes[pos + 1..]);
        if let Some(i) = locals.iter().position(|l| *l == key_local)
            && out[i].is_none()
        {
            out[i] = attr
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .ok()
                .map(std::borrow::Cow::into_owned);
        }
    }
    out
}

/// Parses a 6-character hexadecimal OOXML color string to an [`RgbColor`].
///
/// Returns `None` for the special value `"auto"`, the empty string, or any
/// string that is not exactly 6 valid hex digits. No `#` prefix is expected
/// (OOXML stores hex colors without it, e.g. `w:color w:val="FF0000"`).
#[must_use]
pub fn hex_color(s: &str) -> Option<RgbColor> {
    if s == "auto" || s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some(RgbColor::new(
        f32::from(r) / 255.0,
        f32::from(g) / 255.0,
        f32::from(b) / 255.0,
    ))
}

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
        // `clear`, texture patterns, or unknown → background fill only.
        _ => fill_rgb,
    }
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

#[cfg(test)]
#[path = "xml_util_tests.rs"]
mod tests;
