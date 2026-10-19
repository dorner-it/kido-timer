// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

//! Results of one discipline (Gruppenstafette 2027, concept §5).
//!
//! The scoring rules live in `kido_core::scoring` — this module only
//! maps stored rows onto its inputs ([`build_scoring_input`]) and enriches
//! its output for display ([`build_results_response`]). Both are pure so
//! they are tested without DynamoDB; the HTTP handler in
//! `routes::results` just loads the rows and calls them.
//!
//! Mapping rules (docs/contract-pr1-2027.md):
//! - `completed = run.status == completed`; every other status is passed
//!   with `completed = false` and therefore ignored by scoring.
//! - `measured_ms = run.original_time_ms` — with hand times the mean is
//!   already stored there. `Run.correction_ms` is a legacy display value
//!   and is **never** added (scoring adds 15 s per counting SZ itself).
//! - penalties: kind + status of every penalty of the run, all statuses
//!   (`rejected` ones are passed through; scoring ignores them).
//! - teams: one per entry of the discipline with a `wg`; entries without
//!   a WG (legacy rows) are excluded and reported in `missing_wg`.
//! - roster: the team's runners (event-level), lineup: `run.lineup`.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, NaiveDate, Utc};
use kido_core::scoring::{
    self, InvalidReason, PenaltyInput, RankStatus, RunInput, RunnerInput, TeamInput,
};
use serde::Serialize;
use uuid::Uuid;

use crate::models::{
    Discipline, Event, Penalty, Run, RunStatus, Runner, Team, TeamEntry, TimeSource, TimerMode,
    Wertungsgruppe,
};

/// Display name used when an entry references a team row that no longer
/// exists.
pub const UNKNOWN_TEAM_NAME: &str = "Unbekannte Mannschaft";

/// Inputs for [`kido_core::scoring::compute_results`] plus the entries
/// that had to be left out.
#[derive(Debug, Clone, PartialEq)]
pub struct ScoringInput {
    pub teams: Vec<TeamInput>,
    pub runs: Vec<RunInput>,
    /// Competition day = event date (Tagessieger ages).
    pub competition_day: NaiveDate,
    /// Team ids of entries without a WG, in entry order (start number).
    pub missing_wg: Vec<Uuid>,
}

/// Map stored rows of one discipline onto the scoring inputs.
///
/// Only entries, runs and penalties of `discipline_id` are used, so the
/// caller may pass event-wide lists. A team with more than one entry in
/// the discipline (should not happen) is taken once, lowest start number
/// first.
pub fn build_scoring_input(
    event: &Event,
    discipline_id: Uuid,
    entries: &[TeamEntry],
    runners: &[Runner],
    runs: &[Run],
    penalties: &[Penalty],
) -> ScoringInput {
    let mut entries: Vec<&TeamEntry> = entries
        .iter()
        .filter(|e| e.discipline_id == discipline_id)
        .collect();
    entries.sort_by_key(|e| (e.start_number, e.team_id));

    let mut seen: HashSet<Uuid> = HashSet::new();
    let mut teams = Vec::new();
    let mut missing_wg = Vec::new();
    for entry in entries {
        if !seen.insert(entry.team_id) {
            continue;
        }
        let Some(wg) = entry.wg else {
            missing_wg.push(entry.team_id);
            continue;
        };
        let mut roster: Vec<&Runner> = runners
            .iter()
            .filter(|r| r.team_id == entry.team_id)
            .collect();
        roster.sort_by_key(|r| (r.position, r.id));
        teams.push(TeamInput {
            team_id: entry.team_id,
            start_number: entry.start_number,
            wg,
            runners: roster
                .into_iter()
                .map(|r| RunnerInput {
                    runner_id: r.id,
                    birth_date: r.birth_date,
                })
                .collect(),
        });
    }

    let runs = runs
        .iter()
        .filter(|r| r.discipline_id == discipline_id)
        .map(|run| RunInput {
            run_id: run.id,
            team_id: run.team_id,
            run_number: run.run_number,
            completed: run.status == RunStatus::Completed,
            // Never `+ correction_ms` — see module docs.
            measured_ms: run.original_time_ms,
            penalties: penalties
                .iter()
                .filter(|p| p.run_id == run.id)
                .map(|p| PenaltyInput {
                    kind: p.kind,
                    status: p.status,
                })
                .collect(),
            lineup: run.lineup.clone(),
        })
        .collect();

    ScoringInput {
        teams,
        runs,
        competition_day: event.date,
        missing_wg,
    }
}

