// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The custom section's field resolution. Moved with `resolve` into
//! `custom_mode.rs` (Spec 08 T5.3) when the module split.
use super::*;

fn f(vals: [&str; 4]) -> [String; 4] {
    vals.map(str::to_string)
}

#[test]
fn resolve_per_mode() {
    assert_eq!(
        resolve(Mode::Hex, &f(["#00FF00", "", "", ""])),
        Some((0, 255, 0))
    );
    assert_eq!(
        resolve(Mode::Rgb, &f(["255", "0", "0", ""])),
        Some((255, 0, 0))
    );
    assert_eq!(resolve(Mode::Rgb, &f(["300", "0", "0", ""])), None);
    assert_eq!(
        resolve(Mode::Hsl, &f(["240", "100", "50", ""])),
        Some((0, 0, 255))
    );
    assert_eq!(
        resolve(Mode::Hsv, &f(["0", "0", "100", ""])),
        Some((255, 255, 255))
    );
    assert_eq!(
        resolve(Mode::Cmyk, &f(["0", "100", "100", "0"])),
        Some((255, 0, 0))
    );
    assert_eq!(resolve(Mode::Rgb, &f(["", "0", "0", ""])), None);
    assert_eq!(resolve(Mode::Hex, &f(["not-a-color", "", "", ""])), None);
}

/// **The fields are empty while the square is the source, and that is the whole
/// reason Apply had to stop reading them** (Spec 08 T5.3).
///
/// Dragging the square never writes the fields back — that is `custom_source`'s
/// "last edited wins" rule, and it is correct. The consequence is that
/// `resolve(mode, fields)` is `None` for a colour the reader can plainly see in
/// the preview, so a button that gated on the preview and acted on the fields
/// looked live and did nothing. A screen sitting found it; this states the fact
/// underneath it so the next reader does not reach for `fields` again.
#[test]
fn the_typed_fields_do_not_speak_for_a_dragged_colour() {
    let empty = [const { String::new() }; 4];
    for (name, mode) in [
        ("Hex", Mode::Hex),
        ("Rgb", Mode::Rgb),
        ("Hsl", Mode::Hsl),
        ("Hsv", Mode::Hsv),
        ("Cmyk", Mode::Cmyk),
    ] {
        assert_eq!(
            resolve(mode, &empty),
            None,
            "{name}: untouched fields must not resolve to a colour",
        );
    }
}
