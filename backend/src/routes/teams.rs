// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post, put};
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::AuthenticatedUser;
use crate::db::teams;
use crate::error::{AppError, AppResult};
use crate::models::Team;
use crate::routes::competitions::require_owned;
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
    pub start_number: u32,
}

pub async fn create(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(competition_id): Path<Uuid>,
    Json(input): Json<TeamInput>,
) -> AppResult<(StatusCode, Json<Team>)> {
    require_owned(&state, &auth, competition_id).await?;
    if input.name.trim().is_empty() {
        return Err(AppError::Validation("team name must not be empty".into()));
    }
    let team = Team {
        id: Uuid::new_v4(),
        competition_id,
        name: input.name.trim().to_string(),
        start_number: input.start_number,
    };
    teams::put(&state.db, &team).await?;
    Ok((StatusCode::CREATED, Json(team)))
}

pub async fn update(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((competition_id, team_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<TeamInput>,
) -> AppResult<Json<Team>> {
    require_owned(&state, &auth, competition_id).await?;
    if input.name.trim().is_empty() {
        return Err(AppError::Validation("team name must not be empty".into()));
    }
    let existing = teams::list_for_competition(&state.db, competition_id)
        .await?
        .into_iter()
        .find(|t| t.id == team_id)
        .ok_or_else(|| AppError::NotFound(format!("team {team_id}")))?;

    if existing.start_number != input.start_number {
        teams::delete(&state.db, &existing).await?;
    }

    let team = Team {
        id: team_id,
        competition_id,
        name: input.name.trim().to_string(),
        start_number: input.start_number,
    };
    teams::put(&state.db, &team).await?;
    Ok(Json(team))
}

pub async fn remove(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((competition_id, team_id)): Path<(Uuid, Uuid)>,
) -> AppResult<StatusCode> {
    require_owned(&state, &auth, competition_id).await?;
    let existing = teams::list_for_competition(&state.db, competition_id)
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
    Path((competition_id, team_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<Vec<crate::models::Runner>>> {
    require_owned(&state, &auth, competition_id).await?;
    let runners = crate::db::runners::list_for_team(&state.db, competition_id, team_id).await?;
    Ok(Json(runners))
}