// ===== response =====

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ResultsResponse {
    pub event: EventHeader,
    pub discipline: DisciplineHeader,
    pub generated_at: DateTime<Utc>,
    /// Any ranking row or Tagessieger row is provisional (a counting
    /// error is still pending).
    pub provisional: bool,
    pub wg_rankings: Vec<WgRankingView>,
    pub tagessieger: Vec<TagessiegerView>,
    /// Entries without a WG — not ranked.
    pub missing_wg: Vec<TeamRef>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EventHeader {
    pub id: Uuid,
    pub name: String,
    pub date: NaiveDate,
    pub location: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DisciplineHeader {
    pub id: Uuid,
    pub name: String,
    pub mode: TimerMode,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TeamRef {
    pub team_id: Uuid,
    pub team_name: String,
    pub start_number: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WgRankingView {
    pub wg: Wertungsgruppe,
    /// `"WG2"`.
    pub wg_label: &'static str,
    /// `"Jungen bis 14"`.
    pub wg_label_de: &'static str,
    pub rows: Vec<RankRowView>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RankRowView {
    pub team_id: Uuid,
    pub team_name: String,
    pub start_number: u32,
    /// `None` for `not_started`; shared last place for `all_invalid`.
    pub place: Option<u32>,
    pub status: RankStatus,
    pub best_run_id: Option<Uuid>,
    pub best_total_ms: Option<u32>,
    pub provisional: bool,
    /// Completed runs only, by run number.
    pub runs: Vec<RunView>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RunView {
    pub run_id: Uuid,
    pub run_number: u32,
    pub lane: Option<u8>,
    pub time_source: TimeSource,
    pub hand_time_1_ms: Option<u32>,
    pub hand_time_2_ms: Option<u32>,
    /// `original_time_ms` (hand-time mean with `handstopp`).
    pub measured_ms: Option<u32>,
    pub sz_count: u32,
    pub fv_count: u32,
    /// measured + 15 s × SZ; `None` when invalid.
    pub total_ms: Option<u32>,
    pub valid: bool,
    pub invalid_reason: Option<InvalidReason>,
    pub provisional: bool,
    /// The team's counting run.
    pub is_best: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TagessiegerView {
    pub place: u32,
    pub team_id: Uuid,
    pub team_name: String,
    pub start_number: u32,
    pub wg: Wertungsgruppe,
    pub wg_label: &'static str,
    pub run_id: Uuid,
    pub run_number: Option<u32>,
    /// Bestzeit (total of the counting run).
    pub total_ms: u32,
    pub ages: Vec<i32>,
    /// Rounded to 1 decimal (display).
    pub avg_age: f64,
    /// Rounded to 4 decimals (display).
    pub factor: f64,
    pub gw_ms: u32,
    /// Ages taken from the full roster because the lineup was empty.
    pub lineup_fallback: bool,
    pub provisional: bool,
}

/// Round `value` to `decimals` places (half away from zero).
pub fn round_to(value: f64, decimals: i32) -> f64 {
    let f = 10f64.powi(decimals);
    (value * f).round() / f
}

/// Compute and enrich the results of one discipline.
#[allow(clippy::too_many_arguments)]
pub fn build_results_response(
    event: &Event,
    discipline: &Discipline,
    entries: &[TeamEntry],
    teams: &[Team],
    runners: &[Runner],
    runs: &[Run],
    penalties: &[Penalty],
    generated_at: DateTime<Utc>,
) -> ResultsResponse {
    let input = build_scoring_input(event, discipline.id, entries, runners, runs, penalties);
    let results = scoring::compute_results(&input.teams, &input.runs, input.competition_day);

    let team_names: HashMap<Uuid, &str> = teams.iter().map(|t| (t.id, t.name.as_str())).collect();
    let name_of = |id: Uuid| -> String {
        team_names
            .get(&id)
            .map(|s| (*s).to_string())
            .unwrap_or_else(|| UNKNOWN_TEAM_NAME.to_string())
    };
    let runs_by_id: HashMap<Uuid, &Run> = runs
        .iter()
        .filter(|r| r.discipline_id == discipline.id)
        .map(|r| (r.id, r))
        .collect();

    let wg_rankings: Vec<WgRankingView> = results
        .wg_rankings
        .into_iter()
        .map(|ranking| WgRankingView {
            wg: ranking.wg,
            wg_label: ranking.wg.label(),
            wg_label_de: ranking.wg.label_de(),
            rows: ranking
                .rows
                .into_iter()
                .map(|row| RankRowView {
                    team_id: row.team_id,
                    team_name: name_of(row.team_id),
                    start_number: row.start_number,
                    place: row.place,
                    status: row.status,
                    best_run_id: row.best_run_id,
                    best_total_ms: row.best_total_ms,
                    provisional: row.provisional,
                    runs: row
                        .runs
                        .iter()
                        .map(|r| {
                            let stored = runs_by_id.get(&r.run_id);
                            RunView {
                                run_id: r.run_id,
                                run_number: r.run_number,
                                lane: stored.and_then(|s| s.lane),
                                time_source: stored.map(|s| s.time_source).unwrap_or_default(),
                                hand_time_1_ms: stored.and_then(|s| s.hand_time_1_ms),
                                hand_time_2_ms: stored.and_then(|s| s.hand_time_2_ms),
                                measured_ms: stored.and_then(|s| s.original_time_ms),
                                sz_count: r.sz_count,
                                fv_count: r.fv_count,
                                total_ms: r.total_ms,
                                valid: r.valid,
                                invalid_reason: r.invalid_reason,
                                provisional: r.provisional,
                                is_best: row.best_run_id == Some(r.run_id),
                            }
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect();

    let tagessieger: Vec<TagessiegerView> = results
        .tagessieger
        .into_iter()
        .map(|t| TagessiegerView {
            place: t.place,
            team_id: t.team_id,
            team_name: name_of(t.team_id),
            start_number: t.start_number,
            wg: t.wg,
            wg_label: t.wg.label(),
            run_id: t.run_id,
            run_number: runs_by_id.get(&t.run_id).map(|r| r.run_number),
            total_ms: t.total_ms,
            ages: t.ages,
            avg_age: round_to(t.avg_age, 1),
            factor: round_to(t.factor, 4),
            gw_ms: t.gw_ms,
            lineup_fallback: t.lineup_fallback,
            provisional: t.provisional,
        })
        .collect();

    let start_numbers: HashMap<Uuid, u32> = entries
        .iter()
        .filter(|e| e.discipline_id == discipline.id)
        .map(|e| (e.team_id, e.start_number))
        .collect();
    let missing_wg = input
        .missing_wg
        .iter()
        .map(|&team_id| TeamRef {
            team_id,
            team_name: name_of(team_id),
            start_number: start_numbers.get(&team_id).copied().unwrap_or(0),
        })
        .collect();

    let provisional = wg_rankings
        .iter()
        .flat_map(|w| &w.rows)
        .any(|r| r.provisional)
        || tagessieger.iter().any(|t| t.provisional);

    ResultsResponse {
        event: EventHeader {
            id: event.id,
            name: event.name.clone(),
            date: event.date,
            location: event.location.clone(),
        },
        discipline: DisciplineHeader {
            id: discipline.id,
            name: discipline.name.clone(),
            mode: discipline.mode,
        },
        generated_at,
        provisional,
        wg_rankings,
        tagessieger,
        missing_wg,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{JudgeMode, PenaltyKind, PenaltyStatus, SyncMode};

    fn ts() -> DateTime<Utc> {
        "2027-06-12T10:00:00Z".parse().unwrap()
    }

    fn day() -> NaiveDate {
        NaiveDate::from_ymd_opt(2027, 6, 12).unwrap()
    }

    fn event() -> Event {
        Event {
            id: Uuid::new_v4(),
            owner_sub: "sub".into(),
            name: "Kreisausscheid".into(),
            date: day(),
            location: Some("Musterstadt".into()),
            is_active: true,
            current_discipline_id: None,
            current_run_id: None,
            current_run_ids: vec![],
            decider_tokens: ["a".into(), "b".into(), "c".into(), "d".into()],
            station_token: String::new(),
            created_at: ts(),
            updated_at: ts(),
        }
    }

    fn discipline(event_id: Uuid) -> Discipline {
        Discipline {
            id: Uuid::new_v4(),
            event_id,
            name: "Gruppenstafette".into(),
            mode: TimerMode::Gruppenstaffette,
            lane_count: 2,
            judge_mode: JudgeMode::PerStation,
            sync_mode: SyncMode::Live,
            position: 1,
            created_at: ts(),
            updated_at: ts(),
        }
    }

    fn team(name: &str) -> Team {
        Team {
            id: Uuid::new_v4(),
            event_id: Uuid::nil(),
            name: name.into(),
            position: 1,
            created_at: ts(),
            updated_at: ts(),
        }
    }

    fn entry(d: &Discipline, team_id: Uuid, start: u32, wg: Option<Wertungsgruppe>) -> TeamEntry {
        TeamEntry {
            team_id,
            discipline_id: d.id,
            event_id: d.event_id,
            start_number: start,
            lane: None,
            wg,
            wg_history: vec![],
            created_at: ts(),
            updated_at: ts(),
        }
    }

    fn runner(team_id: Uuid, birth: (i32, u32, u32), position: u32) -> Runner {
        Runner {
            id: Uuid::new_v4(),
            team_id,
            event_id: Uuid::nil(),
            first_name: "A".into(),
            last_name: "B".into(),
            birth_date: NaiveDate::from_ymd_opt(birth.0, birth.1, birth.2).unwrap(),
            position,
            gender: None,
            created_at: ts(),
            updated_at: ts(),
        }
    }

    fn run(d: &Discipline, team_id: Uuid, n: u32, status: RunStatus, t: Option<u32>) -> Run {
        Run {
            id: Uuid::new_v4(),
            event_id: d.event_id,
            discipline_id: d.id,
            team_id,
            runner_id: None,
            run_number: n,
            lane: Some(1),
            status,
            original_time_ms: t,
            correction_ms: 0,
            started_at: None,
            ended_at: None,
            lineup: vec![],
            lineup_confirmed_at: None,
            lineup_confirmed_by: None,
            time_source: TimeSource::Lichtschranke,
            hand_time_1_ms: None,
            hand_time_2_ms: None,
            penalties_accepted_at: None,
            created_at: ts(),
            updated_at: ts(),
        }
    }

    fn penalty(r: &Run, kind: PenaltyKind, status: PenaltyStatus) -> Penalty {
        Penalty {
            id: Uuid::new_v4(),
            event_id: r.event_id,
            discipline_id: r.discipline_id,
            run_id: r.id,
            team_id: r.team_id,
            runner_id: None,
            kind,
            code: None,
            station: None,
            is_disqualification: kind == PenaltyKind::Fv,
            decider_slot: 0,
            delta_ms: if kind == PenaltyKind::Sz { 15_000 } else { 0 },
            reason_code: None,
            status,
            created_at: ts(),
            reviewed_at: None,
            reviewed_by: None,
        }
    }

    // ===== build_scoring_input =====

    #[test]
    fn completed_flag_only_for_completed_status() {
        let e = event();
        let d = discipline(e.id);
        let t = team("A");
        let entries = [entry(&d, t.id, 1, Some(Wertungsgruppe::Wg2))];
        let runs: Vec<Run> = [
            RunStatus::Pending,
            RunStatus::Active,
            RunStatus::Completed,
            RunStatus::Cancelled,
        ]
        .into_iter()
        .enumerate()
        .map(|(i, s)| run(&d, t.id, i as u32 + 1, s, Some(40_000)))
        .collect();
        let input = build_scoring_input(&e, d.id, &entries, &[], &runs, &[]);
        let flags: Vec<bool> = input.runs.iter().map(|r| r.completed).collect();
        assert_eq!(flags, vec![false, false, true, false]);
    }

    #[test]
    fn measured_is_original_time_and_correction_is_ignored() {
        let e = event();
        let d = discipline(e.id);
        let t = team("A");
        let entries = [entry(&d, t.id, 1, Some(Wertungsgruppe::Wg2))];
        let mut r = run(&d, t.id, 1, RunStatus::Completed, Some(47_230));
        r.correction_ms = 15_000; // legacy value written by approve
        let p = penalty(&r, PenaltyKind::Sz, PenaltyStatus::Approved);
        let input = build_scoring_input(&e, d.id, &entries, &[], &[r], &[p]);
        assert_eq!(input.runs[0].measured_ms, Some(47_230));

        // End to end: SZ counted exactly once (47.23 + 15).
        let results = scoring::compute_results(&input.teams, &input.runs, input.competition_day);
        assert_eq!(results.wg_rankings[0].rows[0].best_total_ms, Some(62_230));
    }

    #[test]
    fn hand_time_mean_is_used_as_measured() {
        let e = event();
        let d = discipline(e.id);
        let t = team("A");
        let entries = [entry(&d, t.id, 1, Some(Wertungsgruppe::Wg1))];
        let mut r = run(&d, t.id, 1, RunStatus::Completed, Some(47_240));
        r.time_source = TimeSource::Handstopp;
        r.hand_time_1_ms = Some(47_230);
        r.hand_time_2_ms = Some(47_245);
        let input = build_scoring_input(&e, d.id, &entries, &[], &[r], &[]);
        assert_eq!(input.runs[0].measured_ms, Some(47_240));
    }

    #[test]
    fn missing_time_stays_none() {
        let e = event();
        let d = discipline(e.id);
        let t = team("A");
        let entries = [entry(&d, t.id, 1, Some(Wertungsgruppe::Wg1))];
        let r = run(&d, t.id, 1, RunStatus::Completed, None);
        let input = build_scoring_input(&e, d.id, &entries, &[], &[r], &[]);
        assert_eq!(input.runs[0].measured_ms, None);
    }

    #[test]
    fn entries_without_wg_are_excluded_and_reported() {
        let e = event();
        let d = discipline(e.id);
        let (a, b, c) = (team("A"), team("B"), team("C"));
        let entries = [
            entry(&d, c.id, 3, None),
            entry(&d, a.id, 1, Some(Wertungsgruppe::Wg2)),
            entry(&d, b.id, 2, None),
        ];
        let input = build_scoring_input(&e, d.id, &entries, &[], &[], &[]);
        assert_eq!(input.teams.len(), 1);
        assert_eq!(input.teams[0].team_id, a.id);
        assert_eq!(input.missing_wg, vec![b.id, c.id]);
    }

    #[test]
    fn penalties_pass_through_with_status_incl_rejected() {
        let e = event();
        let d = discipline(e.id);
        let t = team("A");
        let entries = [entry(&d, t.id, 1, Some(Wertungsgruppe::Wg2))];
        let r = run(&d, t.id, 1, RunStatus::Completed, Some(40_000));
        let other = run(&d, t.id, 2, RunStatus::Completed, Some(41_000));
        let ps = [
            penalty(&r, PenaltyKind::Sz, PenaltyStatus::Approved),
            penalty(&r, PenaltyKind::Fv, PenaltyStatus::Rejected),
            penalty(&r, PenaltyKind::Sz, PenaltyStatus::Pending),
            penalty(&other, PenaltyKind::Fv, PenaltyStatus::Approved),
        ];
        let input = build_scoring_input(&e, d.id, &entries, &[], &[r, other], &ps);
        assert_eq!(
            input.runs[0].penalties,
            vec![
                PenaltyInput {
                    kind: PenaltyKind::Sz,
                    status: PenaltyStatus::Approved
                },
                PenaltyInput {
                    kind: PenaltyKind::Fv,
                    status: PenaltyStatus::Rejected
                },
                PenaltyInput {
                    kind: PenaltyKind::Sz,
                    status: PenaltyStatus::Pending
                },
            ]
        );
        assert_eq!(input.runs[1].penalties.len(), 1);

        // Rejected FV does not invalidate; pending SZ makes it provisional.
        let scored = scoring::score_run(&input.runs[0]).unwrap();
        assert!(scored.valid);
        assert!(scored.provisional);
        assert_eq!(scored.total_ms, Some(70_000));
    }

    #[test]
    fn lineup_and_roster_are_mapped() {
        let e = event();
        let d = discipline(e.id);
        let t = team("A");
        let other = team("B");
        let r1 = runner(t.id, (2015, 1, 1), 2);
        let r2 = runner(t.id, (2014, 5, 5), 1);
        let foreign = runner(other.id, (2010, 1, 1), 1);
        let entries = [entry(&d, t.id, 4, Some(Wertungsgruppe::Wg2))];
        let mut r = run(&d, t.id, 1, RunStatus::Completed, Some(40_000));
        r.lineup = vec![r1.id];
        let input = build_scoring_input(
            &e,
            d.id,
            &entries,
            &[r1.clone(), r2.clone(), foreign],
            &[r],
            &[],
        );
        assert_eq!(input.competition_day, day());
        let team_in = &input.teams[0];
        assert_eq!(team_in.start_number, 4);
        assert_eq!(team_in.wg, Wertungsgruppe::Wg2);
        // Sorted by position; foreign runner excluded.
        let ids: Vec<Uuid> = team_in.runners.iter().map(|r| r.runner_id).collect();
        assert_eq!(ids, vec![r2.id, r1.id]);
        assert_eq!(input.runs[0].lineup, vec![r1.id]);
    }

    #[test]
    fn other_disciplines_are_filtered_out() {
        let e = event();
        let d = discipline(e.id);
        let d2 = discipline(e.id);
        let t = team("A");
        let entries = [
            entry(&d, t.id, 1, Some(Wertungsgruppe::Wg2)),
            entry(&d2, t.id, 9, None),
        ];
        let runs = [
            run(&d, t.id, 1, RunStatus::Completed, Some(1)),
            run(&d2, t.id, 1, RunStatus::Completed, Some(2)),
        ];
        let input = build_scoring_input(&e, d.id, &entries, &[], &runs, &[]);
        assert_eq!(input.teams.len(), 1);
        assert!(input.missing_wg.is_empty());
        assert_eq!(input.runs.len(), 1);
        assert_eq!(input.runs[0].measured_ms, Some(1));
    }

    #[test]
    fn duplicate_entries_for_one_team_are_taken_once() {
        let e = event();
        let d = discipline(e.id);
        let t = team("A");
        let entries = [
            entry(&d, t.id, 5, Some(Wertungsgruppe::Wg3)),
            entry(&d, t.id, 2, Some(Wertungsgruppe::Wg2)),
        ];
        let input = build_scoring_input(&e, d.id, &entries, &[], &[], &[]);
        assert_eq!(input.teams.len(), 1);
        assert_eq!(input.teams[0].start_number, 2);
        assert_eq!(input.teams[0].wg, Wertungsgruppe::Wg2);
    }

    // ===== build_results_response =====

    struct Fixture {
        e: Event,
        d: Discipline,
        teams: Vec<Team>,
        entries: Vec<TeamEntry>,
        runners: Vec<Runner>,
        runs: Vec<Run>,
        penalties: Vec<Penalty>,
    }

    impl Fixture {
        fn response(&self) -> ResultsResponse {
            build_results_response(
                &self.e,
                &self.d,
                &self.entries,
                &self.teams,
                &self.runners,
                &self.runs,
                &self.penalties,
                ts(),
            )
        }
    }

    /// WG2: A (ranked, best run 2, hand time on run 2), B (all invalid),
    /// C (not started). WG1: D (ranked, pending SZ). E: missing WG.
    fn fixture() -> Fixture {
        let e = event();
        let d = discipline(e.id);
        let teams: Vec<Team> = ["Alpha", "Bravo", "Charlie", "Delta", "Echo"]
            .into_iter()
            .map(team)
            .collect();
        let (a, b, c, dd, ee) = (
            teams[0].id,
            teams[1].id,
            teams[2].id,
            teams[3].id,
            teams[4].id,
        );
        let entries = vec![
            entry(&d, a, 1, Some(Wertungsgruppe::Wg2)),
            entry(&d, b, 2, Some(Wertungsgruppe::Wg2)),
            entry(&d, c, 3, Some(Wertungsgruppe::Wg2)),
            entry(&d, dd, 4, Some(Wertungsgruppe::Wg1)),
            entry(&d, ee, 5, None),
        ];
        // A: two runners aged 12 and 13 on the day.
        let ra1 = runner(a, (2015, 1, 1), 1);
        let ra2 = runner(a, (2014, 1, 1), 2);
        // D: one runner aged 9 → only roster (empty lineup → fallback).
        let rd = runner(dd, (2018, 1, 1), 1);
        let runners = vec![ra1.clone(), ra2.clone(), rd];

        let mut a1 = run(&d, a, 1, RunStatus::Completed, Some(50_000));
        a1.lineup = vec![ra1.id];
        let mut a2 = run(&d, a, 2, RunStatus::Completed, Some(45_000));
        a2.lineup = vec![ra1.id, ra2.id];
        a2.time_source = TimeSource::Handstopp;
        a2.hand_time_1_ms = Some(44_990);
        a2.hand_time_2_ms = Some(45_010);
        a2.correction_ms = 99_999; // must be ignored
        let b1 = run(&d, b, 1, RunStatus::Completed, Some(40_000));
        let b2 = run(&d, b, 2, RunStatus::Completed, None);
        let c1 = run(&d, c, 1, RunStatus::Pending, None);
        let d1 = run(&d, dd, 1, RunStatus::Completed, Some(60_000));
        let penalties = vec![
            penalty(&a1, PenaltyKind::Sz, PenaltyStatus::Approved),
            penalty(&b1, PenaltyKind::Fv, PenaltyStatus::Approved),
            penalty(&d1, PenaltyKind::Sz, PenaltyStatus::Pending),
        ];
        Fixture {
            e,
            d,
            teams,
            entries,
            runners,
            runs: vec![a1, a2, b1, b2, c1, d1],
            penalties,
        }
    }

    #[test]
    fn response_header_and_missing_wg() {
        let f = fixture();
        let res = f.response();
        assert_eq!(res.event.name, "Kreisausscheid");
        assert_eq!(res.event.date, day());
        assert_eq!(res.event.location.as_deref(), Some("Musterstadt"));
        assert_eq!(res.discipline.id, f.d.id);
        assert_eq!(res.discipline.mode, TimerMode::Gruppenstaffette);
        assert_eq!(res.generated_at, ts());
        assert_eq!(
            res.missing_wg,
            vec![TeamRef {
                team_id: f.teams[4].id,
                team_name: "Echo".into(),
                start_number: 5
            }]
        );
    }

    #[test]
    fn response_wg_rankings_are_enriched() {
        let f = fixture();
        let res = f.response();
        let wgs: Vec<&str> = res.wg_rankings.iter().map(|w| w.wg_label).collect();
        assert_eq!(wgs, vec!["WG1", "WG2"]);
        let wg2 = &res.wg_rankings[1];
        assert_eq!(wg2.wg_label_de, "Jungen bis 14");
        let names: Vec<&str> = wg2.rows.iter().map(|r| r.team_name.as_str()).collect();
        assert_eq!(names, vec!["Alpha", "Bravo", "Charlie"]);

        let alpha = &wg2.rows[0];
        assert_eq!(alpha.place, Some(1));
        assert_eq!(alpha.status, RankStatus::Ranked);
        assert_eq!(alpha.best_total_ms, Some(45_000));
        assert_eq!(alpha.runs.len(), 2);
        let (r1, r2) = (&alpha.runs[0], &alpha.runs[1]);
        assert_eq!(r1.sz_count, 1);
        assert_eq!(r1.measured_ms, Some(50_000));
        assert_eq!(r1.total_ms, Some(65_000));
        assert!(!r1.is_best);
        assert_eq!(r2.time_source, TimeSource::Handstopp);
        assert_eq!(r2.hand_time_1_ms, Some(44_990));
        assert_eq!(r2.hand_time_2_ms, Some(45_010));
        assert_eq!(r2.total_ms, Some(45_000));
        assert!(r2.is_best);
        assert_eq!(r2.lane, Some(1));

        let bravo = &wg2.rows[1];
        assert_eq!(bravo.status, RankStatus::AllInvalid);
        assert_eq!(bravo.place, Some(2));
        assert_eq!(
            bravo.runs[0].invalid_reason,
            Some(InvalidReason::Fehlversuch)
        );
        assert_eq!(bravo.runs[0].fv_count, 1);
        assert_eq!(bravo.runs[1].invalid_reason, Some(InvalidReason::NoTime));

        let charlie = &wg2.rows[2];
        assert_eq!(charlie.status, RankStatus::NotStarted);
        assert_eq!(charlie.place, None);
        assert!(charlie.runs.is_empty());
    }

    #[test]
    fn response_tagessieger_is_enriched_and_rounded() {
        let f = fixture();
        let res = f.response();
        assert_eq!(res.tagessieger.len(), 2);
        let alpha = res
            .tagessieger
            .iter()
            .find(|t| t.team_name == "Alpha")
            .unwrap();
        assert_eq!(alpha.wg_label, "WG2");
        assert_eq!(alpha.run_number, Some(2));
        assert_eq!(alpha.ages, vec![12, 13]);
        assert_eq!(alpha.avg_age, 12.5);
        assert_eq!(alpha.factor, 0.89);
        assert_eq!(alpha.total_ms, 45_000);
        assert_eq!(alpha.gw_ms, 40_050);
        assert!(!alpha.lineup_fallback);

        let delta = res
            .tagessieger
            .iter()
            .find(|t| t.team_name == "Delta")
            .unwrap();
        assert!(delta.lineup_fallback);
        assert_eq!(delta.ages, vec![9]);
        assert_eq!(delta.factor, 0.82);
        assert_eq!(delta.total_ms, 75_000);
        assert!(delta.provisional);
        assert_eq!(alpha.place, 1);
        assert_eq!(delta.place, 2);
    }

    #[test]
    fn response_provisional_flag() {
        let mut f = fixture();
        assert!(f.response().provisional);
        for p in &mut f.penalties {
            if p.status == PenaltyStatus::Pending {
                p.status = PenaltyStatus::Approved;
            }
        }
        assert!(!f.response().provisional);
    }

    #[test]
    fn unknown_team_gets_fallback_name() {
        let mut f = fixture();
        f.teams.retain(|t| t.name != "Alpha");
        let res = f.response();
        assert_eq!(res.wg_rankings[1].rows[0].team_name, UNKNOWN_TEAM_NAME);
    }

    #[test]
    fn round_to_helper() {
        assert_eq!(round_to(12.25, 1), 12.3);
        assert_eq!(round_to(11.333_333, 1), 11.3);
        assert_eq!(round_to(0.886_666_6, 4), 0.8867);
        assert_eq!(round_to(1.0, 4), 1.0);
    }

    #[test]
    fn response_json_shape() {
        let f = fixture();
        let j = serde_json::to_value(f.response()).unwrap();
        assert_eq!(j["event"]["date"], "2027-06-12");
        assert_eq!(j["discipline"]["mode"], "gruppenstaffette");
        assert_eq!(j["provisional"], true);
        let wg2 = &j["wg_rankings"][1];
        assert_eq!(wg2["wg"], "wg2");
        assert_eq!(wg2["wg_label"], "WG2");
        assert_eq!(wg2["rows"][1]["status"], "all_invalid");
        assert_eq!(wg2["rows"][1]["runs"][0]["invalid_reason"], "fehlversuch");
        assert_eq!(wg2["rows"][1]["runs"][1]["invalid_reason"], "no_time");
        assert_eq!(wg2["rows"][2]["status"], "not_started");
        assert_eq!(wg2["rows"][2]["place"], serde_json::Value::Null);
        assert_eq!(wg2["rows"][0]["runs"][1]["time_source"], "handstopp");
        let ts0 = &j["tagessieger"][0];
        for key in [
            "place",
            "team_id",
            "team_name",
            "start_number",
            "wg",
            "wg_label",
            "run_id",
            "run_number",
            "total_ms",
            "ages",
            "avg_age",
            "factor",
            "gw_ms",
            "lineup_fallback",
            "provisional",
        ] {
            assert!(ts0.get(key).is_some(), "tagessieger.{key}");
        }
        assert_eq!(j["missing_wg"][0]["team_name"], "Echo");
        assert_eq!(j["generated_at"], "2027-06-12T10:00:00Z");
    }
}
