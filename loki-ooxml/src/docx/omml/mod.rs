// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! OMML (Office Math Markup Language) ⇄ `MathML` conversion.
//!
//! DOCX stores mathematical content as OMML (`m:oMath` / `m:oMathPara`,
//! ECMA-376 §22.1). The format-neutral document model stores math as a single
//! `MathML` string in [`loki_doc_model::content::inline::Inline::Math`] (`MathML` is
//! the W3C interchange standard and ODF's native math representation). This
//! module converts between the two on import and export.
//!
//! # Covered constructs
//!
//! The converter is bidirectional and mutually inverse over the common
//! construct set: text runs (`m:r`/`m:t` ⇄ `<mi>`/`<mn>`/`<mo>`), fractions
//! (`m:f` ⇄ `<mfrac>`), super/subscripts (`m:sSup`/`m:sSub`/`m:sSubSup` ⇄
//! `<msup>`/`<msub>`/`<msubsup>`), and radicals (`m:rad` ⇄ `<msqrt>`/`<mroot>`).
//! delimiters (`m:d` ⇄ `<mfenced>`), n-ary operators (`m:nary` ⇄
//! `<munderover>`/`<msubsup>` &c. with an `<mo>` operator base), matrices
//! (`m:m` ⇄ `<mtable>`), accents (`m:acc` ⇄ `<mover accent="true">`), and
//! limits (`m:limLow`/`m:limUpp` ⇄ `<munder>`/`<mover>`) — see `read_structs`
//! / `write_structs` (5.8). Unrecognised elements pass their content through
//! (best effort).

mod read;
mod read_structs;
mod write;
mod write_structs;

pub(crate) use read::read_math;
pub(crate) use write::write_omath;

/// The `MathML` namespace URI placed on the root `<math>` element.
pub(crate) const MATHML_NS: &str = "http://www.w3.org/1998/Math/MathML";

/// The OMML namespace URI declared on `w:document` so `m:`-prefixed math
/// elements resolve. ECMA-376 §22.1.
pub(crate) const OMML_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/math";

/// A minimal generic XML element tree used as the intermediate representation
/// for both conversion directions.
#[derive(Debug, Clone, Default)]
pub(crate) struct XmlNode {
    /// Local element name (namespace prefix stripped).
    pub tag: String,
    /// Direct character data of this element (concatenated text events).
    pub text: String,
    /// Child elements, in document order.
    pub children: Vec<XmlNode>,
    /// Attributes as `(local name, unescaped value)` pairs, in document order
    /// (OMML property values like `m:chr m:val="…"` and `MathML` attributes
    /// like `open`/`accent` live here).
    pub attrs: Vec<(String, String)>,
}

impl XmlNode {
    /// Returns the first direct child with the given local `tag`, if any.
    pub fn child(&self, tag: &str) -> Option<&XmlNode> {
        self.children.iter().find(|c| c.tag == tag)
    }

    /// Returns the value of the attribute with the given local `name`, if any.
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
}

/// Escapes the five XML predefined entities for use in element text.
pub(crate) fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
#[path = "omml_tests.rs"]
mod tests;
