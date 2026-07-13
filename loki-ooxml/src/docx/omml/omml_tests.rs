// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Round-trip tests for the `OMML` ⇄ `MathML` converter.

use quick_xml::Reader;
use quick_xml::Writer;
use quick_xml::events::Event;

use super::{read_math, write_omath};

/// Drives the OMML reader over `xml`, returning `(mathml, is_display)`.
fn to_mathml(xml: &str) -> (String, bool) {
    let mut reader = Reader::from_reader(xml.as_bytes());
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let is_math = {
                    let ln = e.local_name();
                    let n = ln.as_ref();
                    n == b"oMath" || n == b"oMathPara"
                };
                if is_math {
                    let owned = e.to_owned();
                    return read_math(&mut reader, &owned).expect("read_math");
                }
                buf.clear();
            }
            Ok(Event::Eof) => panic!("no oMath element in fixture"),
            _ => buf.clear(),
        }
    }
}

/// Emits `OMML` for `mathml` and returns the serialized string.
fn to_omml(mathml: &str, display: bool) -> String {
    let mut out = Vec::new();
    {
        let mut w = Writer::new(&mut out);
        write_omath(&mut w, mathml, display);
    }
    String::from_utf8(out).expect("utf8")
}

/// Asserts that `mathml` survives a `MathML` → OMML → `MathML` cycle unchanged.
fn assert_stable(mathml: &str, display: bool) {
    let omml = to_omml(mathml, display);
    let (back, back_display) = to_mathml(&omml);
    assert_eq!(back, mathml, "round-trip changed MathML (omml: {omml})");
    assert_eq!(back_display, display);
}

const NS: &str = "http://www.w3.org/1998/Math/MathML";

#[test]
fn fraction_one_half() {
    let omml = "<m:oMath><m:f><m:num><m:r><m:t>1</m:t></m:r></m:num>\
                <m:den><m:r><m:t>2</m:t></m:r></m:den></m:f></m:oMath>";
    let (mathml, display) = to_mathml(omml);
    assert!(!display);
    assert_eq!(
        mathml,
        format!("<math xmlns=\"{NS}\"><mfrac><mn>1</mn><mn>2</mn></mfrac></math>")
    );
    assert_stable(&mathml, display);
}

#[test]
fn superscript_x_squared() {
    let omml = "<m:oMath><m:sSup><m:e><m:r><m:t>x</m:t></m:r></m:e>\
                <m:sup><m:r><m:t>2</m:t></m:r></m:sup></m:sSup></m:oMath>";
    let (mathml, _) = to_mathml(omml);
    assert_eq!(
        mathml,
        format!("<math xmlns=\"{NS}\"><msup><mi>x</mi><mn>2</mn></msup></math>")
    );
    assert_stable(&mathml, false);
}

#[test]
fn subscript_a_i() {
    let omml = "<m:oMath><m:sSub><m:e><m:r><m:t>a</m:t></m:r></m:e>\
                <m:sub><m:r><m:t>i</m:t></m:r></m:sub></m:sSub></m:oMath>";
    let (mathml, _) = to_mathml(omml);
    assert_eq!(
        mathml,
        format!("<math xmlns=\"{NS}\"><msub><mi>a</mi><mi>i</mi></msub></math>")
    );
    assert_stable(&mathml, false);
}

#[test]
fn square_root() {
    let omml = "<m:oMath><m:rad><m:radPr><m:degHide m:val=\"1\"/></m:radPr>\
                <m:deg/><m:e><m:r><m:t>x</m:t></m:r></m:e></m:rad></m:oMath>";
    let (mathml, _) = to_mathml(omml);
    assert_eq!(
        mathml,
        format!("<math xmlns=\"{NS}\"><msqrt><mi>x</mi></msqrt></math>")
    );
    assert_stable(&mathml, false);
}

#[test]
fn nth_root() {
    let omml = "<m:oMath><m:rad><m:deg><m:r><m:t>3</m:t></m:r></m:deg>\
                <m:e><m:r><m:t>x</m:t></m:r></m:e></m:rad></m:oMath>";
    let (mathml, _) = to_mathml(omml);
    assert_eq!(
        mathml,
        format!("<math xmlns=\"{NS}\"><mroot><mi>x</mi><mn>3</mn></mroot></math>")
    );
    assert_stable(&mathml, false);
}

#[test]
fn display_math_wrapper() {
    let omml = "<m:oMathPara><m:oMath><m:r><m:t>E</m:t></m:r></m:oMath></m:oMathPara>";
    let (mathml, display) = to_mathml(omml);
    assert!(display);
    assert_eq!(mathml, format!("<math xmlns=\"{NS}\"><mi>E</mi></math>"));
    assert_stable(&mathml, true);
}

#[test]
fn compound_sum_expression() {
    // a + b/c with an operator run; exercises multi-element mrow wrapping.
    let omml = "<m:oMath><m:r><m:t>a</m:t></m:r><m:r><m:t>+</m:t></m:r>\
                <m:f><m:num><m:r><m:t>b</m:t></m:r></m:num>\
                <m:den><m:r><m:t>c</m:t></m:r></m:den></m:f></m:oMath>";
    let (mathml, _) = to_mathml(omml);
    assert_eq!(
        mathml,
        format!(
            "<math xmlns=\"{NS}\"><mi>a</mi><mo>+</mo>\
             <mfrac><mi>b</mi><mi>c</mi></mfrac></math>"
        )
    );
    assert_stable(&mathml, false);
}

// ── Structured constructs (5.8) ──────────────────────────────────────────────

