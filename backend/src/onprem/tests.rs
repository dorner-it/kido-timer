// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use super::*;
use crate::models::{JudgeMode, StationKind, SyncMode, TimerMode};
use chrono::NaiveDate;

fn ts() -> DateTime<Utc> {
    "2027-06-12T10:00:00Z".parse().unwrap()
}

fn later() -> DateTime<Utc> {
    "2027-06-12T18:00:00Z".parse().unwrap()
}

fn even_later() -> DateTime<Utc> {
    "2027-06-13T09:00:00Z".parse().unwrap()
}

struct Fx {
    disc: Discipline,
    team: Uuid,
    other_team: Uuid,
    roster: Vec<Runner>,
    runs: Vec<Run>,
}

fn runner(event_id: Uuid, team_id: Uuid, position: u32) -> Runner {
    Runner {
        id: Uuid::new_v4(),
        team_id,
        event_id,
        first_name: "A".into(),
        last_name: "B".into(),
        birth_date: NaiveDate::from_ymd_opt(2014, 1, 1).unwrap(),
        position,
        gender: None,
        created_at: ts(),
        updated_at: ts(),
    }
}

fn run(disc: &Discipline, team_id: Uuid, run_number: u32, lane: u8) -> Run {
    Run {
        id: Uuid::new_v4(),
        event_id: disc.event_id,
        discipline_id: disc.id,
        team_id,
        runner_id: None,
        run_number,
        lane: Some(lane),
        status: RunStatus::Pending,
        original_time_ms: None,
        correction_ms: 0,
        started_at: None,
        ended_at: None,
        lineup: Vec::new(),
        lineup_confirmed_at: None,
        lineup_confirmed_by: None,
        time_source: TimeSource::Lichtschranke,
        hand_time_1_ms: None,
        hand_time_2_ms: None,
        created_at: ts(),
        updated_at: ts(),
    }
}

fn fx() -> Fx {
    let event_id = Uuid::new_v4();
    let disc = Discipline {
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
    };
    let team = Uuid::new_v4();
    let other_team = Uuid::new_v4();
    let roster = vec![
        runner(event_id, team, 1),
        runner(event_id, team, 2),
        runner(event_id, other_team, 1),
    ];
    let runs = vec![
        run(&disc, team, 1, 1),
        run(&disc, other_team, 2, 2),
        run(&disc, team, 3, 1),
    ];
    Fx {
        disc,
        team,
        other_team,
        roster,
        runs,
    }
}

const SZ_CODE: &str = "knotengestell.ein_knoten_falsch";
const FV_CODE: &str = "knotengestell.mehrere_knoten_falsch";

fn pen(fx: &Fx, run: &Run, code: Option<&str>, status: PenaltyStatus) -> Penalty {
    Penalty {
        id: Uuid::new_v4(),
        event_id: fx.disc.event_id,
        discipline_id: fx.disc.id,
        run_id: run.id,
        team_id: run.team_id,
        runner_id: None,
        kind: PenaltyKind::Sz,
        code: code.map(str::to_string),
        station: code.map(|_| StationKind::Knotengestell),
        is_disqualification: false,
        decider_slot: 0,
        delta_ms: 15_000,
        reason_code: None,
        status,
        created_at: ts(),
        reviewed_at: None,
        reviewed_by: None,
    }
}

fn reviewed(mut p: Penalty, status: PenaltyStatus, at: DateTime<Utc>, by: &str) -> Penalty {
    p.status = status;
    p.reviewed_at = Some(at);
    p.reviewed_by = Some(by.into());
    p
}

fn upload_run(r: &Run) -> UploadRun {
    UploadRun {
        id: r.id,
        run_number: r.run_number,
        status: RunStatus::Completed,
        original_time_ms: Some(47_230),
        started_at: Some(ts()),
        ended_at: Some(ts()),
        lineup: Vec::new(),
        lineup_confirmed_at: None,
        lineup_confirmed_by: None,
        time_source: TimeSource::Lichtschranke,
        hand_time_1_ms: None,
        hand_time_2_ms: None,
    }
}

/// In-memory "database" the plan is applied to.
#[derive(Debug, Clone, PartialEq)]
struct Db {
    runs: Vec<Run>,
    penalties: Vec<Penalty>,
    current_disc: Option<Uuid>,
    current_ids: Vec<Uuid>,
}

impl Db {
    fn new(fx: &Fx, penalties: Vec<Penalty>) -> Self {
        Db {
            runs: fx.runs.clone(),
            penalties,
            current_disc: None,
            current_ids: Vec::new(),
        }
    }

