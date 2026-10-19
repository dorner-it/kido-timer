// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

//! PR 1 wire contract (`docs/contract-pr1-2027.md`) — route table,
//! public catalogue endpoint and the pure validation helpers behind the
//! judge (decider) endpoints. Handlers that need DynamoDB delegate every
//! rule to these helpers, so they are tested here without AWS.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::{DateTime, NaiveDate, Utc};
use http_body_util::BodyExt;
use ki_do_timer_backend::models::{
    LineupConfirmedBy, Penalty, PenaltyKind, PenaltyStatus, Run, RunStatus, Runner, StationKind,
    TimeSource, TimerMode,
};
use ki_do_timer_backend::routes::decider::{
    StationLineupInput, SubmitPenaltyInput, SubmitStationPenaltyInput, build_slot_penalty,
    build_station_penalty, check_lineup_station, order_current_runs, resolve_target_run,
    validate_station_submission,
};
use ki_do_timer_backend::routes::runs::{apply_lineup, validate_lineup};
use tower::ServiceExt;
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

fn runner(team_id: Uuid) -> Runner {
    Runner {
        id: Uuid::new_v4(),
        team_id,
        event_id: Uuid::nil(),
        first_name: "A".into(),
        last_name: "B".into(),
        birth_date: NaiveDate::from_ymd_opt(2015, 1, 1).unwrap(),
        position: 1,
        gender: None,
        created_at: ts(),
        updated_at: ts(),
    }
}

// ===== route table =====

#[test]
fn full_route_table_builds_without_conflicts() {
    // axum/matchit panics on conflicting routes at insertion time.
    let _ = ki_do_timer_backend::app::api_router();
}

// ===== GET /api/catalogue =====

