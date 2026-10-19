// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

//! PR 2 (judge app): the station state carries the penalties of the
//! current runs so the judge sees the review status of own entries.

use chrono::{DateTime, Duration, Utc};
use ki_do_timer_backend::models::{
    Penalty, PenaltyStatus, Run, RunStatus, StationKind, TimeSource,
};
use ki_do_timer_backend::routes::decider::{
    StationStateQuery, build_station_penalty, current_station_penalties,
};
use uuid::Uuid;

fn ts() -> DateTime<Utc> {
    "2027-06-12T10:00:00Z".parse().unwrap()
}

fn run(lane: Option<u8>) -> Run {
    Run {
        id: Uuid::new_v4(),
        event_id: Uuid::new_v4(),
        discipline_id: Uuid::new_v4(),
        team_id: Uuid::new_v4(),
        runner_id: None,
        run_number: 1,
        lane,
        status: RunStatus::Active,
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
        penalties_accepted_at: None,
        created_at: ts(),
        updated_at: ts(),
    }
}

fn penalty(run: &Run, station: StationKind, minutes: i64) -> Penalty {
    let entry = kido_core::CATALOGUE
        .iter()
        .find(|e| e.station == station)
        .expect("catalogue entry for station");
    build_station_penalty(
        Uuid::new_v4(),
        run,
        station,
        entry,
        None,
        ts() + Duration::minutes(minutes),
    )
}

#[test]
fn only_penalties_of_current_runs() {
    let current = run(Some(1));
    let old = run(Some(1));
    let a = penalty(&current, StationKind::Knotengestell, 0);
    let b = penalty(&old, StationKind::Knotengestell, 1);
    let out = current_station_penalties(vec![a.clone(), b], &[current.id], None);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].id, a.id);
}

#[test]
fn station_filter_applies_when_given() {
    let r = run(Some(1));
    let k = penalty(&r, StationKind::Knotengestell, 0);
    let s = penalty(&r, StationKind::Startlinie, 1);
    let all = vec![k.clone(), s.clone()];
    let only_k = current_station_penalties(all.clone(), &[r.id], Some(StationKind::Knotengestell));
    assert_eq!(only_k.iter().map(|p| p.id).collect::<Vec<_>>(), vec![k.id]);
    let both = current_station_penalties(all, &[r.id], None);
    assert_eq!(both.len(), 2);
}

#[test]
fn both_parallel_runs_are_included_and_sorted_by_created_at() {
    let lane1 = run(Some(1));
    let lane2 = run(Some(2));
    let late = penalty(&lane1, StationKind::Verteilerlinie, 5);
    let early = penalty(&lane2, StationKind::Verteilerlinie, 1);
    let out = current_station_penalties(
        vec![late.clone(), early.clone()],
        &[lane1.id, lane2.id],
        Some(StationKind::Verteilerlinie),
    );
    assert_eq!(
        out.iter().map(|p| p.id).collect::<Vec<_>>(),
        vec![early.id, late.id]
    );
}

#[test]
fn reviewed_penalties_stay_visible() {
    let r = run(None);
    let mut rejected = penalty(&r, StationKind::Angriffslinie, 0);
    rejected.status = PenaltyStatus::Rejected;
    let mut approved = penalty(&r, StationKind::Angriffslinie, 1);
    approved.status = PenaltyStatus::Approved;
    let out = current_station_penalties(vec![rejected, approved], &[r.id], None);
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].status, PenaltyStatus::Rejected);
    assert_eq!(out[1].status, PenaltyStatus::Approved);
}

#[test]
fn no_current_runs_yields_nothing() {
    let r = run(None);
    let p = penalty(&r, StationKind::Startlinie, 0);
    assert!(current_station_penalties(vec![p], &[], None).is_empty());
}

#[test]
fn state_query_accepts_canonical_and_legacy_station_keys() {
    let q: StationStateQuery = serde_json::from_str(r#"{"station":"knotengestell"}"#).unwrap();
    assert_eq!(q.station, Some(StationKind::Knotengestell));
    let q: StationStateQuery = serde_json::from_str(r#"{"station":"knoten"}"#).unwrap();
    assert_eq!(q.station, Some(StationKind::Knotengestell));
    let q: StationStateQuery = serde_json::from_str("{}").unwrap();
    assert_eq!(q.station, None);
}
