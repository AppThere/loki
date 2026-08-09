// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! A token-bucket rate limiter for macro dialogs (spec §5.5): a "misbehaving
//! macro" that shows dialogs faster than the bucket refills is suspended
//! (`RuntimeError::dialog_rate_limited`). Modal dialogs already block on a user
//! reply, so this is defence-in-depth against a machine-speed dialog storm
//! rather than the primary anti-spoof control (the badged frame, spec §5.5).

use std::time::Instant;

/// ~5 dialogs per 10 s (spec §5.5), starting full so a normal interactive burst
/// is never throttled.
const CAPACITY: f64 = 5.0;
const REFILL_PER_SEC: f64 = CAPACITY / 10.0;

/// A leaky token bucket over wall-clock time.
pub(crate) struct DialogRate {
    tokens: f64,
    last: Instant,
}

impl DialogRate {
    pub(crate) fn new() -> Self {
        Self {
            tokens: CAPACITY,
            last: Instant::now(),
        }
    }

    /// Refills by the elapsed time, then consumes one token. Returns `false`
    /// when the bucket is empty — the caller must suspend the run.
    pub(crate) fn try_consume(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last).as_secs_f64();
        self.last = now;
        self.tokens = (self.tokens + elapsed * REFILL_PER_SEC).min(CAPACITY);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_the_initial_burst_then_suspends() {
        let mut r = DialogRate::new();
        // The five back-to-back dialogs of a full bucket are allowed; the sixth,
        // arriving before any meaningful refill, is refused (suspend).
        for _ in 0..CAPACITY as u32 {
            assert!(r.try_consume(), "the initial burst is allowed");
        }
        assert!(!r.try_consume(), "a storm past the bucket is refused");
    }
}
