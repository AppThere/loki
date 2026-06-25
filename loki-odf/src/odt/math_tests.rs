// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the embedded-formula `MathML` (de)serialiser.

use super::{extract_mathml, object_content_xml};

const NS: &str = "http://www.w3.org/1998/Math/MathML";

#[test]
fn round_trips_canonical_mathml() {
    let mathml = format!("<math xmlns=\"{NS}\"><mfrac><mn>1</mn><mn>2</mn></mfrac></math>");
    let xml = object_content_xml(&mathml);
    let back = extract_mathml(xml.as_bytes()).expect("math present");
    assert_eq!(back, mathml);
}

#[test]
fn canonicalises_prefixed_and_whitespaced_input() {
    // A foreign formula with a namespace prefix and pretty-print whitespace
    // normalises to the same canonical string.
    let foreign = "<?xml version=\"1.0\"?>\n\
        <math:math xmlns:math=\"http://www.w3.org/1998/Math/MathML\">\n  \
        <math:msqrt><math:mi>x</math:mi></math:msqrt>\n\
        </math:math>";
    let back = extract_mathml(foreign.as_bytes()).expect("math present");
    assert_eq!(
        back,
        format!("<math xmlns=\"{NS}\"><msqrt><mi>x</mi></msqrt></math>")
    );
}

#[test]
fn no_math_returns_none() {
    assert!(extract_mathml(b"<office:document/>").is_none());
}
