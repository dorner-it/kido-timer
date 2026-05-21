// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, put};
use chrono::Utc;
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::AuthenticatedUser;
use crate::db::{disciplines, runs};
use crate::error::{AppError, AppResult};
use crate::models::{Run, RunStatus};
use crate::routes::events::require_owned;
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
    let _disc = disciplines::find(&state.db, event_id, discipline_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("discipline {discipline_id}")))?;
    if let Some(lane) = input.lane {
        if !(1..=2).contains(&lane) {
            return Err(AppError::Validation("lane must be 1 or 2".into()));
        }
    }
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
