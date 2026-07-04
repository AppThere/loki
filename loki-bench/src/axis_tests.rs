// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The two-axis split (Spec 06 §5) and the per-metric mapping (D1), verified
//! without a window or a GPU.

use super::*;

#[test]
fn portable_is_agent_runnable_and_device_is_not() {
    assert!(Axis::Portable.is_agent_runnable());
    assert!(!Axis::Portable.is_hardware_only());
    assert!(Axis::DeviceBound.is_hardware_only());
    assert!(!Axis::DeviceBound.is_agent_runnable());
}

#[test]
fn axis_slugs_are_stable() {
    assert_eq!(Axis::Portable.slug(), "portable");
    assert_eq!(Axis::DeviceBound.slug(), "device");
}

#[test]
fn allocation_and_op_metrics_are_portable() {
    // D1: allocation bytes/counts are the portable, continuously-tracked signal.
    for m in [
        Metric::AllocBytes,
        Metric::AllocBlocks,
        Metric::OpCount,
        Metric::RenderCost,
    ] {
        assert_eq!(m.axis(), Axis::Portable, "{m:?} must be portable");
        assert!(m.is_agent_runnable(), "{m:?} must run headless");
    }
}

#[test]
fn timing_rss_and_frame_time_are_device_bound() {
    for m in [Metric::WallTime, Metric::PeakRss, Metric::FrameTime] {
        assert_eq!(m.axis(), Axis::DeviceBound, "{m:?} must be device-bound");
        assert!(
            !m.is_agent_runnable(),
            "{m:?} must not run in the agent env"
        );
    }
}
