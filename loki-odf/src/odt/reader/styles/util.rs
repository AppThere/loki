// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2025 AppThere contributors

//! Shared XML utility for styles submodules.

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::error::{OdfError, OdfResult};

/// Skip the current open element and all its descendants, stopping after the
/// matching end tag. Assumes the `Start` event has already been consumed.
pub(super) fn skip_element(reader: &mut Reader<&[u8]>, end_local: &[u8]) -> OdfResult<()> {
    let mut buf = Vec::new();
    let mut depth: u32 = 1;

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(_)) => depth += 1,
            Ok(Event::End(ref e)) => {
                depth -= 1;
                if depth == 0 && e.local_name().into_inner() == end_local {
                    return Ok(());
                }
                if depth == 0 {
                    return Ok(());
                }
            }
            Ok(Event::Eof) => return Ok(()),
            Err(e) => {
                return Err(OdfError::Xml {
                    part: "styles.xml".to_string(),
                    source: e,
                });
            }
            _ => {}
        }
    }
}
