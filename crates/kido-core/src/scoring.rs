// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

//! Run results, WG rankings and the Tagessieger (Gruppenstafette 2027).
//!
//! Rules (concept §5, Mirko's answers Q6/Q9/Q13, user decision 2026-10-16):
//!
//! **Run result**
//! - Only runs with `completed == true` are scored; others are ignored.
//! - Counting errors = status `Pending` or `Approved` (`Rejected` ignored).
//!   Any counting `Pending` error marks the result `provisional`.
//! - valid = no counting FV. Invalid runs have no time.
//! - total = measured_ms + 15 s × counting SZ.
//! - A completed run without `measured_ms` is invalid (`no_time`).
//!
//! **WG ranking** (one per WG present in the input)
//! - Best valid run per team counts (lowest total; tie → lower
//!   `run_number`).
//! - Places: standard competition ranking 1, 1, 3 comparing totals
//!   rounded to 1/100 s.
//! - Teams with completed runs but none valid share **last place** =
//!   number of ranked teams + 1, listed after the ranked teams.
//! - Teams without any completed run: `NotStarted`, no place, listed last.
//! - Order within equal places / groups: start_number ascending.
//!
//! **Tagessieger** (across all WGs, one per discipline)
//! - Variant (a): the team's **best-time run** (same run as in the WG
//!   ranking) with that run's lineup.
//! - Age per runner = completed years on the competition day
//!   ([`crate::wg::age_on_day`]).
//! - avg age = arithmetic mean over the run's lineup. If the lineup is
//!   empty, fall back to the full team roster and set `lineup_fallback`.
//!   If that is empty too, the team is left out.
//! - factor = 1 − 2·(18 − avg)/100   (e.g. avg 12 → 0.88, avg 18 → 1.0).
//! - GW-Zeit = total × factor, rounded to 1/100 s; ranked 1, 1, 3.
//! - Teams without a valid run are left out.
//! - Order: place, then start_number.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::penalty::{PenaltyKind, PenaltyStatus};
use crate::wg::Wertungsgruppe;

/// Minimal penalty view needed for scoring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PenaltyInput {
    pub kind: PenaltyKind,
    pub status: PenaltyStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunInput {
    pub run_id: Uuid,
    pub team_id: Uuid,
    pub run_number: u32,
    /// Run is finished (status `completed`). Pending/active/cancelled
    /// runs are passed with `false` and ignored.
    pub completed: bool,
    /// Measured time incl. operator correction (original + correction),
    /// or the hand-time mean. `None` = no time.
    pub measured_ms: Option<u32>,
    pub penalties: Vec<PenaltyInput>,
    /// Runner ids who actually ran (from the Startlinie judge).
    pub lineup: Vec<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunnerInput {
    pub runner_id: Uuid,
    pub birth_date: NaiveDate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamInput {
    pub team_id: Uuid,
    pub start_number: u32,
    pub wg: Wertungsgruppe,
    /// Full roster incl. substitutes.
    pub runners: Vec<RunnerInput>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvalidReason {
    /// At least one counting FV.
    Fehlversuch,
    /// Completed but no time recorded.
    NoTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunResult {
    pub run_id: Uuid,
    pub team_id: Uuid,
    pub run_number: u32,
    pub valid: bool,
    pub invalid_reason: Option<InvalidReason>,
    /// measured + SZ penalties; `None` when invalid.
    pub total_ms: Option<u32>,
    pub sz_count: u32,
    pub fv_count: u32,
    pub provisional: bool,
}

/// Score one run. Returns `None` for runs that are not completed.
pub fn score_run(run: &RunInput) -> Option<RunResult> {
    todo!()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RankStatus {
    Ranked,
    /// Started, but every completed run is invalid → shared last place.
    AllInvalid,
    /// No completed run → "n.a.", no place.
    NotStarted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RankRow {
    pub team_id: Uuid,
    pub start_number: u32,
    pub place: Option<u32>,
    pub status: RankStatus,
    /// The counting run (best valid run) — `None` unless `Ranked`.
    pub best_run_id: Option<Uuid>,
    pub best_total_ms: Option<u32>,
    /// All scored (completed) runs of the team, by run_number.
    pub runs: Vec<RunResult>,
    /// Any run of the team is provisional.
    pub provisional: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WgRanking {
    pub wg: Wertungsgruppe,
    pub rows: Vec<RankRow>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TagessiegerRow {
    pub team_id: Uuid,
    pub start_number: u32,
    pub wg: Wertungsgruppe,
    pub run_id: Uuid,
    pub total_ms: u32,
    /// Ages (completed years on the competition day) of the runners used.
    pub ages: Vec<i32>,
    pub avg_age: f64,
    pub factor: f64,
    /// total × factor, rounded to 1/100 s (in ms, multiple of 10).
    pub gw_ms: u32,
    pub place: u32,
    pub lineup_fallback: bool,
    pub provisional: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Results {
    /// Sorted by WG (Wg1 first); only WGs with at least one team.
    pub wg_rankings: Vec<WgRanking>,
    pub tagessieger: Vec<TagessiegerRow>,
}

/// Compute WG rankings and the Tagessieger list.
/// `competition_day` is the discipline's event date.
pub fn compute_results(
    teams: &[TeamInput],
    runs: &[RunInput],
    competition_day: NaiveDate,
) -> Results {
    todo!()
}

/// Factor = 1 − 2·(18 − avg_age)/100.
pub fn tagessieger_factor(avg_age: f64) -> f64 {
    todo!()
}

/// Standard competition ranking ("1, 1, 3") over already-sorted keys:
/// returns the place for each position. Keys must be ascending.
pub fn competition_ranks<K: PartialEq>(sorted_keys: &[K]) -> Vec<u32> {
    todo!()
}
