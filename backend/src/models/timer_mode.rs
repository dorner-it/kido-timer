// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

//! Discipline-specific timer modes. The type lives in the shared
//! `kido-core` crate (used by backend + desktop); this module re-exports
//! it so existing call sites (`crate::models::timer_mode::TimerMode`)
//! keep compiling.
//!
//! Wire values (`schema_version >= 3`): `gruppenstaffette`,
//! `loeschangriff`. The legacy v1 strings (`single_lane`,
//! `two_lane_parallel`, `relay`, `individual`) are rejected.

pub use kido_core::discipline::{JudgeMode, TimerMode};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&TimerMode::Gruppenstaffette).unwrap(),
            "\"gruppenstaffette\""
        );
        assert_eq!(
            serde_json::to_string(&TimerMode::Loeschangriff).unwrap(),
            "\"loeschangriff\""
        );
    }

    #[test]
    fn round_trip() {
        for mode in [TimerMode::Gruppenstaffette, TimerMode::Loeschangriff] {
            let json = serde_json::to_string(&mode).unwrap();
            let parsed: TimerMode = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, mode);
        }
    }

    #[test]
    fn rejects_legacy_variants() {
        for legacy in [
            "\"single_lane\"",
            "\"two_lane_parallel\"",
            "\"relay\"",
            "\"individual\"",
        ] {
            assert!(
                serde_json::from_str::<TimerMode>(legacy).is_err(),
                "legacy variant {legacy} must be rejected"
            );
        }
    }

    #[test]
    fn judge_mode_wire_values_and_default() {
        assert_eq!(JudgeMode::default(), JudgeMode::PerStation);
        assert_eq!(
            serde_json::to_string(&JudgeMode::PerStation).unwrap(),
            "\"per_station\""
        );
        assert_eq!(
            serde_json::to_string(&JudgeMode::PerStationAndLane).unwrap(),
            "\"per_station_and_lane\""
        );
        assert!(serde_json::from_str::<JudgeMode>("\"per_lane\"").is_err());
    }

    // --- behaviour owned by kido-core (panics with `todo!()` until the
    // kido-core bodies are merged) ---

    #[test]
    fn max_lanes_per_mode() {
        assert_eq!(TimerMode::Gruppenstaffette.max_lanes(), 2);
        assert_eq!(TimerMode::Loeschangriff.max_lanes(), 4);
    }

    #[test]
    fn allowed_and_default_lane_counts() {
        assert_eq!(TimerMode::Gruppenstaffette.allowed_lane_counts(), &[1, 2]);
        assert_eq!(TimerMode::Loeschangriff.allowed_lane_counts(), &[4]);
        assert_eq!(TimerMode::Gruppenstaffette.default_lane_count(), 2);
        assert_eq!(TimerMode::Loeschangriff.default_lane_count(), 4);
    }
}
