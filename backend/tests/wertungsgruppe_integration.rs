// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

//! Cross-module tests for the 2027 Wertungsgruppe flow.
//!
//! The WG is **ticked by the club** (input on the TeamEntry) and never
//! derived; the old `Wertungsgruppe::derive` + gender logic is gone. The
//! backend only computes `wg_warnings` on read via
//! `kido_core::wg::check_roster` with the **event year** (31.12.-age).
//! A full route-level test would need DynamoDB Local + Cognito session
//! plumbing that this crate doesn't set up, so we exercise the seams the
//! routes rely on.

use axum::response::IntoResponse;
use chrono::{NaiveDate, TimeZone, Utc};
use http_body_util::BodyExt;
use ki_do_timer_backend::models::team_entry::compute_wg_warnings;
use ki_do_timer_backend::models::{Runner, TeamEntry, TeamEntryView, Wertungsgruppe};
use ki_do_timer_backend::routes::team_entries::{TeamEntryInput, require_wg};
use uuid::Uuid;

fn mk_runner(team_id: Uuid, birth: (i32, u32, u32)) -> Runner {
    let now = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    Runner {
        id: Uuid::new_v4(),
        team_id,
        event_id: Uuid::nil(),
        first_name: "Test".into(),
        last_name: "Runner".into(),
        birth_date: NaiveDate::from_ymd_opt(birth.0, birth.1, birth.2).unwrap(),
        position: 1,
        gender: None,
        created_at: now,
        updated_at: now,
    }
}

fn mk_entry(team_id: Uuid, wg: Option<Wertungsgruppe>) -> TeamEntry {
    let now = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    TeamEntry {
        team_id,
        discipline_id: Uuid::nil(),
        event_id: Uuid::nil(),
        start_number: 1,
        lane: None,
        wg,
        wg_history: Vec::new(),
        created_at: now,
        updated_at: now,
    }
}

#[test]
fn wg_is_taken_from_input_not_derived() {
    // Mixed / any roster: whatever the club ticks is stored.
    let body = serde_json::json!({
        "team_id": Uuid::nil(), "start_number": 4, "lane": 1, "wg": "wg5"
    });
    let input: TeamEntryInput = serde_json::from_value(body).unwrap();
    assert_eq!(require_wg(input.wg).unwrap(), Wertungsgruppe::Wg5);
}

#[tokio::test]
async fn missing_wg_maps_to_400_validation() {
    let body = serde_json::json!({ "team_id": Uuid::nil(), "start_number": 4, "lane": null });
    let input: TeamEntryInput = serde_json::from_value(body).unwrap();
    let response = require_wg(input.wg).unwrap_err().into_response();
    assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["error"]["code"], "validation");
    assert!(json["error"]["message"].as_str().unwrap().contains("wg"));
}

#[test]
fn legacy_entry_without_wg_has_no_warnings() {
    let team = Uuid::new_v4();
    // An 18-year-old would be "zu alt" for any WG below 4 — but without a
    // WG there is nothing to check.
    let roster = vec![mk_runner(team, (2009, 1, 1))];
    assert!(compute_wg_warnings(&mk_entry(team, None), &roster, 2027).is_empty());
}

#[test]
fn entry_with_empty_roster_has_no_warnings() {
    let team = Uuid::new_v4();
    assert!(compute_wg_warnings(&mk_entry(team, Some(Wertungsgruppe::Wg1)), &[], 2027).is_empty());
}

// --- behaviour owned by kido-core::wg::check_roster (panics with
// `todo!()` until the kido-core bodies are merged) ---

#[test]
fn fitting_roster_has_no_warnings() {
    let team = Uuid::new_v4();
    // 2027 − 2013 = 14 → fits WG2 (bis 14).
    let roster = vec![
        mk_runner(team, (2013, 12, 31)),
        mk_runner(team, (2015, 1, 1)),
    ];
    let entry = mk_entry(team, Some(Wertungsgruppe::Wg2));
    assert!(compute_wg_warnings(&entry, &roster, 2027).is_empty());
}

#[test]
fn too_old_runner_is_warned_with_event_year_age() {
    let team = Uuid::new_v4();
    // Born 2012 → 15 on 31.12.2027 → too old for WG2, regardless of
    // the birthday (31.12.-age, not age on the competition day).
    let old = mk_runner(team, (2012, 12, 31));
    let ok = mk_runner(team, (2014, 6, 1));
    let entry = mk_entry(team, Some(Wertungsgruppe::Wg2));
    let warnings = compute_wg_warnings(&entry, &[old.clone(), ok], 2027);
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].runner_id, old.id);
    assert_eq!(warnings[0].age, 15);
    assert_eq!(warnings[0].wg, Wertungsgruppe::Wg2);
    assert_eq!(warnings[0].reason, "zu alt");
}

#[test]
fn too_young_for_wg1_is_warned() {
    let team = Uuid::new_v4();
    // 2027 − 2020 = 7 → below WG1's minimum of 8.
    let young = mk_runner(team, (2020, 1, 1));
    let entry = mk_entry(team, Some(Wertungsgruppe::Wg1));
    let warnings = compute_wg_warnings(&entry, std::slice::from_ref(&young), 2027);
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].reason, "zu jung");
}

#[test]
fn runners_of_other_teams_are_ignored() {
    let (team, other) = (Uuid::new_v4(), Uuid::new_v4());
    let roster = vec![
        mk_runner(other, (2000, 1, 1)),
        mk_runner(team, (2014, 1, 1)),
    ];
    let entry = mk_entry(team, Some(Wertungsgruppe::Wg2));
    assert!(compute_wg_warnings(&entry, &roster, 2027).is_empty());
}

#[test]
fn view_serializes_warnings_on_read() {
    let team = Uuid::new_v4();
    let old = mk_runner(team, (2008, 1, 1));
    let entry = mk_entry(team, Some(Wertungsgruppe::Wg3));
    let view = TeamEntryView::new(entry, std::slice::from_ref(&old), 2027);
    let json = serde_json::to_value(&view).unwrap();
    assert_eq!(json["wg"], "wg3");
    assert_eq!(
        json["wg_warnings"][0]["runner_id"],
        serde_json::json!(old.id)
    );
    assert_eq!(json["wg_warnings"][0]["age"], 19);
    assert_eq!(json["wg_warnings"][0]["wg"], "wg3");
    assert_eq!(json["wg_warnings"][0]["reason"], "zu alt");
}
