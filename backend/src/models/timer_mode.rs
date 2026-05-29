// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use serde::{Deserialize, Serialize};

/// Discipline-specific timer modes for fire-department running disciplines.
///
/// Replaces the previous generic v1 enum (`single_lane`, `two_lane_parallel`,
/// `relay`, `individual`) with the two disciplines actually run on TRV Kocab
/// hardware in the field. The lane count is intrinsic to the discipline and
/// drives UI (lane pickers, deciders) and import validation.
///
/// Both the web app and the standalone desktop app mirror this list verbatim
/// (`schema_version = 3`). Adding a variant requires a coordinated release.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimerMode {
    /// Gruppenstaffette — 2 lanes, group-vs-group relay format.
    Gruppenstaffette,
    /// Löschangriff — 4 lanes, classic fire-attack discipline.
    Loeschangriff,
}

impl TimerMode {
    /// Number of timer lanes the discipline occupies on the TRV Kocab
    /// hardware. Drives lane-picker visibility in the organiser UI and
    /// validation of `TeamEntry.lane` on the backend.
    pub const fn lane_count(self) -> u8 {
        match self {
            TimerMode::Gruppenstaffette => 2,
            TimerMode::Loeschangriff => 4,
        }
    }
}

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
    fn lane_counts() {
        assert_eq!(TimerMode::Gruppenstaffette.lane_count(), 2);
        assert_eq!(TimerMode::Loeschangriff.lane_count(), 4);
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
}