    fn upload(
        &mut self,
        fx: &Fx,
        input: &OnpremUploadInput,
        now: DateTime<Utc>,
    ) -> AppResult<UploadPlan> {
        let plan = plan_upload(
            &fx.disc,
            &self.runs,
            &self.penalties,
            &fx.roster,
            (self.current_disc, &self.current_ids),
            input,
            now,
        )?;
        for r in &plan.runs_to_put {
            let slot = self.runs.iter_mut().find(|x| x.id == r.id).unwrap();
            *slot = r.clone();
        }
        for p in &plan.penalties_to_put {
            match self.penalties.iter_mut().find(|x| x.id == p.id) {
                Some(slot) => *slot = p.clone(),
                None => self.penalties.push(p.clone()),
            }
        }
        if let Some(ids) = &plan.current_run_ids {
            self.current_disc = Some(fx.disc.id);
            self.current_ids = ids.clone();
        }
        Ok(plan)
    }

    fn run(&self, id: Uuid) -> &Run {
        self.runs.iter().find(|r| r.id == id).unwrap()
    }

    fn pen(&self, id: Uuid) -> &Penalty {
        self.penalties.iter().find(|p| p.id == id).unwrap()
    }
}

// ===== merge_review =====

#[test]
fn status_rank_orders_pending_below_reviewed() {
    assert!(status_rank(PenaltyStatus::Pending) < status_rank(PenaltyStatus::Approved));
    assert_eq!(
        status_rank(PenaltyStatus::Approved),
        status_rank(PenaltyStatus::Rejected)
    );
}

#[test]
fn merge_new_row_takes_incoming_and_fills_reviewer() {
    let f = fx();
    let p = pen(&f, &f.runs[0], Some(SZ_CODE), PenaltyStatus::Approved);
    let m = merge_review(None, &p, later());
    assert_eq!(m.status, PenaltyStatus::Approved);
    assert_eq!(m.reviewed_at, Some(later()));
    assert_eq!(m.reviewed_by.as_deref(), Some(ONPREM_REVIEWER));
    assert!(!m.conflict);

    let p = reviewed(p, PenaltyStatus::Rejected, ts(), "fbl");
    let m = merge_review(None, &p, later());
    assert_eq!(m.status, PenaltyStatus::Rejected);
    assert_eq!(m.reviewed_at, Some(ts()));
    assert_eq!(m.reviewed_by.as_deref(), Some("fbl"));
}

#[test]
fn merge_pending_never_carries_review_fields() {
    let f = fx();
    let mut p = pen(&f, &f.runs[0], Some(SZ_CODE), PenaltyStatus::Pending);
    p.reviewed_at = Some(ts());
    p.reviewed_by = Some("x".into());
    let m = merge_review(None, &p, later());
    assert_eq!(m.status, PenaltyStatus::Pending);
    assert_eq!((m.reviewed_at, m.reviewed_by), (None, None));
}

#[test]
fn reviewed_beats_pending_in_both_directions() {
    let f = fx();
    let pending = pen(&f, &f.runs[0], Some(SZ_CODE), PenaltyStatus::Pending);
    let approved = reviewed(pending.clone(), PenaltyStatus::Approved, ts(), "desk");
    let rejected = reviewed(pending.clone(), PenaltyStatus::Rejected, ts(), "web");

    // stored pending, incoming reviewed → incoming
    let m = merge_review(Some(&pending), &approved, later());
    assert_eq!(m.status, PenaltyStatus::Approved);
    assert_eq!(m.reviewed_by.as_deref(), Some("desk"));
    let m = merge_review(Some(&pending), &rejected, later());
    assert_eq!(m.status, PenaltyStatus::Rejected);

    // stored reviewed, incoming pending → stored
    for stored in [&approved, &rejected] {
        let m = merge_review(Some(stored), &pending, later());
        assert_eq!(m.status, stored.status);
        assert_eq!(m.reviewed_at, stored.reviewed_at);
        assert_eq!(m.reviewed_by, stored.reviewed_by);
        assert!(!m.conflict);
    }
}

#[test]
fn approved_vs_rejected_keeps_stored_and_flags_conflict() {
    let f = fx();
    let base = pen(&f, &f.runs[0], Some(SZ_CODE), PenaltyStatus::Pending);
    let web = reviewed(base.clone(), PenaltyStatus::Approved, ts(), "web");
    let desk = reviewed(base, PenaltyStatus::Rejected, later(), "desk");
    let m = merge_review(Some(&web), &desk, even_later());
    assert_eq!(m.status, PenaltyStatus::Approved);
    assert_eq!(m.reviewed_by.as_deref(), Some("web"));
    assert_eq!(m.reviewed_at, Some(ts()));
    assert!(m.conflict);

    let m = merge_review(Some(&desk), &web, even_later());
    assert_eq!(m.status, PenaltyStatus::Rejected);
    assert!(m.conflict);
}

