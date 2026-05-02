// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::AuthenticatedUser;
use crate::auth::session::random_token;
use crate::db::{competitions, runners, runs, teams};
use crate::error::{AppError, AppResult};
use crate::models::{Competition, Runner, Run, Team, TimerMode};
use crate::state::AppState;

pub fn router() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/", get(list).post(create))
        .route("/{id}", get(detail).put(update).delete(remove))
        .route("/{id}/activate", post(activate))
        .route("/{id}/current-run", post(set_current_run))
        .route("/{id}/decider-tokens/rotate", post(rotate_decider_tokens))
}

// ===== load helpers =====

async fn load_owned(
    state: &AppState,
    auth: &AuthenticatedUser,
    id: Uuid,
) -> AppResult<Competition> {
    let comp = competitions::get(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("competition {id}")))?;
    if comp.owner_sub != auth.user.sub {
        return Err(AppError::Forbidden);
    }
    Ok(comp)
}

pub async fn require_owned(
    state: &AppState,
    auth: &AuthenticatedUser,
    id: Uuid,
) -> AppResult<Competition> {
    load_owned(state, auth, id).await
}

fn fresh_decider_tokens() -> [String; 4] {
    [
        random_token(24),
        random_token(24),
        random_token(24),
        random_token(24),
    ]
}

// ===== list / create =====

pub async fn list(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
) -> AppResult<Json<Vec<Competition>>> {
    let comps = competitions::list_for_owner(&state.db, &auth.user.sub).await?;
    Ok(Json(comps))
}

#[derive(Deserialize)]
pub struct CreateCompetitionInput {
    pub name: String,
    pub date: NaiveDate,
    pub mode: TimerMode,
    pub location: Option<String>,
}

pub async fn create(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Json(input): Json<CreateCompetitionInput>,
) -> AppResult<(StatusCode, Json<Competition>)> {
    if input.name.trim().is_empty() {
        return Err(AppError::Validation("name must not be empty".into()));
    }
    let now = Utc::now();
    let comp = Competition {
        id: Uuid::new_v4(),
        owner_sub: auth.user.sub,
        name: input.name.trim().to_string(),
        date: input.date,
        location: input.location.map(|l| l.trim().to_string()).filter(|l| !l.is_empty()),
        mode: input.mode,
        is_active: false,
        current_run_id: None,
        decider_tokens: fresh_decider_tokens(),
        created_at: now,
        updated_at: now,
    };
    competitions::put(&state.db, &comp).await?;
    Ok((StatusCode::CREATED, Json(comp)))
}

// ===== detail =====

#[derive(Serialize)]
pub struct CompetitionDetail {
    pub competition: Competition,
    pub teams: Vec<Team>,
    pub runners: Vec<Runner>,
    pub runs: Vec<Run>,
}

pub async fn detail(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<CompetitionDetail>> {
    let comp = load_owned(&state, &auth, id).await?;
    let team_list = teams::list_for_competition(&state.db, id).await?;

    let mut all_runners: Vec<Runner> = Vec::new();
    for team in &team_list {
        let mut team_runners = runners::list_for_team(&state.db, id, team.id).await?;
        all_runners.append(&mut team_runners);
    }

    let run_list = runs::list_for_competition(&state.db, id).await?;

    Ok(Json(CompetitionDetail {
        competition: comp,
        teams: team_list,
        runners: all_runners,
        runs: run_list,
    }))
}

// ===== update / delete =====

#[derive(Deserialize)]
pub struct UpdateCompetitionInput {
    pub name: Option<String>,
    pub date: Option<NaiveDate>,
    pub mode: Option<TimerMode>,
    pub location: Option<Option<String>>,
}

pub async fn update(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateCompetitionInput>,
) -> AppResult<Json<Competition>> {
    let mut comp = load_owned(&state, &auth, id).await?;
    if let Some(name) = input.name {
        if name.trim().is_empty() {
            return Err(AppError::Validation("name must not be empty".into()));
        }
        comp.name = name.trim().to_string();
    }
    if let Some(date) = input.date {
        comp.date = date;
    }
    if let Some(mode) = input.mode {
        comp.mode = mode;
    }
    if let Some(location) = input.location {
        comp.location = location.map(|l| l.trim().to_string()).filter(|l| !l.is_empty());
    }
    comp.updated_at = Utc::now();
    competitions::put(&state.db, &comp).await?;
    Ok(Json(comp))
}

pub async fn remove(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    load_owned(&state, &auth, id).await?;
    competitions::cascade_delete(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ===== activate / current run / rotate tokens =====

pub async fn activate(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Competition>> {
    load_owned(&state, &auth, id).await?;
    competitions::activate_exclusive(&state.db, &auth.user.sub, id).await?;
    let updated = competitions::get(&state.db, id)
        .await?
        .ok_or_else(|| AppError::Internal("competition vanished mid-activate".into()))?;
    Ok(Json(updated))
}

#[derive(Deserialize)]
pub struct SetCurrentRunInput {
    pub run_id: Option<Uuid>,
}

pub async fn set_current_run(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(id): Path<Uuid>,
    Json(input): Json<SetCurrentRunInput>,
) -> AppResult<Json<Competition>> {
    load_owned(&state, &auth, id).await?;
    competitions::set_current_run(&state.db, id, input.run_id).await?;
    let updated = competitions::get(&state.db, id)
        .await?
        .ok_or_else(|| AppError::Internal("competition vanished mid-update".into()))?;
    Ok(Json(updated))
}

pub async fn rotate_decider_tokens(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Competition>> {
    load_owned(&state, &auth, id).await?;
    competitions::rotate_decider_tokens(&state.db, id, fresh_decider_tokens()).await?;
    let updated = competitions::get(&state.db, id)
        .await?
        .ok_or_else(|| AppError::Internal("competition vanished mid-rotate".into()))?;
    Ok(Json(updated))
}

