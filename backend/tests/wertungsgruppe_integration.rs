// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

//! Cross-module integration test for the `Wertungsgruppe` flow.
//!
//! A full route-level test would need DynamoDB Local + Cognito session
//! plumbing that this crate's existing `tests/integration.rs` does not
//! yet set up (only the healthcheck is wired in). Instead we exercise
//! the two seams the new route relies on:
//!
//!   1. `Wertungsgruppe::derive` — the pure function the route calls.
//!   2. `AppError::WgInconsistent` → HTTP 400 with `code: wg_inconsistent`
//!      — the same envelope the route produces via `IntoResponse`.

use axum::response::IntoResponse;
use chrono::{NaiveDate, TimeZone, Utc};
use http_body_util::BodyExt;
use ki_do_timer_backend::error::AppError;
use ki_do_timer_backend::models::{Gender, Runner, WgError, Wertungsgruppe};
use uuid::Uuid;

fn mk_runner(birth_year: i32, gender: Option<Gender>) -> Runner {
    let now = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    Runner {
        id: Uuid::nil(),
        team_id: Uuid::nil(),
        event_id: Uuid::nil(),
        first_name: "Test".into(),
        last_name: "Runner".into(),
        birth_date: NaiveDate::from_ymd_opt(birth_year, 6, 15).unwrap(),
        position: 1,
        gender,
        created_at: now,
        updated_at: now,
    }
}

#[test]
fn consistent_roster_yields_wg_value() {
    // Two boys, max age 14 → WG2 (the TeamEntry would be stamped Wg2).
    let roster = vec![
        mk_runner(2012, Some(Gender::Male)),
        mk_runner(2014, Some(Gender::Male)),
    ];
    let stichtag = NaiveDate::from_ymd_opt(2026, 6, 1).unwrap();
    let wg = Wertungsgruppe::derive(&roster, stichtag).expect("derive succeeds");
    assert_eq!(wg, Wertungsgruppe::Wg2);
}

#[tokio::test]
async fn inconsistent_roster_maps_to_400_wg_inconsistent() {
    // Mixed gender at age 14 (only WG1 ≤10 allows mixed) → inconsistent.
    let roster = vec![
        mk_runner(2014, Some(Gender::Male)),
        mk_runner(2014, Some(Gender::Female)),
    ];
    let stichtag = NaiveDate::from_ymd_opt(2026, 6, 1).unwrap();
    let WgError::Inconsistent { reason } =
        Wertungsgruppe::derive(&roster, stichtag).unwrap_err();
    assert!(!reason.is_empty(), "reason must be human-readable");

    // The route shape: wrap WgError into AppError::WgInconsistent, then
    // assert the IntoResponse path produces 400 with the right code.
    let app_err = AppError::WgInconsistent(reason);
    let response = app_err.into_response();
    assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);

    let body = response
        .into_body()
        .collect()
        .await
        .expect("body collects")
        .to_bytes();
    let json: serde_json::Value =
        serde_json::from_slice(&body).expect("body is JSON");
    assert_eq!(json["error"]["code"], "wg_inconsistent");
    assert!(
        !json["error"]["message"].as_str().unwrap_or("").is_empty(),
        "human-readable message"
    );
}