#[test]
fn same_reviewed_status_keeps_stored_review_fields_without_conflict() {
    let f = fx();
    let base = pen(&f, &f.runs[0], Some(SZ_CODE), PenaltyStatus::Pending);
    let web = reviewed(base.clone(), PenaltyStatus::Approved, ts(), "web");
    let desk = reviewed(base, PenaltyStatus::Approved, later(), "desk");
    let m = merge_review(Some(&web), &desk, even_later());
    assert_eq!(m.reviewed_by.as_deref(), Some("web"));
    assert!(!m.conflict);
}

// ===== normalize_penalty =====

fn ctx_maps(f: &Fx, stored: &[Penalty]) -> (HashMap<Uuid, Run>, HashMap<Uuid, Penalty>) {
    (
        f.runs.iter().map(|r| (r.id, r.clone())).collect(),
        stored.iter().map(|p| (p.id, p.clone())).collect(),
    )
}

fn normalize(f: &Fx, stored: &[Penalty], p: &Penalty) -> AppResult<(Penalty, bool)> {
    let (runs, pens) = ctx_maps(f, stored);
    normalize_penalty(
        p,
        &DisciplineContext {
            discipline: &f.disc,
            runs: &runs,
            stored_penalties: &pens,
        },
        later(),
    )
}

#[test]
fn kind_and_delta_are_recomputed_from_catalogue() {
    let f = fx();
    // Client claims SZ with a bogus delta for an FV code.
    let mut p = pen(&f, &f.runs[0], Some(FV_CODE), PenaltyStatus::Pending);
    p.kind = PenaltyKind::Sz;
    p.delta_ms = 999;
    p.is_disqualification = false;
    let (row, _) = normalize(&f, &[], &p).unwrap();
    assert_eq!(row.kind, PenaltyKind::Fv);
    assert_eq!(row.delta_ms, 0);
    assert!(row.is_disqualification);

    let mut p = pen(&f, &f.runs[0], Some(SZ_CODE), PenaltyStatus::Pending);
    p.kind = PenaltyKind::Fv;
    p.delta_ms = 0;
    p.is_disqualification = true;
    let (row, _) = normalize(&f, &[], &p).unwrap();
    assert_eq!(row.kind, PenaltyKind::Sz);
    assert_eq!(row.delta_ms, 15_000);
    assert!(!row.is_disqualification);
}

#[test]
fn team_and_runner_come_from_the_run() {
    let f = fx();
    let mut p = pen(&f, &f.runs[0], Some(SZ_CODE), PenaltyStatus::Pending);
    p.team_id = Uuid::new_v4();
    p.runner_id = Some(Uuid::new_v4());
    let (row, _) = normalize(&f, &[], &p).unwrap();
    assert_eq!(row.team_id, f.team);
    assert_eq!(row.runner_id, None);
}

#[test]
fn unknown_code_rejected() {
    let f = fx();
    let p = pen(
        &f,
        &f.runs[0],
        Some("knotengestell.gibt_es_nicht"),
        PenaltyStatus::Pending,
    );
    let err = normalize(&f, &[], &p).unwrap_err();
    assert_eq!(err.code(), "validation");
    // Unknown code without station.
    let mut p = p;
    p.station = None;
    assert_eq!(normalize(&f, &[], &p).unwrap_err().code(), "validation");
}

#[test]
fn code_station_mismatch_rejected() {
    let f = fx();
    let mut p = pen(&f, &f.runs[0], Some(SZ_CODE), PenaltyStatus::Pending);
    p.station = Some(StationKind::Startlinie);
    let err = normalize(&f, &[], &p).unwrap_err();
    assert!(
        err.to_string().contains("gehört nicht zur Station"),
        "{err}"
    );
}

#[test]
fn station_not_valid_for_mode_rejected() {
    let f = fx();
    let mut p = pen(&f, &f.runs[0], Some(SZ_CODE), PenaltyStatus::Pending);
    p.station = Some(StationKind::Podest);
    assert_eq!(normalize(&f, &[], &p).unwrap_err().code(), "validation");
}

#[test]
fn missing_station_is_taken_from_catalogue_and_legacy_alias_is_canonical() {
    let f = fx();
    let mut p = pen(&f, &f.runs[0], Some(SZ_CODE), PenaltyStatus::Pending);
    p.station = None;
    let (row, _) = normalize(&f, &[], &p).unwrap();
    assert_eq!(row.station, Some(StationKind::Knotengestell));

    // Legacy key `knoten` on the wire parses to the canonical station.
    let mut j = serde_json::to_value(&p).unwrap();
    j["station"] = serde_json::json!("knoten");
    let p: Penalty = serde_json::from_value(j).unwrap();
    let (row, _) = normalize(&f, &[], &p).unwrap();
    assert_eq!(row.station, Some(StationKind::Knotengestell));
}

