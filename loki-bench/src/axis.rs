// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The two-axis split (Spec 06 §5) and the per-metric axis mapping (decision D1).

/// Which axis a benchmark belongs to — the single fact that decides *where it
/// runs* and *whether its number is tracked across machines*.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Axis {
    /// Hardware-independent: allocation bytes/counts, op counts, `vello_cpu`
    /// render cost. Runs headless in the agent environment with no GPU; its
    /// numbers are diffable across machines and over time.
    Portable,
    /// Hardware-dependent: GPU frame-time, wall-clock latency, real peak RSS.
    /// Runs only on real hardware; a local reference point, not a tracked signal.
    DeviceBound,
}

impl Axis {
    /// `true` when this axis runs headless in the agent environment (no GPU).
    #[must_use]
    pub fn is_agent_runnable(self) -> bool {
        matches!(self, Axis::Portable)
    }

    /// `true` when this axis needs real hardware and must not run in CI/agent.
    #[must_use]
    pub fn is_hardware_only(self) -> bool {
        matches!(self, Axis::DeviceBound)
    }

    /// A stable slug for baseline files and report grouping.
    #[must_use]
    pub fn slug(self) -> &'static str {
        match self {
            Axis::Portable => "portable",
            Axis::DeviceBound => "device",
        }
    }
}

/// A concrete quantity a benchmark measures. Each maps to exactly one [`Axis`],
/// which is where the §5 table becomes code: allocation bytes/counts are the
/// portable, continuously-tracked memory signal (D1); RSS and frame-time are
/// device-local reality checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Metric {
    /// Total heap bytes allocated over a region (dhat). Portable.
    AllocBytes,
    /// Total heap allocations over a region (dhat). Portable.
    AllocBlocks,
    /// A domain op count (model/layout/IO/style operations). Portable.
    OpCount,
    /// `vello_cpu` render cost — the hardware-independent render proxy. Portable.
    RenderCost,
    /// Wall-clock latency of an operation. Device-bound (CPU/scheduler variance).
    WallTime,
    /// Real resident-set-size peak. Device-bound (allocator/OS overhead).
    PeakRss,
    /// GPU frame time on the production paint path. Device-bound.
    FrameTime,
}

impl Metric {
    /// The axis this metric is tracked on (Spec 06 §5 / D1).
    #[must_use]
    pub fn axis(self) -> Axis {
        match self {
            Metric::AllocBytes | Metric::AllocBlocks | Metric::OpCount | Metric::RenderCost => {
                Axis::Portable
            }
            Metric::WallTime | Metric::PeakRss | Metric::FrameTime => Axis::DeviceBound,
        }
    }

    /// `true` when this metric can be captured headless in the agent environment.
    #[must_use]
    pub fn is_agent_runnable(self) -> bool {
        self.axis().is_agent_runnable()
    }
}

#[cfg(test)]
#[path = "axis_tests.rs"]
mod tests;
