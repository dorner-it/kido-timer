//! Results (WG rankings + Tagessieger) computed on the desktop with the
//! shared `kido_core::scoring` — identical to the web, needed on-prem.
//!
//! Mapping rules (same as the backend, `docs/contract-pr1-2027.md`):
//! - only runs of this discipline; `completed` = status `completed`
//!   (pending / active / cancelled runs are passed but ignored by scoring),
//! - measured time = `original_time_ms` (light barrier or hand-time mean);
//!   the legacy `correction_ms` is **never** added (SZ would count twice),
//! - penalties = the snapshot's penalties of the run (kind + status),
//! - lineup = `Run.lineup`,
//! - teams = team entries of this discipline **with** a WG; entries
//!   without WG ("WG fehlt") are excluded and listed separately.

use chrono::NaiveDate;
use kido_core::scoring::{self, PenaltyInput, Results, RunInput, RunnerInput, TeamInput};
use serde::Serialize;
use uuid::Uuid;

use crate::cloud::types::{DisciplinePayload, PenaltyKind, RunStatus, StationKind};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ResultsResponse {
    pub results: Results,
    /// Team ids of entries without WG — not ranked.
    pub teams_without_wg: Vec<Uuid>,
    /// Runner ids skipped because their birth date is unreadable.
    pub runners_without_birth_date: Vec<Uuid>,
    /// `YYYY-MM-DD` used for the Tagessieger ages.
    pub competition_day: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScoringInput {
    pub teams: Vec<TeamInput>,
    pub runs: Vec<RunInput>,
    pub competition_day: NaiveDate,
    pub teams_without_wg: Vec<Uuid>,
    pub runners_without_birth_date: Vec<Uuid>,
}

fn parse_date(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s.get(..10).unwrap_or(s), "%Y-%m-%d").ok()
}

/// Map a snapshot to the `kido_core::scoring` inputs.
pub fn scoring_input(snapshot: &DisciplinePayload) -> Result<ScoringInput, String> {
    let discipline_id = snapshot.discipline.id;
    let competition_day = parse_date(&snapshot.event.date).ok_or_else(|| {
        format!(
            "Wettkampfdatum {:?} ist kein gültiges Datum",
            snapshot.event.date
        )
    })?;

    let mut teams = Vec::new();
    let mut teams_without_wg = Vec::new();
    let mut runners_without_birth_date = Vec::new();
    let mut entries: Vec<_> = snapshot
        .team_entries
        .iter()
        .filter(|e| e.discipline_id == discipline_id)
        .collect();
    entries.sort_by_key(|e| (e.start_number, e.team_id));
    for entry in entries {
        let Some(wg) = entry.wg else {
            teams_without_wg.push(entry.team_id);
            continue;
        };
        let runners = snapshot
            .runners
            .iter()
            .filter(|r| r.team_id == entry.team_id)
            .filter_map(|r| match parse_date(&r.birth_date) {
                Some(birth_date) => Some(RunnerInput {
                    runner_id: r.id,
                    birth_date,
                }),
                None => {
                    runners_without_birth_date.push(r.id);
                    None
                }
            })
            .collect();
        teams.push(TeamInput {
            team_id: entry.team_id,
            start_number: entry.start_number,
            wg,
            runners,
        });
    }

    let runs = snapshot
        .runs
        .iter()
        .filter(|r| r.discipline_id == discipline_id)
        .map(|r| RunInput {
            run_id: r.id,
            team_id: r.team_id,
            run_number: r.run_number,
            completed: r.status == RunStatus::Completed,
            // Never `original + correction_ms` — see module docs.
            measured_ms: r.original_time_ms,
            penalties: snapshot
                .approved_penalties
                .iter()
                .filter(|p| p.run_id == r.id)
                .map(|p| PenaltyInput {
                    kind: p.kind,
                    status: p.status,
                })
                .collect(),
            lineup: r.lineup.clone(),
        })
        .collect();

    Ok(ScoringInput {
        teams,
        runs,
        competition_day,
        teams_without_wg,
        runners_without_birth_date,
    })
}