#[test]
fn foreign_discipline_or_event_rejected() {
    let f = fx();
    let mut p = pen(&f, &f.runs[0], Some(SZ_CODE), PenaltyStatus::Pending);
    p.discipline_id = Uuid::new_v4();
    assert_eq!(normalize(&f, &[], &p).unwrap_err().code(), "validation");
    let mut p = pen(&f, &f.runs[0], Some(SZ_CODE), PenaltyStatus::Pending);
    p.event_id = Uuid::new_v4();
    assert_eq!(normalize(&f, &[], &p).unwrap_err().code(), "validation");
}

#[test]
fn foreign_run_rejected() {
    let f = fx();
    let mut p = pen(&f, &f.runs[0], Some(SZ_CODE), PenaltyStatus::Pending);
    p.run_id = Uuid::new_v4();
    let err = normalize(&f, &[], &p).unwrap_err();
    assert!(err.to_string().contains("Lauf"), "{err}");
}

#[test]
fn penalty_id_stored_under_other_discipline_rejected() {
    let f = fx();
    let p = pen(&f, &f.runs[0], Some(SZ_CODE), PenaltyStatus::Pending);
    let mut foreign = p.clone();
    foreign.discipline_id = Uuid::new_v4();
    let err = normalize(&f, &[foreign], &p).unwrap_err();
    assert!(err.to_string().contains("anderen Wettkampf"), "{err}");
}

#[test]
fn moving_a_stored_penalty_to_another_run_rejected() {
    let f = fx();
    let stored = pen(&f, &f.runs[0], Some(SZ_CODE), PenaltyStatus::Pending);
    let mut moved = stored.clone();
    moved.run_id = f.runs[2].id;
    let err = normalize(&f, &[stored], &moved).unwrap_err();
    assert!(err.to_string().contains("anderen Lauf"), "{err}");
}

#[test]
fn legacy_row_without_code_only_when_stored_and_content_kept() {
    let f = fx();
    let mut legacy = pen(&f, &f.runs[0], None, PenaltyStatus::Pending);
    legacy.decider_slot = 2;
    legacy.delta_ms = 5_000;
    legacy.reason_code = Some("kleiner_fehler".into());

    // Not stored → rejected.
    let err = normalize(&f, &[], &legacy).unwrap_err();
    assert!(err.to_string().contains("Fehlercode fehlt"), "{err}");

    // Stored → content kept, status merged.
    let mut incoming = reviewed(legacy.clone(), PenaltyStatus::Approved, ts(), "desk");
    incoming.delta_ms = 1; // ignored
    incoming.kind = PenaltyKind::Fv; // ignored
    let (row, _) = normalize(&f, std::slice::from_ref(&legacy), &incoming).unwrap();
    assert_eq!(row.delta_ms, 5_000);
    assert_eq!(row.kind, PenaltyKind::Sz);
    assert_eq!(row.decider_slot, 2);
    assert_eq!(row.reason_code.as_deref(), Some("kleiner_fehler"));
    assert_eq!(row.code, None);
    assert_eq!(row.status, PenaltyStatus::Approved);
    assert_eq!(row.reviewed_by.as_deref(), Some("desk"));
}

#[test]
fn stored_created_at_is_kept() {
    let f = fx();
    let stored = pen(&f, &f.runs[0], Some(SZ_CODE), PenaltyStatus::Pending);
    let mut incoming = stored.clone();
    incoming.created_at = later();
    let (row, _) = normalize(&f, std::slice::from_ref(&stored), &incoming).unwrap();
    assert_eq!(row.created_at, ts());
}

// ===== apply_run_update =====

#[test]
fn run_update_applies_only_contract_fields() {
    let f = fx();
    let stored = &f.runs[0];
    let mut up = upload_run(stored);
    up.lineup = vec![f.roster[0].id, f.roster[1].id];
    up.lineup_confirmed_at = Some(ts());
    up.lineup_confirmed_by = Some(LineupConfirmedBy::Startlinie);
    let run = apply_run_update(stored, &up, &f.roster, later()).unwrap();
    assert_eq!(run.status, RunStatus::Completed);
    assert_eq!(run.original_time_ms, Some(47_230));
    assert_eq!(run.lineup.len(), 2);
    assert_eq!(run.lineup_confirmed_by, Some(LineupConfirmedBy::Startlinie));
    assert_eq!(run.updated_at, later());
    // Untouched fields.
    assert_eq!(run.team_id, stored.team_id);
    assert_eq!(run.lane, stored.lane);
    assert_eq!(run.created_at, stored.created_at);
    assert_eq!(run.correction_ms, stored.correction_ms);
}

#[test]
fn run_update_without_change_keeps_updated_at() {
    let f = fx();
    let stored = &f.runs[0];
    let up = upload_run(stored);
    let once = apply_run_update(stored, &up, &f.roster, later()).unwrap();
    let twice = apply_run_update(&once, &up, &f.roster, even_later()).unwrap();
    assert_eq!(once, twice);
}

