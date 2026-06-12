// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Helper for extracting the raw `office:version` attribute from content.xml.

// ── Private helpers ────────────────────────────────────────────────────────────

/// Extract the raw value of the `office:version` attribute from `content.xml`
/// bytes without fully re-parsing; returns `None` if absent.
pub(super) fn raw_version_attr(content: &[u8]) -> Option<String> {
    use quick_xml::Reader;
    use quick_xml::events::Event;

    let mut reader = Reader::from_reader(content);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e) | Event::Empty(ref e)) => {
                let local = {
                    let b = e.local_name().into_inner();
                    if let Some(p) = b.iter().rposition(|&x| x == b':') {
                        b[p + 1..].to_vec()
                    } else {
                        b.to_vec()
                    }
                };
                if local == b"document-content" || local == b"document" {
                    return e.attributes().flatten().find_map(|attr| {
                        let key = attr.key.as_ref();
                        let key_local = if let Some(p) = key.iter().rposition(|&x| x == b':') {
                            &key[p + 1..]
                        } else {
                            key
                        };
                        if key_local == b"version" {
                            attr.unescape_value().ok().map(std::borrow::Cow::into_owned)
                        } else {
                            None
                        }
                    });
                }
                buf.clear();
            }
            Ok(Event::Eof) | Err(_) => return None,
            _ => buf.clear(),
        }
    }
}
