// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{post, put};
use chrono::{NaiveDate, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::AuthenticatedUser;
use crate::db::runners;
use crate::error::{AppError, AppResult};
use crate::models::{Gender, Runner};
use crate::routes::events::require_owned;
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
    /// Required on **new** writes (validated in `validate`); the field
    /// itself stays `Option<_>` because pre-migration runner rows in
    /// DDB may still surface here on update via the existing-row merge.
    #[serde(default)]
    pub gender: Option<Gender>,
}

pub async fn create(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((event_id, team_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<RunnerInput>,
) -> AppResult<(StatusCode, Json<Runner>)> {
    require_owned(&state, &auth, event_id).await?;
    validate(&input, /* require_gender = */ true)?;
    let now = Utc::now();
    let runner = Runner {
        id: Uuid::new_v4(),
        team_id,
        event_id,
        first_name: input.first_name.trim().to_string(),
        last_name: input.last_name.trim().to_string(),
        birth_date: input.birth_date,
        position: input.position,
        gender: input.gender,
        created_at: now,
        updated_at: now,
    };
    runners::put(&state.db, &runner).await?;
    Ok((StatusCode::CREATED, Json(runner)))
}

pub async fn update(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((event_id, team_id, runner_id)): Path<(Uuid, Uuid, Uuid)>,
    Json(input): Json<RunnerInput>,
) -> AppResult<Json<Runner>> {
    require_owned(&state, &auth, event_id).await?;
    // Updating a *pre-migration* runner is allowed without gender (the
    // server will keep the previously-stored value, which may also be
    // None). Once any value is present, callers should resend it.
    validate(&input, /* require_gender = */ false)?;

    let existing = runners::list_for_team(&state.db, event_id, team_id)
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
        event_id,
        first_name: input.first_name.trim().to_string(),
        last_name: input.last_name.trim().to_string(),
        birth_date: input.birth_date,
        position: input.position,
        // Prefer the incoming gender; fall back to the previously-stored
        // one so PUT bodies that omit the field don't accidentally wipe
        // it for pre-migration runners.
        gender: input.gender.or(existing.gender),
        created_at: existing.created_at,
        updated_at: Utc::now(),
    };
    runners::put(&state.db, &runner).await?;
    Ok(Json(runner))
}

pub async fn remove(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((event_id, team_id, runner_id)): Path<(Uuid, Uuid, Uuid)>,
) -> AppResult<StatusCode> {
    require_owned(&state, &auth, event_id).await?;
    let existing = runners::list_for_team(&state.db, event_id, team_id)
        .await?
        .into_iter()
        .find(|r| r.id == runner_id)
        .ok_or_else(|| AppError::NotFound(format!("runner {runner_id}")))?;
    runners::delete(&state.db, &existing).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn validate(input: &RunnerInput, require_gender: bool) -> AppResult<()> {
    if input.first_name.trim().is_empty() {
        return Err(AppError::Validation("first_name must not be empty".into()));
    }
    if input.last_name.trim().is_empty() {
        return Err(AppError::Validation("last_name must not be empty".into()));
    }
    // Gender is mandatory on new writes — the WG derivation can't
    // dispatch all-male vs all-female rosters otherwise. We keep the
    // model field optional on read to tolerate pre-migration items.
    if require_gender && input.gender.is_none() {
        return Err(AppError::Validation("gender must be set".into()));
    }
    Ok(())
}
