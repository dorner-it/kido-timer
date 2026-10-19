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
///
/// Computed in `u64` (no overflow); a result above `u32::MAX` (only
/// possible for absurd inputs) saturates.
pub fn hand_time_mean(t1_ms: u32, t2_ms: u32) -> u32 {
    let sum = u64::from(t1_ms) + u64::from(t2_ms);
    // mean / 10 ms = sum / 20; half up → (sum + 10) / 20.
    let centis = (sum + 10) / 20;
    u32::try_from(centis * 10).unwrap_or(u32::MAX)
}

/// Round a millisecond value to 1/100 s (half up). Used for comparisons
/// ("compared at 1/100 s") and display.
///
/// Examples: 47_234 → 47_230, 47_235 → 47_240, 47_239 → 47_240.
/// Saturates at the largest multiple of 10 that fits in `u64`.
pub fn round_to_centis(ms: u64) -> u64 {
    let centis = ms / 10 + u64::from(ms % 10 >= 5);
    centis.checked_mul(10).unwrap_or(u64::MAX / 10 * 10)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_source_wire_values() {
        assert_eq!(TimeSource::default(), TimeSource::Lichtschranke);
        assert_eq!(
            serde_json::to_string(&TimeSource::Lichtschranke).unwrap(),
            "\"lichtschranke\""
        );
        assert_eq!(
            serde_json::to_string(&TimeSource::Handstopp).unwrap(),
            "\"handstopp\""
        );
        let h: TimeSource = serde_json::from_str("\"handstopp\"").unwrap();
        assert_eq!(h, TimeSource::Handstopp);
        assert!(serde_json::from_str::<TimeSource>("\"hand\"").is_err());
    }

    #[test]
    fn hand_time_mean_doc_example() {
        assert_eq!(hand_time_mean(47_230, 47_245), 47_240);
        assert_eq!(hand_time_mean(47_245, 47_230), 47_240);
    }

    #[test]
    fn hand_time_mean_cases() {
        // equal times
        assert_eq!(hand_time_mean(47_230, 47_230), 47_230);
        assert_eq!(hand_time_mean(47_234, 47_234), 47_230);
        assert_eq!(hand_time_mean(47_235, 47_235), 47_240);
        // odd sum: mean x.5 ms
        assert_eq!(hand_time_mean(47_230, 47_239), 47_230); // 47_234.5
        assert_eq!(hand_time_mean(47_230, 47_241), 47_240); // 47_235.5
        assert_eq!(hand_time_mean(47_230, 47_240), 47_240); // 47_235 exactly → up
        assert_eq!(hand_time_mean(47_230, 47_238), 47_230); // 47_234
        assert_eq!(hand_time_mean(0, 0), 0);
        assert_eq!(hand_time_mean(0, 9), 0); // 4.5
        assert_eq!(hand_time_mean(0, 10), 10); // 5
        assert_eq!(hand_time_mean(1, 0), 0); // 0.5
    }

    #[test]
    fn hand_time_mean_large_values_no_overflow() {
        let big = 4_000_000_000u32;
        assert_eq!(hand_time_mean(big, big), big);
        assert_eq!(hand_time_mean(big, big + 10), big + 10); // big+5 → up
        assert_eq!(hand_time_mean(u32::MAX - 5, u32::MAX - 5), 4_294_967_290);
        // Would round above u32::MAX → saturates.
        assert_eq!(hand_time_mean(u32::MAX, u32::MAX), u32::MAX);
    }

    #[test]
    fn round_to_centis_cases() {
        assert_eq!(round_to_centis(0), 0);
        assert_eq!(round_to_centis(4), 0);
        assert_eq!(round_to_centis(5), 10);
        assert_eq!(round_to_centis(9), 10);
        assert_eq!(round_to_centis(10), 10);
        assert_eq!(round_to_centis(47_230), 47_230);
        assert_eq!(round_to_centis(47_234), 47_230);
        assert_eq!(round_to_centis(47_235), 47_240);
        assert_eq!(round_to_centis(47_239), 47_240);
        assert_eq!(round_to_centis(99_995), 100_000);
        assert_eq!(round_to_centis(u64::from(u32::MAX)), 4_294_967_300);
        assert_eq!(round_to_centis(u64::MAX), u64::MAX / 10 * 10);
        assert_eq!(round_to_centis(u64::MAX - 9), u64::MAX / 10 * 10);
    }
}