#[test]
fn run_number_mismatch_rejected() {
    let f = fx();
    let mut up = upload_run(&f.runs[0]);
    up.run_number = 99;
    assert!(apply_run_update(&f.runs[0], &up, &f.roster, later()).is_err());
}

#[test]
fn lineup_of_other_team_or_duplicate_rejected() {
    let f = fx();
    let mut up = upload_run(&f.runs[0]);
    up.lineup = vec![f.roster[2].id]; // other team's runner
    let err = apply_run_update(&f.runs[0], &up, &f.roster, later()).unwrap_err();
    assert!(err.to_string().contains("gehört nicht"), "{err}");
    up.lineup = vec![f.roster[0].id, f.roster[0].id];
    let err = apply_run_update(&f.runs[0], &up, &f.roster, later()).unwrap_err();
    assert!(err.to_string().contains("doppelt"), "{err}");
    // Lineup on the other team's run is fine.
    let mut up = upload_run(&f.runs[1]);
    up.lineup = vec![f.roster[2].id];
    assert!(apply_run_update(&f.runs[1], &up, &f.roster, later()).is_ok());
    let _ = f.other_team;
}

#[test]
fn empty_lineup_clears_confirmation() {
    let f = fx();
    let mut up = upload_run(&f.runs[0]);
    up.lineup_confirmed_at = Some(ts());
    up.lineup_confirmed_by = Some(LineupConfirmedBy::Organizer);
    let run = apply_run_update(&f.runs[0], &up, &f.roster, later()).unwrap();
    assert_eq!(run.lineup_confirmed_at, None);
    assert_eq!(run.lineup_confirmed_by, None);
}

#[test]
fn hand_times_must_match_time_source() {
    let f = fx();
    let stored = &f.runs[0];

    // handstopp without both times
    let mut up = upload_run(stored);
    up.time_source = TimeSource::Handstopp;
    up.hand_time_1_ms = Some(47_230);
    assert!(apply_run_update(stored, &up, &f.roster, later()).is_err());
    // zero time
    up.hand_time_2_ms = Some(0);
    assert!(apply_run_update(stored, &up, &f.roster, later()).is_err());
    // original_time_ms not the mean
    up.hand_time_2_ms = Some(47_245);
    up.original_time_ms = Some(47_230);
    assert!(apply_run_update(stored, &up, &f.roster, later()).is_err());
    // mean given → ok
    up.original_time_ms = Some(47_240);
    let run = apply_run_update(stored, &up, &f.roster, later()).unwrap();
    assert_eq!(run.original_time_ms, Some(47_240));
    assert_eq!(run.time_source, TimeSource::Handstopp);
    // no original → mean filled in
    up.original_time_ms = None;
    let run = apply_run_update(stored, &up, &f.roster, later()).unwrap();
    assert_eq!(run.original_time_ms, Some(47_240));

    // lichtschranke with hand times
    let mut up = upload_run(stored);
    up.hand_time_2_ms = Some(47_245);
    assert!(apply_run_update(stored, &up, &f.roster, later()).is_err());
}

// ===== legacy correction =====

#[test]
fn legacy_correction_sums_approved_of_the_run_only() {
    let f = fx();
    let a = reviewed(
        pen(&f, &f.runs[0], Some(SZ_CODE), PenaltyStatus::Pending),
        PenaltyStatus::Approved,
        ts(),
        "x",
    );
    let pending = pen(&f, &f.runs[0], Some(SZ_CODE), PenaltyStatus::Pending);
    let rejected = reviewed(pending.clone(), PenaltyStatus::Rejected, ts(), "x");
    let other_run = reviewed(
        pen(&f, &f.runs[1], Some(SZ_CODE), PenaltyStatus::Pending),
        PenaltyStatus::Approved,
        ts(),
        "x",
    );
    let all = [a, pending, rejected, other_run];
    assert_eq!(legacy_correction_ms(f.runs[0].id, &all), 15_000);
    assert_eq!(legacy_correction_ms(f.runs[2].id, &all), 0);
}

// ===== plan_upload =====

