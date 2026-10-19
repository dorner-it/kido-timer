// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

//! Time source and hand-time fallback.

use serde::{Deserialize, Serialize};

/// Where a run's time came from. Hand times (two stopwatches) are only
/// used when the light barrier fails, e.g. power outage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeSource {
    #[default]
    Lichtschranke,
    Handstopp,
}

/// Mean of the two hand times, rounded to 1/100 s (10 ms), half up.
/// Example: 47_230 and 47_245 → 47_240 (47_237.5 → 47_240).
pub fn hand_time_mean(t1_ms: u32, t2_ms: u32) -> u32 {
    todo!()
}

/// Round a millisecond value to 1/100 s (half up). Used for comparisons
/// ("compared at 1/100 s") and display.
pub fn round_to_centis(ms: u64) -> u64 {
    todo!()
}