/// `m:d` with no `m:dPr` resolves the OMML default parentheses.
#[test]
fn delimiters_default_parentheses() {
    let omml = "<m:oMath><m:d><m:e><m:r><m:t>x</m:t></m:r></m:e></m:d></m:oMath>";
    let (mathml, display) = to_mathml(omml);
    assert!(!display);
    assert_eq!(
        mathml,
        format!(
            "<math xmlns=\"{NS}\"><mfenced open=\"(\" close=\")\" \
             separators=\"|\"><mi>x</mi></mfenced></math>"
        )
    );
    assert_stable(&mathml, false);
}

/// Explicit bracket characters and two separated arguments round-trip.
#[test]
fn delimiters_custom_brackets_and_separator() {
    let omml = "<m:oMath><m:d><m:dPr><m:begChr m:val=\"[\"/><m:sepChr m:val=\";\"/>\
                <m:endChr m:val=\"]\"/></m:dPr>\
                <m:e><m:r><m:t>a</m:t></m:r></m:e>\
                <m:e><m:r><m:t>b</m:t></m:r></m:e></m:d></m:oMath>";
    let (mathml, _) = to_mathml(omml);
    assert_eq!(
        mathml,
        format!(
            "<math xmlns=\"{NS}\"><mfenced open=\"[\" close=\"]\" \
             separators=\";\"><mi>a</mi><mi>b</mi></mfenced></math>"
        )
    );
    assert_stable(&mathml, false);
}

/// An n-ary sum with under/over limits and an operand: the operator becomes a
/// `<munderover>` with an `<mo>` base, the operand a following sibling.
#[test]
fn nary_sum_with_limits_and_operand() {
    let omml = "<m:oMath><m:nary><m:naryPr><m:chr m:val=\"\u{2211}\"/>\
                <m:limLoc m:val=\"undOvr\"/></m:naryPr>\
                <m:sub><m:r><m:t>i</m:t></m:r></m:sub>\
                <m:sup><m:r><m:t>n</m:t></m:r></m:sup>\
                <m:e><m:r><m:t>i</m:t></m:r></m:e></m:nary></m:oMath>";
    let (mathml, _) = to_mathml(omml);
    assert_eq!(
        mathml,
        format!(
            "<math xmlns=\"{NS}\"><munderover><mo>\u{2211}</mo><mi>i</mi>\
             <mi>n</mi></munderover><mi>i</mi></math>"
        )
    );
    assert_stable(&mathml, false);
}

/// An integral defaults its operator (`∫`) and side-set (`subSup`) placement
/// maps to `<msubsup>`; a hidden upper limit degrades to `<msub>`.
#[test]
fn nary_integral_subsup_and_hidden_limit() {
    let omml = "<m:oMath><m:nary><m:naryPr><m:limLoc m:val=\"subSup\"/>\
                <m:supHide m:val=\"1\"/></m:naryPr>\
                <m:sub><m:r><m:t>0</m:t></m:r></m:sub><m:sup/>\
                <m:e><m:r><m:t>f</m:t></m:r></m:e></m:nary></m:oMath>";
    let (mathml, _) = to_mathml(omml);
    assert_eq!(
        mathml,
        format!(
            "<math xmlns=\"{NS}\"><msub><mo>\u{222B}</mo><mn>0</mn></msub>\
             <mi>f</mi></math>"
        )
    );
    assert_stable(&mathml, false);
}

/// A 2×2 matrix maps to `<mtable>`/`<mtr>`/`<mtd>` and round-trips.
#[test]
fn matrix_two_by_two() {
    let omml = "<m:oMath><m:m><m:mr>\
                <m:e><m:r><m:t>a</m:t></m:r></m:e><m:e><m:r><m:t>b</m:t></m:r></m:e>\
                </m:mr><m:mr>\
                <m:e><m:r><m:t>c</m:t></m:r></m:e><m:e><m:r><m:t>d</m:t></m:r></m:e>\
                </m:mr></m:m></m:oMath>";
    let (mathml, _) = to_mathml(omml);
    assert_eq!(
        mathml,
        format!(
            "<math xmlns=\"{NS}\"><mtable><mtr><mtd><mi>a</mi></mtd>\
             <mtd><mi>b</mi></mtd></mtr><mtr><mtd><mi>c</mi></mtd>\
             <mtd><mi>d</mi></mtd></mtr></mtable></math>"
        )
    );
    assert_stable(&mathml, false);
}

/// An accent (x-hat) maps to `<mover accent="true">` and round-trips; the
/// default accent character is the combining circumflex.
#[test]
fn accent_hat_over_identifier() {
    let omml = "<m:oMath><m:acc><m:e><m:r><m:t>x</m:t></m:r></m:e></m:acc></m:oMath>";
    let (mathml, _) = to_mathml(omml);
    assert_eq!(
        mathml,
        format!(
            "<math xmlns=\"{NS}\"><mover accent=\"true\"><mi>x</mi>\
             <mo>\u{0302}</mo></mover></math>"
        )
    );
    assert_stable(&mathml, false);
}

/// A limit (`lim` under an expression) maps `m:limLow` ⇄ `<munder>`.
#[test]
fn lim_low_under() {
    let omml = "<m:oMath><m:limLow><m:e><m:r><m:t>lim</m:t></m:r></m:e>\
                <m:lim><m:r><m:t>n</m:t></m:r></m:lim></m:limLow></m:oMath>";
    let (mathml, _) = to_mathml(omml);
    assert_eq!(
        mathml,
        format!("<math xmlns=\"{NS}\"><munder><mi>lim</mi><mi>n</mi></munder></math>")
    );
    assert_stable(&mathml, false);
}