fn full_upload(f: &Fx) -> (OnpremUploadInput, Vec<Uuid>) {
    let mut up0 = upload_run(&f.runs[0]);
    up0.lineup = vec![f.roster[0].id];
    up0.lineup_confirmed_at = Some(ts());
    up0.lineup_confirmed_by = Some(LineupConfirmedBy::Startlinie);
    let mut up1 = upload_run(&f.runs[1]);
    up1.time_source = TimeSource::Handstopp;
    up1.original_time_ms = None;
    up1.hand_time_1_ms = Some(50_000);
    up1.hand_time_2_ms = Some(50_010);

    let a = pen(f, &f.runs[0], Some(SZ_CODE), PenaltyStatus::Approved); // no reviewer
    let b = reviewed(
        pen(f, &f.runs[0], Some(SZ_CODE), PenaltyStatus::Pending),
        PenaltyStatus::Rejected,
        ts(),
        "desk",
    );
    let c = pen(f, &f.runs[1], Some(FV_CODE), PenaltyStatus::Pending);
    let ids = vec![a.id, b.id, c.id];
    (
        OnpremUploadInput {
            runs: vec![up0, up1],
            penalties: vec![a, b, c],
            current_run_ids: None,
        },
        ids,
    )
}

#[test]
fn upload_applies_runs_penalties_and_correction() {
    let f = fx();
    let mut db = Db::new(&f, Vec::new());
    let (input, ids) = full_upload(&f);
    let plan = db.upload(&f, &input, later()).unwrap();
    assert_eq!(
        plan.response,
        OnpremUploadResponse {
            runs_updated: 2,
            penalties_upserted: 3
        }
    );
    assert_eq!(plan.penalties_to_put.len(), 3);
    assert!(plan.review_conflicts.is_empty());

    let a = db.pen(ids[0]);
    assert_eq!(a.status, PenaltyStatus::Approved);
    assert_eq!(a.reviewed_at, Some(later()));
    assert_eq!(a.reviewed_by.as_deref(), Some(ONPREM_REVIEWER));
    assert_eq!(db.pen(ids[1]).status, PenaltyStatus::Rejected);
    assert_eq!(db.pen(ids[2]).kind, PenaltyKind::Fv);

    let r0 = db.run(f.runs[0].id);
    assert_eq!(r0.status, RunStatus::Completed);
    assert_eq!(r0.lineup, vec![f.roster[0].id]);
    assert_eq!(r0.correction_ms, 15_000, "one approved SZ");
    let r1 = db.run(f.runs[1].id);
    assert_eq!(r1.original_time_ms, Some(50_010));
    assert_eq!(r1.correction_ms, 0, "FV pending → 0");
    // Untouched run.
    assert_eq!(db.run(f.runs[2].id), &f.runs[2]);
}

#[test]
fn applying_the_same_upload_twice_yields_identical_state() {
    let f = fx();
    let mut db = Db::new(&f, Vec::new());
    db.current_disc = Some(f.disc.id);
    db.current_ids = vec![f.runs[0].id];
    let (input, _) = full_upload(&f);
    let first = db.upload(&f, &input, later()).unwrap();
    let after_first = db.clone();
    let second = db.upload(&f, &input, even_later()).unwrap();
    assert_eq!(db, after_first, "state unchanged by the repeat");
    assert!(second.runs_to_put.is_empty());
    assert!(second.penalties_to_put.is_empty());
    assert_eq!(second.current_run_ids, None);
    assert_eq!(first.response, second.response);
}

#[test]
fn penalty_only_upload_recomputes_correction_of_its_run() {
    let f = fx();
    let stored = pen(&f, &f.runs[2], Some(SZ_CODE), PenaltyStatus::Pending);
    let mut db = Db::new(&f, vec![stored.clone()]);
    let input = OnpremUploadInput {
        penalties: vec![reviewed(stored, PenaltyStatus::Approved, ts(), "desk")],
        ..Default::default()
    };
    let plan = db.upload(&f, &input, later()).unwrap();
    assert_eq!(plan.response.runs_updated, 0);
    assert_eq!(plan.runs_to_put.len(), 1);
    assert_eq!(db.run(f.runs[2].id).correction_ms, 15_000);
}

#[test]
fn stored_approved_penalties_of_the_run_count_towards_correction() {
    let f = fx();
    let stored = reviewed(
        pen(&f, &f.runs[0], Some(SZ_CODE), PenaltyStatus::Pending),
        PenaltyStatus::Approved,
        ts(),
        "web",
    );
    let mut db = Db::new(&f, vec![stored]);
    let input = OnpremUploadInput {
        penalties: vec![reviewed(
            pen(&f, &f.runs[0], Some(SZ_CODE), PenaltyStatus::Pending),
            PenaltyStatus::Approved,
            ts(),
            "desk",
        )],
        ..Default::default()
    };
    db.upload(&f, &input, later()).unwrap();
    assert_eq!(db.run(f.runs[0].id).correction_ms, 30_000);
}

