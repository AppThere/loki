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

use crate::display_calibration::DisplayKey;
use crate::display_density::{
    DensitySource, DisplayDensity, css_ppi_from_physical, device_ppi_from_mm,
};

/// What one connected output reports.
///
/// Identity and physical size are **separate answers**, and separating them is
/// the point rather than tidiness. A display whose `mm_width` is zero — a VM, a
/// KVM switch, a monitor with a broken EDID — is exactly the display the reader
/// will calibrate by hand, so a key derived only from a *successful* size query
/// would be `unidentified` for precisely the panels the calibration store exists
/// to remember. Measured on Xvfb, which reports `name="screen"`, a 1280x900
/// crtc, and `mm=0x0`.
struct Output {
    /// The output's RandR name, e.g. `eDP-1`.
    name: String,
    /// Pixel geometry of the crtc driving it.
    width_px: u32,
    height_px: u32,
    /// Physical width in millimetres, as reported — `0` when it is not known.
    mm_width: u32,
}

/// The first **connected** output with a crtc, if any.
///
/// Not the first output: a laptop with an unplugged HDMI port enumerates a
/// disconnected one, and a disconnected output reports zeroes — which would take
/// the rejection path and hide a perfectly good built-in panel behind it.
///
/// One walk for both questions below, so "which display are we on" has a single
/// answer. Two walks with slightly different accept conditions would disagree on
/// a multi-head machine, and the disagreement would be a calibration stored
/// against the panel the reader is not looking at.
fn first_connected_output() -> Option<Output> {
    let (conn, screen_num) = x11rb::connect(None).ok()?;
    let root = conn.setup().roots.get(screen_num)?.root;
    let resources = conn
        .randr_get_screen_resources_current(root)
        .ok()?
        .reply()
        .ok()?;

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

        return Some(Output {
            name: String::from_utf8_lossy(&info.name).into_owned(),
            width_px: u32::from(crtc.width),
            height_px: u32::from(crtc.height),
            mm_width: info.mm_width,
        });
    }
    None
}

/// Queries the display's CSS pixels per inch, or `None` if it cannot be
/// believed.
///
/// `None` covers every failure identically — no X server, no RandR, no connected
/// output, a nonsense physical size — because the caller's response to all of
/// them is the same: offer calibration. Distinguishing them would be an
/// instrument with more precision than any consumer uses.
#[must_use]
pub fn probe_css_ppi(device_scale_factor: f64) -> Option<DisplayDensity> {
    let out = first_connected_output()?;
    let device_ppi = device_ppi_from_mm(out.width_px, out.mm_width)?;
    let css = css_ppi_from_physical(device_ppi, device_scale_factor)?;
    Some(DisplayDensity {
        css_px_per_inch: css,
        source: DensitySource::Platform,
    })
}

/// Identifies the display this session is on, for the calibration store.
///
/// `None` only when there is no X server, no RandR, or no connected output —
/// **not** when the physical size is unknown, which is the whole reason this is
/// a separate query. See [`Output`].
#[must_use]
pub fn probe_display_key() -> Option<DisplayKey> {
    let out = first_connected_output()?;
    Some(DisplayKey::from_output(
        &out.name,
        out.width_px,
        out.height_px,
    ))
}
