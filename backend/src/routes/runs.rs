// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use std::collections::HashSet;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, put};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::AuthenticatedUser;
use crate::db::{disciplines, runners, runs};
use crate::error::{AppError, AppResult};
use crate::models::{LineupConfirmedBy, Run, RunStatus, Runner, TimeSource};
use crate::routes::events::require_owned;
use crate::routes::team_entries::validate_lane;
use crate::state::AppState;

pub fn router() -> axum::Router<AppState> {
    axum::Router::new()
        .route(
            "/{id}/disciplines/{discipline_id}/runs",
            get(list).post(create),
        )
        .route(
            "/{id}/disciplines/{discipline_id}/runs/{run_number}/{run_id}",
            put(update),
        )
        .route(
            "/{id}/disciplines/{discipline_id}/runs/{run_id}/lineup",
            put(set_lineup),
        )
        .route(
            "/{id}/disciplines/{discipline_id}/runs/{run_id}/hand-times",
            put(set_hand_times),
        )
}

pub async fn list(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((event_id, discipline_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<Vec<Run>>> {
    require_owned(&state, &auth, event_id).await?;
    let rs = runs::list_for_discipline(&state.db, event_id, discipline_id).await?;
    Ok(Json(rs))
}

#[derive(Deserialize)]
pub struct CreateRunInput {
    pub team_id: Uuid,
    pub runner_id: Option<Uuid>,
    pub run_number: u32,
    pub lane: Option<u8>,
}

pub async fn create(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((event_id, discipline_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<CreateRunInput>,
) -> AppResult<(StatusCode, Json<Run>)> {
    require_owned(&state, &auth, event_id).await?;
    let disc = disciplines::find(&state.db, event_id, discipline_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("discipline {discipline_id}")))?;
    validate_lane(input.lane, disc.lane_count)?;
    let now = Utc::now();
    let run = Run {
        id: Uuid::new_v4(),
        event_id,
        discipline_id,
        team_id: input.team_id,
        runner_id: input.runner_id,
        run_number: input.run_number,
        lane: input.lane,
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
        created_at: now,
        updated_at: now,
    };
    runs::put(&state.db, &run).await?;
    Ok((StatusCode::CREATED, Json(run)))
}

#[derive(Deserialize)]
pub struct UpdateRunInput {
    pub status: Option<RunStatus>,
    pub original_time_ms: Option<Option<u32>>,
    pub started_at: Option<Option<chrono::DateTime<Utc>>>,
    pub ended_at: Option<Option<chrono::DateTime<Utc>>>,
}

pub async fn update(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((event_id, discipline_id, run_number, run_id)): Path<(Uuid, Uuid, u32, Uuid)>,
    Json(input): Json<UpdateRunInput>,
) -> AppResult<Json<Run>> {
    require_owned(&state, &auth, event_id).await?;

    let mut run = runs::get(&state.db, event_id, discipline_id, run_number, run_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("run {run_id}")))?;

    if let Some(status) = input.status {
        run.status = status;
    }
    if let Some(t) = input.original_time_ms {
        run.original_time_ms = t;
    }
    if let Some(t) = input.started_at {
        run.started_at = t;
    }
    if let Some(t) = input.ended_at {
        run.ended_at = t;
    }
    run.updated_at = Utc::now();

    runs::put(&state.db, &run).await?;
    Ok(Json(run))
}

// ===== lineup =====

/// Body of the organizer and judge lineup endpoints.
#[derive(Debug, Deserialize)]
pub struct LineupInput {
    pub runner_ids: Vec<Uuid>,
}

/// Every id must belong to `roster` (the run's team) and appear at most
/// once. An empty list is allowed (clears the lineup).
pub fn validate_lineup(runner_ids: &[Uuid], roster: &[Runner], team_id: Uuid) -> AppResult<()> {
    let team_runners: HashSet<Uuid> = roster
        .iter()
        .filter(|r| r.team_id == team_id)
        .map(|r| r.id)
        .collect();
    let mut seen = HashSet::with_capacity(runner_ids.len());
    for id in runner_ids {
        if !seen.insert(*id) {
            return Err(AppError::Validation(format!(
                "Läufer {id} ist doppelt in der Besetzung"
            )));
        }
        if !team_runners.contains(id) {
            return Err(AppError::Validation(format!(
                "Läufer {id} gehört nicht zur Mannschaft des Laufs"
            )));
        }
    }
    Ok(())
}

/// Store a (validated) lineup on the run. A non-empty lineup records who
/// confirmed it and when; an empty one clears the confirmation too.
pub fn apply_lineup(
    run: &mut Run,
    runner_ids: Vec<Uuid>,
    by: LineupConfirmedBy,
    now: DateTime<Utc>,
) {
    if runner_ids.is_empty() {
        run.lineup_confirmed_at = None;
        run.lineup_confirmed_by = None;
    } else {
        run.lineup_confirmed_at = Some(now);
        run.lineup_confirmed_by = Some(by);
    }
    run.lineup = runner_ids;
    run.updated_at = now;
}

/// `PUT /api/events/{id}/disciplines/{discipline_id}/runs/{run_id}/lineup`
/// (organizer).
pub async fn set_lineup(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((event_id, discipline_id, run_id)): Path<(Uuid, Uuid, Uuid)>,
    Json(input): Json<LineupInput>,
) -> AppResult<Json<Run>> {
    require_owned(&state, &auth, event_id).await?;
    let mut run = runs::find(&state.db, event_id, discipline_id, run_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("run {run_id}")))?;
    let roster = runners::list_for_team(&state.db, event_id, run.team_id).await?;
    validate_lineup(&input.runner_ids, &roster, run.team_id)?;
    apply_lineup(
        &mut run,
        input.runner_ids,
        LineupConfirmedBy::Organizer,
        Utc::now(),
    );
    runs::put(&state.db, &run).await?;
    Ok(Json(run))
}

// ===== hand times =====

/// Body of the hand-times endpoint: either both times, or `clear: true`.
#[derive(Debug, Default, Deserialize)]
pub struct HandTimesInput {
    #[serde(default)]
    pub hand_time_1_ms: Option<u32>,
    #[serde(default)]
    pub hand_time_2_ms: Option<u32>,
    #[serde(default)]
    pub clear: bool,
}

/// Apply hand times (light barrier failed) or revert to the light
/// barrier.
///
/// - `{ hand_time_1_ms, hand_time_2_ms }` (both > 0): `time_source =
///   handstopp`, `original_time_ms = hand_time_mean(t1, t2)` (1/100 s,
///   half up), `correction_ms = 0`.
/// - `{ clear: true }` (no times): `time_source = lichtschranke`, hand
///   times cleared, `original_time_ms` kept.
pub fn apply_hand_times(
    run: &mut Run,
    input: &HandTimesInput,
    now: DateTime<Utc>,
) -> AppResult<()> {
    if input.clear {
        if input.hand_time_1_ms.is_some() || input.hand_time_2_ms.is_some() {
            return Err(AppError::Validation(
                "clear darf nicht zusammen mit Handzeiten gesendet werden".into(),
            ));
        }
        run.time_source = TimeSource::Lichtschranke;
        run.hand_time_1_ms = None;
        run.hand_time_2_ms = None;
        run.updated_at = now;
        return Ok(());
    }
    let (Some(t1), Some(t2)) = (input.hand_time_1_ms, input.hand_time_2_ms) else {
        return Err(AppError::Validation(
            "hand_time_1_ms und hand_time_2_ms müssen beide angegeben werden (oder clear: true)"
                .into(),
        ));
    };
    if t1 == 0 || t2 == 0 {
        return Err(AppError::Validation("Handzeiten müssen > 0 sein".into()));
    }
    run.time_source = TimeSource::Handstopp;
    run.hand_time_1_ms = Some(t1);
    run.hand_time_2_ms = Some(t2);
    run.original_time_ms = Some(kido_core::timing::hand_time_mean(t1, t2));
    run.correction_ms = 0;
    run.updated_at = now;
    Ok(())
}

/// `PUT /api/events/{id}/disciplines/{discipline_id}/runs/{run_id}/hand-times`
/// (organizer).
pub async fn set_hand_times(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((event_id, discipline_id, run_id)): Path<(Uuid, Uuid, Uuid)>,
    Json(input): Json<HandTimesInput>,
) -> AppResult<Json<Run>> {
    require_owned(&state, &auth, event_id).await?;
    let mut run = runs::find(&state.db, event_id, discipline_id, run_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("run {run_id}")))?;
    apply_hand_times(&mut run, &input, Utc::now())?;
    runs::put(&state.db, &run).await?;
    Ok(Json(run))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn runner(team_id: Uuid) -> Runner {
        Runner {
            id: Uuid::new_v4(),
            team_id,
            event_id: Uuid::nil(),
            first_name: "A".into(),
            last_name: "B".into(),
            birth_date: NaiveDate::from_ymd_opt(2014, 1, 1).unwrap(),
            position: 1,
            gender: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn run(team_id: Uuid) -> Run {
        Run {
            id: Uuid::new_v4(),
            event_id: Uuid::nil(),
            discipline_id: Uuid::nil(),
            team_id,
            runner_id: None,
            run_number: 1,
            lane: Some(1),
            status: RunStatus::Completed,
            original_time_ms: Some(50_000),
            correction_ms: 15_000,
            started_at: None,
            ended_at: None,
            lineup: Vec::new(),
            lineup_confirmed_at: None,
            lineup_confirmed_by: None,
            time_source: TimeSource::Lichtschranke,
            hand_time_1_ms: None,
            hand_time_2_ms: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn lineup_of_own_runners_ok() {
        let team = Uuid::new_v4();
        let roster = vec![runner(team), runner(team), runner(team)];
        let ids = vec![roster[2].id, roster[0].id];
        assert!(validate_lineup(&ids, &roster, team).is_ok());
    }

    #[test]
    fn empty_lineup_ok() {
        assert!(validate_lineup(&[], &[], Uuid::new_v4()).is_ok());
    }

    #[test]
    fn lineup_with_foreign_runner_rejected() {
        let (team, other) = (Uuid::new_v4(), Uuid::new_v4());
        let roster = vec![runner(team), runner(other)];
        let err = validate_lineup(&[roster[0].id, roster[1].id], &roster, team).unwrap_err();
        assert_eq!(err.code(), "validation");
        let err = validate_lineup(&[Uuid::new_v4()], &roster, team).unwrap_err();
        assert_eq!(err.code(), "validation");
    }

    #[test]
    fn lineup_with_duplicates_rejected() {
        let team = Uuid::new_v4();
        let roster = vec![runner(team)];
        let err = validate_lineup(&[roster[0].id, roster[0].id], &roster, team).unwrap_err();
        assert_eq!(err.code(), "validation");
    }

    #[test]
    fn apply_lineup_records_confirmation() {
        let team = Uuid::new_v4();
        let mut r = run(team);
        let ids = vec![Uuid::new_v4()];
        let now: DateTime<Utc> = "2027-06-12T10:00:00Z".parse().unwrap();
        apply_lineup(&mut r, ids.clone(), LineupConfirmedBy::Startlinie, now);
        assert_eq!(r.lineup, ids);
        assert_eq!(r.lineup_confirmed_at, Some(now));
        assert_eq!(r.lineup_confirmed_by, Some(LineupConfirmedBy::Startlinie));

        apply_lineup(&mut r, ids.clone(), LineupConfirmedBy::Organizer, now);
        assert_eq!(r.lineup_confirmed_by, Some(LineupConfirmedBy::Organizer));
    }

    #[test]
    fn apply_empty_lineup_clears() {
        let mut r = run(Uuid::new_v4());
        apply_lineup(
            &mut r,
            vec![Uuid::new_v4()],
            LineupConfirmedBy::Organizer,
            Utc::now(),
        );
        apply_lineup(&mut r, vec![], LineupConfirmedBy::Organizer, Utc::now());
        assert!(r.lineup.is_empty());
        assert_eq!(r.lineup_confirmed_at, None);
        assert_eq!(r.lineup_confirmed_by, None);
    }

    #[test]
    fn hand_times_body_shapes() {
        let set: HandTimesInput = serde_json::from_value(
            serde_json::json!({ "hand_time_1_ms": 47230, "hand_time_2_ms": 47245 }),
        )
        .unwrap();
        assert_eq!(set.hand_time_1_ms, Some(47230));
        assert!(!set.clear);
        let clear: HandTimesInput =
            serde_json::from_value(serde_json::json!({ "clear": true })).unwrap();
        assert!(clear.clear);
    }

    #[test]
    fn hand_times_set_mean_and_handstopp() {
        let mut r = run(Uuid::new_v4());
        let input = HandTimesInput {
            hand_time_1_ms: Some(47_230),
            hand_time_2_ms: Some(47_245),
            clear: false,
        };
        apply_hand_times(&mut r, &input, Utc::now()).unwrap();
        assert_eq!(r.time_source, TimeSource::Handstopp);
        assert_eq!(r.hand_time_1_ms, Some(47_230));
        assert_eq!(r.hand_time_2_ms, Some(47_245));
        assert_eq!(r.original_time_ms, Some(47_240));
        assert_eq!(r.correction_ms, 0);
    }

    #[test]
    fn hand_times_clear_reverts_but_keeps_original() {
        let mut r = run(Uuid::new_v4());
        r.time_source = TimeSource::Handstopp;
        r.hand_time_1_ms = Some(47_230);
        r.hand_time_2_ms = Some(47_245);
        r.original_time_ms = Some(47_240);
        let input = HandTimesInput {
            clear: true,
            ..Default::default()
        };
        apply_hand_times(&mut r, &input, Utc::now()).unwrap();
        assert_eq!(r.time_source, TimeSource::Lichtschranke);
        assert_eq!(r.hand_time_1_ms, None);
        assert_eq!(r.hand_time_2_ms, None);
        assert_eq!(r.original_time_ms, Some(47_240));
    }

    #[test]
    fn hand_times_need_both_values() {
        let mut r = run(Uuid::new_v4());
        let before = r.clone();
        for input in [
            HandTimesInput::default(),
            HandTimesInput {
                hand_time_1_ms: Some(47_230),
                ..Default::default()
            },
            HandTimesInput {
                hand_time_2_ms: Some(47_230),
                ..Default::default()
            },
        ] {
            assert_eq!(
                apply_hand_times(&mut r, &input, Utc::now())
                    .unwrap_err()
                    .code(),
                "validation"
            );
            assert_eq!(r, before, "run untouched on error");
        }
    }

    #[test]
    fn hand_times_reject_zero_and_clear_with_times() {
        let mut r = run(Uuid::new_v4());
        let zero = HandTimesInput {
            hand_time_1_ms: Some(0),
            hand_time_2_ms: Some(47_000),
            clear: false,
        };
        assert!(apply_hand_times(&mut r, &zero, Utc::now()).is_err());
        let both = HandTimesInput {
            hand_time_1_ms: Some(47_000),
            hand_time_2_ms: Some(47_000),
            clear: true,
        };
        assert!(apply_hand_times(&mut r, &both, Utc::now()).is_err());
    }

    #[test]
    fn router_builds_without_route_conflicts() {
        let _ = router();
    }
}