async fn get_catalogue() -> (StatusCode, serde_json::Value) {
    let response = ki_do_timer_backend::routes::catalogue::router::<()>()
        .oneshot(
            Request::builder()
                .uri("/api/catalogue")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn catalogue_is_public_and_mirrors_kido_core() {
    let (status, body) = get_catalogue().await;
    assert_eq!(status, StatusCode::OK);
    let arr = body.as_array().expect("array");
    assert_eq!(arr.len(), kido_core::CATALOGUE.len());
    for (item, entry) in arr.iter().zip(kido_core::CATALOGUE) {
        assert_eq!(item["code"], entry.code);
        assert_eq!(item["label_de"], entry.label_de);
        assert_eq!(
            item["station"],
            serde_json::to_value(entry.station).unwrap()
        );
        assert_eq!(item["kind"], serde_json::to_value(entry.kind).unwrap());
        assert_eq!(
            item.as_object().unwrap().len(),
            4,
            "exactly code, station, kind, label_de"
        );
    }
}

/// Depends on the kido-core catalogue being filled in.
#[tokio::test]
async fn catalogue_covers_every_gruppenstafette_station() {
    let (_, body) = get_catalogue().await;
    let arr = body.as_array().unwrap();
    for station in StationKind::for_mode(TimerMode::Gruppenstaffette) {
        let key = serde_json::to_value(station).unwrap();
        assert!(
            arr.iter().any(|i| i["station"] == key),
            "no catalogue entry for {key}"
        );
    }
    for item in arr {
        let code = item["code"].as_str().unwrap();
        let station = item["station"].as_str().unwrap();
        assert!(code.starts_with(&format!("{station}.")), "{code}");
        assert!(matches!(item["kind"].as_str(), Some("sz" | "fv")));
    }
}

// ===== station submit =====

#[test]
fn station_submit_body_shape() {
    let run_id = Uuid::new_v4();
    let parsed: SubmitStationPenaltyInput = serde_json::from_value(serde_json::json!({
        "station": "knotengestell",
        "run_id": run_id,
        "code": "knotengestell.ein_knoten_falsch",
        "client_id": null
    }))
    .unwrap();
    assert_eq!(parsed.station, StationKind::Knotengestell);
    assert_eq!(parsed.run_id, Some(run_id));
    assert_eq!(
        parsed.code.as_deref(),
        Some("knotengestell.ein_knoten_falsch")
    );

    // Legacy station keys are accepted on input.
    let parsed: SubmitStationPenaltyInput =
        serde_json::from_value(serde_json::json!({ "station": "knoten", "code": "x" })).unwrap();
    assert_eq!(parsed.station, StationKind::Knotengestell);
    assert_eq!(parsed.run_id, None);
}

#[test]
fn target_run_must_be_current() {
    let (a, b, other) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    assert_eq!(resolve_target_run(Some(b), &[a, b]).unwrap(), b);
    let err = resolve_target_run(Some(other), &[a, b]).unwrap_err();
    assert_eq!(err.code(), "conflict");
    assert_eq!(err.status(), StatusCode::CONFLICT);
}

#[test]
fn target_run_optional_only_with_single_current_run() {
    let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
    assert_eq!(resolve_target_run(None, &[a]).unwrap(), a);
    let err = resolve_target_run(None, &[a, b]).unwrap_err();
    assert_eq!(err.code(), "validation");
}

#[test]
fn no_current_run_is_conflict() {
    assert_eq!(
        resolve_target_run(None, &[]).unwrap_err().code(),
        "conflict"
    );
    assert_eq!(
        resolve_target_run(Some(Uuid::new_v4()), &[])
            .unwrap_err()
            .code(),
        "conflict"
    );
}

#[test]
fn unknown_code_rejected() {
    let err = validate_station_submission(
        TimerMode::Gruppenstaffette,
        StationKind::Knotengestell,
        "knotengestell.gibt_es_nicht",
    )
    .unwrap_err();
    assert_eq!(err.code(), "validation");
}

#[test]
fn station_not_allowed_for_mode_rejected() {
    // Podest is Löschangriff-only; checked before the code lookup.
    let err = validate_station_submission(TimerMode::Gruppenstaffette, StationKind::Podest, "x")
        .unwrap_err();
    assert_eq!(err.code(), "validation");
    let err = validate_station_submission(
        TimerMode::Loeschangriff,
        StationKind::Knotengestell,
        "knotengestell.ein_knoten_falsch",
    )
    .unwrap_err();
    assert_eq!(err.code(), "validation");
}

#[test]
fn code_must_belong_to_station() {
    let entry = kido_core::catalogue::for_station(StationKind::Knotengestell)
        .next()
        .expect("catalogue has Knotengestell entries");
    let err = validate_station_submission(
        TimerMode::Gruppenstaffette,
        StationKind::Verteilerlinie,
        entry.code,
    )
    .unwrap_err();
    assert_eq!(err.code(), "validation");
}

#[test]
fn every_catalogue_code_validates_for_its_own_station() {
    assert!(!kido_core::CATALOGUE.is_empty(), "catalogue filled in");
    for entry in kido_core::CATALOGUE {
        let got =
            validate_station_submission(TimerMode::Gruppenstaffette, entry.station, entry.code)
                .unwrap_or_else(|e| panic!("{}: {e}", entry.code));
        assert_eq!(got.code, entry.code);
    }
}

#[test]
fn station_penalty_takes_kind_and_delta_from_catalogue() {
    let r = run(Some(2));
    let sz = kido_core::CATALOGUE
        .iter()
        .find(|e| e.kind == PenaltyKind::Sz)
        .expect("an SZ entry");
    let fv = kido_core::CATALOGUE
        .iter()
        .find(|e| e.kind == PenaltyKind::Fv)
        .expect("an FV entry");

    let p = build_station_penalty(Uuid::new_v4(), &r, sz.station, sz, None, ts());
    assert_eq!(p.kind, PenaltyKind::Sz);
    assert_eq!(p.delta_ms, 15_000);
    assert!(!p.is_disqualification);
    assert_eq!(p.code.as_deref(), Some(sz.code));
    assert_eq!(p.station, Some(sz.station));
    assert_eq!(p.run_id, r.id);
    assert_eq!(p.team_id, r.team_id);
    assert_eq!(p.decider_slot, 0);
    assert_eq!(p.status, PenaltyStatus::Pending);
    assert_eq!(p.created_at, ts());

    let p = build_station_penalty(Uuid::new_v4(), &r, fv.station, fv, None, ts());
    assert_eq!(p.kind, PenaltyKind::Fv);
    assert_eq!(p.delta_ms, 0);
    assert!(p.is_disqualification);

    let json = serde_json::to_value(&p).unwrap();
    assert_eq!(json["kind"], "fv");
    assert_eq!(json["delta_ms"], 0);
    assert_eq!(json["is_disqualification"], true);
}

#[test]
fn station_penalty_resubmit_keeps_created_at() {
    let r = run(None);
    let entry = kido_core::CATALOGUE.first().expect("catalogue filled in");
    let first = build_station_penalty(Uuid::new_v4(), &r, entry.station, entry, None, ts());
    let later: DateTime<Utc> = "2027-06-12T10:05:00Z".parse().unwrap();
    let again = build_station_penalty(first.id, &r, entry.station, entry, Some(&first), later);
    assert_eq!(again.created_at, first.created_at);
    assert_eq!(again.id, first.id);
}

// ===== legacy slot route =====

#[test]
fn legacy_slot_penalty_is_sz_without_code_and_keeps_delta() {
    let r = run(Some(1));
    let input: SubmitPenaltyInput = serde_json::from_value(serde_json::json!({
        "delta_ms": 5000,
        "reason_code": "  kleiner_fehler ",
        "client_id": null
    }))
    .unwrap();
    let p: Penalty = build_slot_penalty(Uuid::new_v4(), &r, 3, &input, None, ts());
    assert_eq!(p.kind, PenaltyKind::Sz);
    assert_eq!(p.code, None);
    assert_eq!(p.station, None);
    assert_eq!(p.delta_ms, 5000);
    assert_eq!(p.decider_slot, 3);
    assert!(!p.is_disqualification);
    assert_eq!(p.reason_code.as_deref(), Some("kleiner_fehler"));
}

// ===== current runs ordering (station state) =====

#[test]
fn current_runs_filtered_and_ordered_by_lane() {
    let lane2 = run(Some(2));
    let lane1 = run(Some(1));
    let not_current = run(Some(1));
    let ids = vec![lane2.id, lane1.id];
    let ordered = order_current_runs(vec![not_current, lane2.clone(), lane1.clone()], &ids);
    assert_eq!(
        ordered.iter().map(|r| r.id).collect::<Vec<_>>(),
        vec![lane1.id, lane2.id]
    );
}

#[test]
fn current_runs_without_lane_go_last_in_selection_order() {
    let a = run(None);
    let b = run(None);
    let c = run(Some(1));
    let ids = vec![b.id, a.id, c.id];
    let ordered = order_current_runs(vec![a.clone(), b.clone(), c.clone()], &ids);
    assert_eq!(
        ordered.iter().map(|r| r.id).collect::<Vec<_>>(),
        vec![c.id, b.id, a.id]
    );
}

#[test]
fn current_runs_skip_unknown_ids() {
    let a = run(Some(1));
    let ordered = order_current_runs(vec![a.clone()], &[Uuid::new_v4(), a.id]);
    assert_eq!(ordered.len(), 1);
    assert_eq!(ordered[0].id, a.id);
}

// ===== judge lineup =====

#[test]
fn judge_lineup_body_shape() {
    let id = Uuid::new_v4();
    let parsed: StationLineupInput =
        serde_json::from_value(serde_json::json!({ "runner_ids": [id] })).unwrap();
    assert_eq!(parsed.lineup.runner_ids, vec![id]);
    assert_eq!(parsed.station, None);
    let parsed: StationLineupInput =
        serde_json::from_value(serde_json::json!({ "runner_ids": [], "station": "start" }))
            .unwrap();
    assert_eq!(parsed.station, Some(StationKind::Startlinie));
}

/// Depends on `StationKind::records_lineup` from kido-core.
#[test]
fn judge_lineup_only_from_startlinie() {
    assert!(check_lineup_station(None).is_ok());
    assert!(check_lineup_station(Some(StationKind::Startlinie)).is_ok());
    for s in [
        StationKind::Verteilerlinie,
        StationKind::Strahlrohrlinie,
        StationKind::Knotengestell,
        StationKind::Angriffslinie,
        StationKind::Podest,
    ] {
        assert_eq!(
            check_lineup_station(Some(s)).unwrap_err().code(),
            "forbidden"
        );
    }
}

#[test]
fn judge_lineup_flow() {
    let mut r = run(Some(1));
    let roster = vec![runner(r.team_id), runner(r.team_id), runner(Uuid::new_v4())];
    // The judge's run must be current.
    assert!(resolve_target_run(Some(r.id), &[r.id]).is_ok());
    // Foreign runner rejected.
    assert!(validate_lineup(&[roster[0].id, roster[2].id], &roster, r.team_id).is_err());
    // Own runners accepted and stamped as confirmed by the Startlinie.
    let ids = vec![roster[1].id, roster[0].id];
    validate_lineup(&ids, &roster, r.team_id).unwrap();
    apply_lineup(&mut r, ids.clone(), LineupConfirmedBy::Startlinie, ts());
    let json = serde_json::to_value(&r).unwrap();
    assert_eq!(json["lineup"], serde_json::json!(ids));
    assert_eq!(json["lineup_confirmed_by"], "startlinie");
    assert_eq!(json["lineup_confirmed_at"], "2027-06-12T10:00:00Z");
}
