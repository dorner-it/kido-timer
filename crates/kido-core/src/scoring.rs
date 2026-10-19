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
//! - Best valid run per team counts (lowest total compared at 1/100 s;
//!   tie → lower `run_number`).
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
//! - avg age = arithmetic mean over the run's lineup. Lineup ids that are
//!   not in the team's roster are ignored (duplicates count once). If the
//!   lineup is empty (or none of its ids is in the roster), fall back to
//!   the full team roster and set `lineup_fallback`. If that is empty
//!   too, the team is left out.
//! - factor = 1 − 2·(18 − avg)/100   (e.g. avg 12 → 0.88, avg 18 → 1.0).
//! - GW-Zeit = total × factor, rounded to 1/100 s; ranked 1, 1, 3.
//! - Teams without a valid run are left out.
//! - Order: place, then start_number.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::penalty::{PenaltyKind, PenaltyStatus, SZ_PENALTY_MS};
use crate::timing::round_to_centis;
use crate::wg::{Wertungsgruppe, age_on_day};

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
    /// Measured time: the light-barrier time (`Run.original_time_ms`) or,
    /// with `time_source = handstopp`, the hand-time mean stored there.
    /// Never add `Run.correction_ms` (legacy display value = sum of
    /// approved `delta_ms`; SZ are added by scoring itself). `None` = no
    /// time.
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
///
/// `provisional` is set when any counting error (SZ or FV) is still
/// `Pending`. If both a counting FV and a missing time apply, the reason
/// is [`InvalidReason::Fehlversuch`]. A total above `u32::MAX` saturates.
pub fn score_run(run: &RunInput) -> Option<RunResult> {
    if !run.completed {
        return None;
    }
    let counting = || run.penalties.iter().filter(|p| p.status.counts());
    let sz_count = counting().filter(|p| p.kind == PenaltyKind::Sz).count() as u32;
    let fv_count = counting().filter(|p| p.kind == PenaltyKind::Fv).count() as u32;
    let provisional = counting().any(|p| p.status == PenaltyStatus::Pending);

    let invalid_reason = if fv_count > 0 {
        Some(InvalidReason::Fehlversuch)
    } else if run.measured_ms.is_none() {
        Some(InvalidReason::NoTime)
    } else {
        None
    };
    let total_ms = match (invalid_reason, run.measured_ms) {
        (None, Some(measured)) => {
            let total = u64::from(measured) + u64::from(sz_count) * u64::from(SZ_PENALTY_MS);
            Some(u32::try_from(total).unwrap_or(u32::MAX))
        }
        _ => None,
    };

    Some(RunResult {
        run_id: run.run_id,
        team_id: run.team_id,
        run_number: run.run_number,
        valid: invalid_reason.is_none(),
        invalid_reason,
        total_ms,
        sz_count,
        fv_count,
        provisional,
    })
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
///
/// Details beyond the module rules:
/// - Runs whose `team_id` is not in `teams` are ignored.
/// - Best run selection compares totals at 1/100 s (like the ranking);
///   equal → lower `run_number`.
/// - Tagessieger ages: lineup ids are resolved against the team's roster
///   (`TeamInput::runners`); ids **not in the roster are ignored**, and
///   duplicate ids count once. If no lineup id resolves (empty lineup or
///   only unknown ids) the full roster is used and `lineup_fallback` is
///   set.
/// - GW-Zeit is computed exactly in integer arithmetic:
///   `gw = total × (64·n + 2·Σages) / (100·n)` (= total × factor), rounded
///   half up to 10 ms. `avg_age` and `factor` are `f64` for display only.
/// - `TagessiegerRow::provisional` = the team's ranking row is
///   provisional (any of its runs), since a pending error on another run
///   could still change which run counts.
pub fn compute_results(
    teams: &[TeamInput],
    runs: &[RunInput],
    competition_day: NaiveDate,
) -> Results {
    let mut wg_rankings = Vec::new();
    let mut candidates: Vec<TagessiegerCandidate<'_>> = Vec::new();

    for wg in Wertungsgruppe::ALL {
        let wg_teams: Vec<&TeamInput> = teams.iter().filter(|t| t.wg == wg).collect();
        if wg_teams.is_empty() {
            continue;
        }

        let mut ranked: Vec<RankRow> = Vec::new();
        let mut all_invalid: Vec<RankRow> = Vec::new();
        let mut not_started: Vec<RankRow> = Vec::new();

        for team in wg_teams {
            let mut team_runs: Vec<RunResult> = runs
                .iter()
                .filter(|r| r.team_id == team.team_id)
                .filter_map(score_run)
                .collect();
            team_runs.sort_by_key(|r| (r.run_number, r.run_id));
            let provisional = team_runs.iter().any(|r| r.provisional);
            let best = best_run(&team_runs).map(|r| (r.run_id, r.total_ms.unwrap_or(0)));

            let mut row = RankRow {
                team_id: team.team_id,
                start_number: team.start_number,
                place: None,
                status: RankStatus::NotStarted,
                best_run_id: None,
                best_total_ms: None,
                runs: team_runs,
                provisional,
            };
            if let Some((run_id, total_ms)) = best {
                row.status = RankStatus::Ranked;
                row.best_run_id = Some(run_id);
                row.best_total_ms = Some(total_ms);
                ranked.push(row);
                let lineup = runs
                    .iter()
                    .find(|r| r.run_id == run_id && r.team_id == team.team_id)
                    .map(|r| r.lineup.as_slice())
                    .unwrap_or(&[]);
                candidates.push(TagessiegerCandidate {
                    team,
                    run_id,
                    total_ms,
                    lineup,
                    provisional,
                });
            } else if !row.runs.is_empty() {
                row.status = RankStatus::AllInvalid;
                all_invalid.push(row);
            } else {
                not_started.push(row);
            }
        }

        ranked.sort_by_key(|r| (rank_key(r.best_total_ms.unwrap_or(0)), r.start_number));
        let keys: Vec<u64> = ranked
            .iter()
            .map(|r| rank_key(r.best_total_ms.unwrap_or(0)))
            .collect();
        for (row, place) in ranked.iter_mut().zip(competition_ranks(&keys)) {
            row.place = Some(place);
        }
        let last_place = ranked.len() as u32 + 1;
        all_invalid.sort_by_key(|r| r.start_number);
        for row in &mut all_invalid {
            row.place = Some(last_place);
        }
        not_started.sort_by_key(|r| r.start_number);

        let mut rows = ranked;
        rows.extend(all_invalid);
        rows.extend(not_started);
        wg_rankings.push(WgRanking { wg, rows });
    }

    let tagessieger = compute_tagessieger(&candidates, competition_day);
    Results {
        wg_rankings,
        tagessieger,
    }
}

/// A ranked team's counting run, input to the Tagessieger list.
struct TagessiegerCandidate<'a> {
    team: &'a TeamInput,
    run_id: Uuid,
    total_ms: u32,
    lineup: &'a [Uuid],
    provisional: bool,
}

