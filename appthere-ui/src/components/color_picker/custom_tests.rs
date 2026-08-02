// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The custom section's field resolution. Extracted from `custom.rs` when the
//! saturation/value square landed and took the file over the ceiling.
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
