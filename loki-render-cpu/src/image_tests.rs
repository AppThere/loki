// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Unit tests for the CPU image painter's data-URI decoding.

use super::decode_data_uri;

/// A 2×1 PNG: one opaque red pixel, one opaque blue.
fn two_pixel_png() -> String {
    use base64::Engine as _;
    let mut buf = std::io::Cursor::new(Vec::new());
    let img = ::image::RgbaImage::from_raw(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 255])
        .expect("2x1 buffer");
    ::image::DynamicImage::ImageRgba8(img)
        .write_to(&mut buf, ::image::ImageFormat::Png)
        .expect("encode");
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(buf.into_inner())
    )
}

#[test]
fn a_data_uri_decodes_to_its_pixels() {
    let (rgba, w, h) = decode_data_uri(&two_pixel_png()).expect("decodes");
    assert_eq!((w, h), (2, 1));
    // Not just "some bytes": the actual pixels, so a decoder that returned a
    // blank buffer of the right size would still fail.
    assert_eq!(rgba, vec![255, 0, 0, 255, 0, 0, 255, 255]);
}

/// An external URL is refused **because of its scheme**, not incidentally.
///
/// The `data:` prefix check is the "no network fetch from a deterministic,
/// offline render path" guarantee. Every other rejection case here happens to
/// fail for a second reason (no comma, or an undecodable payload), so none of
/// them can tell whether the prefix is checked at all — this one carries a
/// comma *and* a valid base64 image after it, and only the prefix rejects it.
#[test]
fn an_external_url_is_refused_even_when_it_looks_decodable() {
    let payload = two_pixel_png()
        .split_once(',')
        .expect("the fixture is a data URI")
        .1
        .to_string();
    let smuggled = format!("https://example.invalid/chart.png?x=1,{payload}");
    assert!(
        decode_data_uri(&smuggled).is_none(),
        "a non-`data:` src must be refused even when the bytes after a comma          would decode"
    );
    // Control: the very same payload *does* decode behind a `data:` prefix, so
    // the refusal above is the scheme and not the bytes.
    assert!(
        decode_data_uri(&format!("data:image/png;base64,{payload}")).is_some(),
        "the control payload must decode, or the test above proves nothing"
    );
}

#[test]
fn an_unresolvable_src_decodes_to_nothing() {
    // Each of these used to be indistinguishable from a decodable image,
    // because the painter drew a placeholder for *everything*.
    for src in [
        "https://example.invalid/chart.png", // external: no runtime fetch
        "data:image/png;base64,not-base64!", // malformed payload
        "data:image/png;base64,",            // empty payload
        "data:image/png",                    // no comma
        "chart.png",                         // not a URI at all
    ] {
        assert!(
            decode_data_uri(src).is_none(),
            "{src:?} must not decode to pixels"
        );
    }
}
