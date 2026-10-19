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
        todo!()
    }

    /// Lane counts the organizer may choose. Gruppenstaffette: `[1, 2]`.
    /// Loeschangriff: `[4]`.
    pub fn allowed_lane_counts(self) -> &'static [u8] {
        todo!()
    }

    /// Default lane count for new disciplines (= `max_lanes`).
    pub const fn default_lane_count(self) -> u8 {
        todo!()
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
