// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Boneyard (`/* … */`) and note (`[[ … ]]`) removal — both are authoring
//! annotations the formatted output must not show (Fountain spec §Boneyard,
//! §Notes). Runs before any line classification so a commented-out scene
//! cannot be classified.

/// Removes boneyard comments and notes. Both can span lines; neither nests.
/// Unterminated markers run to the end of input (the spec leaves this
/// undefined; swallowing matches the major implementations).
pub(crate) fn strip_boneyard_and_notes(text: &str) -> String {
    let no_boneyard = strip_delimited(text, "/*", "*/");
    strip_delimited(&no_boneyard, "[[", "]]")
}

/// Removes every `open…close` span (non-nesting, multi-line).
fn strip_delimited(text: &str, open: &str, close: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(open) {
        out.push_str(&rest[..start]);
        let after_open = &rest[start + open.len()..];
        match after_open.find(close) {
            Some(end) => rest = &after_open[end + close.len()..],
            None => return out, // unterminated: swallow to EOF
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::strip_boneyard_and_notes;

    #[test]
    fn removes_boneyard_and_notes_including_multiline() {
        let src = "Keep /* drop\nthis */ this. [[a note]] End [[unterminated";
        assert_eq!(strip_boneyard_and_notes(src), "Keep  this.  End ");
    }

    #[test]
    fn text_without_markers_is_untouched() {
        assert_eq!(strip_boneyard_and_notes("INT. HOUSE"), "INT. HOUSE");
    }
}
