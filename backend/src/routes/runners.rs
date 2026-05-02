// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{post, put};
use chrono::NaiveDate;
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::AuthenticatedUser;
use crate::db::runners;
use crate::error::{AppError, AppResult};
use crate::models::Runner;
use crate::routes::competitions::require_owned;
use crate::state::AppState;

pub fn router() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/{id}/teams/{team_id}/runners", post(create))
        .route(
            "/{id}/teams/{team_id}/runners/{runner_id}",
            put(update).delete(remove),
        )
}

#[derive(Deserialize)]
pub struct RunnerInput {
    pub first_name: String,
    pub last_name: String,
    pub birth_date: NaiveDate,
    pub position: u32,
}

pub async fn create(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((competition_id, team_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<RunnerInput>,
) -> AppResult<(StatusCode, Json<Runner>)> {
    require_owned(&state, &auth, competition_id).await?;
    validate(&input)?;
    let runner = Runner {
        id: Uuid::new_v4(),
        team_id,
        competition_id,
        first_name: input.first_name.trim().to_string(),
        last_name: input.last_name.trim().to_string(),
        birth_date: input.birth_date,
        position: input.position,
    };
    runners::put(&state.db, &runner).await?;
    Ok((StatusCode::CREATED, Json(runner)))
}

pub async fn update(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((competition_id, team_id, runner_id)): Path<(Uuid, Uuid, Uuid)>,
    Json(input): Json<RunnerInput>,
) -> AppResult<Json<Runner>> {
    require_owned(&state, &auth, competition_id).await?;
    validate(&input)?;

    let existing = runners::list_for_team(&state.db, competition_id, team_id)
        .await?
        .into_iter()
        .find(|r| r.id == runner_id)
        .ok_or_else(|| AppError::NotFound(format!("runner {runner_id}")))?;

    if existing.position != input.position {
        runners::delete(&state.db, &existing).await?;
    }

    let runner = Runner {
        id: runner_id,
        team_id,
        competition_id,
        first_name: input.first_name.trim().to_string(),
        last_name: input.last_name.trim().to_string(),
        birth_date: input.birth_date,
        position: input.position,
    };
    runners::put(&state.db, &runner).await?;
    Ok(Json(runner))
}

pub async fn remove(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((competition_id, team_id, runner_id)): Path<(Uuid, Uuid, Uuid)>,
) -> AppResult<StatusCode> {
    require_owned(&state, &auth, competition_id).await?;
    let existing = runners::list_for_team(&state.db, competition_id, team_id)
        .await?
        .into_iter()
        .find(|r| r.id == runner_id)
        .ok_or_else(|| AppError::NotFound(format!("runner {runner_id}")))?;
    runners::delete(&state.db, &existing).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn validate(input: &RunnerInput) -> AppResult<()> {
    if input.first_name.trim().is_empty() {
        return Err(AppError::Validation("first_name must not be empty".into()));
    }
    if input.last_name.trim().is_empty() {
        return Err(AppError::Validation("last_name must not be empty".into()));
    }
    Ok(())
}
