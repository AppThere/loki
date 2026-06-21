// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Builds the EPUB 3 package document (`package.opf`): metadata, manifest, and
//! spine. EPUB 3.3 §5.

use loki_doc_model::meta::DocumentMeta;

use crate::images::EpubImage;
use crate::opf_meta::build_metadata;
use crate::xml::escape_attr;

/// Renders the full `package.opf` document.
///
/// `identifier` and `modified_iso` are forwarded to [`build_metadata`]; the
/// manifest lists the fixed content/nav/style items plus one item per packaged
/// image, and the spine references the single content document.
#[must_use]
pub fn build_package_opf(
    meta: &DocumentMeta,
    identifier: &str,
    modified_iso: &str,
    images: &[EpubImage],
) -> String {
    let metadata = build_metadata(meta, identifier, modified_iso);
    let image_items = build_image_items(images);
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\" \
         unique-identifier=\"pub-id\" xml:lang=\"en\">\n\
         {metadata}  <manifest>\n\
         <item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/>\n\
         <item id=\"content\" href=\"content.xhtml\" media-type=\"application/xhtml+xml\"/>\n\
         <item id=\"style\" href=\"style.css\" media-type=\"text/css\"/>\n\
         {image_items}  </manifest>\n\
         <spine>\n\
         <itemref idref=\"content\"/>\n\
         </spine>\n\
         </package>\n",
        metadata = metadata,
        image_items = image_items,
    )
}

/// Builds the `<item>` manifest entries for packaged images.
fn build_image_items(images: &[EpubImage]) -> String {
    let mut out = String::new();
    for image in images {
        out.push_str(&format!(
            "    <item id=\"{id}\" href=\"{href}\" media-type=\"{mt}\"/>\n",
            id = escape_attr(&image.id),
            href = escape_attr(&image.href),
            mt = escape_attr(&image.media_type),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_has_manifest_and_spine() {
        let mut meta = DocumentMeta::default();
        meta.title = Some("Doc".into());
        let images = [EpubImage {
            id: "img0".into(),
            href: "images/img0.png".into(),
            media_type: "image/png".into(),
            bytes: vec![1, 2, 3],
        }];
        let opf = build_package_opf(&meta, "urn:uuid:1", "2026-01-01T00:00:00Z", &images);
        assert!(opf.contains("version=\"3.0\""));
        assert!(opf.contains("unique-identifier=\"pub-id\""));
        assert!(opf.contains("properties=\"nav\""));
        assert!(opf.contains("<itemref idref=\"content\"/>"));
        assert!(
            opf.contains("<item id=\"img0\" href=\"images/img0.png\" media-type=\"image/png\"/>")
        );
    }
}
