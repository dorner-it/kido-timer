// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

//! Discipline-level settings: timer mode, lane count, judge mode, sync mode.

use serde::{Deserialize, Serialize};

/// Discipline run on the TRV Kocab hardware. Wire values are stable
/// (`schema_version >= 3`); legacy v1 strings (`single_lane`, …) must be
/// rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimerMode {
    /// Gruppenstafette — 1 or 2 lanes (configurable per discipline).
    Gruppenstaffette,
    /// Löschangriff — 4 lanes (2028, out of scope for scoring).
    Loeschangriff,
}

impl TimerMode {
    /// Maximum number of lanes this discipline can occupy.
    /// Gruppenstaffette = 2, Loeschangriff = 4.
    pub const fn max_lanes(self) -> u8 {
        match self {
            TimerMode::Gruppenstaffette => 2,
            TimerMode::Loeschangriff => 4,
        }
    }

    /// Lane counts the organizer may choose. Gruppenstaffette: `[1, 2]`.
    /// Loeschangriff: `[4]`.
    pub fn allowed_lane_counts(self) -> &'static [u8] {
        match self {
            TimerMode::Gruppenstaffette => &[1, 2],
            TimerMode::Loeschangriff => &[4],
        }
    }

    /// Whether `lane_count` is one of [`Self::allowed_lane_counts`].
    pub fn is_allowed_lane_count(self, lane_count: u8) -> bool {
        self.allowed_lane_counts().contains(&lane_count)
    }

    /// Default lane count for new disciplines (= `max_lanes`).
    pub const fn default_lane_count(self) -> u8 {
        self.max_lanes()
    }
}

/// How Strafrichter are deployed when more than one lane runs in parallel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JudgeMode {
    /// One judge per station watches all lanes and picks the lane/team
    /// for every error (with a single lane the pick is skipped).
    #[default]
    PerStation,
    /// One judge per station *and* lane; the judge only sees the team on
    /// their lane.
    PerStationAndLane,
}

/// Where the competition day is operated.
///
/// The web sets a default per discipline; the desktop can override it on
/// the day (e.g. internet failure). Wire value `offline` is kept for
/// compatibility and means **on-prem**: the desktop app runs a local
/// server in the venue network for judges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncMode {
    #[default]
    Live,
    #[serde(alias = "on_prem")]
    Offline,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ser<T: Serialize>(v: T) -> String {
        serde_json::to_string(&v).unwrap()
    }

    #[test]
    fn timer_mode_wire_values() {
        assert_eq!(ser(TimerMode::Gruppenstaffette), "\"gruppenstaffette\"");
        assert_eq!(ser(TimerMode::Loeschangriff), "\"loeschangriff\"");
        for m in [TimerMode::Gruppenstaffette, TimerMode::Loeschangriff] {
            let back: TimerMode = serde_json::from_str(&ser(m)).unwrap();
            assert_eq!(back, m);
        }
    }

    #[test]
    fn timer_mode_rejects_legacy_v1_strings() {
        for legacy in ["single_lane", "two_lane_parallel", "relay", "individual"] {
            let json = format!("\"{legacy}\"");
            assert!(
                serde_json::from_str::<TimerMode>(&json).is_err(),
                "{legacy} must be rejected"
            );
        }
        assert!(serde_json::from_str::<TimerMode>("\"Gruppenstaffette\"").is_err());
    }

    #[test]
    fn lane_counts() {
        assert_eq!(TimerMode::Gruppenstaffette.max_lanes(), 2);
        assert_eq!(TimerMode::Loeschangriff.max_lanes(), 4);
        assert_eq!(TimerMode::Gruppenstaffette.allowed_lane_counts(), &[1, 2]);
        assert_eq!(TimerMode::Loeschangriff.allowed_lane_counts(), &[4]);
        assert_eq!(TimerMode::Gruppenstaffette.default_lane_count(), 2);
        assert_eq!(TimerMode::Loeschangriff.default_lane_count(), 4);
        for m in [TimerMode::Gruppenstaffette, TimerMode::Loeschangriff] {
            assert_eq!(m.default_lane_count(), m.max_lanes());
            assert!(m.allowed_lane_counts().contains(&m.default_lane_count()));
            assert!(m.allowed_lane_counts().iter().all(|&c| c <= m.max_lanes()));
        }
    }

    #[test]
    fn is_allowed_lane_count() {
        let gs = TimerMode::Gruppenstaffette;
        assert!(!gs.is_allowed_lane_count(0));
        assert!(gs.is_allowed_lane_count(1));
        assert!(gs.is_allowed_lane_count(2));
        assert!(!gs.is_allowed_lane_count(3));
        assert!(!gs.is_allowed_lane_count(4));
        let la = TimerMode::Loeschangriff;
        assert!(!la.is_allowed_lane_count(1));
        assert!(!la.is_allowed_lane_count(2));
        assert!(la.is_allowed_lane_count(4));
    }

    #[test]
    fn judge_mode_wire_values_and_default() {
        assert_eq!(JudgeMode::default(), JudgeMode::PerStation);
        assert_eq!(ser(JudgeMode::PerStation), "\"per_station\"");
        assert_eq!(
            ser(JudgeMode::PerStationAndLane),
            "\"per_station_and_lane\""
        );
        let m: JudgeMode = serde_json::from_str("\"per_station_and_lane\"").unwrap();
        assert_eq!(m, JudgeMode::PerStationAndLane);
        assert!(serde_json::from_str::<JudgeMode>("\"per_lane\"").is_err());
    }

    #[test]
    fn sync_mode_wire_values_and_alias() {
        assert_eq!(SyncMode::default(), SyncMode::Live);
        assert_eq!(ser(SyncMode::Live), "\"live\"");
        assert_eq!(ser(SyncMode::Offline), "\"offline\"");
        let a: SyncMode = serde_json::from_str("\"offline\"").unwrap();
        let b: SyncMode = serde_json::from_str("\"on_prem\"").unwrap();
        assert_eq!(a, SyncMode::Offline);
        assert_eq!(b, SyncMode::Offline);
        // Alias is input-only; output stays canonical.
        assert_eq!(ser(b), "\"offline\"");
        assert!(serde_json::from_str::<SyncMode>("\"cloud\"").is_err());
    }
}
