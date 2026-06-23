// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Static OCF container parts and the canonical part paths for the EPUB.

/// The EPUB media type, written verbatim as the first ZIP entry (stored,
/// uncompressed). EPUB 3.3 / OCF §4.3.
pub const MIMETYPE: &str = "application/epub+zip";

/// Path of the OCF container descriptor inside the ZIP.
pub const CONTAINER_PATH: &str = "META-INF/container.xml";

/// Directory holding the publication resources.
pub const EPUB_DIR: &str = "EPUB";

/// Path of the package document inside the ZIP.
pub const PACKAGE_PATH: &str = "EPUB/package.opf";

/// Path of the navigation document inside the ZIP.
pub const NAV_PATH: &str = "EPUB/nav.xhtml";

/// Path of the single content document inside the ZIP.
pub const CONTENT_PATH: &str = "EPUB/content.xhtml";

/// Path of the stylesheet inside the ZIP.
pub const STYLE_PATH: &str = "EPUB/style.css";

/// The OCF container descriptor pointing readers at the package document.
pub const CONTAINER_XML: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
    "<container version=\"1.0\" xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\">\n",
    "  <rootfiles>\n",
    "    <rootfile full-path=\"EPUB/package.opf\" media-type=\"application/oebps-package+xml\"/>\n",
    "  </rootfiles>\n",
    "</container>\n",
);

/// A minimal reflowable stylesheet for the content document.
pub const STYLE_CSS: &str = "html { font-family: serif; }\n\
body { margin: 5%; line-height: 1.4; }\n\
h1, h2, h3, h4, h5, h6 { font-family: sans-serif; line-height: 1.2; }\n\
pre { white-space: pre-wrap; font-family: monospace; }\n\
blockquote { margin-left: 1.5em; font-style: italic; }\n\
table { border-collapse: collapse; margin: 1em 0; }\n\
th, td { border: 1px solid #999; padding: 0.3em 0.5em; }\n\
caption { font-style: italic; margin-bottom: 0.3em; }\n\
figure { margin: 1em 0; text-align: center; }\n\
figcaption { font-size: 0.9em; color: #555; }\n\
img { max-width: 100%; height: auto; }\n";
