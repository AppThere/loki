// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! [`GpuClass`] and the two questions it answers (Spec 08 r68).
//!
//! Extracted from `device_profile.rs` when the second predicate landed and the
//! file crossed the 300-line ceiling. The two predicates belong together: the
//! whole point is that they are different questions, and keeping them adjacent
//! is what makes picking the wrong one visible.

/// Rough capability class of the GPU, from the wgpu adapter.
///
/// Replaces `cfg!(target_os = "android")` as the renderer-path selector: the
/// question the renderer actually asks is "can this device run Vello's compute
/// pipelines", which an emulator on x86 answers differently from a physical
/// Android device (S0.6 §2a, §3).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum GpuClass {
    /// Not yet probed.
    #[default]
    Unknown,
    /// Discrete GPU.
    Discrete,
    /// Integrated GPU.
    Integrated,
    /// Software rasteriser (SwiftShader, llvmpipe) — cannot run Vello compute.
    Software,
    /// No usable adapter; the CPU renderer is the only option.
    None,
}

impl GpuClass {
    /// `true` when the GPU paint path is **hardware**-accelerated.
    ///
    /// Ask this about performance expectations. **Do not ask it about memory** —
    /// see [`Self::allocates_page_textures`], which is a different question with
    /// a different answer for [`Self::Software`].
    #[must_use]
    pub fn is_hardware_accelerated(self) -> bool {
        matches!(self, Self::Discrete | Self::Integrated)
    }

    /// `true` when the renderer allocates page textures on this device.
    ///
    /// # Two questions that agree everywhere except where it matters
    ///
    /// This used to be one predicate, `supports_gpu_paint`, whose *name* asked
    /// "is this GPU-accelerated" while its one caller — the texture budget —
    /// needed "does this allocate page textures". Those agree on every variant
    /// except [`Self::Software`], and Software is precisely the case that was
    /// wrong: a software adapter runs the paint path and allocates page
    /// textures like any other, so the budget concluded "no textures here" and
    /// collapsed to its 24 MiB floor — target *and* survival ceiling — on any
    /// machine where wgpu picked llvmpipe or SwiftShader (a VM, a headless or
    /// remote Linux desktop). The planner then sat permanently in the survival
    /// regime and softened the body text the reader was looking at, which is the
    /// defect ADR L08-026 exists to prevent.
    ///
    /// That is L08-031's shape in a predicate rather than a name. The budget
    /// arm's comment claimed it "never binds anything in practice" — **retracted**:
    /// it was true of the question the name asked and false of the question the
    /// call site asked, and it bound on every software-adapter machine.
    ///
    /// Kept as two predicates rather than a rename because both questions are
    /// real. Software is fast-path-ineligible *and* memory-hungry.
    ///
    /// Written out rather than negated, so a new [`GpuClass`] variant is a
    /// compile error here instead of silently joining whichever side the `!`
    /// puts it on.
    #[must_use]
    pub fn allocates_page_textures(self) -> bool {
        match self {
            // Every adapter class that paints, including the slow one.
            Self::Discrete | Self::Integrated | Self::Software => true,
            // No adapter: the CPU renderer runs and there are no page textures.
            Self::None => false,
            // The caller must not reach here — "not probed yet" is not "no GPU"
            // (L9-009). `texture_budget` maps it to `None` before asking.
            Self::Unknown => false,
        }
    }
}