#[test]
fn upload_does_not_downgrade_web_review_and_reports_conflict() {
    let f = fx();
    let base = pen(&f, &f.runs[0], Some(SZ_CODE), PenaltyStatus::Pending);
    let web_approved = reviewed(base.clone(), PenaltyStatus::Approved, ts(), "web");
    let web_rejected = {
        let mut p = pen(&f, &f.runs[0], Some(SZ_CODE), PenaltyStatus::Pending);
        p = reviewed(p, PenaltyStatus::Rejected, ts(), "web");
        p
    };
    let mut db = Db::new(&f, vec![web_approved.clone(), web_rejected.clone()]);
    let input = OnpremUploadInput {
        penalties: vec![reviewed(base, PenaltyStatus::Rejected, later(), "desk"), {
            let mut p = web_rejected.clone();
            p.status = PenaltyStatus::Pending;
            p.reviewed_at = None;
            p.reviewed_by = None;
            p
        }],
        ..Default::default()
    };
    let plan = db.upload(&f, &input, later()).unwrap();
    assert_eq!(plan.review_conflicts, vec![web_approved.id]);
    assert_eq!(db.pen(web_approved.id), &web_approved);
    assert_eq!(db.pen(web_rejected.id), &web_rejected);
    assert!(plan.penalties_to_put.is_empty());
}

#[test]
fn invalid_row_rejects_whole_upload_before_any_write() {
    let f = fx();
    let db = Db::new(&f, Vec::new());
    let (mut input, _) = full_upload(&f);
    input.penalties.push(pen(
        &f,
        &f.runs[0],
        Some("startlinie.gibt_es_nicht"),
        PenaltyStatus::Pending,
    ));
    let err = plan_upload(
        &f.disc,
        &db.runs,
        &db.penalties,
        &f.roster,
        (None, &[]),
        &input,
        later(),
    )
    .unwrap_err();
    assert_eq!(err.code(), "validation");
}

#[test]
fn foreign_run_in_upload_rejected() {
    let f = fx();
    let mut db = Db::new(&f, Vec::new());
    let mut foreign = f.runs[0].clone();
    foreign.id = Uuid::new_v4();
    foreign.discipline_id = Uuid::new_v4();
    db.runs.push(foreign.clone()); // stored, but of another discipline
    let input = OnpremUploadInput {
        runs: vec![upload_run(&foreign)],
        ..Default::default()
    };
    let err = db.upload(&f, &input, later()).unwrap_err();
    assert!(err.to_string().contains("gehört nicht"), "{err}");
}

#[test]
fn duplicate_ids_in_upload_rejected() {
    let f = fx();
    let mut db = Db::new(&f, Vec::new());
    let up = upload_run(&f.runs[0]);
    let input = OnpremUploadInput {
        runs: vec![up.clone(), up],
        ..Default::default()
    };
    assert!(db.upload(&f, &input, later()).is_err());
    let p = pen(&f, &f.runs[0], Some(SZ_CODE), PenaltyStatus::Pending);
    let input = OnpremUploadInput {
        penalties: vec![p.clone(), p],
        ..Default::default()
    };
    assert!(db.upload(&f, &input, later()).is_err());
}

#[test]
fn current_runs_cleared_by_default_only_for_this_discipline() {
    let f = fx();
    // This discipline is current → cleared.
    let mut db = Db::new(&f, Vec::new());
    db.current_disc = Some(f.disc.id);
    db.current_ids = vec![f.runs[0].id];
    let plan = db
        .upload(&f, &OnpremUploadInput::default(), later())
        .unwrap();
    assert_eq!(plan.current_run_ids, Some(Vec::new()));
    assert!(db.current_ids.is_empty());

    // Explicit [] behaves the same.
    let mut db = Db::new(&f, Vec::new());
    db.current_disc = Some(f.disc.id);
    db.current_ids = vec![f.runs[0].id];
    let input = OnpremUploadInput {
        current_run_ids: Some(Vec::new()),
        ..Default::default()
    };
    assert_eq!(
        db.upload(&f, &input, later()).unwrap().current_run_ids,
        Some(Vec::new())
    );

    // Another discipline is current → untouched.
    let mut db = Db::new(&f, Vec::new());
    let other = Uuid::new_v4();
    db.current_disc = Some(other);
    db.current_ids = vec![Uuid::new_v4()];
    let plan = db
        .upload(&f, &OnpremUploadInput::default(), later())
        .unwrap();
    assert_eq!(plan.current_run_ids, None);
}

#[test]
fn current_runs_set_and_validated() {
    let f = fx();
    let mut db = Db::new(&f, Vec::new());
    let input = OnpremUploadInput {
        current_run_ids: Some(vec![f.runs[0].id, f.runs[1].id]),
        ..Default::default()
    };
    let plan = db.upload(&f, &input, later()).unwrap();
    assert_eq!(plan.current_run_ids, Some(vec![f.runs[0].id, f.runs[1].id]));
    assert_eq!(db.current_disc, Some(f.disc.id));
    // Repeat → nothing to write.
    assert_eq!(
        db.upload(&f, &input, later()).unwrap().current_run_ids,
        None
    );

    // More than lane_count (2).
    let input = OnpremUploadInput {
        current_run_ids: Some(f.runs.iter().map(|r| r.id).collect()),
        ..Default::default()
    };
    assert!(db.upload(&f, &input, later()).is_err());
    // Foreign run.
    let input = OnpremUploadInput {
        current_run_ids: Some(vec![Uuid::new_v4()]),
        ..Default::default()
    };
    assert!(db.upload(&f, &input, later()).is_err());
}