/// Comparison key of a total: rounded to 1/100 s.
fn rank_key(total_ms: u32) -> u64 {
    round_to_centis(u64::from(total_ms))
}

/// Best valid run: lowest total compared at 1/100 s (same resolution as
/// the ranking), tie → lower `run_number`.
fn best_run(runs: &[RunResult]) -> Option<&RunResult> {
    runs.iter()
        .filter(|r| r.valid)
        .filter_map(|r| r.total_ms.map(|t| (r, t)))
        .min_by_key(|&(r, t)| (rank_key(t), r.run_number))
        .map(|(r, _)| r)
}

/// Birth dates used for a run's average age: the lineup resolved against
/// the roster (unknown ids ignored, duplicates once) or, if that yields
/// nobody, the whole roster. Returns `(birth_dates, lineup_fallback)`.
fn tagessieger_birth_dates(team: &TeamInput, lineup: &[Uuid]) -> (Vec<NaiveDate>, bool) {
    let mut seen: Vec<Uuid> = Vec::new();
    let mut dates = Vec::new();
    for id in lineup {
        if seen.contains(id) {
            continue;
        }
        if let Some(runner) = team.runners.iter().find(|r| r.runner_id == *id) {
            seen.push(*id);
            dates.push(runner.birth_date);
        }
    }
    if dates.is_empty() {
        (team.runners.iter().map(|r| r.birth_date).collect(), true)
    } else {
        (dates, false)
    }
}

