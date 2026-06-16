// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Builds the EPUB 3 package document (`package.opf`): metadata, manifest, and
//! spine. EPUB 3.3 §5.

use loki_doc_model::meta::DocumentMeta;

use crate::opf_meta::build_metadata;

/// Renders the full `package.opf` document.
///
/// `identifier` and `modified_iso` are forwarded to [`build_metadata`]; the
/// manifest and spine are fixed because the exporter emits exactly one content
/// document, one navigation document, and one stylesheet.
#[must_use]
pub fn build_package_opf(meta: &DocumentMeta, identifier: &str, modified_iso: &str) -> String {
    let metadata = build_metadata(meta, identifier, modified_iso);
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\" \
         unique-identifier=\"pub-id\" xml:lang=\"en\">\n\
         {metadata}  <manifest>\n\
         <item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/>\n\
         <item id=\"content\" href=\"content.xhtml\" media-type=\"application/xhtml+xml\"/>\n\
         <item id=\"style\" href=\"style.css\" media-type=\"text/css\"/>\n\
         </manifest>\n\
         <spine>\n\
         <itemref idref=\"content\"/>\n\
         </spine>\n\
         </package>\n",
        metadata = metadata,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_has_manifest_and_spine() {
        let mut meta = DocumentMeta::default();
        meta.title = Some("Doc".into());
        let opf = build_package_opf(&meta, "urn:uuid:1", "2026-01-01T00:00:00Z");
        assert!(opf.contains("version=\"3.0\""));
        assert!(opf.contains("unique-identifier=\"pub-id\""));
        assert!(opf.contains("properties=\"nav\""));
        assert!(opf.contains("<itemref idref=\"content\"/>"));
    }
}
