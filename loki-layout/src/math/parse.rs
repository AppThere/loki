// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Minimal MathML parser for the layout engine.
//!
//! Parses the model's canonical MathML string (see
//! `loki_doc_model::content::inline::Inline::Math`) into a lightweight element
//! tree. A hand-rolled scanner is used instead of pulling an XML crate into the
//! otherwise dependency-light `loki-layout`; the input is always well-formed,
//! compact MathML produced by the importers. Structure, token text, and
//! attributes (`mfenced` fences, `mover` accents — 5.8) are retained.

/// A parsed MathML element.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct MNode {
    /// Local element name (prefix stripped), e.g. `"mfrac"`, `"mi"`.
    pub tag: String,
    /// Character data for token elements (`mi`/`mn`/`mo`/`mtext`).
    pub text: String,
    /// Child elements, in document order.
    pub children: Vec<MNode>,
    /// Attributes as `(local name, unescaped value)` pairs.
    pub attrs: Vec<(String, String)>,
}

impl MNode {
    /// Returns the value of the attribute with the given local `name`, if any.
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
}

/// Parses a MathML string, returning the first element (the `<math>` root), or
/// `None` if there is no element.
pub(super) fn parse_mathml(s: &str) -> Option<MNode> {
    let mut p = Parser {
        b: s.as_bytes(),
        pos: 0,
    };
    p.next_element()
}

struct Parser<'a> {
    b: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    /// Scans forward to the next element start and parses it, skipping XML
    /// declarations (`<?…?>`) and comments/doctypes (`<!…>`).
    fn next_element(&mut self) -> Option<MNode> {
        loop {
            while self.pos < self.b.len() && self.b[self.pos] != b'<' {
                self.pos += 1;
            }
            if self.pos >= self.b.len() {
                return None;
            }
            match self.b.get(self.pos + 1) {
                Some(b'?') | Some(b'!') => {
                    self.skip_to_gt();
                }
                _ => return self.parse_element(),
            }
        }
    }

    /// Parses an element whose `<` is at `self.pos`.
    fn parse_element(&mut self) -> Option<MNode> {
        self.pos += 1; // consume '<'
        let name = self.read_name();
        let (attrs, self_closing) = self.read_attrs();
        let mut node = MNode {
            tag: local(&name),
            attrs,
            ..Default::default()
        };
        if self_closing {
            return Some(node);
        }
        loop {
            let text = self.read_text();
            if !text.is_empty() {
                node.text.push_str(&unescape(&text));
            }
            if self.pos >= self.b.len() {
                break;
            }
            // At '<': either a closing tag or a child element.
            if self.b.get(self.pos + 1) == Some(&b'/') {
                self.skip_to_gt();
                break;
            }
            match self.parse_element() {
                Some(child) => node.children.push(child),
                None => break,
            }
        }
        Some(node)
    }

    /// Reads an element/attribute name (until whitespace, `/`, or `>`).
    fn read_name(&mut self) -> String {
        let start = self.pos;
        while self.pos < self.b.len() {
            match self.b[self.pos] {
                b' ' | b'\t' | b'\n' | b'\r' | b'/' | b'>' => break,
                _ => self.pos += 1,
            }
        }
        String::from_utf8_lossy(&self.b[start..self.pos]).into_owned()
    }

    /// Reads attributes up to the end of the start tag, returning them as
    /// `(local name, unescaped value)` pairs plus `true` for a self-closing
    /// (`/>`) tag. A malformed/valueless attribute is recorded with an empty
    /// value and scanning continues.
    fn read_attrs(&mut self) -> (Vec<(String, String)>, bool) {
        let mut attrs = Vec::new();
        while self.pos < self.b.len() {
            match self.b[self.pos] {
                b' ' | b'\t' | b'\n' | b'\r' => self.pos += 1,
                b'>' => {
                    self.pos += 1;
                    return (attrs, false);
                }
                b'/' if self.b.get(self.pos + 1) == Some(&b'>') => {
                    self.pos += 2;
                    return (attrs, true);
                }
                _ => {
                    let name = self.read_attr_name();
                    let value = if self.b.get(self.pos) == Some(&b'=') {
                        self.pos += 1;
                        self.read_attr_value()
                    } else {
                        String::new()
                    };
                    if !name.is_empty() {
                        attrs.push((local(&name), unescape(&value)));
                    }
                }
            }
        }
        (attrs, false)
    }

    /// Reads an attribute name (until `=`, whitespace, `/`, or `>`).
    fn read_attr_name(&mut self) -> String {
        let start = self.pos;
        while self.pos < self.b.len() {
            match self.b[self.pos] {
                b'=' | b' ' | b'\t' | b'\n' | b'\r' | b'/' | b'>' => break,
                _ => self.pos += 1,
            }
        }
        String::from_utf8_lossy(&self.b[start..self.pos]).into_owned()
    }

    /// Reads a quoted attribute value (the `=` already consumed); an unquoted
    /// value reads to the next whitespace or tag end.
    fn read_attr_value(&mut self) -> String {
        match self.b.get(self.pos) {
            Some(&q @ (b'"' | b'\'')) => {
                self.pos += 1;
                let start = self.pos;
                while self.pos < self.b.len() && self.b[self.pos] != q {
                    self.pos += 1;
                }
                let v = String::from_utf8_lossy(&self.b[start..self.pos]).into_owned();
                if self.pos < self.b.len() {
                    self.pos += 1; // closing quote
                }
                v
            }
            _ => self.read_attr_name(),
        }
    }

    /// Reads character data up to the next `<` (or end of input).
    fn read_text(&mut self) -> String {
        let start = self.pos;
        while self.pos < self.b.len() && self.b[self.pos] != b'<' {
            self.pos += 1;
        }
        String::from_utf8_lossy(&self.b[start..self.pos]).into_owned()
    }

    /// Skips to just past the next `>`.
    fn skip_to_gt(&mut self) {
        while self.pos < self.b.len() && self.b[self.pos] != b'>' {
            self.pos += 1;
        }
        if self.pos < self.b.len() {
            self.pos += 1;
        }
    }
}

/// Strips a namespace prefix from a qualified name.
fn local(name: &str) -> String {
    name.rsplit(':').next().unwrap_or(name).to_string()
}

/// Decodes the five XML predefined entities in token text.
fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}
