// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::AuthenticatedUser;
use crate::auth::session::random_token;
use crate::db::{competitions, penalties, runners, runs, teams, users};
use crate::error::{AppError, AppResult};
use crate::export;
use crate::models::{Competition, PenaltyStatus, Runner, Run, SyncMode, Team, TimerMode};
use crate::state::AppState;

pub fn router() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/", get(list).post(create))
        .route("/{id}", get(detail).put(update).delete(remove))
        .route("/{id}/activate", post(activate))
        .route("/{id}/current-run", post(set_current_run))
        .route("/{id}/sync-mode", post(set_sync_mode))
        .route("/{id}/export-file", get(export_file))
        .route("/{id}/import-results", post(import_results))
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
        sync_mode: SyncMode::Live,
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

#[derive(Deserialize)]
pub struct SetSyncModeInput {
    pub sync_mode: SyncMode,
}

pub async fn set_sync_mode(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(id): Path<Uuid>,
    Json(input): Json<SetSyncModeInput>,
) -> AppResult<Json<Competition>> {
    load_owned(&state, &auth, id).await?;
    competitions::set_sync_mode(&state.db, id, input.sync_mode).await?;
    let updated = competitions::get(&state.db, id)
        .await?
        .ok_or_else(|| AppError::Internal("competition vanished mid-update".into()))?;
    Ok(Json(updated))
}

pub async fn export_file(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> AppResult<axum::response::Response> {
    let comp = load_owned(&state, &auth, id).await?;
    let payload = export::build_payload(&state, &auth.user.sub, id).await?;
    let hmac_key = users::get_hmac_key(&state.db, &auth.user.sub).await?;
    let envelope = export::sign(&hmac_key, &payload)?;

    let body = serde_json::to_vec(&envelope)
        .map_err(|e| AppError::Internal(format!("serialize envelope: {e}")))?;

    let filename = format!(
        "kido-{}-{}.kido",
        export::filename_slug(&comp.name),
        comp.date,
    );

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    let disposition = format!("attachment; filename=\"{filename}\"");
    let disposition_value = HeaderValue::from_str(&disposition)
        .map_err(|e| AppError::Internal(format!("build content-disposition: {e}")))?;
    headers.insert(header::CONTENT_DISPOSITION, disposition_value);

    Ok((headers, body).into_response())
}

#[derive(Serialize)]
pub struct ImportResultsResponse {
    pub runs_imported: usize,
    pub penalties_imported: usize,
    pub runs_skipped: usize,
    pub penalties_skipped: usize,
}

pub async fn import_results(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(id): Path<Uuid>,
    Json(envelope): Json<export::Envelope>,
) -> AppResult<Json<ImportResultsResponse>> {
    let comp = load_owned(&state, &auth, id).await?;

    if comp.sync_mode != SyncMode::Offline {
        return Err(AppError::Conflict(
            "Wettkampf ist nicht im Offline-Modus. Wechseln Sie zuerst in den Offline-Modus, bevor Sie Ergebnisse importieren.".into(),
        ));
    }

    if envelope.owner_sub != auth.user.sub {
        return Err(AppError::Forbidden);
    }

    let hmac_key = users::get_hmac_key(&state.db, &auth.user.sub).await?;
    let payload = export::verify(&hmac_key, &envelope)?;

    if payload.competition.id != id {
        return Err(AppError::Validation(
            "Datei gehört zu einem anderen Wettkampf".into(),
        ));
    }
    if payload.owner_sub != auth.user.sub {
        return Err(AppError::Forbidden);
    }

    let mut runs_imported = 0usize;
    let mut runs_skipped = 0usize;
    for run in payload.runs {
        if run.competition_id != id {
            runs_skipped += 1;
            continue;
        }
        runs::put(&state.db, &run).await?;
        runs_imported += 1;
    }

    let mut penalties_imported = 0usize;
    let mut penalties_skipped = 0usize;
    let now = Utc::now();
    for mut penalty in payload.approved_penalties {
        if penalty.competition_id != id {
            penalties_skipped += 1;
            continue;
        }
        // Approved penalties from the offline app are trusted: they were
        // adjudicated by the operator on-site. Force the canonical fields
        // even if the file shape drifts later.
        penalty.status = PenaltyStatus::Approved;
        if penalty.reviewed_at.is_none() {
            penalty.reviewed_at = Some(now);
        }
        if penalty.reviewed_by.is_none() {
            penalty.reviewed_by = Some("offline-import".to_string());
        }
        penalties::put(&state.db, &penalty).await?;
        penalties_imported += 1;
    }

    Ok(Json(ImportResultsResponse {
        runs_imported,
        penalties_imported,
        runs_skipped,
        penalties_skipped,
    }))
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

