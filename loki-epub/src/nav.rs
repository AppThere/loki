// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Builds the EPUB 3 Navigation Document (`nav.xhtml`).
//!
//! The navigation document is a content document carrying one or more `nav`
//! elements. EPUB 3.3 §5.4.1 requires a `nav` with `epub:type="toc"`.

use crate::content::TocEntry;
use crate::xml::{escape_attr, escape_text};

/// The href of the single content document, relative to the navigation
/// document (both live in the `EPUB/` directory).
const CONTENT_HREF: &str = "content.xhtml";

/// Renders the navigation document from the collected table-of-contents
/// entries. When `toc` is empty a single entry pointing at the content
/// document is emitted so the `toc` nav is never empty (an EPUB requirement).
#[must_use]
pub fn build_nav_xhtml(title: &str, toc: &[TocEntry]) -> String {
    let mut list = String::new();
    if toc.is_empty() {
        list.push_str(&format!(
            "      <li><a href=\"{href}\">{label}</a></li>\n",
            href = CONTENT_HREF,
            label = escape_text(title),
        ));
    } else {
        for entry in toc {
            list.push_str(&format!(
                "      <li><a href=\"{href}#{anchor}\">{label}</a></li>\n",
                href = CONTENT_HREF,
                anchor = escape_attr(&entry.anchor),
                label = escape_text(&entry.text),
            ));
        }
    }

    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE html>\n\
         <html xmlns=\"http://www.w3.org/1999/xhtml\" \
         xmlns:epub=\"http://www.idpf.org/2007/ops\" \
         lang=\"en\" xml:lang=\"en\">\n\
         <head>\n  <meta charset=\"utf-8\"/>\n  <title>{title}</title>\n</head>\n\
         <body>\n  <nav epub:type=\"toc\" id=\"toc\" role=\"doc-toc\">\n\
         <h1>{title}</h1>\n    <ol>\n{list}    </ol>\n  </nav>\n</body>\n</html>\n",
        title = escape_text(title),
        list = list,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_toc_falls_back_to_content_link() {
        let xhtml = build_nav_xhtml("My Book", &[]);
        assert!(xhtml.contains("epub:type=\"toc\""));
        assert!(xhtml.contains("content.xhtml\">My Book"));
    }

    #[test]
    fn renders_toc_entries() {
        let toc = vec![TocEntry {
            level: 1,
            anchor: "h1".into(),
            text: "Chapter 1".into(),
        }];
        let xhtml = build_nav_xhtml("Book", &toc);
        assert!(xhtml.contains("content.xhtml#h1\">Chapter 1"));
    }
}
