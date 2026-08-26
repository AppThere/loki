// SPDX-License-Identifier: Apache-2.0

//! The IPP option mapping: loose copies parsing, verbatim page ranges (the
//! `loki-print` validator owns rejection), and the duplex switch.

use loki_print::Duplex;

use super::build_ipp_options;

#[test]
fn options_map_from_field_strings() {
    let o = build_ipp_options("3", " 1-3,5 ", true, "Report".into());
    assert_eq!(o.copies, 3);
    assert_eq!(o.duplex, Duplex::LongEdge);
    assert_eq!(o.page_ranges.as_deref(), Some("1-3,5"));
    assert_eq!(o.job_title.as_deref(), Some("Report"));

    // Blank/junk copies degrade to one copy; blank pages means all pages.
    let o = build_ipp_options("", "", false, "T".into());
    assert_eq!(o.copies, 1);
    assert_eq!(o.duplex, Duplex::Simplex);
    assert_eq!(o.page_ranges, None);
    let o = build_ipp_options("many", "", false, "T".into());
    assert_eq!(o.copies, 1);
}

#[test]
fn a_zero_copies_field_never_becomes_a_zero_copy_job() {
    // "0" parses, so it does not take the `unwrap_or(1)` path that blank and
    // junk take — it reaches `PrintOptions` as 0. That is safe only because
    // the encoder treats 0 and 1 alike and emits no `copies` attribute below
    // 2 (IPP `copies` is integer(1:MAX)); `loki_print`'s
    // `zero_and_one_copies_emit_no_copies_attribute` pins that end. This test
    // pins the half this crate owns: "0" is forwarded as 0, not as some other
    // number that would silently multiply the job.
    let o = build_ipp_options("0", "", false, "T".into());
    assert_eq!(o.copies, 0);

    // The polarity — a real multi-copy request is still forwarded intact, so
    // the assertion above is about the boundary and not about copies being
    // ignored altogether.
    let o = build_ipp_options("2", "", false, "T".into());
    assert_eq!(o.copies, 2);
}

#[test]
fn malformed_ranges_are_refused_downstream_not_silently_sent() {
    // The dialog passes the string through verbatim; the typed refusal
    // happens in loki-print's validator before any bytes reach a printer —
    // prove the coupling holds for the string this dialog would forward.
    let o = build_ipp_options("1", "3-2", false, "T".into());
    let forwarded = o.page_ranges.expect("non-blank range forwards");
    assert!(
        loki_print::parse_page_ranges(&forwarded).is_err(),
        "an inverted range must fail the downstream validator"
    );
}
