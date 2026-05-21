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
use crate::db::{disciplines, events, runners, runs, team_entries, teams};
use crate::error::{AppError, AppResult};
use crate::models::{Discipline, Event, Run, Runner, Team, TeamEntry};
use crate::state::AppState;

pub fn router() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/", get(list).post(create))
        .route("/{id}", get(detail).put(update).delete(remove))
        .route("/{id}/activate", post(activate))
        .route("/{id}/current", post(set_current))
        .route("/{id}/decider-tokens/rotate", post(rotate_decider_tokens))
}

// ===== helpers =====

pub async fn load_owned(
    state: &AppState,
    auth: &AuthenticatedUser,
    id: Uuid,
) -> AppResult<Event> {
    let ev = events::get(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("event {id}")))?;
    if ev.owner_sub != auth.user.sub {
        return Err(AppError::Forbidden);
    }
    Ok(ev)
}

pub async fn require_owned(
    state: &AppState,
    auth: &AuthenticatedUser,
    id: Uuid,
) -> AppResult<Event> {
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
) -> AppResult<Json<Vec<Event>>> {
    let evs = events::list_for_owner(&state.db, &auth.user.sub).await?;
    Ok(Json(evs))
}

#[derive(Deserialize)]
pub struct CreateEventInput {
    pub name: String,
    pub date: NaiveDate,
    pub location: Option<String>,
}

pub async fn create(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Json(input): Json<CreateEventInput>,
) -> AppResult<(StatusCode, Json<Event>)> {
    if input.name.trim().is_empty() {
        return Err(AppError::Validation("name must not be empty".into()));
    }
    let now = Utc::now();
    let ev = Event {
        id: Uuid::new_v4(),
        owner_sub: auth.user.sub,
        name: input.name.trim().to_string(),
        date: input.date,
        location: input
            .location
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty()),
        is_active: false,
        current_discipline_id: None,
        current_run_id: None,
        decider_tokens: fresh_decider_tokens(),
        created_at: now,
        updated_at: now,
    };
    events::put(&state.db, &ev).await?;
    Ok((StatusCode::CREATED, Json(ev)))
}

// ===== detail =====

#[derive(Serialize)]
pub struct EventDetail {
    pub event: Event,
    pub disciplines: Vec<Discipline>,
    pub teams: Vec<Team>,
    pub runners: Vec<Runner>,
    pub team_entries: Vec<TeamEntry>,
    pub runs: Vec<Run>,
}

pub async fn detail(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<EventDetail>> {
    let ev = load_owned(&state, &auth, id).await?;
    let discipline_list = disciplines::list_for_event(&state.db, id).await?;
    let team_list = teams::list_for_event(&state.db, id).await?;
    let runner_list = runners::list_for_event(&state.db, id).await?;
    let entry_list = team_entries::list_for_event(&state.db, id).await?;
    let run_list = runs::list_for_event(&state.db, id).await?;

    Ok(Json(EventDetail {
        event: ev,
        disciplines: discipline_list,
        teams: team_list,
        runners: runner_list,
        team_entries: entry_list,
        runs: run_list,
    }))
}

// ===== update / delete =====

#[derive(Deserialize)]
pub struct UpdateEventInput {
    pub name: Option<String>,
    pub date: Option<NaiveDate>,
    pub location: Option<Option<String>>,
}

pub async fn update(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateEventInput>,
) -> AppResult<Json<Event>> {
    let mut ev = load_owned(&state, &auth, id).await?;
    if let Some(name) = input.name {
        if name.trim().is_empty() {
            return Err(AppError::Validation("name must not be empty".into()));
        }
        ev.name = name.trim().to_string();
    }
    if let Some(date) = input.date {
        ev.date = date;
    }
    if let Some(location) = input.location {
        ev.location = location
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty());
    }
    ev.updated_at = Utc::now();
    events::put(&state.db, &ev).await?;
    Ok(Json(ev))
}

pub async fn remove(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    load_owned(&state, &auth, id).await?;
    events::cascade_delete(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ===== activate / current =====

pub async fn activate(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Event>> {
    load_owned(&state, &auth, id).await?;
    events::activate_exclusive(&state.db, &auth.user.sub, id).await?;
    let updated = events::get(&state.db, id)
        .await?
        .ok_or_else(|| AppError::Internal("event vanished mid-activate".into()))?;
    Ok(Json(updated))
}

#[derive(Deserialize)]
pub struct SetCurrentInput {
    pub discipline_id: Option<Uuid>,
    pub run_id: Option<Uuid>,
}

pub async fn set_current(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(id): Path<Uuid>,
    Json(input): Json<SetCurrentInput>,
) -> AppResult<Json<Event>> {
    let ev = load_owned(&state, &auth, id).await?;
    if input.discipline_id.is_none() && input.run_id.is_some() {
        return Err(AppError::Validation(
            "cannot set current_run_id without a current_discipline_id".into(),
        ));
    }
    if let Some(did) = input.discipline_id {
        let disc = disciplines::find(&state.db, id, did)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("discipline {did}")))?;
        if disc.event_id != id {
            return Err(AppError::Validation(
                "discipline does not belong to this event".into(),
            ));
        }
    }

    // Putting a discipline on the lane implies the event itself becomes
    // the active one (otherwise the decider pages would still see "no
    // active event"). Activating exclusively also deactivates other
    // events the user owns, which matches the single-lane reality.
    if input.discipline_id.is_some() && !ev.is_active {
        events::activate_exclusive(&state.db, &auth.user.sub, id).await?;
    }

    events::set_current(&state.db, id, input.discipline_id, input.run_id).await?;
    let updated = events::get(&state.db, id)
        .await?
        .ok_or_else(|| AppError::Internal("event vanished mid-update".into()))?;
    Ok(Json(updated))
}

pub async fn rotate_decider_tokens(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Event>> {
    load_owned(&state, &auth, id).await?;
    events::rotate_decider_tokens(&state.db, id, fresh_decider_tokens()).await?;
    let updated = events::get(&state.db, id)
        .await?
        .ok_or_else(|| AppError::Internal("event vanished mid-rotate".into()))?;
    Ok(Json(updated))
}
