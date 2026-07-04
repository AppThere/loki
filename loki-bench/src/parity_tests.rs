// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! CPU/GPU parity cadence trigger (Spec 06 M6 / §12): Vello-version extraction
//! and the bump-detection verdict. Pure and headless.

use super::*;

const LOCK: &str = "\
[[package]]
name = \"vello_cpu\"
version = \"9.9.9\"

[[package]]
name = \"vello\"
version = \"0.6.0\"

[[package]]
name = \"vello_encoding\"
version = \"0.6.0\"
";

#[test]
fn extracts_exact_vello_version_not_a_sibling_crate() {
    assert_eq!(vello_version_from_lock(LOCK).as_deref(), Some("0.6.0"));
}

#[test]
fn missing_vello_is_none() {
    assert_eq!(
        vello_version_from_lock("[[package]]\nname = \"loro\"\n"),
        None
    );
}

#[test]
fn marker_round_trips() {
    let m = render_marker("0.6.0");
    assert_eq!(confirmed_version_from_marker(&m).as_deref(), Some("0.6.0"));
}

#[test]
fn same_version_is_up_to_date() {
    assert_eq!(
        parity_status("0.6.0", Some("0.6.0")),
        ParityStatus::UpToDate
    );
    assert!(!parity_status("0.6.0", Some("0.6.0")).is_due());
}

#[test]
fn a_version_bump_is_due() {
    let s = parity_status("0.7.0", Some("0.6.0"));
    assert_eq!(
        s,
        ParityStatus::Due {
            last: "0.6.0".to_string(),
            current: "0.7.0".to_string(),
        }
    );
    assert!(s.is_due());
}

#[test]
fn no_marker_means_never_run() {
    let s = parity_status("0.6.0", None);
    assert_eq!(
        s,
        ParityStatus::NeverRun {
            current: "0.6.0".to_string()
        }
    );
    assert!(s.is_due());
}
