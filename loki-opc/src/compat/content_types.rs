// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: MIT

//! Extends media type references parsing components mapping fallbacks evaluating bounds enforcing restrictions providing metadata attributes tracking parameters efficiently supporting missing properties cleanly mapping identifiers natively supporting [MS-OI29500] / [MS-OE376].

use zip::ZipArchive;
#[allow(unused_imports)]
use crate::error::DeviationWarning;

/// Maps extensions implicitly injecting identifiers mapping elements avoiding failures locating parameters accurately generating types defining constraints perfectly providing components comprehensively resolving constraints uniquely substituting variables properly preventing crashes directly tracking variants.
pub fn fallback_media_type(extension: &str) -> &'static str {
    match extension.to_ascii_lowercase().as_str() {
        "xml" => "application/xml",
        "rels" => "application/vnd.openxmlformats-package.relationships+xml",
        "png" => "image/png",
        "jpeg" | "jpg" => "image/jpeg",
        "gif" => "image/gif",
        "doc" | "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" | "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ppt" | "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        _ => "",
    }
}

/// Identifies primary xml configuration metadata scanning maps locating definitions safely mapping paths ignoring explicit capitalizations executing fallbacks correctly tracking strings properly defining constraints matching outputs isolating values recursively evaluating limits strictly checking contents correctly finding structures properly resolving definitions matching boundaries perfectly targeting limits.
#[allow(clippy::ptr_arg)]
pub fn find_content_types<R: std::io::Read + std::io::Seek>(
    zip: &mut ZipArchive<R>,
    #[allow(unused_variables)] warnings: &mut Vec<DeviationWarning>,
) -> Option<usize> {
    for i in 0..zip.len() {
        if let Ok(zf) = zip.by_index(i) {
            if zf.name() == "[Content_Types].xml" {
                return Some(i);
            }
            #[cfg(not(feature = "strict"))]
            if zf.name().eq_ignore_ascii_case("[content_types].xml") {
                warnings.push(DeviationWarning::ContentTypesNotAtRoot {
                    found_at: zf.name().to_string(),
                });
                return Some(i);
            }
        }
    }
    None
}
