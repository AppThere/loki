// SPDX-License-Identifier: Apache-2.0

//! `tokenize_candidates`: real files with supported extensions become
//! tokens, the detached predicate rides along, and junk paths are skipped
//! rather than failing the boot.

use std::io::Write as _;

use loki_file_access::FileAccessToken;

use super::tokenize_candidates;

fn temp_file(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("loki-startup-open-tests");
    std::fs::create_dir_all(&dir).expect("mkdir");
    let path = dir.join(name);
    let mut f = std::fs::File::create(&path).expect("create");
    f.write_all(b"stub").expect("write");
    path
}

#[test]
fn real_supported_files_become_candidates_with_the_right_posture() {
    let docx = temp_file("startup-a.docx");
    let md = temp_file("startup-b.md");

    let candidates = tokenize_candidates(vec![
        docx.to_string_lossy().into_owned(),
        md.to_string_lossy().into_owned(),
    ]);
    assert_eq!(candidates.len(), 2);
    assert!(!candidates[0].detached, "a document edits in place");
    assert!(candidates[1].detached, "an import-only file opens detached");

    // The serialized token round-trips to the same display name the editor
    // load path will see.
    let token = FileAccessToken::deserialize(&candidates[0].serialized).expect("round-trip");
    assert_eq!(token.display_name(), "startup-a.docx");
}

#[test]
fn missing_files_and_unsupported_formats_are_skipped_not_fatal() {
    let png = temp_file("startup-c.png");
    let candidates = tokenize_candidates(vec![
        "/definitely/not/here.docx".to_string(),
        png.to_string_lossy().into_owned(),
    ]);
    assert!(
        candidates.is_empty(),
        "nonexistent and unsupported paths must both be skipped"
    );
}
