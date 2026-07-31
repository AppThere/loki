// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! [`TextureBudget::derive`] — turning what is known about the device into the
//! two thresholds.
//!
//! Extracted from `budget.rs` (Spec 08 r67) when the override arm grew the
//! reasoning it needed: the file was at the 300-line ceiling, and the derivation
//! is the one cohesive cluster in it that stands alone.

use super::budget::survival::survival_ceiling;
use super::budget::{
    AVAILABLE_RAM_DIVISOR, BUDGET_FLOOR_BYTES, BudgetInputs, BudgetSource, TOTAL_RAM_DIVISOR,
    TextureBudget, clamp, from_parts,
};
use super::budget::{SURVIVAL_AVAILABLE_RAM_DIVISOR, SURVIVAL_TOTAL_RAM_DIVISOR};

impl TextureBudget {
    /// Derives the budget from what is known about the device.
    #[must_use]
    pub fn derive(inputs: BudgetInputs) -> Self {
        if let Some(bytes) = inputs.user_override_bytes {
            // The override sets the **target**. The survival ceiling stays the
            // device's, because it is not a preference — it is the line past
            // which the process is killed, and the user does not get to move a
            // fact about their machine.
            //
            // It used to call `with_baseline_ceiling`, which supplies the 4
            // GiB-available machine's ceiling regardless of the device. On a
            // 2 GiB-available machine that *raises* the ceiling from 256 MiB to
            // 512 MiB — so asking for a **smaller** budget doubled the peak the
            // planner would allow before degrading. That is the r18 delivery
            // defect (a derived ceiling discarded downstream) reproduced inside
            // the derivation itself; `texture_budget_tests.rs` is named for it.
            return match (inputs.available_ram_bytes, inputs.total_ram_bytes) {
                (Some(available), _) => Self::with_ceiling(
                    bytes,
                    survival_ceiling(available / SURVIVAL_AVAILABLE_RAM_DIVISOR, bytes),
                ),
                (None, Some(total)) => Self::with_ceiling(
                    bytes,
                    survival_ceiling(total / SURVIVAL_TOTAL_RAM_DIVISOR, bytes),
                ),
                // Nothing known about the machine: the baseline ceiling is the
                // only one available, which is what it is for.
                (None, None) => Self::with_baseline_ceiling(bytes),
            };
        }
        // A device with no adapter at all allocates no page textures, so this
        // arm is here to make a diagnostic match the device rather than describe
        // a renderer that is not running.
        //
        // Its old comment claimed it "never binds anything in practice", which is
        // **retracted** (r68): the caller derived this flag from a predicate that
        // answered "is this GPU-accelerated", so a software adapter — llvmpipe, a
        // VM, a headless desktop — landed here while painting normally, pinning
        // target and ceiling to the floor. See `GpuClass::allocates_page_textures`.
        if inputs.gpu_paint_path == Some(false) {
            return from_parts(
                BUDGET_FLOOR_BYTES,
                BUDGET_FLOOR_BYTES,
                BudgetSource::NoGpuPaintPath,
            );
        }
        if let Some(available) = inputs.available_ram_bytes {
            let bytes = clamp(available / AVAILABLE_RAM_DIVISOR);
            return from_parts(
                bytes,
                survival_ceiling(available / SURVIVAL_AVAILABLE_RAM_DIVISOR, bytes),
                BudgetSource::AvailableRam,
            );
        }
        if let Some(total) = inputs.total_ram_bytes {
            let bytes = clamp(total / TOTAL_RAM_DIVISOR);
            return from_parts(
                bytes,
                survival_ceiling(total / SURVIVAL_TOTAL_RAM_DIVISOR, bytes),
                BudgetSource::TotalRam,
            );
        }
        Self::baseline()
    }
}