pub fn compute(snapshot: &DisciplinePayload) -> Result<ResultsResponse, String> {
    let input = scoring_input(snapshot)?;
    let results = scoring::compute_results(&input.teams, &input.runs, input.competition_day);
    Ok(ResultsResponse {
        results,
        teams_without_wg: input.teams_without_wg,
        runners_without_birth_date: input.runners_without_birth_date,
        competition_day: input.competition_day.format("%Y-%m-%d").to_string(),
    })
}

/// Error catalogue entry for the UI (`kido_core::CATALOGUE`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CatalogueItem {
    pub code: &'static str,
    pub station: StationKind,
    pub station_label: &'static str,
    pub kind: PenaltyKind,
    pub label_de: &'static str,
}

pub fn catalogue() -> Vec<CatalogueItem> {
    kido_core::CATALOGUE
        .iter()
        .map(|e| CatalogueItem {
            code: e.code,
            station: e.station,
            station_label: e.station.label_de(),
            kind: e.kind,
            label_de: e.label_de,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud::fixtures::{
        id, sample_payload, RUNNER_A1, RUNNER_A2, RUN_A1, RUN_B1, RUN_C1, TEAM_A, TEAM_B, TEAM_C,
    };
    use crate::cloud::types::{PenaltyStatus, Wertungsgruppe};
    use kido_core::scoring::RankStatus;

    fn complete(s: &mut DisciplinePayload, run: &str, ms: u32) {
        let r = s.runs.iter_mut().find(|r| r.id == id(run)).unwrap();
        r.status = RunStatus::Completed;
        r.original_time_ms = Some(ms);
    }

    #[test]
    fn maps_teams_with_wg_only() {
        let input = scoring_input(&sample_payload()).unwrap();
        let ids: Vec<Uuid> = input.teams.iter().map(|t| t.team_id).collect();
        assert_eq!(ids, vec![id(TEAM_A), id(TEAM_B)]);
        assert_eq!(input.teams_without_wg, vec![id(TEAM_C)]);
        assert_eq!(input.teams[0].wg, Wertungsgruppe::Wg2);
        assert_eq!(input.teams[0].runners.len(), 2);
        assert_eq!(
            input.competition_day,
            NaiveDate::from_ymd_opt(2027, 6, 12).unwrap()
        );
    }

    #[test]
    fn correction_ms_is_never_added() {
        let mut s = sample_payload();
        complete(&mut s, RUN_A1, 47_240);
        s.runs[0].correction_ms = 15_000; // legacy mirror of the approved SZ
        let input = scoring_input(&s).unwrap();
        let run = input.runs.iter().find(|r| r.run_id == id(RUN_A1)).unwrap();
        assert_eq!(run.measured_ms, Some(47_240));

        let res = compute(&s).unwrap();
        let row = &res.results.wg_rankings[0].rows[0];
        // 47.24 s + one approved SZ (15 s) = 62.24 s — not 77.24 s.
        assert_eq!(row.best_total_ms, Some(62_240));
    }

    #[test]
    fn non_completed_runs_are_ignored() {
        let mut s = sample_payload();
        s.runs[0].status = RunStatus::Active;
        s.runs[0].original_time_ms = Some(10_000);
        s.runs[1].status = RunStatus::Cancelled;
        s.runs[1].original_time_ms = Some(9_000);
        let input = scoring_input(&s).unwrap();
        assert!(input.runs.iter().all(|r| !r.completed));
        let res = compute(&s).unwrap();
        let rows = &res.results.wg_rankings[0].rows;
        assert!(rows.iter().all(|r| r.status == RankStatus::NotStarted));
        assert!(res.results.tagessieger.is_empty());
    }

    #[test]
    fn team_without_wg_is_excluded_from_rankings() {
        let mut s = sample_payload();
        complete(&mut s, RUN_C1, 30_000); // fastest, but WG missing
        let res = compute(&s).unwrap();
        assert_eq!(res.teams_without_wg, vec![id(TEAM_C)]);
        for ranking in &res.results.wg_rankings {
            assert!(ranking.rows.iter().all(|r| r.team_id != id(TEAM_C)));
        }
        assert!(res
            .results
            .tagessieger
            .iter()
            .all(|r| r.team_id != id(TEAM_C)));
    }

    #[test]
    fn penalties_and_lineup_are_mapped() {
        let mut s = sample_payload();
        complete(&mut s, RUN_A1, 47_240);
        let input = scoring_input(&s).unwrap();
        let run = input.runs.iter().find(|r| r.run_id == id(RUN_A1)).unwrap();
        assert_eq!(run.penalties.len(), 1);
        assert_eq!(run.penalties[0].kind, PenaltyKind::Sz);
        assert_eq!(run.penalties[0].status, PenaltyStatus::Approved);
        assert_eq!(run.lineup, vec![id(RUNNER_A1), id(RUNNER_A2)]);
    }

    #[test]
    fn fv_makes_run_invalid_and_team_shares_last_place() {
        let mut s = sample_payload();
        complete(&mut s, RUN_A1, 40_000);
        complete(&mut s, RUN_B1, 50_000);
        s.approved_penalties[0].kind = PenaltyKind::Fv;
        let res = compute(&s).unwrap();
        let rows = &res.results.wg_rankings[0].rows;
        assert_eq!(rows[0].team_id, id(TEAM_B));
        assert_eq!(rows[0].place, Some(1));
        assert_eq!(rows[1].team_id, id(TEAM_A));
        assert_eq!(rows[1].status, RankStatus::AllInvalid);
        assert_eq!(rows[1].place, Some(2));
    }

    #[test]
    fn tagessieger_uses_lineup_ages_on_competition_day() {
        let mut s = sample_payload();
        complete(&mut s, RUN_A1, 50_000);
        s.approved_penalties.clear();
        let res = compute(&s).unwrap();
        let ts = &res.results.tagessieger[0];
        assert_eq!(ts.team_id, id(TEAM_A));
        // A1 born 2015-06-12 → 12 on 2027-06-12; A2 born 2013-06-13 → 13.
        assert_eq!(ts.ages, vec![12, 13]);
        assert!(!ts.lineup_fallback);
        assert_eq!(res.competition_day, "2027-06-12");
    }

    #[test]
    fn runs_of_other_disciplines_are_ignored() {
        let mut s = sample_payload();
        complete(&mut s, RUN_A1, 50_000);
        s.runs[0].discipline_id = Uuid::new_v4();
        let input = scoring_input(&s).unwrap();
        assert!(input.runs.iter().all(|r| r.run_id != id(RUN_A1)));
    }

    #[test]
    fn invalid_event_date_is_an_error() {
        let mut s = sample_payload();
        s.event.date = "12.06.2027".into();
        assert!(compute(&s).is_err());
    }

    #[test]
    fn unreadable_birth_date_is_reported() {
        let mut s = sample_payload();
        s.runners[0].birth_date = "unbekannt".into();
        let input = scoring_input(&s).unwrap();
        assert_eq!(input.runners_without_birth_date, vec![id(RUNNER_A1)]);
        assert_eq!(input.teams[0].runners.len(), 1);
    }

    #[test]
    fn catalogue_is_complete() {
        let cat = catalogue();
        assert_eq!(cat.len(), kido_core::CATALOGUE.len());
        let first = &cat[0];
        assert_eq!(first.station, StationKind::Startlinie);
        assert_eq!(first.station_label, "Startlinie/Zeitnehmer");
        let json = serde_json::to_value(first).unwrap();
        assert_eq!(json["station"], "startlinie");
        assert!(json["kind"] == "sz" || json["kind"] == "fv");
        assert!(cat.iter().any(|c| c.kind == PenaltyKind::Fv));
    }
}