/// Exact GW-Zeit in ms, rounded half up to 10 ms:
/// `total × (64·n + 2·Σages) / (100·n)`, i.e. `total × factor`.
/// Clamped to `0..=u32::MAX` (rounded down to a multiple of 10).
fn gw_time_ms(total_ms: u32, ages: &[i32]) -> u32 {
    let n = ages.len() as i128;
    debug_assert!(n > 0);
    let sum: i128 = ages.iter().map(|&a| i128::from(a)).sum();
    let num = i128::from(total_ms) * (64 * n + 2 * sum);
    // centis = num / (100·n) / 10, rounded half up.
    let den = 1000 * n;
    let centis = if num <= 0 {
        0
    } else {
        (2 * num + den) / (2 * den)
    };
    let max_centis = i128::from(u32::MAX / 10);
    (centis.min(max_centis) * 10) as u32
}

fn compute_tagessieger(
    candidates: &[TagessiegerCandidate<'_>],
    competition_day: NaiveDate,
) -> Vec<TagessiegerRow> {
    let mut rows: Vec<TagessiegerRow> = candidates
        .iter()
        .filter_map(|c| {
            let (dates, lineup_fallback) = tagessieger_birth_dates(c.team, c.lineup);
            if dates.is_empty() {
                return None;
            }
            let ages: Vec<i32> = dates
                .iter()
                .map(|&b| age_on_day(b, competition_day))
                .collect();
            let avg_age = ages.iter().map(|&a| f64::from(a)).sum::<f64>() / ages.len() as f64;
            let gw_ms = gw_time_ms(c.total_ms, &ages);
            Some(TagessiegerRow {
                team_id: c.team.team_id,
                start_number: c.team.start_number,
                wg: c.team.wg,
                run_id: c.run_id,
                total_ms: c.total_ms,
                ages,
                avg_age,
                factor: tagessieger_factor(avg_age),
                gw_ms,
                place: 0,
                lineup_fallback,
                provisional: c.provisional,
            })
        })
        .collect();
    rows.sort_by_key(|r| (r.gw_ms, r.start_number));
    let keys: Vec<u32> = rows.iter().map(|r| r.gw_ms).collect();
    for (row, place) in rows.iter_mut().zip(competition_ranks(&keys)) {
        row.place = place;
    }
    rows
}

/// Factor = 1 − 2·(18 − avg_age)/100.
pub fn tagessieger_factor(avg_age: f64) -> f64 {
    1.0 - 2.0 * (18.0 - avg_age) / 100.0
}

/// Standard competition ranking ("1, 1, 3") over already-sorted keys:
/// returns the place for each position. Keys must be ascending.
///
/// Example: `[10, 10, 20]` → `[1, 1, 3]`; `[10, 20, 20, 30]` → `[1, 2, 2, 4]`.
pub fn competition_ranks<K: PartialEq>(sorted_keys: &[K]) -> Vec<u32> {
    let mut places = Vec::with_capacity(sorted_keys.len());
    for (i, key) in sorted_keys.iter().enumerate() {
        let place = if i > 0 && sorted_keys[i - 1] == *key {
            places[i - 1]
        } else {
            i as u32 + 1
        };
        places.push(place);
    }
    places
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gw_time_matches_factor() {
        assert_eq!(gw_time_ms(50_000, &[12]), 44_000);
        assert_eq!(gw_time_ms(50_000, &[18, 18]), 50_000);
        assert_eq!(gw_time_ms(60_000, &[10, 11]), 51_000);
        assert_eq!(gw_time_ms(0, &[12]), 0);
    }

    #[test]
    fn gw_time_clamps() {
        // Factor <= 0 (absurd ages) → 0.
        assert_eq!(gw_time_ms(50_000, &[-32]), 0);
        assert_eq!(gw_time_ms(50_000, &[-40]), 0);
        // Factor > 1 with a huge total saturates at a multiple of 10.
        assert_eq!(gw_time_ms(u32::MAX, &[30]), u32::MAX / 10 * 10);
    }

    #[test]
    fn best_run_none_without_valid_runs() {
        let r = RunResult {
            run_id: Uuid::nil(),
            team_id: Uuid::nil(),
            run_number: 1,
            valid: false,
            invalid_reason: Some(InvalidReason::NoTime),
            total_ms: None,
            sz_count: 0,
            fv_count: 0,
            provisional: false,
        };
        assert!(best_run(&[]).is_none());
        assert!(best_run(&[r]).is_none());
    }
}
