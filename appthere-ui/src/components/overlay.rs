// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The reserved root-layer z-index band (Spec 08 r65, I-28).
//!
//! # This module used to be a backdrop mechanism, and the mechanism is gone
//!
//! `use_provide_backdrop` / `AtBackdropHost` / `use_backdrop` let any descendant
//! raise a window-level click-catcher while rendering its popup **in place**.
//! That split does not work in this engine and the module said so: a `z-index`
//! above the backdrop keeps a popup clickable only if the popup is a *sibling*
//! of the backdrop, and Blitz sorts siblings only, so a popup left in place is
//! hit-tested after a root-hosted backdrop whatever value it carries.
//!
//! Its last requester was the ribbon overflow menu, whose controls were dead for
//! exactly that reason. I-28 moved it to [`super::popover::AtPopoverHost`],
//! which hosts both layers at the root — and left a public API that is
//! documented as non-functional, has no callers, and would hand the next
//! consumer the same defect. Deleting it is the L08-043 move: make the wrong
//! thing unavailable rather than warn about it.
//!
//! What survives is the number, because the popover host and the band gate both
//! need it.

/// z-index of the root layer band's floor — the popover host's backdrop.
///
/// # The reserved root-layer band
///
/// This value and up is the band: the backdrop at 40, the popover host at 41,
/// the modal dialogs at 2000+. **Application crates may not enter it**, enforced
/// by `scripts/check-root-layer-band.py`.
///
/// # The r64 rationale is retracted; the band still earns its place
///
/// r64 said an app-crate value here "competes with a root layer in a stacking
/// context the consumer cannot see". Blitz has **no stacking contexts**: each
/// parent sorts its own `paint_children` by `z_index()` among siblings, and
/// hit-testing walks that same list in reverse. Nothing below the root can
/// outrank a root sibling, at any z-index — so the competition described never
/// happens, and an app-crate 1000 is not dangerous for the stated reason.
///
/// What is true, and is worse: **a root-hosted layer outranks every application
/// surface unconditionally.** That is why the band is reserved — not because app
/// values win, but because they cannot, so a value up here is always a consumer
/// that has misunderstood where its overlay lives, and the fix is to host it
/// rather than to raise it. It also means a stale root layer is catastrophic
/// rather than cosmetic, which is what r66 demonstrated.
///
/// The gate duplicates this number as a literal, and says so; if this moves,
/// that moves.
pub const BACKDROP_Z_INDEX: i32 = 40;
