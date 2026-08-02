// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The X11/RandR physical-size query (Spec 08 T5.5).
//!
//! # Why a second X connection, and why that is acceptable here
//!
//! winit already holds one, and already reads this very number — it is what
//! produces the `XRandR reported that the display's 0mm in size` warning. It does
//! not *expose* it: `MonitorHandle` has `size`, `position`, `scale_factor` and no
//! physical dimension at all. The alternatives were to vendor winit for one
//! accessor, or to open a short-lived connection of our own.
//!
//! A connection is the cheaper of the two by a wide margin, and this one is
//! opened **once per session**, reads two integers and closes. Vendoring winit
//! would put a large, actively-developed crate under `[patch.crates-io]` for a
//! single getter, and `docs/patches.md` exists to keep that list short and
//! removable.
//!
//! # No `unsafe`, on the standard `device_probe_macos` set
//!
//! `x11rb` is pure Rust and is **already in the lockfile** — winit's own X11
//! backend uses it — so this adds a manifest line and no transitive weight. That
//! is the same measurement that chose `sysinfo` over an `unsafe_code` exception
//! for the memory probe (Spec 08 r55), applied to the same crate-level rule.
//!
//! # What this cannot do
//!
//! X11 only. Wayland (`wl_output` geometry), macOS (`CGDisplayScreenSize`) and
//! Windows (EDID) are named by T5.5 and are **not written** — each needs its own
//! crate and, more to the point, a machine to verify on. They are absent rather
//! than stubbed, so `probe_css_ppi` returning `None` on those platforms is
//! indistinguishable from a display that did not answer, which is exactly the
//! state the calibration fallback exists for.

use x11rb::connection::Connection;
use x11rb::protocol::randr::ConnectionExt as _;

use crate::display_density::{
    DensitySource, DisplayDensity, css_ppi_from_physical, device_ppi_from_mm,
};

/// Queries the display's CSS pixels per inch, or `None` if it cannot be
/// believed.
///
/// `None` covers every failure identically — no X server, no RandR, no connected
/// output, a nonsense physical size — because the caller's response to all of
/// them is the same: offer calibration. Distinguishing them would be an
/// instrument with more precision than any consumer uses.
#[must_use]
pub fn probe_css_ppi(device_scale_factor: f64) -> Option<DisplayDensity> {
    let (conn, screen_num) = x11rb::connect(None).ok()?;
    let root = conn.setup().roots.get(screen_num)?.root;

    let resources = conn
        .randr_get_screen_resources_current(root)
        .ok()?
        .reply()
        .ok()?;

    // The first **connected** output with a usable size wins. Not the first
    // output: a laptop with an unplugged HDMI port enumerates a disconnected
    // one, and a disconnected output reports zeroes — which would take the
    // rejection path and hide a perfectly good built-in panel behind it.
    for output in resources.outputs {
        let Ok(info) = conn.randr_get_output_info(output, resources.config_timestamp) else {
            continue;
        };
        let Ok(info) = info.reply() else { continue };
        if info.connection != x11rb::protocol::randr::Connection::CONNECTED || info.crtc == 0 {
            continue;
        }
        let Ok(crtc) = conn.randr_get_crtc_info(info.crtc, resources.config_timestamp) else {
            continue;
        };
        let Ok(crtc) = crtc.reply() else { continue };

        let Some(device_ppi) = device_ppi_from_mm(u32::from(crtc.width), info.mm_width) else {
            continue;
        };
        if let Some(css) = css_ppi_from_physical(device_ppi, device_scale_factor) {
            return Some(DisplayDensity {
                css_px_per_inch: css,
                source: DensitySource::Platform,
            });
        }
    }
    None
}