#[test]
fn upload_body_parses_contract_example() {
    let run_id = Uuid::new_v4();
    let runner_id = Uuid::new_v4();
    let f = fx();
    let p = pen(&f, &f.runs[0], Some(SZ_CODE), PenaltyStatus::Pending);
    let body = serde_json::json!({
        "runs": [{
            "id": run_id, "run_number": 1, "status": "completed",
            "original_time_ms": 47230, "started_at": "2027-06-12T10:00:00Z",
            "ended_at": "2027-06-12T10:01:00Z",
            "lineup": [runner_id], "lineup_confirmed_at": "2027-06-12T10:00:00Z",
            "lineup_confirmed_by": "startlinie",
            "time_source": "lichtschranke", "hand_time_1_ms": null, "hand_time_2_ms": null
        }],
        "penalties": [p],
        "current_run_ids": []
    });
    let input: OnpremUploadInput = serde_json::from_value(body).unwrap();
    assert_eq!(input.runs[0].lineup, vec![runner_id]);
    assert_eq!(input.penalties.len(), 1);
    assert_eq!(input.current_run_ids, Some(Vec::new()));

    let empty: OnpremUploadInput = serde_json::from_value(serde_json::json!({})).unwrap();
    assert!(empty.runs.is_empty() && empty.penalties.is_empty());
    assert_eq!(empty.current_run_ids, None);

    let out = serde_json::to_value(OnpremUploadResponse {
        runs_updated: 1,
        penalties_upserted: 2,
    })
    .unwrap();
    assert_eq!(
        out,
        serde_json::json!({ "runs_updated": 1, "penalties_upserted": 2 })
    );
}

// ===== plan_import_penalties (A1) =====

#[test]
fn import_merges_all_statuses_and_skips_invalid_rows() {
    let f = fx();
    let stored_approved = reviewed(
        pen(&f, &f.runs[0], Some(SZ_CODE), PenaltyStatus::Pending),
        PenaltyStatus::Approved,
        ts(),
        "web",
    );
    let mut downgraded = stored_approved.clone();
    downgraded.status = PenaltyStatus::Pending;
    downgraded.reviewed_at = None;
    downgraded.reviewed_by = None;
    let new_pending = pen(&f, &f.runs[1], Some(SZ_CODE), PenaltyStatus::Pending);
    let new_rejected = reviewed(
        pen(&f, &f.runs[1], Some(SZ_CODE), PenaltyStatus::Pending),
        PenaltyStatus::Rejected,
        ts(),
        "desk",
    );
    let bad_code = pen(&f, &f.runs[1], Some("x.y"), PenaltyStatus::Pending);
    let mut foreign = pen(&f, &f.runs[1], Some(SZ_CODE), PenaltyStatus::Pending);
    foreign.discipline_id = Uuid::new_v4();

    let plan = plan_import_penalties(
        &f.disc,
        &f.runs,
        std::slice::from_ref(&stored_approved),
        &[
            downgraded,
            new_pending.clone(),
            new_rejected.clone(),
            bad_code,
            foreign,
        ],
        later(),
    );
    assert_eq!(plan.imported, 3);
    assert_eq!(plan.skipped, 2);
    let put_ids: Vec<Uuid> = plan.to_put.iter().map(|p| p.id).collect();
    assert_eq!(put_ids, vec![new_pending.id, new_rejected.id]);
    // stored approved row stays; correction of run 0 = 15 s (stored
    // approved) — run 0 currently has 0 → recompute.
    assert_eq!(plan.corrections, vec![(f.runs[0].id, 15_000)]);
}

#[test]
fn import_twice_is_a_noop_the_second_time() {
    let f = fx();
    let p = reviewed(
        pen(&f, &f.runs[0], Some(SZ_CODE), PenaltyStatus::Pending),
        PenaltyStatus::Approved,
        ts(),
        "desk",
    );
    let first = plan_import_penalties(&f.disc, &f.runs, &[], std::slice::from_ref(&p), later());
    assert_eq!(first.to_put.len(), 1);
    let mut runs = f.runs.clone();
    runs[0].correction_ms = first.corrections[0].1;
    let second = plan_import_penalties(&f.disc, &runs, &first.to_put, &[p], even_later());
    assert!(second.to_put.is_empty());
    assert!(second.corrections.is_empty());
    assert_eq!(second.imported, 1);
}
