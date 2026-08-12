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
