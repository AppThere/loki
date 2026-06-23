// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: MIT

//! Content-types compatibility shims: fallback media types for unmapped
//! extensions and locating a `[Content_Types].xml` part that some writers place
//! off-root or with non-canonical casing ([MS-OI29500] / [MS-OE376]).

#[allow(unused_imports)]
use crate::error::DeviationWarning;
use zip::ZipArchive;

/// Returns a best-guess media type for a file `extension` when the package
/// omits it from `[Content_Types].xml`. Returns `""` for unknown extensions.
pub fn fallback_media_type(extension: &str) -> &'static str {
    match extension.to_ascii_lowercase().as_str() {
        "xml" => "application/xml",
        "rels" => "application/vnd.openxmlformats-package.relationships+xml",
        "png" => "image/png",
        "jpeg" | "jpg" => "image/jpeg",
        "gif" => "image/gif",
        "doc" | "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" | "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ppt" | "pptx" => {
            "application/vnd.openxmlformats-officedocument.presentationml.presentation"
        }
        _ => "",
    }
}

/// Finds the index of the `[Content_Types].xml` entry in a ZIP archive.
///
/// Prefers the exact, spec-mandated root name. Outside the `strict` feature it
/// also accepts a case-insensitive match anywhere in the archive, recording a
/// [`DeviationWarning::ContentTypesNotAtRoot`]. Returns `None` if absent.
#[allow(clippy::ptr_arg)]
pub fn find_content_types<R: std::io::Read + std::io::Seek>(
    zip: &mut ZipArchive<R>,
    #[allow(unused_variables)] warnings: &mut Vec<DeviationWarning>,
) -> Option<usize> {
    for i in 0..zip.len() {
        // `let … else` keeps a single level of nesting regardless of which
        // feature set is active. Under `--features strict` the case-insensitive
        // fallback below is compiled out, which would otherwise leave a sole
        // inner `if` and trip `clippy::collapsible_if`.
        let Ok(zf) = zip.by_index(i) else { continue };
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
    None
}
