// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post, put};
use chrono::Utc;
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::AuthenticatedUser;
use crate::db::teams;
use crate::error::{AppError, AppResult};
use crate::models::Team;
use crate::routes::events::require_owned;
use crate::state::AppState;

pub fn router() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/{id}/teams", post(create))
        .route("/{id}/teams/{team_id}", put(update).delete(remove))
        .route("/{id}/teams/{team_id}/runners", get(list_runners))
}

#[derive(Deserialize)]
pub struct TeamInput {
    pub name: String,
    pub position: u32,
}

pub async fn create(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(event_id): Path<Uuid>,
    Json(input): Json<TeamInput>,
) -> AppResult<(StatusCode, Json<Team>)> {
    require_owned(&state, &auth, event_id).await?;
    if input.name.trim().is_empty() {
        return Err(AppError::Validation("team name must not be empty".into()));
    }
    let now = Utc::now();
    let team = Team {
        id: Uuid::new_v4(),
        event_id,
        name: input.name.trim().to_string(),
        position: input.position,
        created_at: now,
        updated_at: now,
    };
    teams::put(&state.db, &team).await?;
    Ok((StatusCode::CREATED, Json(team)))
}

pub async fn update(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((event_id, team_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<TeamInput>,
) -> AppResult<Json<Team>> {
    require_owned(&state, &auth, event_id).await?;
    if input.name.trim().is_empty() {
        return Err(AppError::Validation("team name must not be empty".into()));
    }
    let existing = teams::list_for_event(&state.db, event_id)
        .await?
        .into_iter()
        .find(|t| t.id == team_id)
        .ok_or_else(|| AppError::NotFound(format!("team {team_id}")))?;

    if existing.position != input.position {
        teams::delete(&state.db, &existing).await?;
    }

    let team = Team {
        id: team_id,
        event_id,
        name: input.name.trim().to_string(),
        position: input.position,
        created_at: existing.created_at,
        updated_at: Utc::now(),
    };
    teams::put(&state.db, &team).await?;
    Ok(Json(team))
}

pub async fn remove(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((event_id, team_id)): Path<(Uuid, Uuid)>,
) -> AppResult<StatusCode> {
    require_owned(&state, &auth, event_id).await?;
    let existing = teams::list_for_event(&state.db, event_id)
        .await?
        .into_iter()
        .find(|t| t.id == team_id)
        .ok_or_else(|| AppError::NotFound(format!("team {team_id}")))?;
    teams::delete(&state.db, &existing).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_runners(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((event_id, team_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<Vec<crate::models::Runner>>> {
    require_owned(&state, &auth, event_id).await?;
    let rs = crate::db::runners::list_for_team(&state.db, event_id, team_id).await?;
    Ok(Json(rs))
}
